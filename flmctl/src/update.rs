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

pub async fn run(ctx: &FlameContext, application: &Option<String>) -> Result<(), FlameError> {

    match application {
        Some(application) => update_application(ctx, application).await?,
        None => {
            return Err(FlameError::InvalidConfig("application is required".to_string()));
        }
    }

    Ok(())
}

async fn update_application(ctx: &FlameContext, application: &str) -> Result<(), FlameError> {
    if !Path::new(&application).is_file() {
        return Err(FlameError::InvalidConfig(format!("<{application}> is not a file")));
    }

    let contents =
        fs::read_to_string(application).map_err(|e| FlameError::Internal(e.to_string()))?;
    let app: ApplicationYaml =
        serde_yaml::from_str(&contents).map_err(|e| FlameError::Internal(e.to_string()))?;

    let app_attr = ApplicationAttributes::try_from(&app)?;

    let conn = flame::client::connect(&ctx.endpoint).await?;

    conn.update_application(app.metadata.name, app_attr).await?;
    
    Ok(())
}