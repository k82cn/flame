# /// script
# dependencies = [
#   "openai",
#   "flamepy",
#   "langchain_deepseek",
# ]
# [tool.uv.sources]
# flamepy = { path = "/usr/local/flame/sdk/python" }
# ///

import os
import flamepy
from langchain_deepseek import ChatDeepSeek

from apis import SysPrompt, Question, Answer

ins = flamepy.FlameInstance()

llm = ChatDeepSeek(
    model="deepseek-chat",
    temperature=0,
    max_tokens=None,
    timeout=None,
    max_retries=2,
)

sys_prompt = """
You are a helpful assistant.
"""

@ins.context
def sys_context(sp: SysPrompt):
    global sys_prompt
    sys_prompt = sp.prompt

@ins.entrypoint
def weather_agent(q: Question) -> Answer:
    global sys_prompt
    global llm

    response = llm.invoke([
        ("system", sys_prompt),
        ("human", q.question)
    ])

    return Answer(answer=response.content)

if __name__ == "__main__":
    ins.run()
