# /// script
# dependencies = [
#   "flamepy",
#   "ddgs",
#   "langchain",
#   "langgraph",
#   "langchain-deepseek",
#   "langchain-community",
#   "qdrant-client>=1.14.1",
# ]
# [tool.uv.sources]
# flamepy = { path = "/usr/local/flame/sdk/python" }
# ///

import flamepy
import asyncio
import qdrant_client
from qdrant_client.models import VectorParams, Distance

from langchain_core.tools import tool
from langchain.chat_models import init_chat_model
from langgraph.prebuilt import create_react_agent
from langchain_community.utilities import DuckDuckGoSearchAPIWrapper
from langchain_community.tools import DuckDuckGoSearchResults

from apis import Question, Answer, WebPage

class Counter(flamepy.TaskInformer):
    """
    Count the number of failed, succeed and error tasks.
    """
    def __init__(self):
        super().__init__()
        self.failed = 0
        self.succeed = 0
        self.error = 0
    
    def on_update(self, task: flamepy.Task):
        if task.is_failed():
            self.failed += 1
        elif task.is_completed():
            self.succeed += 1

    def on_error(self, _: flamepy.FlameError):
        self.error += 1

@tool
def search_topics(topic: str) -> list[str]:
    """
    Search the topic on the internet by DuckDuckGo and return the list of links found about the topic.

    Args:
        topic: the topic to search on the internet

    Returns:
        list[str]: the list of links found about the topic
    """

    wrapper = DuckDuckGoSearchAPIWrapper(time="d", max_results=20)
    search = DuckDuckGoSearchResults(api_wrapper=wrapper, source="news", output_format="list")
    items = search.invoke(topic)

    return [item["link"] for item in items]

async def crawl_web_pages(urls: list[str]) -> int:
    """
    Crawl the web and persist the content of the web page to the vector database.
    Return the number of urls crawled successfully.

    Args:
        urls: list of urls to crawl
    """
    crawler = await flamepy.create_session("crawler")

    counter = Counter()

    tasks = []
    for url in urls:
        tasks.append(crawler.invoke(WebPage(url=url), informer=counter))

    await asyncio.gather(*tasks)

    await crawler.close()

    return counter.succeed

@tool
def crawl_web(urls: list[str]) -> int:
    """
    Crawl the web and persist the content of the web page to the vector database.
    Return the number of urls crawled successfully.

    Args:
        urls: list of urls to crawl

    Returns:
        int: the number of urls crawled successfully
    """

    return asyncio.run(crawl_web_pages(urls))


llm = init_chat_model("deepseek-chat", model_provider="deepseek")
agent = create_react_agent(llm, [search_topics, crawl_web])

client = qdrant_client.QdrantClient(host="qdrant", port=6333)
if not client.collection_exists("sra"):
    client.create_collection(
        collection_name="sra",
        vectors_config=VectorParams(size=2560, distance=Distance.COSINE),
    )

ins = flamepy.FlameInstance()

sys_prompt = """
You are a data collector agent for research. You will identify the best links for the research topics via search engine,
then crawl the web and persist the crawled content to the vector database via tools.
"""

@ins.entrypoint
def collector_agent(q: Question) -> Answer:
    messages = [
        {"role": "system", "content": sys_prompt},
        {"role": "user", "content": q.topic}
    ]

    for step in agent.stream({"messages": messages}, stream_mode="values"):
        messages = messages + step["messages"]
        print(step["messages"][-1].pretty_print())

    return Answer(answer=messages[-1].content)

if __name__ == "__main__":
    ins.run()
