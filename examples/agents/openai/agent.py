# Copyright 2025 The Flame Authors.
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#     http://www.apache.org/licenses/LICENSE-2.0
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

import os
import logging

from openai import AsyncOpenAI
from agents import Agent, Runner, SQLiteSession, set_tracing_disabled, enable_verbose_stdout_logging, set_default_openai_client, set_default_openai_api

import flamepy
from apis import SysPrompt, Question, Answer

logger = logging.getLogger(__name__)

# Set the default OpenAI client to the DeepSeek client
ds_client = AsyncOpenAI(base_url="https://api.deepseek.com", api_key=os.getenv("DEEPSEEK_API_KEY"))
set_default_openai_client(ds_client)
set_tracing_disabled(True)
enable_verbose_stdout_logging()
set_default_openai_api("chat_completions")

# Creat a FlameInstance
ins = flamepy.FlameInstance()

# Create agent
agent = Agent(
    name="openai-agent-example",
    model="deepseek-chat",
)

session = None

@ins.context
async def my_context(sp: SysPrompt):
    global session
    global agent

    agent.instructions = sp.prompt
    session = SQLiteSession("ssn_" + ins.session_id, "conversations.db")

@ins.entrypoint
async def my_agent(q: Question) -> Answer:
    global session
    global agent

    result = await Runner.run(
        agent,
        q.question,
        session=session
    )

    return Answer(answer=result.final_output)

if __name__ == "__main__":
    logging.basicConfig(level=logging.DEBUG)
    ins.run()
