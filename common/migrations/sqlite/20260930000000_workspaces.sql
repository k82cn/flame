-- Preserve existing resources in the default workspace while changing every
-- operational key to the scoped name. UUIDs are retained only as metadata.
CREATE TABLE workspaces (name TEXT PRIMARY KEY, create_at INTEGER NOT NULL);
INSERT INTO workspaces VALUES ('default', CAST(strftime('%s','now') AS INTEGER));

CREATE TABLE applications_new (
  id TEXT NOT NULL, workspace TEXT NOT NULL, name TEXT NOT NULL,
  version INTEGER NOT NULL, shim INTEGER NOT NULL, image TEXT, description TEXT,
  labels TEXT, command TEXT, arguments TEXT, environments TEXT,
  working_directory TEXT, max_instances INTEGER NOT NULL, delay_release INTEGER NOT NULL,
  schema TEXT, url TEXT, installer TEXT, creation_time INTEGER NOT NULL,
  state INTEGER NOT NULL, PRIMARY KEY(workspace,name),
  FOREIGN KEY(workspace) REFERENCES workspaces(name)
);
INSERT INTO applications_new
SELECT printf('%s-%s-4%s-%s%s-%s',lower(hex(randomblob(4))),lower(hex(randomblob(2))),lower(substr(hex(randomblob(2)),2)),substr('89ab',abs(random())%4+1,1),lower(substr(hex(randomblob(2)),2)),lower(hex(randomblob(6)))),
  'default',name,version,shim,image,description,labels,command,arguments,environments,
  working_directory,max_instances,delay_release,schema,url,installer,creation_time,state
FROM applications;
-- Legacy cache object paths are <application>/<session>/<object>.
UPDATE applications_new SET url =
  substr(url, 1, instr(url, '://') + 2) ||
  substr(substr(url, instr(url, '://') + 3), 1, instr(substr(url, instr(url, '://') + 3), '/') - 1) ||
  '/default' || substr(substr(url, instr(url, '://') + 3), instr(substr(url, instr(url, '://') + 3), '/'))
WHERE lower(substr(url, 1, instr(url, '://') - 1)) IN ('grpc', 'grpcs', 'grpc+tls', 'grpcs-proxy')
  AND instr(substr(url, instr(url, '://') + 3), '/') > 0;
DROP TABLE applications;
ALTER TABLE applications_new RENAME TO applications;

CREATE TABLE sessions_new (
  id TEXT NOT NULL, workspace TEXT NOT NULL, name TEXT NOT NULL,
  application TEXT NOT NULL, version INTEGER NOT NULL, common_data BLOB, tokens TEXT NOT NULL,
  creation_time INTEGER NOT NULL, completion_time INTEGER, state INTEGER NOT NULL,
  min_instances INTEGER NOT NULL, max_instances INTEGER, batch_size INTEGER NOT NULL,
  priority INTEGER NOT NULL, resreq_cpu INTEGER, resreq_memory INTEGER, resreq_gpu INTEGER,
  PRIMARY KEY(workspace,name),
  FOREIGN KEY(workspace,application) REFERENCES applications(workspace,name)
);
INSERT INTO sessions_new
SELECT printf('%s-%s-4%s-%s%s-%s',lower(hex(randomblob(4))),lower(hex(randomblob(2))),lower(substr(hex(randomblob(2)),2)),substr('89ab',abs(random())%4+1,1),lower(substr(hex(randomblob(2)),2)),lower(hex(randomblob(6)))),
  'default',id,application,version,common_data,tokens,creation_time,completion_time,state,
  min_instances,max_instances,batch_size,priority,resreq_cpu,resreq_memory,resreq_gpu
FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;

CREATE TABLE tasks_new (
  id TEXT NOT NULL, workspace TEXT NOT NULL, session TEXT NOT NULL, name TEXT NOT NULL,
  version INTEGER NOT NULL, input BLOB, output BLOB, affinity TEXT,
  creation_time INTEGER NOT NULL, completion_time INTEGER, state INTEGER NOT NULL,
  PRIMARY KEY(workspace,session,name),
  FOREIGN KEY(workspace,session) REFERENCES sessions(workspace,name) ON DELETE CASCADE
);
INSERT INTO tasks_new
SELECT printf('%s-%s-4%s-%s%s-%s',lower(hex(randomblob(4))),lower(hex(randomblob(2))),lower(substr(hex(randomblob(2)),2)),substr('89ab',abs(random())%4+1,1),lower(substr(hex(randomblob(2)),2)),lower(hex(randomblob(6)))),
  'default',ssn_id,CAST(id AS TEXT),version,input,output,affinity,creation_time,completion_time,state
FROM tasks;
DROP TABLE tasks;
ALTER TABLE tasks_new RENAME TO tasks;

ALTER TABLE nodes ADD COLUMN id TEXT;
UPDATE nodes SET id=printf('%s-%s-4%s-%s%s-%s',lower(hex(randomblob(4))),lower(hex(randomblob(2))),lower(substr(hex(randomblob(2)),2)),substr('89ab',abs(random())%4+1,1),lower(substr(hex(randomblob(2)),2)),lower(hex(randomblob(6))));

CREATE TABLE executors_new (
  id TEXT NOT NULL, workspace TEXT NOT NULL, name TEXT NOT NULL,
  node TEXT NOT NULL, application TEXT NOT NULL, resreq_cpu INTEGER NOT NULL,
  resreq_memory INTEGER NOT NULL, resreq_gpu INTEGER NOT NULL, shim INTEGER NOT NULL,
  task TEXT, session TEXT, creation_time INTEGER NOT NULL, state INTEGER NOT NULL,
  PRIMARY KEY(workspace,name),
  FOREIGN KEY(node) REFERENCES nodes(name) ON DELETE CASCADE
);
INSERT INTO executors_new
SELECT printf('%s-%s-4%s-%s%s-%s',lower(hex(randomblob(4))),lower(hex(randomblob(2))),lower(substr(hex(randomblob(2)),2)),substr('89ab',abs(random())%4+1,1),lower(substr(hex(randomblob(2)),2)),lower(hex(randomblob(6)))),
  'default',id,node,application,resreq_cpu,resreq_memory,resreq_gpu,shim,
  CAST(task_id AS TEXT),ssn_id,creation_time,state FROM executors;
DROP TABLE executors;
ALTER TABLE executors_new RENAME TO executors;
CREATE INDEX idx_executors_node ON executors(node);
CREATE INDEX idx_executors_state ON executors(state);
CREATE INDEX idx_executors_session ON executors(workspace,session);
