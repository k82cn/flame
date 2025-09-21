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

import flamepy

from e2e import TestRequest, TestResponse, TestContext

instance = flamepy.FlameInstance()

sys_context = None

@instance.entrypoint
def e2e_service_entrypoint(req: TestRequest) -> TestResponse:
    return TestResponse(output=req.input, common_data=sys_context)

@instance.context
def e2e_service_context(ctx: TestContext = None):
    global sys_context
    if ctx is not None:
        sys_context = ctx.common_data

if __name__ == "__main__":
    instance.run()