import flamepy
from apis import SysPrompt, Question

LANGCHAIN_AGENT_NAME = "langchain-agent"

def ask_agent():
    weather_sys_prompt = SysPrompt(prompt="You are a weather forecaster.")
    news_sys_prompt = SysPrompt(prompt="You are a news reporter.")

    question = Question(question="Who are you?")

    agent = flamepy.create_session(LANGCHAIN_AGENT_NAME, weather_sys_prompt)
    output = agent.invoke(question)
    print(output.answer)

    agent = flamepy.create_session(LANGCHAIN_AGENT_NAME, news_sys_prompt)
    output = agent.invoke(question)
    print(output.answer)

if __name__ == "__main__":
    ask_agent()