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
from agents import (
    Agent,
    Runner,
    set_tracing_disabled,
    enable_verbose_stdout_logging,
    set_default_openai_client,
    set_default_openai_api,
)

import flamepy
from apis import MyContext, Question, Answer

from agents.memory.session import SessionABC
from agents.items import TResponseInputItem
from typing import List


logger = logging.getLogger(__name__)

# Set the default OpenAI client to the DeepSeek client
ds_client = AsyncOpenAI(
    base_url="https://api.deepseek.com", api_key=os.getenv("DEEPSEEK_API_KEY")
)
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

    def __init__(self, ctx: MyContext):
        self._messages: List[TResponseInputItem] = []
        if ctx.messages is not None:
            for message in ctx.messages:
                self._messages.append(json.loads(message))

    async def get_items(self, limit: int | None = None) -> List[TResponseInputItem]:
        """Retrieve conversation history for this session."""
        return self._messages[:limit] if limit is not None else self._messages

    async def add_items(self, items: List[TResponseInputItem]) -> None:
        """Store new items for this session."""
        self._messages.extend(items)

    async def pop_item(self) -> TResponseInputItem | None:
        """Remove and return the most recent item from this session."""
        return self._messages.pop()

    async def clear_session(self) -> None:
        """Clear all items for this session."""
        self._messages.clear()

    def history(self) -> List[str]:
        """Get the history of this session."""
        return [json.dumps(item) for item in self._messages]


@ins.entrypoint
async def my_agent(q: Question) -> Answer:
    global agent

    ctx = ins.context()

    logger.info(f"ctx: {ctx}, question: {q}")

    if ctx is not None and isinstance(ctx, MyContext):
        session = MyCustomSession(ctx)
        agent.instructions = ctx.prompt

        result = await Runner.run(agent, q.question, session=session)

        ctx.messages = session.history()

        logger.info(f"Update context: {ctx}")
        ins.update_context(ctx)
        logger.info(f"Update context done")
    else:
        logger.info(f"Run agent without session")
        result = await Runner.run(agent, q.question)

    return Answer(answer=result.final_output)


if __name__ == "__main__":
    logging.basicConfig(level=logging.DEBUG)
    ins.run()
