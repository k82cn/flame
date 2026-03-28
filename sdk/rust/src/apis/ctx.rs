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

use serde_derive::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;
use tonic::transport::{Certificate, ClientTlsConfig};

use crate::apis::FlameError;

const DEFAULT_FLAME_CONF: &str = "flame.yaml";

/// Client TLS configuration for connecting to Flame services.
///
/// Note: To disable TLS for development, use http:// instead of https://
/// in the endpoint URL.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlameClientTls {
    /// Path to CA certificate for server verification
    #[serde(default)]
    pub ca_file: Option<String>,
}

impl FlameClientTls {
    /// Load client TLS config for tonic.
    ///
    /// If ca_file is specified, use it; otherwise use system CA bundle.
    /// The domain parameter is used for server name verification.
    pub fn client_tls_config(&self, domain: &str) -> Result<ClientTlsConfig, FlameError> {
        let mut config = ClientTlsConfig::new().domain_name(domain);

        if let Some(ref ca_file) = self.ca_file {
            let ca = fs::read_to_string(ca_file).map_err(|e| {
                FlameError::InvalidConfig(format!("failed to read ca_file <{}>: {}", ca_file, e))
            })?;
            config = config.ca_certificate(Certificate::from_pem(ca));
        }

        Ok(config)
    }
}

/// Cluster configuration within a context.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlameClusterConfig {
    /// Cluster endpoint URL (e.g., "https://flame-session-manager:8080")
    pub endpoint: String,
    /// TLS configuration for cluster connection (optional)
    #[serde(default)]
    pub tls: Option<FlameClientTls>,
}

impl FlameClusterConfig {
    /// Check if cluster endpoint requires TLS (https:// scheme)
    pub fn requires_tls(&self) -> bool {
        self.endpoint.starts_with("https://")
    }
}

/// Cache configuration within a context.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlameClientCache {
    /// Cache endpoint URL (e.g., "grpcs://flame-object-cache:9090")
    #[serde(default)]
    pub endpoint: Option<String>,
    /// TLS configuration for cache connection (optional)
    #[serde(default)]
    pub tls: Option<FlameClientTls>,
    /// Local storage path for cache (optional)
    #[serde(default)]
    pub storage: Option<String>,
}

/// Package configuration for application deployment.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlamePackage {
    /// Storage URL for the package (e.g., "file:///var/lib/flame/packages")
    #[serde(default)]
    pub storage: Option<String>,
    /// Patterns to exclude from the package
    #[serde(default)]
    pub excludes: Vec<String>,
}

/// Runner configuration for application execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlameRunner {
    /// Runner template name
    #[serde(default)]
    pub template: Option<String>,
}

/// A named context containing cluster, cache, and package configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlameContextEntry {
    /// Name of this context
    pub name: String,
    /// Cluster configuration
    pub cluster: FlameClusterConfig,
    /// Cache configuration (optional)
    #[serde(default)]
    pub cache: Option<FlameClientCache>,
    /// Package configuration (optional)
    #[serde(default)]
    pub package: Option<FlamePackage>,
    /// Runner configuration (optional)
    #[serde(default)]
    pub runner: Option<FlameRunner>,
}

/// Root configuration structure for flame.yaml
///
/// Example configuration:
/// ```yaml
/// current-context: flame
/// contexts:
///   - name: flame
///     cluster:
///       endpoint: "https://flame-session-manager:8080"
///       tls:
///         ca_file: "/etc/flame/certs/ca.crt"
///     cache:
///       endpoint: "grpcs://flame-object-cache:9090"
///       tls:
///         ca_file: "/etc/flame/certs/cache-ca.crt"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlameContext {
    #[serde(rename = "current-context")]
    pub current_context: String,
    pub contexts: Vec<FlameContextEntry>,
}

impl FlameContext {
    /// Get the current context entry.
    pub fn get_current_context(&self) -> Result<&FlameContextEntry, FlameError> {
        self.contexts
            .iter()
            .find(|c| c.name == self.current_context)
            .ok_or(FlameError::InvalidConfig(format!(
                "Context <{}> not found",
                self.current_context
            )))
    }

    pub fn from_file(fp: Option<String>) -> Result<Self, FlameError> {
        let fp = match fp {
            None => {
                format!("{}/.flame/{}", env!("HOME", "."), DEFAULT_FLAME_CONF)
            }
            Some(path) => path,
        };

        if !Path::new(&fp).is_file() {
            return Err(FlameError::InvalidConfig(format!("<{fp}> is not a file")));
        }

        let contents =
            fs::read_to_string(fp.clone()).map_err(|e| FlameError::Internal(e.to_string()))?;
        let ctx: FlameContext =
            serde_yaml::from_str(&contents).map_err(|e| FlameError::Internal(e.to_string()))?;

        tracing::debug!("Load FlameContext from <{fp}>: {ctx}");

        Ok(ctx)
    }
}

impl Display for FlameContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "current_context: {}, contexts: {}",
            self.current_context,
            self.contexts.len()
        )
    }
}
