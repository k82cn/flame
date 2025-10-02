/*
Copyright 2023 The Flame Authors.
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

use std::cmp::Ordering;
use std::sync::Arc;
use stdng::collections;

use crate::controller::ControllerPtr;
use crate::model::{ExecutorInfo, ExecutorInfoPtr, NodeInfoPtr, SessionInfoPtr, SnapShotPtr};
use crate::scheduler::actions::{ActionPtr, AllocateAction, DispatchAction, ShuffleAction};
use crate::scheduler::plugins::{PluginManager, PluginManagerPtr};
use common::apis::ExecutorState;

use common::FlameError;

const DEFAULT_SCHEDULE_INTERVAL: u64 = 500;

pub struct Context {
    pub snapshot: SnapShotPtr,
    pub controller: ControllerPtr,
    pub actions: Vec<ActionPtr>,
    pub plugins: PluginManagerPtr,
    pub schedule_interval: u64,
}

impl Context {
    pub fn new(controller: ControllerPtr) -> Result<Self, FlameError> {
        let snapshot = controller.snapshot()?;
        let plugins = PluginManager::setup(&snapshot.clone())?;

        Ok(Context {
            snapshot,
            plugins,
            controller,
            actions: vec![
                DispatchAction::new_ptr(),
                AllocateAction::new_ptr(),
                ShuffleAction::new_ptr(),
            ],
            schedule_interval: DEFAULT_SCHEDULE_INTERVAL,
        })
    }

    pub fn is_underused(&self, ssn: &SessionInfoPtr) -> Result<bool, FlameError> {
        self.plugins.is_underused(ssn)
    }

    pub fn is_preemptible(&self, ssn: &SessionInfoPtr) -> Result<bool, FlameError> {
        self.plugins.is_preemptible(ssn)
    }

    pub fn is_allocatable(
        &self,
        node: &NodeInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<bool, FlameError> {
        self.plugins.is_allocatable(node, ssn)
    }

    pub fn is_available(
        &self,
        exec: &ExecutorInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<bool, FlameError> {
        self.plugins.is_available(exec, ssn)
    }

    pub async fn bind_session(
        &self,
        exec: &ExecutorInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<(), FlameError> {
        self.controller
            .bind_session(exec.id.clone(), ssn.id)
            .await?;
        self.plugins.on_session_bind(ssn.clone())?;
        self.snapshot
            .update_executor_state(exec.clone(), ExecutorState::Binding)?;

        Ok(())
    }

    pub async fn pipeline_session(
        &self,
        exec: &ExecutorInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<(), FlameError> {
        self.plugins.on_session_bind(ssn.clone())?;

        // self.snapshot
        //     .update_executor_state(exec.clone(), ExecutorState::Binding)?;

        Ok(())
    }

    pub async fn unbind_session(
        &self,
        exec: &ExecutorInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<(), FlameError> {
        self.controller.unbind_executor(exec.id.clone()).await?;
        self.plugins.on_session_unbind(ssn.clone())?;
        self.snapshot
            .update_executor_state(exec.clone(), ExecutorState::Unbinding)?;

        Ok(())
    }

    pub async fn create_executor(
        &self,
        node: &NodeInfoPtr,
        ssn: &SessionInfoPtr,
    ) -> Result<(), FlameError> {
        let executor = self
            .controller
            .create_executor(node.name.clone(), ssn.id)
            .await?;

        let exec_info = ExecutorInfo::from(&executor);
        self.snapshot.add_executor(Arc::new(exec_info))?;

        self.plugins.on_create_executor(node.clone(), ssn.clone())?;

        Ok(())
    }

    pub async fn release_executor(&self, exec: &ExecutorInfoPtr) -> Result<(), FlameError> {
        self.controller.release_executor(exec.id.clone()).await?;

        self.snapshot
            .update_executor_state(exec.clone(), ExecutorState::Releasing)?;

        Ok(())
    }
}

pub fn ssn_order_fn(ctx: &Context) -> impl collections::Cmp<SessionInfoPtr> {
    SsnOrderFn {
        plugin_mgr: ctx.plugins.clone(),
    }
}

struct SsnOrderFn {
    plugin_mgr: PluginManagerPtr,
}

impl collections::Cmp<SessionInfoPtr> for SsnOrderFn {
    fn cmp(&self, t1: &SessionInfoPtr, t2: &SessionInfoPtr) -> Ordering {
        self.plugin_mgr.ssn_order_fn(t1, t2)
    }
}
