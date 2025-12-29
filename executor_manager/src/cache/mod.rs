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
use std::time::Duration;

use regex::Regex;

use async_trait::async_trait;
use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;
use tonic::{transport::Server, Request, Response, Status};
use url::Url;

use self::rpc::{
    object_cache_server::{ObjectCache, ObjectCacheServer},
    DeleteObjectRequest, GetObjectRequest, PutObjectRequest, Result as RpcResult,
};
use ::rpc::flame as rpc;

use common::ctx::FlameCache;
use common::lock_ptr;
use common::{FlameError, MutexPtr};

mod client;
mod types;

pub use client::ObjectCacheClient;
pub use types::{Object, ObjectEndpoint, ObjectMetadata};

struct FlameObjectCache {
    endpoint: ObjectEndpoint,
    objects: MutexPtr<HashMap<String, Object>>,
}

#[async_trait]
impl ObjectCache for FlameObjectCache {
    async fn put(
        &self,
        request: Request<PutObjectRequest>,
    ) -> Result<Response<rpc::ObjectMetadata>, Status> {
        let req = request.into_inner();

        let uuid = uuid::Uuid::new_v4().to_string();
        let object = Object {
            uuid: uuid.clone(),
            name: req.name.clone(),
            version: 1,
            data: req.data,
        };

        let endpoint = format!(
            "{}://{}:{}/{}",
            self.endpoint.scheme, self.endpoint.host, self.endpoint.port, uuid
        );
        let metadata = ObjectMetadata {
            endpoint,
            version: 1,
            size: object.data.len() as u64,
        };

        let mut objects = lock_ptr!(self.objects)?;
        objects.insert(uuid.clone(), object);
        tracing::debug!("Object put: {}", uuid);

        Ok(Response::new(metadata.into()))
    }

    async fn get(
        &self,
        request: Request<GetObjectRequest>,
    ) -> Result<Response<rpc::Object>, Status> {
        let req = request.into_inner();

        let objects = lock_ptr!(self.objects)?;
        if let Some(obj) = objects.get(&req.uuid) {
            tracing::debug!("Object get: {}", req.uuid);
            Ok(Response::new(obj.clone().into()))
        } else {
            tracing::debug!("Object not found: {}", req.uuid);
            Err(Status::not_found("Object not found"))
        }
    }

    async fn update(
        &self,
        request: Request<rpc::Object>,
    ) -> Result<Response<rpc::ObjectMetadata>, Status> {
        let mut obj = request.into_inner();
        let mut objects = lock_ptr!(self.objects)?;

        if objects.contains_key(&obj.uuid) {
            let Some(object) = objects.get(&obj.uuid) else {
                return Err(Status::not_found("Object not found"));
            };

            if object.version > obj.version {
                return Err(Status::failed_precondition("Object version is old"));
            }

            obj.version = object.version + 1;
            objects.insert(obj.uuid.clone(), obj.clone().into());

            let endpoint = format!(
                "{}://{}:{}/{}",
                self.endpoint.scheme, self.endpoint.host, self.endpoint.port, obj.uuid
            );

            let metadata = ObjectMetadata {
                endpoint,
                version: obj.version,
                size: obj.data.len() as u64,
            };

            tracing::debug!("Object updated: {}", obj.uuid);

            Ok(Response::new(metadata.into()))
        } else {
            Err(Status::not_found("Object not found"))
        }
    }

    async fn delete(
        &self,
        request: Request<DeleteObjectRequest>,
    ) -> Result<Response<rpc::Result>, Status> {
        let req = request.into_inner();
        let mut objects = self.objects.lock().unwrap();
        let existed = objects.remove(&req.uuid).is_some();

        tracing::debug!("Object deleted: {}", req.uuid);

        Ok(Response::new(rpc::Result {
            return_code: if existed { 0 } else { -1 },
            message: None,
        }))
    }
}

pub async fn run(cache_config: &FlameCache) -> Result<(), FlameError> {
    let endpoint = ObjectEndpoint::try_from(cache_config)?;
    let address_str = format!("{}:{}", endpoint.host, endpoint.port);
    let address = address_str.parse().map_err(|e| {
        FlameError::InvalidConfig(format!("failed to parse url <{address_str}>: {e}"))
    })?;

    let cache = FlameObjectCache {
        endpoint,
        objects: common::new_ptr(HashMap::new()),
    };

    tracing::info!("Listening object cache at {address_str}");

    Server::builder()
        .tcp_keepalive(Some(Duration::from_secs(1)))
        .add_service(ObjectCacheServer::new(cache))
        .serve(address)
        .await
        .map_err(|e| {
            tracing::error!("Object cache server exited with error: {e}");
            FlameError::Network(e.to_string())
        })?;

    Ok(())
}

#[cfg(test)]
mod cache_test {
    use super::*;

    use uuid::Uuid;

    use crate::cache::client::ObjectCacheClient;
    use crate::cache::{Object, ObjectEndpoint};

    #[tokio::test]
    async fn test_cache() {
        common::init_logger();

        let endpoint_str = String::from("http://127.0.0.1:3456");
        let endpoint = ObjectEndpoint::try_from(endpoint_str.as_str()).unwrap();

        let network_interfaces = NetworkInterface::show().unwrap();
        let mut netiface = String::from("eth0");
        for iface in network_interfaces {
            let addrs = iface.addr.iter().filter(|addr| addr.ip().is_loopback());
            if addrs.count() > 0 {
                netiface = iface.name.clone();
                break;
            }
        }

        let cache_config = FlameCache {
            endpoint: endpoint_str,
            network_interface: netiface.to_string(),
        };

        let cc = cache_config.clone();
        let _srv = tokio::task::spawn(async move {
            run(&cc).await.unwrap();
        });

        tokio::time::sleep(Duration::from_millis(250)).await;

        let mut client = ObjectCacheClient::connect(&cache_config).await.unwrap();
        let object_info = client
            .put_object("test".to_string(), vec![1, 2, 3, 4])
            .await
            .unwrap();

        assert_eq!(object_info.version, 1);
        assert_eq!(object_info.size, 4);
        let endpoint = ObjectEndpoint::try_from(object_info.endpoint.as_str()).unwrap();
        assert_eq!(endpoint.host, "127.0.0.1");
        assert_eq!(endpoint.port, 3456);
        assert_eq!(endpoint.scheme, "http");
        assert!(endpoint.uuid.is_some());

        let uuid = endpoint.uuid.unwrap();
        let object = client.get_object(uuid.clone()).await.unwrap();
        assert_eq!(object.name, "test");
        assert_eq!(object.version, 1);
        assert_eq!(object.data, vec![1, 2, 3, 4]);
        assert_eq!(object.uuid, uuid);

        let mut object = object.clone();
        object.data = vec![5, 6, 7, 8];
        let object_info = client.update_object(object).await.unwrap();
        assert_eq!(object_info.version, 2);
        assert_eq!(object_info.size, 4);
        let endpoint = ObjectEndpoint::try_from(object_info.endpoint.as_str()).unwrap();
        assert_eq!(endpoint.host, "127.0.0.1");
        assert_eq!(endpoint.port, 3456);
        assert_eq!(endpoint.scheme, "http");
        assert!(endpoint.uuid.is_some());
        assert_eq!(endpoint.uuid.unwrap(), uuid);
        let object = client.get_object(uuid.clone()).await.unwrap();
        assert_eq!(object.name, "test");
        assert_eq!(object.version, 2);
        assert_eq!(object.data, vec![5, 6, 7, 8]);
        assert_eq!(object.uuid, uuid);
    }
}
