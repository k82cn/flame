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

#[tokio::test]
async fn test_new_pod_manager() -> Result<(), FlameError> {
    let pm = PodManager::new("/run/containerd/containerd.sock", &default_test_runtime()).await?;

    assert!(!pm.version().is_empty());

    Ok(())
}

#[tokio::test]
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
    let status = pod.status.unwrap();

    let pod = pm.get_pod(&status.id).await?;

    assert_eq!(status.state, PodState::Ready);
    assert!(!pod.spec.containers.is_empty());

    pm.stop_pod(&status.id).await?;

    Ok(())
}

#[tokio::test]
async fn test_list_pods() -> Result<(), FlameError> {
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

    let pods = pm.list_pods().await?;
    assert!(!pods.is_empty());

    for pod in pods {
        let status = pod.status.unwrap();
        assert!(!pod.spec.containers.is_empty());
        pm.stop_pod(&status.id).await?;
    }

    let pods = pm.list_pods().await?;
    assert!(pods.is_empty());

    Ok(())
}
