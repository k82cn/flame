# /// script
# dependencies = [
#   "langchain",
#   "flamepy",
# ]
# [tool.uv.sources]
# flamepy = { path = "/usr/local/flame/sdk/python" }
# ///

import flamepy
import asyncio
from apis import SysPrompt, Question, Answer

LANGCHAIN_AGENT_NAME = "langchain-agent"

async def ask_agent():
    weather_sys_prompt = SysPrompt(prompt="You are a weather forecaster.")
    news_sys_prompt = SysPrompt(prompt="You are a news reporter.")
    
    question = Question(question="Who are you?")

    agent = await flamepy.create_session(LANGCHAIN_AGENT_NAME, weather_sys_prompt)
    task = await agent.invoke(question)
    answer = Answer.from_json(task.output)
    print(answer.answer)

    agent = await flamepy.create_session(LANGCHAIN_AGENT_NAME, news_sys_prompt)
    task = await agent.invoke(question)
    answer = Answer.from_json(task.output)
    print(answer.answer)

if __name__ == "__main__":
    asyncio.run(ask_agent())