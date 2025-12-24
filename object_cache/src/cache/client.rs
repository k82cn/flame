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

use crate::cache::{CacheEndpoint, Object, ObjectInfo};
use common::FlameError;

pub async fn get(url: &str) -> Result<Object, FlameError> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| FlameError::Network(e.to_string()))?;
    let data = response
        .bytes()
        .await
        .map_err(|e| FlameError::Network(e.to_string()))?;

    let object =
        bson::deserialize_from_slice(&data).map_err(|e| FlameError::Internal(e.to_string()))?;

    Ok(object)
}

pub async fn put(endpoint: &CacheEndpoint, object: &Object) -> Result<ObjectInfo, FlameError> {
    let data = bson::serialize_to_vec(object).map_err(|e| FlameError::Internal(e.to_string()))?;
    let client = reqwest::Client::new();
    let response = client
        .put(endpoint.objects())
        .header("Content-Type", "application/bson")
        .body(data)
        .send()
        .await
        .map_err(|e| FlameError::Network(e.to_string()))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|e| FlameError::Network(e.to_string()))?;

    let object_info = serde_json::from_slice::<ObjectInfo>(&bytes)
        .map_err(|e| FlameError::Internal(e.to_string()))?;

    Ok(object_info)
}
