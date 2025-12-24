/*
Copyright 2023 The Flame Authors.
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

use std::fmt::{Display, Formatter};
use std::fs;
use std::path::Path;

use serde_derive::{Deserialize, Serialize};

use crate::apis::ResourceRequirement;
use crate::FlameError;
use crate::Shim;

const DEFAULT_FLAME_CONF: &str = "flame-conf.yaml";
const DEFAULT_CONTEXT_NAME: &str = "flame";
const DEFAULT_FLAME_ENDPOINT: &str = "http://127.0.0.1:8080";
const DEFAULT_SLOT: &str = "cpu=1,mem=2g";
const DEFAULT_POLICY: &str = "proportion";
const DEFAULT_STORAGE: &str = "sqlite://flame.db";
const DEFAULT_MAX_EXECUTORS_PER_NODE: u32 = 128;
const DEFAULT_SHIM: &str = "host";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FlameContextYaml {
    pub cluster: FlameClusterYaml,
    pub executors: FlameExecutorsYaml,
    pub cache: Option<FlameCacheYaml>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FlameClusterYaml {
    pub name: String,
    pub endpoint: String,
    pub slot: Option<String>,
    pub policy: Option<String>,
    pub storage: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FlameExecutorsYaml {
    pub shim: Option<String>,
    pub limits: Option<FlameExecutorLimitsYaml>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FlameCacheYaml {
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FlameExecutorLimitsYaml {
    pub max_executors: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct FlameContext {
    pub cluster: FlameCluster,
    pub executors: FlameExecutors,
    pub cache: Option<FlameCache>,
}

#[derive(Debug, Clone)]
pub struct FlameCluster {
    pub name: String,
    pub endpoint: String,
    pub slot: ResourceRequirement,
    pub policy: String,
    pub storage: String,
}

#[derive(Debug, Clone, Default)]
pub struct FlameExecutors {
    pub shim: Shim,
    pub limits: FlameExecutorLimits,
}

#[derive(Debug, Clone, Default)]
pub struct FlameCache {
    pub endpoint: String,
}

#[derive(Debug, Clone)]
pub struct FlameExecutorLimits {
    pub max_executors: u32,
}

impl Display for FlameContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "name: {}, endpoint: {}",
            self.cluster.name, self.cluster.endpoint
        )
    }
}

impl FlameContext {
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
        let ctx: FlameContextYaml =
            serde_yaml::from_str(&contents).map_err(|e| FlameError::Internal(e.to_string()))?;

        tracing::debug!("Load FrameContext from <{fp}>: {ctx:?}");

        FlameContext::try_from(ctx)
    }
}

impl TryFrom<FlameContextYaml> for FlameContext {
    type Error = FlameError;
    fn try_from(ctx: FlameContextYaml) -> Result<Self, Self::Error> {
        Ok(FlameContext {
            cluster: ctx.cluster.try_into()?,
            executors: ctx.executors.try_into()?,
            cache: ctx.cache.map(FlameCache::try_from).transpose()?,
        })
    }
}

impl TryFrom<FlameClusterYaml> for FlameCluster {
    type Error = FlameError;
    fn try_from(cluster: FlameClusterYaml) -> Result<Self, Self::Error> {
        Ok(FlameCluster {
            name: cluster.name,
            endpoint: cluster.endpoint,
            slot: ResourceRequirement::from(&cluster.slot.unwrap_or(DEFAULT_SLOT.to_string())),
            policy: cluster.policy.unwrap_or(DEFAULT_POLICY.to_string()),
            storage: cluster.storage.unwrap_or(DEFAULT_STORAGE.to_string()),
        })
    }
}

impl TryFrom<FlameExecutorsYaml> for FlameExecutors {
    type Error = FlameError;
    fn try_from(executors: FlameExecutorsYaml) -> Result<Self, Self::Error> {
        Ok(FlameExecutors {
            shim: Shim::try_from(executors.shim.unwrap_or(DEFAULT_SHIM.to_string()))?,
            limits: executors
                .limits
                .map(FlameExecutorLimits::try_from)
                .unwrap_or_else(|| Ok(FlameExecutorLimits::default()))?,
        })
    }
}

impl TryFrom<FlameExecutorLimitsYaml> for FlameExecutorLimits {
    type Error = FlameError;
    fn try_from(limits: FlameExecutorLimitsYaml) -> Result<Self, Self::Error> {
        Ok(FlameExecutorLimits {
            max_executors: limits
                .max_executors
                .unwrap_or(DEFAULT_MAX_EXECUTORS_PER_NODE),
        })
    }
}

impl Default for FlameExecutorLimits {
    fn default() -> Self {
        FlameExecutorLimits {
            max_executors: DEFAULT_MAX_EXECUTORS_PER_NODE,
        }
    }
}

impl Default for FlameCluster {
    fn default() -> Self {
        FlameCluster {
            name: DEFAULT_CONTEXT_NAME.to_string(),
            endpoint: DEFAULT_FLAME_ENDPOINT.to_string(),
            slot: ResourceRequirement::from(&DEFAULT_SLOT.to_string()),
            policy: DEFAULT_POLICY.to_string(),
            storage: DEFAULT_STORAGE.to_string(),
        }
    }
}

impl TryFrom<FlameCacheYaml> for FlameCache {
    type Error = FlameError;
    fn try_from(cache: FlameCacheYaml) -> Result<Self, Self::Error> {
        Ok(FlameCache { endpoint: cache.endpoint })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_flame_context_from_file() -> Result<(), FlameError> {
        let context_string = r#"---
cluster:
  name: flame
  endpoint: "http://flame-session-manager:8080"
  slot: "cpu=1,mem=1g"
  policy: priority
  storage: sqlite://flame.db
executors:
  shim: host
  limits:
    max_executors: 10
        "#;

        let tmp_dir = TempDir::new().unwrap();
        let tmp_file = tmp_dir.path().join("flame-conf.yaml");

        fs::write(&tmp_file, context_string).map_err(|e| FlameError::Internal(e.to_string()))?;

        let ctx = FlameContext::from_file(Some(tmp_file.to_string_lossy().to_string()))
            .map_err(|e| FlameError::Internal(e.to_string()))?;
        assert_eq!(ctx.cluster.name, "flame");
        assert_eq!(ctx.cluster.endpoint, "http://flame-session-manager:8080");
        assert_eq!(ctx.cluster.slot, ResourceRequirement::from("cpu=1,mem=1g"));
        assert_eq!(ctx.cluster.policy, "priority");
        assert_eq!(ctx.cluster.storage, "sqlite://flame.db");
        assert_eq!(ctx.executors.shim, Shim::Host);
        assert_eq!(ctx.executors.limits.max_executors, 10);

        Ok(())
    }
}
