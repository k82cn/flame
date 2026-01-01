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

use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use regex::Regex;
use serde_derive::{Deserialize, Serialize};
use url::Url;

use common::{apis::SessionID, ctx::FlameCache, FlameError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub version: u64,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMetadata {
    pub endpoint: String,
    pub version: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEndpoint {
    pub scheme: String,
    pub host: String,
    pub port: u16,
}

impl CacheEndpoint {
    pub fn object_endpoint(&self, session_id: &SessionID, object_id: &String) -> String {
        format!(
            "{}://{}:{}/objects/{}/{}",
            self.scheme, self.host, self.port, session_id, object_id
        )
    }

    fn get_host(cache_config: &FlameCache) -> Result<String, FlameError> {
        let network_interfaces =
            NetworkInterface::show().map_err(|e| FlameError::Network(e.to_string()))?;

        let reg = Regex::new(cache_config.network_interface.as_str())
            .map_err(|e| FlameError::InvalidConfig(e.to_string()))?;
        let host = network_interfaces
            .iter()
            .find(|iface| reg.is_match(iface.name.as_str()))
            .ok_or(FlameError::InvalidConfig(format!(
                "network interface <{}> not found",
                cache_config.network_interface
            )))?
            .clone();

        Ok(host
            .addr
            .iter()
            .find(|ip| ip.ip().is_ipv4())
            .ok_or(FlameError::InvalidConfig(format!(
                "network interface <{}> has no IPv4 addresses",
                cache_config.network_interface
            )))?
            .ip()
            .to_string())
    }
}

impl TryFrom<&FlameCache> for CacheEndpoint {
    type Error = FlameError;

    fn try_from(cache_config: &FlameCache) -> Result<Self, Self::Error> {
        let endpoint = CacheEndpoint::try_from(&cache_config.endpoint)?;
        let host = Self::get_host(cache_config)?;

        Ok(Self {
            scheme: endpoint.scheme,
            host,
            port: endpoint.port,
        })
    }
}

impl TryFrom<&String> for CacheEndpoint {
    type Error = FlameError;

    fn try_from(endpoint: &String) -> Result<Self, Self::Error> {
        let url = Url::parse(endpoint)
            .map_err(|_| FlameError::InvalidConfig(format!("invalid endpoint <{}>", endpoint)))?;

        Ok(Self {
            scheme: url.scheme().to_string(),
            host: url
                .host_str()
                .ok_or(FlameError::InvalidConfig(format!(
                    "no host in endpoint <{}>",
                    endpoint
                )))?
                .to_string(),
            port: url.port().unwrap_or(9090),
        })
    }
}
