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

use clap::Parser;
use common::FlameError;
use common::ctx::FlameContext;

use object_cache::cache;

#[derive(Parser)]
#[command(name = "flame-object-cache")]
#[command(author = "xflops <support@xflops.io>")]
#[command(version = "0.1.0")]
#[command(about = "Flame Object Cache", long_about = None)]
struct Cli {
    #[arg(long, default_value = "~/.flame/flame.yaml")]
    flame_conf: Option<String>,
}

#[tokio::main]
pub async fn main() -> Result<(), FlameError> {
    common::init_logger()?;

    let cli = Cli::parse();
    let ctx = FlameContext::from_file(cli.flame_conf.clone())?;

    let Some(cache_config) = ctx.cache else {
        return Err(FlameError::InvalidConfig(format!(
            "No cache configuration in <{}>",
            cli.flame_conf.clone().unwrap_or_default()
        )));
    };

    let objcache = cache::new_ptr(&cache_config)?;

    objcache.run().await.expect("Failed to start object cache");

    Ok(())
}
