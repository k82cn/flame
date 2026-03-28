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

use tokio::sync::mpsc;

use common::apis::ExecutorState;
use common::{ctx::FlameClusterContext, FlameError};
use stdng::{lock_ptr, MutexPtr};

use crate::client::BackendClient;
use crate::executor::{self, Executor, ExecutorPtr};
use crate::stream_handler::StreamHandler;

/// Messages sent from StreamHandler to ExecutorManager
pub enum ExecutorMessage {
    /// Single executor update (handles both initial sync and ongoing updates)
    Update(Executor),
}

pub struct ExecutorManager {
    ctx: FlameClusterContext,
    executors: MutexPtr<HashMap<String, ExecutorPtr>>,
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
            executors: Arc::new(Mutex::new(HashMap::new())),
            client,
        })
    }

    /// Runs the executor manager in streaming mode using WatchNode.
    ///
    /// Flow:
    /// 1. StreamHandler calls RegisterNode on each connection (handles failover)
    /// 2. StreamHandler starts WatchNode stream to receive executor updates
    /// 3. Process executor messages and maintain local state
    pub async fn run(&mut self) -> Result<(), FlameError> {
        // Create channel for executor messages
        let (executor_tx, mut executor_rx) = mpsc::channel::<ExecutorMessage>(32);

        // Clone what we need for the stream handler
        let client = self.client.clone();

        // Share executors reference with StreamHandler for re-registration
        let executors_for_handler = self.executors.clone();

        // Spawn the stream handler (long-running, self-recovering task)
        // StreamHandler handles register_node + watch_node on each connection
        let stream_handle = tokio::spawn(async move {
            let mut handler = StreamHandler::new(client, executors_for_handler);
            handler.run(executor_tx).await;
        });

        tracing::info!(
            "Starting executor manager in streaming mode with shim <{:?}>",
            self.ctx.cluster.executors.shim
        );

        // Process executor messages from the stream
        while let Some(msg) = executor_rx.recv().await {
            match msg {
                ExecutorMessage::Update(executor) => {
                    self.handle_executor_update(executor)?;
                }
            }
        }

        // Wait for stream handler to finish
        let _ = stream_handle.await;

        Ok(())
    }

    /// Handles an executor update by deriving and executing the appropriate action.
    ///
    /// Action derivation logic:
    /// - If state is Released -> Remove from map
    /// - If ID is new -> Create and start executor
    /// - Otherwise -> Log debug message (existing executor, no action needed)
    fn handle_executor_update(&mut self, mut executor: Executor) -> Result<(), FlameError> {
        let executor_id = executor.id.clone();
        let state = executor.state;

        let mut executors = lock_ptr!(self.executors)?;

        // 1. If state is Released, remove from map
        if state == ExecutorState::Released {
            tracing::info!(
                "Removing executor <{}> from map (state={:?})",
                executor_id,
                state
            );
            executors.remove(&executor_id);
            return Ok(());
        }

        // 2. If ID is new (not in map), create and start executor
        if !executors.contains_key(&executor_id) {
            tracing::info!(
                "Creating executor <{}> (state={:?}, shim={:?})",
                executor_id,
                state,
                self.ctx.cluster.executors.shim
            );
            executor.context = Some(self.ctx.clone());
            // Set the shim from the executor-manager's configuration
            executor.shim = self.ctx.cluster.executors.shim;

            let executor_ptr = Arc::new(Mutex::new(executor));
            executors.insert(executor_id.clone(), executor_ptr.clone());
            executor::start(self.client.clone(), executor_ptr);
            return Ok(());
        }

        // 3. Otherwise (existing ID, not Released), just log debug message
        if let Some(existing) = executors.get(&executor_id) {
            let existing = lock_ptr!(existing)?;
            tracing::debug!(
                "Executor <{}> already exists (current_state={:?}, received_state={:?})",
                executor_id,
                existing.state,
                state
            );
        }

        Ok(())
    }
}

pub async fn run(ctx: &FlameClusterContext) -> Result<(), FlameError> {
    let mut manager = ExecutorManager::new(ctx).await?;
    manager.run().await?;

    Ok(())
}
