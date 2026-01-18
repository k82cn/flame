import flamepy
from flamepy.agent import Agent
from apis import SysPrompt, Question

LANGCHAIN_AGENT_NAME = "langchain-agent"

def ask_agent():
    weather_sys_prompt = SysPrompt(prompt="You are a weather forecaster.")
    news_sys_prompt = SysPrompt(prompt="You are a news reporter.")

    question = Question(question="Who are you?")

    agent = Agent(LANGCHAIN_AGENT_NAME, ctx=weather_sys_prompt)
    output = agent.invoke(question)
    print(output.answer)
    agent.close()

    agent = Agent(LANGCHAIN_AGENT_NAME, ctx=news_sys_prompt)
    output = agent.invoke(question)
    print(output.answer)
    agent.close()

if __name__ == "__main__":
    ask_agent()