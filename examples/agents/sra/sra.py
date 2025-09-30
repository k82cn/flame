# /// script
# dependencies = [
#   "flamepy",
#   "langchain",
#   "langgraph",
#   "langchain-deepseek",
#   "langchain-community",
# ]
# [tool.uv.sources]
# flamepy = { path = "/usr/local/flame/sdk/python" }
# ///

import flamepy
import asyncio

from langchain_core.tools import tool
from langchain.chat_models import init_chat_model
from langgraph.prebuilt import create_react_agent

from apis import Question, Answer

async def collect_data(topic: str) -> str:
    """
    Collect the necessary information from the web based on the topic.
    Return the necessary information from the web.
    """
    collector = await flamepy.create_session("collector")
    output = await collector.invoke(Question(topic=topic))
    answer = Answer.from_json(output)
    await collector.close()

    return answer.answer

@tool
def data_collector_agent(topic: str) -> str:
    """
    Collect the information from the web based on the topic.
    All the information will be persisted to the vector database for other agents to use.
    A summary of collection status will be returned.

    Args:
        topic: the topic to collect the information from the web

    Returns:
        str: the summary of collection status
    """

    return asyncio.run(collect_data(topic))


async def write_report(topic: str) -> str:
    writer = await flamepy.create_session("writer")
    
    output = await writer.invoke(Question(topic=topic))
    answer = Answer.from_json(output)
    await writer.close()

    return answer.answer

@tool
def report_writer_agent(topic: str) -> str:
    """
    Write the report based on the topic and the necessary information from the vector database.
    The report will be returned in a markdown format as a string.

    Args:
        topic: the topic to write the report

    Returns:
        str: the report in a markdown format as a string
    """

    return asyncio.run(write_report(topic))


llm = init_chat_model("deepseek-chat", model_provider="deepseek")
agent = create_react_agent(llm, [data_collector_agent, report_writer_agent])

ins = flamepy.FlameInstance()

sys_prompt = """
You are the entrypoint of SRA (Simple Research Agent) which is a multi-agent system.
You will try to understand the research topic from the user and organize collector and writer agents to build the report.
As the supervisor of SRA, you should follow the following rules:
    1. You should understand the research topic from the user.
    2. You should collect the necessary information based on the understanding of the user's topic.
    3. You should build a plan to organize the collector and writer agents to build the report.
    4. By default, it should be a research report of this year.
"""

@ins.entrypoint
def sra(q: Question) -> Answer:
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
