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

use crate::apis::FlameError;

const DEFAULT_FLAME_CONF: &str = "flame.yaml";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FlameContext {
    #[serde(rename = "current-cluster")]
    pub current_cluster: String,
    pub clusters: Vec<FlameCluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlameCluster {
    pub name: String,
    pub endpoint: String,
}

impl FlameContext {
    pub fn get_current_cluster(&self) -> Result<&FlameCluster, FlameError> {
        self.clusters
            .iter()
            .find(|c| c.name == self.current_cluster)
            .ok_or(FlameError::InvalidConfig(format!(
                "Cluster <{}> not found",
                self.current_cluster
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

        tracing::debug!("Load FrameContext from <{fp}>: {ctx}");

        Ok(ctx)
    }
}

impl Display for FlameContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "current_cluster: {}, clusters: {}",
            self.current_cluster,
            self.clusters.len()
        )
    }
}
