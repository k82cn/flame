/*
Copyright 2026 The Flame Authors.
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

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use common::ctx::FlameCluster;
use common::FlameError;
use rpc::flame::v1::frontend_client::FrontendClient;
use rpc::flame::v1::{self as flame_rpc, ListSessionsRequest};
use tokio::time::{interval, MissedTickBehavior};
use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::Request;

use crate::cache::{ObjectCache, ObjectKey, ObjectMetadata};

const LIST_SESSIONS_TIMEOUT: Duration = Duration::from_secs(5);
const STALE_GRACE_MILLIS: i64 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SessionSnapshot {
    creation_time: i64,
}

type SessionMap = HashMap<String, SessionSnapshot>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StaleReason {
    PreviousSession,
    MissingSession,
}

#[derive(Clone, Debug)]
struct Candidate {
    session: String,
    metadata: ObjectMetadata,
    reason: StaleReason,
}

#[derive(Default)]
struct ReconcileStats {
    candidates: usize,
    deleted: usize,
    failures: usize,
}

#[async_trait]
trait SessionLister: Send {
    async fn list_sessions(&mut self) -> Result<SessionMap, FlameError>;
}

struct FrontendSessionLister {
    client: FrontendClient<Channel>,
}

impl FrontendSessionLister {
    fn new(cluster: &FlameCluster) -> Result<Self, FlameError> {
        let mut endpoint = Endpoint::from_shared(cluster.endpoint.clone()).map_err(|error| {
            FlameError::InvalidConfig(format!(
                "invalid FSM frontend endpoint <{}>: {}",
                cluster.endpoint, error
            ))
        })?;
        if cluster.requires_tls() {
            let tls = match cluster.tls.as_ref() {
                Some(tls) => tls.client_tls_config()?,
                None => ClientTlsConfig::new().with_native_roots(),
            };
            endpoint = endpoint.tls_config(tls).map_err(|error| {
                FlameError::InvalidConfig(format!(
                    "invalid TLS configuration for FSM frontend <{}>: {}",
                    cluster.endpoint, error
                ))
            })?;
        }
        Ok(Self {
            client: FrontendClient::new(endpoint.connect_lazy()),
        })
    }
}

#[async_trait]
impl SessionLister for FrontendSessionLister {
    async fn list_sessions(&mut self) -> Result<SessionMap, FlameError> {
        // No application or workspace filter: absence is meaningful only in a
        // complete cluster-wide snapshot.
        let mut request = Request::new(ListSessionsRequest::default());
        request.set_timeout(LIST_SESSIONS_TIMEOUT);
        let sessions = self
            .client
            .list_sessions(request)
            .await
            .map_err(|error| FlameError::Network(error.to_string()))?
            .into_inner()
            .sessions;
        validate_sessions(sessions)
    }
}

#[async_trait]
trait GarbageCollectableCache: Send + Sync {
    async fn list_all(&self) -> Result<Vec<ObjectMetadata>, FlameError>;
    async fn delete_if_unchanged(&self, expected: &ObjectMetadata) -> Result<bool, FlameError>;
}

#[async_trait]
impl GarbageCollectableCache for ObjectCache {
    async fn list_all(&self) -> Result<Vec<ObjectMetadata>, FlameError> {
        ObjectCache::list_all(self).await
    }

    async fn delete_if_unchanged(&self, expected: &ObjectMetadata) -> Result<bool, FlameError> {
        ObjectCache::delete_if_unchanged(self, expected).await
    }
}

pub(crate) struct SessionGarbageCollector {
    cache: Arc<dyn GarbageCollectableCache>,
    sessions: Box<dyn SessionLister>,
    interval: Duration,
}

impl SessionGarbageCollector {
    pub(crate) fn new(
        cache: Arc<ObjectCache>,
        cluster: &FlameCluster,
        interval: Duration,
    ) -> Result<Self, FlameError> {
        Ok(Self {
            cache,
            sessions: Box::new(FrontendSessionLister::new(cluster)?),
            interval,
        })
    }

    pub(crate) async fn run(mut self) {
        let mut ticker = interval(self.interval);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            let started = std::time::Instant::now();
            match self.reconcile(now_millis()).await {
                Ok(stats) => tracing::info!(
                    candidates = stats.candidates,
                    deleted = stats.deleted,
                    failures = stats.failures,
                    elapsed_ms = started.elapsed().as_millis(),
                    "session cache garbage collection completed"
                ),
                Err(error) => tracing::warn!(
                    error = %error,
                    elapsed_ms = started.elapsed().as_millis(),
                    "session cache garbage collection skipped"
                ),
            }
        }
    }

    async fn reconcile(&mut self, now: i64) -> Result<ReconcileStats, FlameError> {
        let objects = self.cache.list_all().await?;
        if objects.is_empty() {
            return Ok(ReconcileStats::default());
        }

        let sessions = self.sessions.list_sessions().await?;
        let mut candidates = Vec::new();
        for metadata in objects {
            let key = match ObjectKey::try_from(metadata.key.as_str()) {
                Ok(key) => key,
                Err(error) => {
                    tracing::warn!(key = %metadata.key, error = %error,
                        "ignoring invalid cache key during garbage collection");
                    continue;
                }
            };
            if matches!(key.session.as_str(), "pkg" | "bootstrap" | "shared") {
                continue;
            }
            let session = key.to_prefix();
            if let Some(reason) =
                classify_stale(sessions.get(&session), metadata.creation_time, now)
            {
                candidates.push(Candidate {
                    session,
                    metadata,
                    reason,
                });
            }
        }

        let mut stats = ReconcileStats {
            candidates: candidates.len(),
            ..ReconcileStats::default()
        };
        if !candidates.is_empty() {
            // Recheck the complete snapshot before deleting anything. A failed
            // second list leaves all objects intact.
            let current = self.sessions.list_sessions().await?;
            candidates.retain_mut(|candidate| {
                match classify_stale(
                    current.get(&candidate.session),
                    candidate.metadata.creation_time,
                    now,
                ) {
                    Some(reason) => {
                        candidate.reason = reason;
                        true
                    }
                    None => false,
                }
            });
        }

        for candidate in candidates {
            match self.cache.delete_if_unchanged(&candidate.metadata).await {
                Ok(true) => {
                    stats.deleted += 1;
                    tracing::debug!(session = %candidate.session, key = %candidate.metadata.key,
                        reason = ?candidate.reason, "deleted stale cache object");
                }
                Ok(false) => {
                    tracing::debug!(session = %candidate.session, key = %candidate.metadata.key,
                    "stale cache candidate changed before deletion")
                }
                Err(error) => {
                    stats.failures += 1;
                    tracing::warn!(session = %candidate.session, key = %candidate.metadata.key,
                        error = %error, "failed to delete stale cache object");
                }
            }
        }
        Ok(stats)
    }

    #[cfg(test)]
    fn with_dependencies(
        cache: Arc<dyn GarbageCollectableCache>,
        sessions: Box<dyn SessionLister>,
    ) -> Self {
        Self {
            cache,
            sessions,
            interval: Duration::from_secs(60),
        }
    }
}

fn validate_sessions(sessions: Vec<flame_rpc::Session>) -> Result<SessionMap, FlameError> {
    let mut result = HashMap::with_capacity(sessions.len());
    for session in sessions {
        let metadata = session.metadata.ok_or_else(|| {
            FlameError::InvalidState("ListSessions returned a session without metadata".into())
        })?;
        let gid = format!("{}/{}", metadata.workspace, metadata.name);
        let key = ObjectKey::from_path(&gid).map_err(|error| {
            FlameError::InvalidState(format!(
                "ListSessions returned invalid session path: {error}"
            ))
        })?;
        if key.object_id.is_some() || key.is_all_sessions() {
            return Err(FlameError::InvalidState(format!(
                "ListSessions returned inconsistent session <{}>",
                metadata.id
            )));
        }
        let spec = session.spec.ok_or_else(|| {
            FlameError::InvalidState(format!(
                "ListSessions returned session <{}> without spec",
                metadata.id
            ))
        })?;
        let (workspace, _) =
            common::apis::parse_application_path(&spec.application).map_err(|error| {
                FlameError::InvalidState(format!(
                    "ListSessions returned invalid parent application: {error}"
                ))
            })?;
        if workspace != key.workspace {
            return Err(FlameError::InvalidState(format!(
                "ListSessions returned session <{}> in wrong workspace",
                metadata.id
            )));
        }
        let status = session.status.ok_or_else(|| {
            FlameError::InvalidState(format!(
                "ListSessions returned session <{}> without status",
                metadata.id
            ))
        })?;
        if status.creation_time <= 0 || flame_rpc::SessionState::try_from(status.state).is_err() {
            return Err(FlameError::InvalidState(format!(
                "ListSessions returned invalid status for <{}>",
                metadata.id
            )));
        }
        if result
            .insert(
                gid.clone(),
                SessionSnapshot {
                    creation_time: status.creation_time,
                },
            )
            .is_some()
        {
            return Err(FlameError::InvalidState(format!(
                "ListSessions returned duplicate session <{}>",
                metadata.id
            )));
        }
    }
    Ok(result)
}

fn classify_stale(
    session: Option<&SessionSnapshot>,
    object_creation_time: i64,
    now: i64,
) -> Option<StaleReason> {
    match session {
        Some(snapshot)
            if object_creation_time.saturating_add(STALE_GRACE_MILLIS) < snapshot.creation_time =>
        {
            Some(StaleReason::PreviousSession)
        }
        Some(_) => None,
        None if object_creation_time.saturating_add(STALE_GRACE_MILLIS) < now => {
            Some(StaleReason::MissingSession)
        }
        None => None,
    }
}

fn now_millis() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(millis).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, VecDeque};
    use std::sync::Mutex;

    fn object(key: &str, creation_time: i64) -> ObjectMetadata {
        ObjectMetadata {
            endpoint: String::new(),
            key: key.into(),
            version: 1,
            size: 1,
            delta_count: 0,
            creation_time,
            data_type: "raw".into(),
        }
    }
    fn session(id: &str, creation_time: i64) -> flame_rpc::Session {
        let (workspace, name) = id.split_once('/').unwrap();
        flame_rpc::Session {
            metadata: Some(flame_rpc::Metadata {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.into(),
                workspace: workspace.into(),
            }),
            spec: Some(flame_rpc::SessionSpec {
                application: format!("{workspace}/app"),
                ..Default::default()
            }),
            status: Some(flame_rpc::SessionStatus {
                creation_time,
                state: flame_rpc::SessionState::Open as i32,
                ..Default::default()
            }),
        }
    }
    struct MockLister {
        responses: VecDeque<Result<SessionMap, FlameError>>,
    }
    #[async_trait]
    impl SessionLister for MockLister {
        async fn list_sessions(&mut self) -> Result<SessionMap, FlameError> {
            self.responses.pop_front().expect("unexpected session list")
        }
    }
    struct MockCache {
        objects: Vec<ObjectMetadata>,
        deleted: Mutex<Vec<String>>,
        fail: HashSet<String>,
    }
    #[async_trait]
    impl GarbageCollectableCache for MockCache {
        async fn list_all(&self) -> Result<Vec<ObjectMetadata>, FlameError> {
            Ok(self.objects.clone())
        }
        async fn delete_if_unchanged(&self, expected: &ObjectMetadata) -> Result<bool, FlameError> {
            if self.fail.contains(&expected.key) {
                return Err(FlameError::Storage("injected failure".into()));
            }
            self.deleted.lock().unwrap().push(expected.key.clone());
            Ok(true)
        }
    }
    fn collector(
        cache: Arc<MockCache>,
        responses: Vec<Result<SessionMap, FlameError>>,
    ) -> SessionGarbageCollector {
        SessionGarbageCollector::with_dependencies(
            cache,
            Box::new(MockLister {
                responses: responses.into(),
            }),
        )
    }

    #[test]
    fn validates_complete_session_identity() {
        let sessions = validate_sessions(vec![session("team/run", 100)]).unwrap();
        assert_eq!(sessions["team/run"].creation_time, 100);
        let mut wrong = session("team/run", 100);
        wrong.spec.as_mut().unwrap().application = "other/app".into();
        assert!(validate_sessions(vec![wrong]).is_err());
        assert!(
            validate_sessions(vec![session("team/run", 100), session("team/run", 200)]).is_err()
        );
    }

    #[tokio::test]
    async fn removes_missing_session_but_preserves_reserved_namespaces() {
        let cache = Arc::new(MockCache {
            objects: vec![
                object("team/run/data", 1),
                object("team/pkg/archive", 1),
                object("team/bootstrap/config", 1),
                object("team/shared/value", 1),
            ],
            deleted: Mutex::new(Vec::new()),
            fail: HashSet::new(),
        });
        let mut gc = collector(cache.clone(), vec![Ok(HashMap::new()), Ok(HashMap::new())]);
        let stats = gc.reconcile(20_000).await.unwrap();
        assert_eq!(stats.deleted, 1);
        assert_eq!(&*cache.deleted.lock().unwrap(), &["team/run/data"]);
    }

    #[tokio::test]
    async fn rechecks_before_deletion_and_aborts_on_incomplete_snapshot() {
        let cache = Arc::new(MockCache {
            objects: vec![object("team/run/data", 1)],
            deleted: Mutex::new(Vec::new()),
            fail: HashSet::new(),
        });
        let appeared = validate_sessions(vec![session("team/run", 5_000)]).unwrap();
        let mut gc = collector(cache.clone(), vec![Ok(HashMap::new()), Ok(appeared)]);
        assert_eq!(gc.reconcile(20_000).await.unwrap().deleted, 0);
        assert!(cache.deleted.lock().unwrap().is_empty());
        let mut gc = collector(
            cache.clone(),
            vec![
                Ok(HashMap::new()),
                Err(FlameError::Network("unavailable".into())),
            ],
        );
        assert!(gc.reconcile(20_000).await.is_err());
        assert!(cache.deleted.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn recreated_session_removes_only_old_objects() {
        let cache = Arc::new(MockCache {
            objects: vec![
                object("team/run/old", 1),
                object("team/run/current", 16_000),
            ],
            deleted: Mutex::new(Vec::new()),
            fail: HashSet::new(),
        });
        let current = validate_sessions(vec![session("team/run", 15_000)]).unwrap();
        let mut gc = collector(cache.clone(), vec![Ok(current.clone()), Ok(current)]);
        assert_eq!(gc.reconcile(30_000).await.unwrap().deleted, 1);
        assert_eq!(&*cache.deleted.lock().unwrap(), &["team/run/old"]);
    }
}
