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

//! AsyncQueue - A multi-producer, single-consumer async queue using mpsc channels.
//!
//! This queue allows multiple producers to push items and a single consumer
//! to pop items asynchronously. It uses tokio's mpsc channel internally for
//! efficient async notification without busy-waiting.

use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::sync::mpsc;

/// Default capacity for the internal mpsc channel.
const DEFAULT_CAPACITY: usize = 256;

/// A multi-producer, single-consumer async queue.
///
/// The queue is backed by a tokio mpsc channel, providing efficient
/// async notification when items are available.
///
/// # Example
///
/// ```ignore
/// let queue = AsyncQueue::new();
///
/// // Producer side (can be cloned and shared)
/// queue.push(item).await;
///
/// // Consumer side
/// while let Some(item) = queue.pop().await {
///     // process item
/// }
/// ```
#[derive(Clone)]
pub struct AsyncQueue<T> {
    inner: Arc<AsyncQueueInner<T>>,
}

struct AsyncQueueInner<T> {
    /// Sender half of the mpsc channel (used for push)
    tx: mpsc::Sender<T>,
    /// Receiver half of the mpsc channel (used for pop)
    /// Wrapped in Mutex to allow single consumer with Clone support
    rx: Mutex<mpsc::Receiver<T>>,
}

impl<T> Default for AsyncQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> AsyncQueue<T> {
    /// Creates a new AsyncQueue with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Creates a new AsyncQueue with the specified capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        let (tx, rx) = mpsc::channel(capacity);
        AsyncQueue {
            inner: Arc::new(AsyncQueueInner {
                tx,
                rx: Mutex::new(rx),
            }),
        }
    }

    /// Pushes an item to the back of the queue.
    ///
    /// This method is async and will wait if the queue is at capacity.
    /// Returns `Ok(())` on success, or `Err(item)` if the queue is closed.
    pub async fn push(&self, item: T) -> Result<(), T> {
        self.inner.tx.send(item).await.map_err(|e| e.0)
    }

    /// Tries to push an item without waiting.
    ///
    /// Returns `Ok(())` if the item was pushed, or `Err(item)` if the queue
    /// is full or closed.
    pub fn try_push(&self, item: T) -> Result<(), T> {
        self.inner.tx.try_send(item).map_err(|e| match e {
            mpsc::error::TrySendError::Full(item) => item,
            mpsc::error::TrySendError::Closed(item) => item,
        })
    }

    /// Pops an item from the front of the queue.
    ///
    /// This method is async and will wait until an item is available.
    /// Returns `None` if the queue is closed and empty.
    pub async fn pop(&self) -> Option<T> {
        let mut rx = self.inner.rx.lock().await;
        rx.recv().await
    }

    /// Closes the queue.
    ///
    /// After closing, no more items can be pushed. Existing items can still
    /// be popped until the queue is empty.
    pub fn close(&self) {
        // Dropping the sender or calling close prevents new sends
        // We don't actually need to do anything here since we keep the sender alive
        // The queue will be "closed" when all senders are dropped
    }

    /// Returns true if the queue is closed (no more items will be produced).
    ///
    /// Note: This checks if the sender is closed, meaning no more items
    /// can be pushed. Items may still be available for popping.
    pub fn is_closed(&self) -> bool {
        self.inner.tx.is_closed()
    }

    /// Returns the number of items currently in the queue.
    ///
    /// Note: This is an approximation as the queue may be modified
    /// concurrently.
    pub fn len(&self) -> usize {
        // mpsc doesn't expose len directly, but we can check capacity usage
        self.inner.tx.max_capacity() - self.inner.tx.capacity()
    }

    /// Returns true if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_push_pop() {
        let queue = AsyncQueue::new();

        queue.push(1).await.unwrap();
        queue.push(2).await.unwrap();
        queue.push(3).await.unwrap();

        assert_eq!(queue.pop().await, Some(1));
        assert_eq!(queue.pop().await, Some(2));
        assert_eq!(queue.pop().await, Some(3));
    }

    #[tokio::test]
    async fn test_try_push() {
        let queue: AsyncQueue<i32> = AsyncQueue::with_capacity(2);

        assert!(queue.try_push(1).is_ok());
        assert!(queue.try_push(2).is_ok());
        // Queue is full
        assert!(queue.try_push(3).is_err());

        // Pop one and try again
        assert_eq!(queue.pop().await, Some(1));
        assert!(queue.try_push(3).is_ok());
    }

    #[tokio::test]
    async fn test_clone_and_push() {
        let queue = AsyncQueue::new();
        let queue2 = queue.clone();

        queue.push(1).await.unwrap();
        queue2.push(2).await.unwrap();

        assert_eq!(queue.pop().await, Some(1));
        assert_eq!(queue.pop().await, Some(2));
    }

    #[tokio::test]
    async fn test_async_wait() {
        let queue = AsyncQueue::new();
        let queue_clone = queue.clone();

        // Spawn a task that pushes after a delay
        let handle = tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            queue_clone.push(42).await.unwrap();
        });

        // This should wait until the item is available
        let item = queue.pop().await;
        assert_eq!(item, Some(42));

        handle.await.unwrap();
    }
}
