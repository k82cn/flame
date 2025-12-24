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

use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use serde::{Deserialize, Serialize};

use common::FlameError;
use common::MutexPtr;
use common::apis::SessionID;
use common::ctx::FlameCache;
use common::lock_ptr;

pub mod client;

pub type ObjectCachePtr = Arc<ObjectCache>;

pub fn new_ptr(config: &FlameCache) -> Result<ObjectCachePtr, FlameError> {
    let mut endpoint = CacheEndpoint::try_from(&config.endpoint)?;
    let network_interfaces =
        NetworkInterface::show().map_err(|e| FlameError::Network(e.to_string()))?;
    let host = network_interfaces
        .iter()
        .find(|iface| iface.name == config.network_interface)
        .ok_or(FlameError::InvalidConfig(format!(
            "network interface <{}> not found",
            config.network_interface
        )))?
        .clone();

    endpoint.host = host
        .addr
        .iter()
        .find(|ip| ip.ip().is_ipv4())
        .ok_or(FlameError::InvalidConfig(format!(
            "network interface <{}> has no IPv4 addresses",
            config.network_interface
        )))?
        .ip()
        .to_string();

    Ok(Arc::new(ObjectCache {
        endpoint,
        objects: common::new_ptr(HashMap::new()),
    }))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Object {
    pub uid: String,
    pub session_id: SessionID,
    pub version: u64,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub endpoint: String,
    pub version: u64,
    pub size: usize,
}

#[derive(Clone, Debug)]
pub struct ObjectCache {
    endpoint: CacheEndpoint,
    objects: MutexPtr<HashMap<SessionID, HashMap<String, Object>>>,
}

#[derive(Clone, Debug)]
pub struct CacheEndpoint {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl ObjectCache {
    pub async fn run(&self) -> Result<(), FlameError> {
        let state = self.clone();

        let endpoint = self.endpoint.clone();
        let host = endpoint.host;
        let port = endpoint.port;

        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(state.clone()))
                .service(
                    web::resource("/objects/{session_id}/{uid}").route(web::get().to(get_object)),
                )
                .service(web::resource("/objects").route(web::put().to(put_object)))
                .service(
                    web::resource("/objects/{session_id}").route(web::delete().to(delete_session)),
                )
        })
        .bind((host, port))?;

        server.addrs().iter().for_each(|addr| {
            tracing::info!("Listening object cache at {addr}");
        });

        server.run().await?;

        Ok(())
    }

    pub fn get(&self, session_id: &SessionID, uid: &str) -> Result<Option<Object>, FlameError> {
        let objects = lock_ptr!(self.objects)?;
        let Some(objects) = objects.get(session_id) else {
            return Ok(None);
        };
        let Some(object) = objects.get(uid) else {
            return Ok(None);
        };
        Ok(Some(object.clone()))
    }

    pub fn put(&self, object: Object) -> Result<(), FlameError> {
        let mut objects = lock_ptr!(self.objects)?;
        let objects = objects.entry(object.session_id).or_insert(HashMap::new());

        objects.insert(object.uid.clone(), object.clone());
        Ok(())
    }

    pub fn delete(&self, session_id: &SessionID) -> Result<(), FlameError> {
        let mut objects = lock_ptr!(self.objects)?;
        objects
            .remove(session_id)
            .ok_or(FlameError::NotFound(format!(
                "session <{}> not found",
                session_id
            )))?;

        Ok(())
    }
}

async fn get_object(
    cache: web::Data<ObjectCache>,
    path: web::Path<(SessionID, String)>,
) -> impl Responder {
    let (session_id, uid) = path.into_inner();
    let object = match cache.get(&session_id, &uid) {
        Ok(Some(object)) => object,
        Ok(None) => return HttpResponse::NotFound().finish(),
        Err(e) => return HttpResponse::InternalServerError().body(e.to_string()),
    };

    match bson::serialize_to_vec(&object) {
        Ok(data) => HttpResponse::Ok()
            .content_type("application/bson")
            .body(data),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn put_object(cache: web::Data<ObjectCache>, body: web::Bytes) -> impl Responder {
    let data = body.to_vec();
    let object: Object = match bson::deserialize_from_slice(&data) {
        Ok(v) => v,
        Err(e) => return HttpResponse::BadRequest().body(e.to_string()),
    };

    let endpoint = format!(
        "{}://{}:{}/objects/{}/{}",
        cache.endpoint.scheme.clone(),
        cache.endpoint.host.clone(),
        cache.endpoint.port.clone(),
        object.session_id,
        object.uid,
    );

    let object_info = ObjectInfo {
        endpoint,
        version: object.version,
        size: data.len(),
    };

    match cache.put(object) {
        Ok(()) => HttpResponse::Ok().json(object_info),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn delete_session(
    cache: web::Data<ObjectCache>,
    path: web::Path<SessionID>,
) -> impl Responder {
    let session_id = path.into_inner();
    match cache.delete(&session_id) {
        Ok(()) => HttpResponse::NoContent().finish(),
        Err(FlameError::NotFound(_)) => HttpResponse::NotFound().finish(),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

impl TryFrom<&String> for CacheEndpoint {
    type Error = FlameError;
    fn try_from(endpoint: &String) -> Result<Self, Self::Error> {
        let url = url::Url::parse(endpoint)
            .map_err(|_| FlameError::InvalidConfig(format!("invalid endpoint <{}>", endpoint)))?;

        Ok(CacheEndpoint {
            scheme: url.scheme().to_string(),
            host: url.host_str().unwrap_or("0.0.0.0").to_string(),
            port: url.port().unwrap_or(8080),
        })
    }
}

impl CacheEndpoint {
    pub fn objects(&self) -> String {
        format!(
            "{}://{}:{}/objects",
            self.scheme.clone(),
            self.host.clone(),
            self.port.clone(),
        )
    }
}
