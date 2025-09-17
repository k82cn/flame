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

use chrono::{DateTime, Utc};
use common::{FlameError, apis::ApplicationContext};
use rand::distr::{Alphanumeric, SampleString};
use rand::rngs::ThreadRng;
use std::collections::HashMap;
use uuid::Uuid;

use crate::cri_v1::{
    ContainerConfig, ContainerMetadata, DnsConfig as CriDnsConfig, ImageSpec, KeyValue,
    LinuxContainerConfig, LinuxPodSandboxConfig, LinuxSandboxSecurityContext, PodSandboxConfig,
    PodSandboxMetadata, Signal,
};

#[derive(Debug, Clone)]
pub struct Container {
    pub name: String,
    pub image: String,
    pub command: Option<String>,
    pub args: Vec<String>,
    pub envs: HashMap<String, String>,
    pub working_directory: String,
}

#[derive(Debug, Clone)]
pub struct DnsConfig {
    pub servers: Vec<String>,
    pub searches: Vec<String>,
    pub options: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub name: String,
    pub namespace: String,
    pub uid: String,
    pub creation_time: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Pod {
    pub metadata: Metadata,
    pub spec: PodSpec,
    pub status: Option<PodStatus>,
}

#[derive(Debug, Clone)]
pub struct PodSpec {
    pub containers: Vec<Container>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PodState {
    Pending = 0,
    Running = 1,
    Succeed = 2,
    Failed = 3,
}

#[derive(Debug, Clone)]
pub struct PodStatus {
    pub state: PodState,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone)]
pub struct Condition {
    pub name: String,
    pub status: String,
    pub last_transition_time: String,
    pub reason: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub privileged: bool,
}

#[derive(Debug, Clone)]
pub struct PodRuntime {
    pub runtime_handler: String,
    pub log_directory: String,
    pub cgroup_parent: String,
    pub dns_config: DnsConfig,
    pub security_context: SecurityContext,
}

impl TryFrom<&ApplicationContext> for Pod {
    type Error = FlameError;

    fn try_from(app: &ApplicationContext) -> Result<Self, Self::Error> {
        let suffix = Alphanumeric.sample_string(&mut ThreadRng::default(), 8);
        let name = format!("{}-{}", app.name.clone(), suffix);
        let uid = Uuid::new_v4().to_string();
        let image = app.image.clone().ok_or(FlameError::InvalidConfig(format!(
            "image is empty for application {}",
            app.name
        )))?;

        Ok(Self {
            metadata: Metadata {
                name: name.clone(),
                namespace: app.name.clone(),
                uid,
                creation_time: Utc::now(),
            },
            spec: PodSpec {
                containers: vec![Container {
                    name: name.clone(),
                    image: image.clone(),
                    command: app.command.clone(),
                    args: app.arguments.clone(),
                    envs: app.environments.clone(),
                    working_directory: app.working_directory.clone().unwrap_or_default(),
                }],
            },
            status: None,
        })
    }
}

impl From<(&Metadata, &PodRuntime)> for PodSandboxConfig {
    fn from((metadata, runtime): (&Metadata, &PodRuntime)) -> Self {
        Self {
            metadata: Some(PodSandboxMetadata {
                name: metadata.name.clone(),
                uid: metadata.uid.clone(),
                namespace: metadata.namespace.clone(),
                attempt: 0,
            }),
            annotations: HashMap::new(),
            hostname: metadata.name.clone(),
            log_directory: runtime.log_directory.clone(),
            dns_config: Some(runtime.dns_config.clone().into()),
            port_mappings: Vec::new(),
            labels: HashMap::new(),
            linux: Some(LinuxPodSandboxConfig {
                cgroup_parent: runtime.cgroup_parent.clone(),
                security_context: Some(runtime.security_context.clone().into()),
                sysctls: HashMap::new(),
                overhead: None,
                resources: None,
            }),
            windows: None,
        }
    }
}

impl From<SecurityContext> for LinuxSandboxSecurityContext {
    fn from(security_context: SecurityContext) -> Self {
        Self {
            privileged: security_context.privileged,
            ..LinuxSandboxSecurityContext::default()
        }
    }
}

impl From<DnsConfig> for CriDnsConfig {
    fn from(dns_config: DnsConfig) -> Self {
        Self {
            servers: dns_config.servers.clone(),
            searches: dns_config.searches.clone(),
            options: dns_config.options.clone(),
        }
    }
}

impl From<(&Container, &PodRuntime)> for ContainerConfig {
    fn from((container, runtime): (&Container, &PodRuntime)) -> Self {
        let command = {
            if let Some(command) = &container.command {
                vec![command.clone()]
            } else {
                vec![]
            }
        };

        let envs = container
            .envs
            .clone()
            .into_iter()
            .map(|(k, v)| KeyValue { key: k, value: v })
            .collect();

        Self {
            metadata: Some(ContainerMetadata {
                name: container.name.clone(),
                attempt: 0,
            }),
            image: Some(ImageSpec {
                annotations: HashMap::new(),
                image: container.image.clone(),
                runtime_handler: runtime.runtime_handler.clone(),
                user_specified_image: container.image.clone(),
            }),
            command,
            args: container.args.clone(),
            envs,
            working_dir: container.working_directory.clone(),
            linux: Some(LinuxContainerConfig {
                resources: None,
                security_context: None,
            }),
            labels: HashMap::new(),
            annotations: HashMap::new(),
            log_path: format!("{}/{}.log", runtime.log_directory, container.name),
            stdin: false,
            stdin_once: false,
            tty: false,
            mounts: Vec::new(),
            devices: Vec::new(),
            windows: None,
            stop_signal: Signal::Sigterm.into(),
            cdi_devices: Vec::new(),
        }
    }
}
