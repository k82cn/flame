# /// script
# dependencies = [
#   "flamepy",
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
import requests
import qdrant_client

from langchain_core.tools import tool
from langchain.chat_models import init_chat_model
from langgraph.prebuilt import create_react_agent

from apis import Question, Answer, Script, EmbedRequest

async def run_script_async(code: str) -> str:
    script_runner = await flamepy.create_session("flmexec")

    output = await script_runner.invoke(Script(language="python", code=code))

    await script_runner.close()

    return output.decode("utf-8")

@tool
def run_script(code: str) -> str:
    """
    Run the python script and return the result.
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
        str: the result of the script
    """

    return asyncio.run(run_script_async(code))


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

    client = qdrant_client.QdrantClient(host="qdrant", port=6333)

    embedding_req = EmbedRequest(inputs=topic)
    embedding_data = embedding_req.model_dump_json().encode("utf-8")
    resp = requests.post("http://embedding-api:8000/embed", data=embedding_data)
    if resp.status_code != 200:
        return []
    vector = resp.json()["vector"]

    results = client.search(collection_name="sra", query_vector=vector, limit=10)

    return [result.payload["content"] for result in results]
    

llm = init_chat_model("deepseek-chat", model_provider="deepseek")
agent = create_react_agent(llm, [run_script, collect_data])

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

@ins.entrypoint
def writer_agent(q: Question) -> Answer:

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
