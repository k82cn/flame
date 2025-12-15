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

use std::{fs, path::Path};

use flame_rs as flame;
use flame_rs::{
    apis::{FlameContext, FlameError},
    client::ApplicationAttributes,
};

use crate::apis::ApplicationYaml;

pub async fn run(ctx: &FlameContext, path: &String) -> Result<(), FlameError> {
    if !Path::new(&path).is_file() {
        return Err(FlameError::InvalidConfig(format!("<{path}> is not a file")));
    }

    let contents =
        fs::read_to_string(path.clone()).map_err(|e| FlameError::Internal(e.to_string()))?;

    let current_cluster = ctx.get_current_cluster()?;
    let conn = flame::client::connect(&current_cluster.endpoint).await?;

    let documents: Vec<&str> = contents
        .split("\n---\n")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for doc in documents {
        let app: ApplicationYaml =
            serde_yaml::from_str(doc).map_err(|e| FlameError::Internal(e.to_string()))?;

        let app_attr = ApplicationAttributes::try_from(&app)?;

        conn.register_application(app.metadata.name, app_attr)
            .await?;
    }

    Ok(())
}
