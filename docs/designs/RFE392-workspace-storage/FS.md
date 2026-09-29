# Workspace paths and storage

Flame uses internal typed global identifiers (GIDs): `workspace/app` for an application,
`workspace/session` for a session, and `workspace/session/task-number` for a
task. The server assigns task numbers; callers choose the other names. A
session belongs to an application, but its name is unique within the workspace.
Application, session, and task metadata store the workspace name and a
persistent UUID in `id` for debugging. End users address resources by
workspace and name; controllers derive GIDs from those fields and the task's
session parent. App-facing contexts carry workspace and resource names, and a
task carries its server-assigned number. Workspace itself has direct `name`
and `creation_time` fields without metadata. A caller that
omits a workspace uses `default` until authenticated workspace selection is
introduced by the security change.

Workspaces are explicit resources exposed by `CreateWorkspace` and
`ListWorkspaces`. The server creates `default` on startup. Application creation
requires an existing workspace; it does not create one implicitly. Names are
single safe path components, and `pkg`, `bootstrap`, and `shared` are reserved
session names for cache use.

The object cache uses `workspace/session/object`. Its disk storage and garbage
collection preserve the workspace segment, so equal session and object names
in different workspaces remain independent.

## SQLite layout

The configured storage root contains a control database at `flame.db` for
cluster-wide node and executor state. Each workspace has its own directory and
database at `<storage>/<workspace>/flame.db`. The `SqliteEngine` keeps a pool map
keyed by workspace name. `CreateWorkspace` builds the database and metadata in
a staging directory, then publishes the workspace directory atomically after
migrations complete. Interrupted staging directories are ignored on restart.
On restart, workspace directories provide the catalog and the engine reopens
their databases. Session event files live under
`<storage>/<workspace>/events/<session>`.

Application, session, and task rows use their persistent UUID in `id`; there
is no separate UUID column. App and session names are unique within their
workspace database. Tasks are uniquely addressed by `(session name, number)`.
Session rows refer to their application by local app name. The engine adds the
workspace prefix at the internal storage boundary.

Configuration that ends in `.db` resolves the new storage root to the same
path without that extension. Existing single-file databases are not migrated.
