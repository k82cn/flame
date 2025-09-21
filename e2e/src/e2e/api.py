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
from typing import Optional

class TestContext(flamepy.Request):
    common_data: Optional[str] = None

class TestRequest(flamepy.Request):
    input: Optional[str] = None

class TestResponse(flamepy.Response):
    output: Optional[str] = None
    common_data: Optional[str] = None
