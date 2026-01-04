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
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;

use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use async_trait::async_trait;
use network_interface::NetworkInterface;
use network_interface::NetworkInterfaceConfig;
use stdng::{lock_ptr, new_ptr, MutexPtr};
use tonic::{transport::Server, Request, Response, Status};
use url::Url;

use common::apis::SessionID;
use common::ctx::FlameCache;
use common::FlameError;

mod types;
pub use types::{CacheEndpoint, Object, ObjectMetadata};

struct ObjectCache {
    endpoint: CacheEndpoint,
    objects: MutexPtr<HashMap<SessionID, HashMap<String, Object>>>,
}

impl ObjectCache {
    async fn put(
        &self,
        session_id: SessionID,
        data: Vec<u8>,
    ) -> Result<ObjectMetadata, FlameError> {
        let uuid = uuid::Uuid::new_v4().to_string();
        let object = Object { version: 1, data };

        let endpoint = self.endpoint.object_endpoint(&session_id, &uuid);
        let metadata = ObjectMetadata {
            endpoint: endpoint.clone(),
            version: 1,
            size: object.data.len() as u64,
        };

        let mut objects = lock_ptr!(self.objects)?;
        objects
            .entry(session_id.clone())
            .or_default()
            .insert(uuid.clone(), object);

        tracing::debug!("Object put: {}", endpoint);

        Ok(metadata)
    }

    async fn get(&self, session_id: SessionID, uuid: String) -> Result<Object, FlameError> {
        let objects = lock_ptr!(self.objects)?;
        let objects = objects
            .get(&session_id)
            .ok_or(FlameError::NotFound(format!(
                "session <{session_id}> not found"
            )))?;
        let object = objects
            .get(&uuid)
            .ok_or(FlameError::NotFound(format!("object <{uuid}> not found")))?;

        tracing::debug!(
            "Object get: {}",
            self.endpoint.object_endpoint(&session_id, &uuid)
        );

        Ok(object.clone())
    }

    async fn update(
        &self,
        session_id: SessionID,
        uuid: String,
        new_object: Object,
    ) -> Result<ObjectMetadata, FlameError> {
        let mut objects = lock_ptr!(self.objects)?;
        let mut objects = objects
            .get_mut(&session_id)
            .ok_or(FlameError::NotFound(format!(
                "session <{session_id}> not found"
            )))?;
        let old_object = objects
            .get(&uuid)
            .ok_or(FlameError::NotFound(format!("object <{}> not found", uuid)))?;

        if old_object.version > new_object.version {
            return Err(FlameError::VersionMismatch(format!(
                "object <{}> version is old",
                uuid
            )));
        }

        let mut new_object = new_object.clone();
        new_object.version = old_object.version + 1;
        objects.insert(uuid.clone(), new_object.clone());

        let endpoint = self.endpoint.object_endpoint(&session_id, &uuid);
        let metadata = ObjectMetadata {
            endpoint: endpoint.clone(),
            version: new_object.version,
            size: new_object.data.len() as u64,
        };

        tracing::debug!("Object update: {}", endpoint);

        Ok(metadata)
    }

    async fn delete(&self, session_id: SessionID) -> Result<(), FlameError> {
        let mut objects = lock_ptr!(self.objects)?;
        objects
            .remove(&session_id)
            .ok_or(FlameError::NotFound(format!(
                "session <{session_id}> not found"
            )))?;

        tracing::debug!("Session deleted: <{session_id}>");

        Ok(())
    }
}

pub async fn run(cache_config: &FlameCache) -> Result<(), FlameError> {
    let endpoint = CacheEndpoint::try_from(cache_config)?;
    let address_str = format!("{}:{}", endpoint.host, endpoint.port);

    let cache = Arc::new(ObjectCache {
        endpoint,
        objects: new_ptr(HashMap::new()),
    });

    tracing::info!("Listening object cache at {address_str}");

    let svc = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(Arc::clone(&cache)))
            .route(
                "/objects/{session_id}/{object_id}",
                web::get().to(get_object),
            )
            .route(
                "/objects/{session_id}/{object_id}",
                web::put().to(update_object),
            )
            .route("/objects/{session_id}", web::post().to(put_object))
            .route("/objects/{session_id}", web::delete().to(delete_session))
    })
    .bind(address_str)?;

    svc.run().await?;

    Ok(())
}

// Handler to get object metadata
async fn get_object(
    path: web::Path<(String, String)>,
    data: web::Data<Arc<ObjectCache>>,
) -> impl Responder {
    let (session_id, object_id) = path.into_inner();
    match data.get(session_id, object_id).await {
        Ok(object) => HttpResponse::Ok().json(object),
        Err(e) => HttpResponse::NotFound().body(format!("Error: {:?}", e)),
    }
}

// Handler to put object
async fn put_object(
    path: web::Path<String>,
    body: web::Bytes,
    data: web::Data<Arc<ObjectCache>>,
) -> impl Responder {
    let session_id = path.into_inner();

    let metadata = data.put(session_id.clone(), body.to_vec()).await;

    match metadata {
        Ok(metadata) => HttpResponse::Ok().json(metadata),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {:?}", e)),
    }
}

// Handler to update object
async fn update_object(
    path: web::Path<(String, String)>,
    body: web::Bytes,
    data: web::Data<Arc<ObjectCache>>,
) -> impl Responder {
    let (session_id, object_id) = path.into_inner();

    let Ok(object) = serde_json::from_slice(&body) else {
        return HttpResponse::BadRequest().body("Invalid object");
    };

    let metadata = data
        .update(session_id.clone(), object_id.clone(), object)
        .await;

    match metadata {
        Ok(metadata) => HttpResponse::Ok().json(metadata),
        Err(e) => HttpResponse::InternalServerError().body(format!("Error: {:?}", e)),
    }
}

// Handler to delete session
async fn delete_session(
    path: web::Path<String>,
    data: web::Data<Arc<ObjectCache>>,
) -> impl Responder {
    let session_id = path.into_inner();
    match data.delete(session_id).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => HttpResponse::NotFound().body(format!("Error: {:?}", e)),
    }
}
