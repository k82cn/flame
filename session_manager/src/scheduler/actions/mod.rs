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

use std::sync::Arc;

use async_trait::async_trait;

use crate::scheduler::Context;
use crate::FlameError;

pub use allocate::AllocateAction;
pub use backfill::BackfillAction;
pub use dispatch::DispatchAction;
pub use shuffle::ShuffleAction;

mod allocate;
mod backfill;
mod dispatch;
mod shuffle;

pub type ActionPtr = Arc<dyn Action>;

#[async_trait]
pub trait Action: Send + Sync + 'static {
    async fn execute(&self, ctx: &mut Context) -> Result<(), FlameError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller;
    use crate::model::{ALL_NODE, OPEN_SESSION};
    use crate::scheduler::allocator::Allocator;
    use crate::scheduler::dispatcher::Dispatcher;
    use crate::scheduler::ControllerPtr;
    use crate::storage;
    use chrono::Duration;
    use chrono::Utc;
    use common::apis::{
        Application, ApplicationAttributes, Node, NodeInfo, NodeState, ResourceRequirement, Shim,
    };
    use common::ctx::FlameContext;
    use std::collections::HashMap;
    use std::sync::Arc;
    use uuid::Uuid;
    // use tracing_test::traced_test;

    fn new_test_application() -> ApplicationAttributes {
        ApplicationAttributes {
            image: None,
            command: None,
            description: None,
            labels: Vec::new(),
            arguments: Vec::new(),
            working_directory: "/tmp".to_string(),
            environments: HashMap::new(),
            shim: Shim::Host,
            max_instances: 10,
            delay_release: Duration::seconds(0),
            schema: None,
        }
    }

    fn new_test_node(name: String) -> Node {
        Node {
            name,
            allocatable: ResourceRequirement {
                cpu: 64,
                memory: 100 * 1024 * 1024 * 1024,
            },
            capacity: ResourceRequirement {
                cpu: 64,
                memory: 100 * 1024 * 1024 * 1024,
            },
            info: NodeInfo {
                arch: "x86_64".to_string(),
                os: "linux".to_string(),
            },
            state: NodeState::Ready,
        }
    }

    struct TestEnv {
        url: String,
        pub controller: ControllerPtr,
    }

    impl TestEnv {
        pub fn new() -> Result<Self, FlameError> {
            let filter = tracing_subscriber::EnvFilter::try_from_default_env()?
                .add_directive("h2=error".parse()?)
                .add_directive("hyper_util=error".parse()?)
                .add_directive("sqlx=error".parse()?)
                .add_directive("tower=error".parse()?);

            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_test_writer()
                .with_target(true)
                .with_ansi(false)
                .init();

            let url = format!("/tmp/flame_test_env_{}.db", Utc::now().timestamp());
            let config = FlameContext {
                storage: format!("sqlite:///{url}"),
                ..Default::default()
            };

            let storage = tokio_test::block_on(storage::new_ptr(&config))?;
            let controller = controller::new_ptr(storage.clone());

            Ok(Self { url, controller })
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            std::fs::remove_file(&self.url).unwrap();
        }
    }

    /// Test the allocation of void executors to underused sessions.
    #[test]
    fn test_allocate_void_executors() -> Result<(), FlameError> {
        let env = TestEnv::new()?;
        let controller = env.controller.clone();

        tokio_test::block_on(
            controller.register_application("flmtest".to_string(), new_test_application()),
        )?;
        tokio_test::block_on(controller.register_node(&new_test_node("node_1".to_string())))?;
        let ssn_1 =
            tokio_test::block_on(controller.create_session("flmtest".to_string(), 1, None))?;
        tokio_test::block_on(controller.create_task(ssn_1.id, None))?;

        for i in 0..10 {
            let snapshot = controller.snapshot()?;
            let dispatcher = Arc::new(Dispatcher::new(snapshot.clone(), controller.clone())?);
            let allocator = Arc::new(Allocator::new(snapshot.clone(), controller.clone())?);

            let mut ctx = Context {
                snapshot: snapshot.clone(),
                dispatcher,
                allocator,
                actions: vec![],
                schedule_interval: 1000,
            };

            let alloc = AllocateAction::new_ptr();
            tokio_test::block_on(alloc.execute(&mut ctx))?;

            let ssn_list = snapshot.find_sessions(OPEN_SESSION)?;
            assert_eq!(ssn_list.len(), 1);
            assert_eq!(ssn_list.values().next().unwrap().id, ssn_1.id);

            let node_list = snapshot.find_nodes(ALL_NODE)?;
            assert_eq!(node_list.len(), 1);
            assert_eq!(node_list.values().next().unwrap().name, "node_1");

            let exec_list = tokio_test::block_on(controller.list_executor())?;
            assert_eq!(exec_list.len(), 1);
        }

        Ok(())
    }
}
