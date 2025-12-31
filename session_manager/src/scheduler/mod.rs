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

use async_trait::async_trait;
use std::sync::Arc;
use std::{thread, time};

use crate::controller::ControllerPtr;
use crate::scheduler::ctx::Context;

use crate::FlameThread;
use common::ctx::FlameContext;
use common::FlameError;

mod actions;
mod ctx;
mod plugins;

pub fn new(controller: ControllerPtr) -> Arc<dyn FlameThread> {
    Arc::new(ScheduleRunner { controller })
}

struct ScheduleRunner {
    controller: ControllerPtr,
}

#[async_trait]
impl FlameThread for ScheduleRunner {
    async fn run(&self, _flame_ctx: FlameContext) -> Result<(), FlameError> {
        loop {
            let mut ctx = Context::new(self.controller.clone())?;

            for action in ctx.actions.clone() {
                if let Err(e) = action.execute(&mut ctx).await {
                    tracing::error!("Failed to run scheduling: {e}");
                    break;
                };
            }

            let delay = time::Duration::from_millis(ctx.schedule_interval);
            thread::sleep(delay);
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use crate::controller;
    use crate::model::{ALL_NODE, OPEN_SESSION};
    use crate::scheduler::actions::{AllocateAction, DispatchAction};
    use crate::scheduler::ctx::Context;
    use crate::scheduler::plugins::PluginManager;
    use crate::scheduler::ControllerPtr;
    use crate::storage;
    use chrono::Duration;
    use chrono::Utc;
    use common::apis::{
        Application, ApplicationAttributes, Node, NodeInfo, NodeState, ResourceRequirement, Shim,
    };
    use common::ctx::FlameCluster;
    use common::ctx::FlameContext;
    use common::FlameError;
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
            let filter = tracing_subscriber::EnvFilter::from_default_env()
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
                cluster: FlameCluster {
                    storage: format!("sqlite:///{url}"),
                    ..Default::default()
                },

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
    fn test_allocate_executors() -> Result<(), FlameError> {
        let env = TestEnv::new()?;
        let controller = env.controller.clone();

        let mut rng = rand::rng();
        let task_num = rng.random_range(..10);

        tokio_test::block_on(
            controller.register_application("flmtest".to_string(), new_test_application()),
        )?;
        tokio_test::block_on(controller.register_node(&new_test_node("node_1".to_string())))?;
        let ssn_1_id = format!("ssn-1-{}", Utc::now().timestamp());
        let ssn_1 = tokio_test::block_on(controller.create_session(
            ssn_1_id.clone(),
            "flmtest".to_string(),
            1,
            None,
        ))?;

        for _ in 0..task_num {
            tokio_test::block_on(controller.create_task(ssn_1.id.clone(), None))?;
        }

        for i in 0..10 {
            let snapshot = controller.snapshot()?;
            let plugins = PluginManager::setup(&snapshot.clone())?;

            let mut ctx = Context {
                snapshot: snapshot.clone(),
                controller: controller.clone(),
                plugins,
                actions: vec![],
                schedule_interval: 1000,
            };

            let dispatch = DispatchAction::new_ptr();
            tokio_test::block_on(dispatch.execute(&mut ctx))?;

            let alloc = AllocateAction::new_ptr();
            tokio_test::block_on(alloc.execute(&mut ctx))?;

            let ssn_list = snapshot.find_sessions(OPEN_SESSION)?;
            assert_eq!(ssn_list.len(), 1);
            assert_eq!(ssn_list.values().next().unwrap().id, ssn_1.id.clone());

            let node_list = snapshot.find_nodes(ALL_NODE)?;
            assert_eq!(node_list.len(), 1);
            assert_eq!(node_list.values().next().unwrap().name, "node_1");

            let exec_list = tokio_test::block_on(controller.list_executor())?;
            assert_eq!(exec_list.len(), task_num);
        }

        Ok(())
    }
}
