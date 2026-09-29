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

#[cfg(test)]
mod tests {
    use crate::apis::{ResourceRequirement, SessionAttributes, TaskResult, TaskState};
    use crate::ctx::{FlameCluster, FlameClusterContext};
    use crate::storage;
    use stdng::lock_ptr;

    fn test_context() -> FlameClusterContext {
        FlameClusterContext {
            cluster: FlameCluster {
                storage: "none".to_string(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn create_session_attr(id: &str) -> SessionAttributes {
        SessionAttributes {
            name: id.rsplit('/').next().unwrap().to_string(),
            workspace: "default".to_string(),
            application: "test-app".to_string(),
            common_data: None,
            min_instances: 1,
            max_instances: None,
            batch_size: 1,
            priority: 0,
            resreq: Some(ResourceRequirement::default()),
        }
    }

    mod create_task {
        use super::*;

        #[tokio::test]
        async fn creates_task_in_session() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/task-test-ssn");
            storage.create_session(attr).await.unwrap();

            let task = storage
                .create_task("default/task-test-ssn".to_string(), None, None)
                .await
                .unwrap();

            assert_eq!(task.state, TaskState::Pending);
        }

        #[tokio::test]
        async fn creates_task_with_input() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/task-input-ssn");
            storage.create_session(attr).await.unwrap();

            let input = bytes::Bytes::from(vec![1u8, 2, 3]);
            let task = storage
                .create_task(
                    "default/task-input-ssn".to_string(),
                    Some(input.clone()),
                    None,
                )
                .await
                .unwrap();

            assert_eq!(task.input, Some(input));
        }

        #[tokio::test]
        async fn creates_task_with_affinity() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/task-affinity-ssn");
            storage.create_session(attr).await.unwrap();

            let affinity = std::collections::HashSet::from([
                bytes::Bytes::from_static(b"model-a:prefix-1"),
                bytes::Bytes::from_static(b"model-a:prefix-2"),
            ]);
            let task = storage
                .create_task(
                    "default/task-affinity-ssn".to_string(),
                    None,
                    Some(crate::apis::TaskOptions {
                        affinity: affinity.clone(),
                    }),
                )
                .await
                .unwrap();

            assert_eq!(task.affinity, affinity);
            let session = storage
                .get_session_ptr("default/task-affinity-ssn".to_string())
                .unwrap();
            let session = lock_ptr!(session).unwrap();
            let pending = session
                .tasks_index
                .get(&crate::apis::TaskState::Pending)
                .unwrap();
            assert_eq!(pending.len(), 1);
            assert_eq!(
                lock_ptr!(pending.values().next().unwrap())
                    .unwrap()
                    .affinity,
                affinity
            );
        }

        #[tokio::test]
        async fn creates_multiple_tasks_with_unique_ids() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/multi-task-ssn");
            storage.create_session(attr).await.unwrap();

            let task1 = storage
                .create_task("default/multi-task-ssn".to_string(), None, None)
                .await
                .unwrap();
            let task2 = storage
                .create_task("default/multi-task-ssn".to_string(), None, None)
                .await
                .unwrap();

            assert_ne!(task1.id, task2.id);
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_session() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let result = storage
                .create_task("nonexistent-ssn".to_string(), None, None)
                .await;
            assert!(result.is_err());
        }
    }

    mod get_task {
        use super::*;

        #[tokio::test]
        async fn returns_task_by_id() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/get-task-ssn");
            storage.create_session(attr).await.unwrap();

            let created_task = storage
                .create_task("default/get-task-ssn".to_string(), None, None)
                .await
                .unwrap();

            let retrieved_task = storage
                .get_task("default/get-task-ssn".to_string(), created_task.number)
                .unwrap();

            assert_eq!(retrieved_task.number, created_task.number);
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_task() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/no-task-ssn");
            storage.create_session(attr).await.unwrap();

            let result = storage.get_task("default/no-task-ssn".to_string(), 999);
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_session() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let result = storage.get_task("nonexistent-ssn".to_string(), 1);
            assert!(result.is_err());
        }

        #[tokio::test]
        async fn get_task_ptr_returns_pointer() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/ptr-task-ssn");
            storage.create_session(attr).await.unwrap();

            let created_task = storage
                .create_task("default/ptr-task-ssn".to_string(), None, None)
                .await
                .unwrap();

            let gid = crate::apis::TaskGID::from_session_path(
                "default/ptr-task-ssn",
                created_task.number,
            )
            .unwrap();
            let task_ptr = storage.get_task_ptr(gid).unwrap();
            let task = lock_ptr!(task_ptr).unwrap();

            assert_eq!(task.number, created_task.number);
        }
    }

    mod list_tasks {
        use super::*;

        #[tokio::test]
        async fn returns_empty_list_for_session_with_no_tasks() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/empty-task-ssn");
            storage.create_session(attr).await.unwrap();

            let tasks = storage
                .list_tasks("default/empty-task-ssn".to_string())
                .unwrap();
            assert!(tasks.is_empty());
        }

        #[tokio::test]
        async fn returns_all_tasks_in_session() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/list-task-ssn");
            storage.create_session(attr).await.unwrap();

            for _ in 0..5 {
                storage
                    .create_task("default/list-task-ssn".to_string(), None, None)
                    .await
                    .unwrap();
            }

            let tasks = storage
                .list_tasks("default/list-task-ssn".to_string())
                .unwrap();
            assert_eq!(tasks.len(), 5);
        }

        #[tokio::test]
        async fn returns_error_for_nonexistent_session() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let result = storage.list_tasks("nonexistent-ssn".to_string());
            assert!(result.is_err());
        }
    }

    mod update_task_state {
        use super::*;

        #[tokio::test]
        async fn updates_task_to_running() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/update-state-ssn");
            storage.create_session(attr).await.unwrap();

            let task = storage
                .create_task("default/update-state-ssn".to_string(), None, None)
                .await
                .unwrap();

            let ssn_ptr = storage
                .get_session_ptr("default/update-state-ssn".to_string())
                .unwrap();
            let gid =
                crate::apis::TaskGID::from_session_path("default/update-state-ssn", task.number)
                    .unwrap();
            let task_ptr = storage.get_task_ptr(gid).unwrap();

            storage
                .update_task_state(ssn_ptr, task_ptr, TaskState::Running, None)
                .await
                .unwrap();

            let updated_task = storage
                .get_task("default/update-state-ssn".to_string(), task.number)
                .unwrap();
            assert_eq!(updated_task.state, TaskState::Running);
        }

        #[tokio::test]
        async fn updates_task_with_message() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/state-msg-ssn");
            storage.create_session(attr).await.unwrap();

            let task = storage
                .create_task("default/state-msg-ssn".to_string(), None, None)
                .await
                .unwrap();

            let ssn_ptr = storage
                .get_session_ptr("default/state-msg-ssn".to_string())
                .unwrap();
            let gid = crate::apis::TaskGID::from_session_path("default/state-msg-ssn", task.number)
                .unwrap();
            let task_ptr = storage.get_task_ptr(gid).unwrap();

            storage
                .update_task_state(
                    ssn_ptr,
                    task_ptr,
                    TaskState::Running,
                    Some("Starting execution".to_string()),
                )
                .await
                .unwrap();

            let updated_task = storage
                .get_task("default/state-msg-ssn".to_string(), task.number)
                .unwrap();
            assert_eq!(updated_task.state, TaskState::Running);
        }
    }

    mod update_task_result {
        use super::*;

        #[tokio::test]
        async fn updates_task_to_succeeded() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/result-ssn");
            storage.create_session(attr).await.unwrap();

            let task = storage
                .create_task("default/result-ssn".to_string(), None, None)
                .await
                .unwrap();

            let ssn_ptr = storage
                .get_session_ptr("default/result-ssn".to_string())
                .unwrap();
            let gid =
                crate::apis::TaskGID::from_session_path("default/result-ssn", task.number).unwrap();
            let task_ptr = storage.get_task_ptr(gid).unwrap();

            let result = TaskResult {
                state: TaskState::Succeed,
                message: None,
                output: Some(bytes::Bytes::from(vec![42u8, 43, 44])),
            };

            storage
                .update_task_result(ssn_ptr, task_ptr, result)
                .await
                .unwrap();

            let updated_task = storage
                .get_task("default/result-ssn".to_string(), task.number)
                .unwrap();
            assert_eq!(updated_task.state, TaskState::Succeed);
            assert!(updated_task.output.is_some());
            assert!(updated_task.completion_time.is_some());
        }

        #[tokio::test]
        async fn updates_task_to_failed_with_message() {
            let ctx = test_context();
            let storage = storage::new_ptr(&ctx).await.unwrap();

            let attr = create_session_attr("default/fail-result-ssn");
            storage.create_session(attr).await.unwrap();

            let task = storage
                .create_task("default/fail-result-ssn".to_string(), None, None)
                .await
                .unwrap();

            let ssn_ptr = storage
                .get_session_ptr("default/fail-result-ssn".to_string())
                .unwrap();
            let gid =
                crate::apis::TaskGID::from_session_path("default/fail-result-ssn", task.number)
                    .unwrap();
            let task_ptr = storage.get_task_ptr(gid).unwrap();

            let result = TaskResult {
                state: TaskState::Failed,
                message: Some("Something went wrong".to_string()),
                output: None,
            };

            storage
                .update_task_result(ssn_ptr, task_ptr, result)
                .await
                .unwrap();

            let updated_task = storage
                .get_task("default/fail-result-ssn".to_string(), task.number)
                .unwrap();
            assert_eq!(updated_task.state, TaskState::Failed);
        }
    }
}
