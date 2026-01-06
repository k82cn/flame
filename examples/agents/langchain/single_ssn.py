
import flamepy
import asyncio
from apis import SysPrompt, Question

LANGCHAIN_AGENT_NAME = "langchain-agent"

async def ask_agent():
    sys_prompt = SysPrompt(prompt="You are a weather forecaster.")
    question = Question(question="Who are you?")

    agent = await flamepy.create_session(LANGCHAIN_AGENT_NAME, sys_prompt)
    output = await agent.invoke(question)

    print(output.answer)

if __name__ == "__main__":
    asyncio.run(ask_agent())