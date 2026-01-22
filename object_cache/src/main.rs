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
use common::ctx::FlameClusterContext;

use crate::cache::run;

mod cache;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to flame configuration file
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    common::init_logger()?;

    let args = Args::parse();
    let ctx = FlameClusterContext::from_file(args.config)?;

    let cache_config = ctx.cache.ok_or_else(|| {
        common::FlameError::InvalidConfig("Cache configuration not found".to_string())
    })?;

    run(&cache_config).await?;

    Ok(())
}
