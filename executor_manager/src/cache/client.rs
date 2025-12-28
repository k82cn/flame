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

use ::rpc::flame as rpc;

use tonic::transport::Channel;
use tonic::Request;

use crate::cache::types::{Object, ObjectMetadata};
use common::{ctx::FlameCache, FlameError};
use rpc::object_cache_client::ObjectCacheClient as FlameObjectCacheClient;
use rpc::{DeleteObjectRequest, GetObjectRequest, PutObjectRequest};

pub struct ObjectCacheClient {
    client: FlameObjectCacheClient<Channel>,
}

impl ObjectCacheClient {
    pub async fn connect(cache_config: &FlameCache) -> Result<Self, FlameError> {
        let endpoint = &cache_config.endpoint;
        let client = FlameObjectCacheClient::connect(endpoint.clone())
            .await
            .map_err(|e| FlameError::Network(e.to_string()))?;
        Ok(Self { client })
    }

    pub async fn put_object(
        &mut self,
        name: String,
        data: Vec<u8>,
    ) -> Result<ObjectMetadata, FlameError> {
        let req = PutObjectRequest { name, data };
        let resp = self
            .client
            .put(Request::new(req))
            .await
            .map_err(|e| FlameError::Network(e.to_string()))?;
        let meta: rpc::ObjectMetadata = resp.into_inner();
        Ok(meta.into())
    }

    pub async fn get_object(&mut self, uuid: String) -> Result<Object, FlameError> {
        let req = GetObjectRequest { uuid };
        let resp = self
            .client
            .get(Request::new(req))
            .await
            .map_err(|e| FlameError::Network(e.to_string()))?;
        let obj: rpc::Object = resp.into_inner();
        Ok(obj.into())
    }

    pub async fn delete_object(&mut self, uuid: String) -> Result<bool, FlameError> {
        let req = DeleteObjectRequest { uuid };
        let resp = self
            .client
            .delete(Request::new(req))
            .await
            .map_err(|e| FlameError::Network(e.to_string()))?;
        let result = resp.into_inner();
        Ok(result.return_code == 0)
    }

    pub async fn update_object(&mut self, object: Object) -> Result<ObjectMetadata, FlameError> {
        let resp = self
            .client
            .update(Request::new(object.into()))
            .await
            .map_err(|e| FlameError::Network(e.to_string()))?;
        let meta: rpc::ObjectMetadata = resp.into_inner();
        Ok(meta.into())
    }
}
