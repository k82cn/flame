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

from dataclasses import dataclass
from typing import List
import json

from agents.memory.session import SessionABC
from agents.items import TResponseInputItem


@dataclass
class MyContext:
    prompt: str
    messages: List[str] = None


@dataclass
class Question:
    question: str


@dataclass
class Answer:
    answer: str


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
