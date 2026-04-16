"""
Copyright 2025 The Flame Authors.
Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at
    http://www.apache.org/licenses/LICENSE-2.0
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
"""

from typing import Optional

from flamepy.core import FlameService, SessionContext, TaskContext, run


class BenchmarkService(FlameService):
    def on_session_enter(self, context: SessionContext):
        pass

    def on_task_invoke(self, context: TaskContext) -> Optional[bytes]:
        return b"ok"

    def on_session_leave(self):
        pass


if __name__ == "__main__":
    run(BenchmarkService())
