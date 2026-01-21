/*
Copyright 2025 Flame Authors.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
 */

use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};
use std::{thread, time};

use common::apis::{ExecutorState, Node};
use common::{ctx::FlameClusterContext, FlameError};
use stdng::{lock_ptr, MutexPtr};

use crate::client::BackendClient;
use crate::executor::{self, Executor, ExecutorPtr};

pub struct ExecutorManager {
    ctx: FlameClusterContext,
    executors: HashMap<String, ExecutorPtr>,
    client: BackendClient,
}

impl ExecutorManager {
    pub async fn new(ctx: &FlameClusterContext) -> Result<Self, FlameError> {
        // Create the Flame directory.
        fs::create_dir_all("/tmp/flame/shim")
            .map_err(|e| FlameError::Internal(format!("failed to create shim directory: {e}")))?;

        let client = BackendClient::new(ctx).await?;

        Ok(Self {
            ctx: ctx.clone(),
            executors: HashMap::new(),
            client,
        })
    }

    pub async fn run(&mut self) -> Result<(), FlameError> {
        let mut node = Node::new();
        self.client.register_node(&node).await?;
        let one_second = time::Duration::from_secs(1);

        tracing::debug!("Starting executor manager loop...");
        loop {
            node.refresh();

            // TODO(k82cn): also sync the executors in that node.
            let mut executors = self.client.sync_node(&node, vec![]).await?;

            for mut executor in &mut executors {
                if self.executors.contains_key(&executor.id) {
                    // If the executor is already running, skip it.
                    continue;
                }

                // Skip the released executors.
                if executor.state == ExecutorState::Released {
                    continue;
                }

                tracing::debug!("Executor <{}> is starting.", executor.id);

                // Put the context into the executor.
                executor.context = Some(self.ctx.clone());

                let executor_ptr = Arc::new(Mutex::new(executor.clone()));
                self.executors
                    .insert(executor.id.clone(), executor_ptr.clone());
                executor::start(self.client.clone(), executor_ptr.clone());
            }

            // Remove the released executors.
            self.executors
                .retain(|_, e| e.lock().unwrap().state != ExecutorState::Released);

            tracing::debug!(
                "There are {} executors in node {}",
                executors.len(),
                node.name
            );

            thread::sleep(one_second);
        }

        Ok(())
    }
}

pub async fn run(ctx: &FlameClusterContext) -> Result<(), FlameError> {
    let mut manager = ExecutorManager::new(ctx).await?;
    manager.run().await?;

    Ok(())
}
