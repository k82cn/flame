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

use std::time::Duration;

use ::common::ctx::FlameCache;

use flame_object_cache::cache::client;
use flame_object_cache::cache::{self, CacheEndpoint, Object};

use uuid::Uuid;

#[actix_rt::test]
async fn test_put_and_get_object() {
    // Setup the cache endpoint and object cache
    let endpoint_str = String::from("http://127.0.0.1:3456");
    let endpoint = CacheEndpoint::try_from(&endpoint_str).unwrap();

    let session_id = 1_i64;
    let object_uid = Uuid::new_v4().to_string();

    let test_object = Object {
        uid: object_uid.clone(),
        session_id,
        version: 1,
        data: vec![1, 3, 3, 7],
    };

    // Start the server in background
    let endpoint_str_clone = endpoint_str.clone();
    let _srv = actix_rt::spawn(async move {
        let cache_endpoint = CacheEndpoint::try_from(&endpoint_str_clone).unwrap();
        let flame_cache = FlameCache {
            endpoint: cache_endpoint.objects(),
            network_interface: "eth0".to_string(),
        };
        let cache = cache::new_ptr(&flame_cache).unwrap();

        cache.run().await.expect("Failed to start object cache");
    });

    // Give server time to start
    actix_web::rt::time::sleep(Duration::from_millis(250)).await;

    // Client: Put object
    let object_info = client::put(&endpoint, &test_object)
        .await
        .expect("Failed to put object");
    assert_eq!(object_info.version, 1);
    assert_eq!(object_info.size, 127);

    // The endpoint returned should be valid and include our identifiers
    assert!(
        object_info.endpoint.contains(&session_id.to_string()),
        "Endpoint does not contain session_id"
    );
    assert!(
        object_info.endpoint.contains(&object_uid),
        "Endpoint does not contain object_uid"
    );

    // Client: Get object
    let gotten_obj = client::get(&object_info.endpoint)
        .await
        .expect("Failed to get object");
    assert_eq!(gotten_obj.uid, test_object.uid);
    assert_eq!(gotten_obj.session_id, test_object.session_id);
    assert_eq!(gotten_obj.version, test_object.version);
    assert_eq!(gotten_obj.data, test_object.data);
}
