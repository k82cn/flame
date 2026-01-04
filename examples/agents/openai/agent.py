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
import json

from openai import AsyncOpenAI
from agents import Agent, Runner, set_tracing_disabled, enable_verbose_stdout_logging, set_default_openai_client, set_default_openai_api

import flamepy
from apis import SysPrompt, Question, Answer

from agents.memory.session import SessionABC
from agents.items import TResponseInputItem
from typing import List


logger = logging.getLogger(__name__)

# Set the default OpenAI client to the DeepSeek client
ds_client = AsyncOpenAI(base_url="https://api.deepseek.com",
                        api_key=os.getenv("DEEPSEEK_API_KEY"))
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

class MyCustomSession(SessionABC):
    """Custom session implementation following the Session protocol."""

    def __init__(self):
        self._events = []

    async def get_items(self, limit: int | None = None) -> List[TResponseInputItem]:
        """Retrieve conversation history for this session."""
        return self._events[:limit] if limit is not None else self._events

    async def add_items(self, items: List[TResponseInputItem]) -> None:
        """Store new items for this session."""
        for item in items:
            await ins.record_event(299, json.dumps(item))
        self._events.extend(items)

    async def pop_item(self) -> TResponseInputItem | None:
        """Remove and return the most recent item from this session."""
        return self._events.pop()

    async def clear_session(self) -> None:
        """Clear all items for this session."""
        self._events.clear()


@ins.entrypoint
async def my_agent(q: Question) -> Answer:
    global agent

    session = MyCustomSession()

    ctx = ins.context()
    if ctx is not None and isinstance(ctx, SysPrompt):
        agent.instructions = ctx.prompt

    result = await Runner.run(agent, q.question, session=session)

    await ins.record_event(299, "Agent finished")

    return Answer(answer=result.final_output)

if __name__ == "__main__":
    logging.basicConfig(level=logging.DEBUG)
    ins.run()
