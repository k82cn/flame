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

#![allow(dead_code)]
#![allow(unused_variables)]

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::{Context, Poll};

use crate::{apis::FlameError, lock_ptr};

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
enum AsyncQueueState {
    Open,
    Closed,
}

#[derive(Clone)]
pub struct AsyncQueue<T> {
    queue: Arc<Mutex<AsyncQueueImpl<T>>>,
}

impl<T> Default for AsyncQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AsyncQueue<T> {
    pub fn new() -> Self {
        AsyncQueue {
            queue: Arc::new(Mutex::new(AsyncQueueImpl::new())),
        }
    }

    pub fn push_back(&self, item: T) -> Result<(), FlameError> {
        let mut queue = lock_ptr!(self.queue)?;
        queue.push_back(item);

        Ok(())
    }

    pub async fn pop_front(&self) -> Option<T> {
        WaitForItemFuture {
            queue: self.queue.clone(),
        }
        .await
    }

    pub async fn close(&self) -> Result<(), FlameError> {
        {
            let mut queue = lock_ptr!(self.queue)?;
            queue.close();
        }

        WaitForCloseFuture {
            queue: self.queue.clone(),
        }
        .await
    }

    pub fn is_closed(&self) -> bool {
        let queue = lock_ptr!(self.queue);
        match queue {
            Ok(queue) => queue.is_closed(),
            Err(e) => {
                tracing::error!("Failed to lock queue: {e}");
                true
            }
        }
    }
}

struct AsyncQueueImpl<T> {
    queue: VecDeque<T>,
    state: AsyncQueueState,
}

impl<T> AsyncQueueImpl<T> {
    pub fn new() -> Self {
        AsyncQueueImpl {
            queue: VecDeque::new(),
            state: AsyncQueueState::Open,
        }
    }

    pub fn push_back(&mut self, item: T) {
        self.queue.push_back(item);
    }

    pub fn pop_front(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    pub fn close(&mut self) {
        self.state = AsyncQueueState::Closed;
    }

    pub fn is_closed(&self) -> bool {
        self.state == AsyncQueueState::Closed
    }
}

struct WaitForItemFuture<T> {
    queue: Arc<Mutex<AsyncQueueImpl<T>>>,
}

impl<T> Future for WaitForItemFuture<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let queue = lock_ptr!(self.queue);
        match queue {
            Ok(mut queue) => {
                let item = queue.pop_front();
                if item.is_some() {
                    Poll::Ready(item)
                } else if queue.is_closed() {
                    Poll::Ready(None)
                } else {
                    ctx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Err(e) => {
                tracing::error!("Failed to lock queue: {e}");
                Poll::Ready(None)
            }
        }
    }
}

struct WaitForCloseFuture<T> {
    queue: Arc<Mutex<AsyncQueueImpl<T>>>,
}

impl<T> Future for WaitForCloseFuture<T> {
    type Output = Result<(), FlameError>;

    fn poll(self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let queue = lock_ptr!(self.queue);
        match queue {
            Ok(queue) => {
                if queue.queue.is_empty() {
                    Poll::Ready(Ok(()))
                } else {
                    ctx.waker().wake_by_ref();
                    Poll::Pending
                }
            }
            Err(e) => {
                tracing::error!("Failed to lock queue: {e}");
                Poll::Ready(Err(FlameError::Internal(e.to_string())))
            }
        }
    }
}
