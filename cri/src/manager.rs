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

use chrono::Utc;
use hyper_util::rt::TokioIo;
use std::collections::HashMap;
use tokio::net::UnixStream;
use tonic::transport::Channel;
use tonic::transport::{Endpoint, Uri};
use tower::service_fn;
use tracing::info;

use common::{FlameError, apis::ApplicationContext, trace::TraceFn, trace_fn};

use crate::apis::{Container, Pod, PodMetadata, PodRuntime, PodSpec, PodState, PodStatus};
use crate::cri_v1::image_service_client::ImageServiceClient;
use crate::cri_v1::runtime_service_client::RuntimeServiceClient;
use crate::cri_v1::{
    ContainerConfig, CreateContainerRequest, ImageSpec, ListContainersRequest,
    ListPodSandboxRequest, PodSandboxConfig, PodSandboxState, PodSandboxStatusRequest,
    PullImageRequest, RunPodSandboxRequest, StartContainerRequest, StopPodSandboxRequest,
    VersionRequest,
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
        let mut pod = Pod::new(app)?;
        let sandbox_config = PodSandboxConfig::from((&pod, &self.runtime));

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
            let resp = self
                .rt_client
                .create_container(request)
                .await
                .map_err(FlameError::from)?;
            let container_id = resp.into_inner().container_id.clone();

            // Start the container.
            let request = StartContainerRequest {
                container_id: container_id.clone(),
            };
            self.rt_client
                .start_container(request)
                .await
                .map_err(FlameError::from)?;
        }
        Ok(pod)
    }

    pub async fn stop_pod(&mut self, id: &str) -> Result<(), FlameError> {
        let request = StopPodSandboxRequest {
            pod_sandbox_id: id.to_string(),
        };

        self.rt_client
            .stop_pod_sandbox(request)
            .await
            .map_err(FlameError::from)?;

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

        let state = PodSandboxState::try_from(sandbox_status.state).map_err(FlameError::from)?;
        let state = PodState::from(state);

        let request = ListContainersRequest { filter: None };
        let resp = self
            .rt_client
            .list_containers(request)
            .await
            .map_err(FlameError::from)?;

        let mut containers = Vec::new();
        for container in resp.into_inner().containers {
            if container.pod_sandbox_id != id {
                continue;
            }
            containers.push(Container::try_from(container)?);
        }

        Ok(Pod {
            metadata: PodMetadata {
                name: metadata.name,
                namespace: metadata.namespace,
                uid: metadata.uid.clone(),
                creation_time: Utc::now(),
            },
            spec: PodSpec { containers },
            status: Some(PodStatus { id: sandbox_status.id, state }),
        })
    }

    pub async fn list_pods(&mut self) -> Result<Vec<Pod>, FlameError> {
        let request = ListPodSandboxRequest { filter: None };

        let pods = self
            .rt_client
            .list_pod_sandbox(request)
            .await
            .map_err(FlameError::from)?;
        let pods = pods.into_inner();

        let request = ListContainersRequest { filter: None };
        let resp = self
            .rt_client
            .list_containers(request)
            .await
            .map_err(FlameError::from)?;

        let mut containers = HashMap::new();
        for c in resp.into_inner().containers {
            containers
                .entry(c.pod_sandbox_id.clone())
                .or_insert(Vec::new())
                .push(Container::try_from(c)?);
        }

        let mut pods = pods
            .items
            .into_iter()
            .map(Pod::try_from)
            .collect::<Result<Vec<Pod>, FlameError>>()?;

        for pod in &mut pods {
            let status = pod.status.clone().ok_or(FlameError::InvalidState("pod status is empty".to_string()))?;
            pod.spec.containers = containers
                .get(&status.id)
                .unwrap_or(&Vec::new())
                .clone();
        }

        Ok(pods)
    }
}
