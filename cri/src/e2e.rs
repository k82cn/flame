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
use std::collections::HashMap;

use common::{FlameError, apis::ApplicationContext, apis::Shim};
use cri_rs::{DnsConfig, PodManager, PodRuntime, PodState, SecurityContext};

const RUNTIME_HANDLER: &str = "runc";
const LOG_DIRECTORY: &str = "/var/log/flame";
const CGROUP_PARENT: &str = "/system.slice";

fn default_test_runtime() -> PodRuntime {
    PodRuntime {
        runtime_handler: RUNTIME_HANDLER.to_string(),
        log_directory: LOG_DIRECTORY.to_string(),
        cgroup_parent: CGROUP_PARENT.to_string(),
        security_context: SecurityContext { privileged: false },
        dns_config: DnsConfig {
            servers: vec![],
            searches: vec![],
            options: vec![],
        },
    }
}

// #[tokio::test]
async fn test_new_pod_manager() -> Result<(), FlameError> {
    let pm = PodManager::new("/run/containerd/containerd.sock", &default_test_runtime()).await?;

    assert!(!pm.version().is_empty());

    Ok(())
}

// #[tokio::test]
async fn test_run_pod() -> Result<(), FlameError> {
    let mut pm =
        PodManager::new("/run/containerd/containerd.sock", &default_test_runtime()).await?;
    assert!(!pm.version().is_empty());

    let app = ApplicationContext {
        name: "test-pod".to_string(),
        image: Some("nginx".to_string()),
        command: None,
        arguments: vec![],
        environments: HashMap::new(),
        shim: Shim::Host,
        working_directory: None,
    };

    let pod = pm.run_pod(&app).await?;

    let pod = pm.get_pod(&pod.metadata.uid).await?;

    assert_eq!(pod.status.unwrap().state, PodState::Running);

    Ok(())
}

#[tokio::main]
pub async fn main() -> Result<(), FlameError> {
    test_run_pod().await?;
    println!("test_run_pod result: Done");

    test_new_pod_manager().await?;
    println!("test_new_pod_manager result: Done");

    Ok(())
}
