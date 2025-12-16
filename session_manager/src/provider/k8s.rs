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

pub struct K8sProvider {
    controller: ControllerPtr,
}

impl K8sProvider {
    pub fn new(controller: ControllerPtr) -> Self {
        Self { controller }
    }
}

#[async_trait]
impl Provider for K8sProvider {
    async fn run(&self, ctx: FlameContext) -> Result<(), FlameError> {
        // TODO(k82cn): implement the k8s provider for Flame:
        //   - Setup a cache to watch the pods belong to Flame, e.g. 'xflops.io/flame/application=xxx'
        //   - Retrieve the total number of tasks of each application from controller as resource request.
        //   - Compare the cache with the resource request:
        //       * If the cache is less than the resource request, create new pods steps by steps.
        //       * If the cache is greater than the resource request, delete the pod whose executor is unbound.
        //
        // Here're also some enhancements in other components:
        //   - The executor manager will be a sidecar of the Pod and it can only manage one single instance (max_instance = 1).
        //   - The executor manager should create the executor during startup, and register it to the session manager.
        //   - Add a new shim, named 'sidecar', which will not stop the instance when unbinding; depends on provider to delete the pod.
        //   - The scheduler should not dispatch tasks if the application of executor is mismatched.
        todo!()
    }
}
