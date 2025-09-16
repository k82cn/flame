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

use hyper_util::rt::TokioIo;
use log::info;
use std::collections::HashMap;
use tokio::net::UnixStream;
use tonic::transport::Channel;
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;
use chrono::{DateTime, Utc};

use common::{FlameError, apis::ApplicationContext, trace::TraceFn, trace_fn};

use crate::apis::{Pod, PodRuntime, Metadata, PodSpec, PodStatus, PodState};
use crate::cri_v1::image_service_client::ImageServiceClient;
use crate::cri_v1::runtime_service_client::RuntimeServiceClient;
use crate::cri_v1::{
    ContainerConfig, CreateContainerRequest, ImageSpec, LinuxPodSandboxConfig, PodSandboxConfig, PodSandboxStatusRequest, PullImageRequest,
    RunPodSandboxRequest, VersionRequest, StartContainerRequest, ListPodSandboxRequest, PodSandboxFilter, PodSandboxStateValue, PodSandboxState,
    StopPodSandboxRequest,
};

pub struct PodManager {
    rt_client: RuntimeServiceClient<Channel>,
    img_client: ImageServiceClient<Channel>,
    runtime: PodRuntime,
    version: String,
}

impl PodManager {
    async fn new_channel(endpoint: &str) -> Result<Channel, FlameError> {
        let endpoint = endpoint.to_string();
        let channel = Endpoint::try_from("http://[::]:50051")
            .unwrap()
            .connect_with_connector({
                let service_addr = endpoint.clone();

                service_fn(move |_: Uri| {
                    let service_addr = service_addr.clone();
                    async move {
                        UnixStream::connect(service_addr)
                            .await
                            .map(TokioIo::new)
                            .map_err(std::io::Error::other)
                    }
                })
            })
            .await
            .map_err(|e| {
                FlameError::Network(format!("failed to connect to service <{endpoint}>: {e}"))
            })?;

        Ok(channel)
    }

    pub async fn new(endpoint: &str, runtime: &PodRuntime) -> Result<Self, FlameError> {
        trace_fn!("PodManager::new");

        let channel = Self::new_channel(endpoint).await?;
        let mut rt_client = RuntimeServiceClient::new(channel);

        let channel = Self::new_channel(endpoint).await?;
        let img_client = ImageServiceClient::new(channel);

        // Get the version of runtime service.
        let request = VersionRequest {
            version: "*".to_string(),
        };

        let version = rt_client
            .version(request.clone())
            .await
            .map_err(|e| FlameError::Network(e.to_string()))?;
        let resp = version.into_inner();

        info!(
            "{} runtime: {}/{}",
            "cri", resp.runtime_name, resp.runtime_version
        );

        let version = format!("{}/{}", resp.runtime_name, resp.runtime_version);

        Ok(Self {
            rt_client,
            img_client,
            runtime: runtime.clone(),
            version,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub async fn run_pod(&mut self, app: &ApplicationContext) -> Result<Pod, FlameError> {
        let mut pod = Pod::try_from(app)?;
        let sandbox_config = PodSandboxConfig::from((&pod.metadata, &self.runtime));

        for container in &pod.spec.containers {
            let request = PullImageRequest {
                image: Some(ImageSpec {
                    annotations: HashMap::new(),
                    image: container.image.clone(),
                    runtime_handler: self.runtime.runtime_handler.clone(),
                    user_specified_image: container.image.clone(),
                }),
                auth: None,
                sandbox_config: Some(sandbox_config.clone()),
            };
            self.img_client
                .pull_image(request)
                .await
                .map_err(FlameError::from)?;
        }

        let request = RunPodSandboxRequest {
            config: Some(sandbox_config.clone()),
            runtime_handler: self.runtime.runtime_handler.clone(),
        };

        let resp = self
            .rt_client
            .run_pod_sandbox(request)
            .await
            .map_err(FlameError::from)?
            .into_inner();

        // Update the pod metadata with the sandbox ID.
        pod.metadata.uid = resp.pod_sandbox_id.clone();
        
        let pod_sandbox_id = resp.pod_sandbox_id.clone();
        // Create the containers.

        for container in &pod.spec.containers {
            let container_config = ContainerConfig::from((container, &self.runtime));
            let request = CreateContainerRequest {
                pod_sandbox_id: pod_sandbox_id.clone(),
                config: Some(container_config),
                sandbox_config: Some(sandbox_config.clone()),
            };
            let resp = self.rt_client.create_container(request).await.map_err(FlameError::from)?;
            let container_id = resp.into_inner().container_id.clone();

            // Start the container.
            let request = StartContainerRequest {
                container_id: container_id.clone(),
            };
            self.rt_client.start_container(request).await.map_err(FlameError::from)?;
        }
        Ok(pod)
    }

    pub async fn stop_pod(&mut self, id: &str) -> Result<(), FlameError> {
        let request = StopPodSandboxRequest {
            pod_sandbox_id: id.to_string(),
        };

        self.rt_client.stop_pod_sandbox(request).await.map_err(FlameError::from)?;
        
        Ok(())
    }

    pub async fn get_pod(&mut self, id: &str) -> Result<Pod, FlameError> {
        let request = PodSandboxStatusRequest {
            pod_sandbox_id: id.to_string(),
            verbose: true,
        };

        let resp = self
            .rt_client
            .pod_sandbox_status(request)
            .await
            .map_err(FlameError::from)?;

        let status = resp.into_inner();

        let sandbox_status = status.status.clone().unwrap();
        let metadata = sandbox_status.metadata.clone().unwrap();

        Ok(Pod {
            metadata: Metadata {
                name: metadata.name,
                namespace: metadata.namespace,
                uid: metadata.uid,
                creation_time: Utc::now(),
            },
            spec: PodSpec {
                containers: Vec::new(),
            },
            status: Some(PodStatus {
                state: PodState::Running,
                conditions: Vec::new(),
            }),
        })
    }

    pub async fn list_pods(&mut self) -> Result<Vec<Pod>, FlameError> {
        let request = ListPodSandboxRequest {
            filter: None,
        };

        let pods = self
            .rt_client
            .list_pod_sandbox(request)
            .await
            .map_err(FlameError::from)?;
        let _ = pods.into_inner();

        todo!()
    }
}
