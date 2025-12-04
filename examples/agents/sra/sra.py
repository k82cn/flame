import flamepy
import asyncio
import qdrant_client
from qdrant_client.models import VectorParams, Distance
import logging

from langchain.globals import set_verbose, set_debug
from langchain_core.tools import tool
from langchain.chat_models import init_chat_model
from langchain.agents import create_agent
from langchain.messages import HumanMessage
from langchain_community.utilities import DuckDuckGoSearchAPIWrapper
from langchain_community.tools import DuckDuckGoSearchResults

from apis import Question, Answer, Script, WebPage
from embed import EmbeddingClient

logger = logging.getLogger(__name__)
logger.setLevel(logging.DEBUG)

set_verbose(True)
set_debug(True)

script_runner = None
web_crawler = None

async def run_script_async(code: str) -> str:
    global script_runner
    if script_runner is None:
        script_runner = await flamepy.create_session("flmexec")

    output = await script_runner.invoke(Script(language="python", code=code))

    return output.decode("utf-8")

@tool
def run_script(code: str) -> str:
    """
    Run the python script and return the result. The stdout of the script will be returned as a string.
    The script will be launched by `uv run` command with the dependencies declared in the script.
    For example, if the script depends on `numpy`, you should declare the dependencies in the script like this:
    ```
    # /// script
    # dependencies = [
    #   "numpy",
    # ]
    # ///
    ```
    Reference to https://docs.astral.sh/uv/guides/scripts/ for more details about how to declare the dependencies.

    Args:
        code: the python code to run

    Returns:
        str: the stdout of the script
    """

    try:
        loop = asyncio.get_running_loop()
        return loop.run_until_completion(run_script_async(code))
    except RuntimeError:
        return asyncio.run(run_script_async(code))

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


async def web_search_async(topics: list[str]) -> int:
    """
    Search the web for the topics and persist the content of the web page to the vector database.
    Return the number of urls crawled successfully.

    Args:
        topics: the topics to search the web for

    Returns:
        int: the number of urls crawled successfully
    """

    global web_crawler
    if web_crawler is None:
        web_crawler = await flamepy.create_session("crawler")

    wrapper = DuckDuckGoSearchAPIWrapper(time="d", max_results=20)
    search = DuckDuckGoSearchResults(api_wrapper=wrapper, source="news", output_format="list")

    counter = Counter()

    tasks = []
    for topic in topics:
        items = search.invoke(topic)
        for item in items:
            task = web_crawler.invoke(WebPage(url=item["link"]), informer=counter)
            tasks.append(task)

    await asyncio.gather(*tasks)

    return counter.succeed

@tool
def web_search(topics: list[str]) -> int:
    """
    Search the web for the topics and persist the content of the web page to the vector database.
    Return the number of urls crawled successfully.

    Args:
        topics: the topics to search the web for

    Returns:
        int: the number of urls crawled successfully
    """
    try:
        loop = asyncio.get_running_loop()
        return loop.run_until_completion(web_search_async(topics))
    except RuntimeError:
        return asyncio.run(web_search_async(topics))

@tool
def collect_data(topic: str) -> list[str]:
    """
    Collect the necessary information from the vector database based on the topic.
    The information will be returned as a list of strings.

    Args:
        topic: the topic to collect the information from the vector database

    Returns:
        list[str]: the list of contents from the vector database
    """

    embedding_client = EmbeddingClient()
    vector = embedding_client.embed(topic)

    db_client = qdrant_client.QdrantClient(host="qdrant", port=6333)

    results = db_client.query_points(collection_name="sra", query=vector, limit=3)

    logger.debug(f"collect_data results: {results}")
    payloads = [result.payload for result in results.points]
    logger.debug(f"collect_data payloads: {payloads}")

    return payloads

ins = flamepy.FlameInstance()

sys_prompt = """
You are a writer agent for research; you will write the research paper based on the research topics and the necessary information from the tools.
As a writer, you should follow the following rules:
    1. You should write the research paper in a professional and academic style.
    2. The research paper should also include a prediction section, which should be based on the necessary information from the tools.
    3. Try to use python script to do the calculation and prediction.
    4. You should use the necessary information from the vector database to write the research paper.
    5. The research paper should be written in a concise and clear manner.
    6. The research paper should be written in a logical and coherent manner.
    7. The research paper should be written in a consistent manner.
    8. The research paper should be written in a markdown format.
"""

llm = init_chat_model("deepseek-chat", model_provider="deepseek")
agent = create_agent(model=llm,
                     tools=[run_script, collect_data, web_search],
                     system_prompt=sys_prompt)

client = qdrant_client.QdrantClient(host="qdrant", port=6333)
if not client.collection_exists("sra"):
    client.create_collection(
        collection_name="sra",
        vectors_config=VectorParams(size=1024, distance=Distance.COSINE),
    )

@ins.entrypoint
def sra(q: Question) -> Answer:
    logger.debug(f"sra input: {q.topic}")
    output = agent.invoke({"messages": [HumanMessage(q.topic)]})

    logger.debug(f"sra output: {output}")
    messages = [msg.content for msg in output["messages"]]
    logger.debug(f"sra messages: {messages}")

    return Answer(answer="\n".join(messages))


if __name__ == "__main__":
    ins.run()
