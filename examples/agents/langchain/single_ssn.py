
import flamepy
from apis import SysPrompt, Question

LANGCHAIN_AGENT_NAME = "langchain-agent"

def ask_agent():
    sys_prompt = SysPrompt(prompt="You are a weather forecaster.")
    question = Question(question="Who are you?")

    agent = flamepy.create_session(LANGCHAIN_AGENT_NAME, sys_prompt)
    output = agent.invoke(question)

    print(output.answer)

if __name__ == "__main__":
    ask_agent()