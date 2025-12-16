/*
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
*/

use common::{ctx::FlameContext, FlameError};
use async_trait::async_trait;

use crate::provider::Provider;
use crate::controller::ControllerPtr;

pub struct NoneProvider {
}

impl NoneProvider {
    pub fn new(_: ControllerPtr) -> Self {
        Self {}
    }
}

#[async_trait]
impl Provider for NoneProvider {
    async fn run(&self, _: FlameContext) -> Result<(), FlameError> {
        Ok(())
    }
}
