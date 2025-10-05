CREATE TABLE IF NOT EXISTS events (
    owner              TEXT NOT NULL,
    parent             TEXT,
    code               INTEGER NOT NULL,
    message            TEXT,
    creation_time      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS events_owner_index ON events (owner, code);
CREATE INDEX IF NOT EXISTS events_owner_parent_index ON events (owner, parent, code);
