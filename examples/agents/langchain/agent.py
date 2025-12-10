
import flamepy

from langchain_deepseek import ChatDeepSeek
from langchain.agents import create_agent
from langchain.messages import HumanMessage

from apis import SysPrompt, Question, Answer

ins = flamepy.FlameInstance()

llm = ChatDeepSeek(
    model="deepseek-chat",
    temperature=0,
    max_tokens=None,
    timeout=None,
    max_retries=2,
)

agent = create_agent(
    model=llm,
)

@ins.context
def sys_context(sp: SysPrompt):
    agent.system_prompt = sp.prompt

@ins.entrypoint
def weather_agent(q: Question) -> Answer:
    output = agent.invoke({
        "messages": [HumanMessage(q.question)]
    })

    aimsgs = output["messages"][-1]

    return Answer(answer=aimsgs.content)

if __name__ == "__main__":
    ins.run()
