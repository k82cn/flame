CREATE TABLE IF NOT EXISTS sessions (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    application     TEXT NOT NULL,

    common_data     BLOB,

    creation_time   INTEGER NOT NULL,
    completion_time INTEGER,

    state           INTEGER NOT NULL
);

CREATE INDEX idx_sessions_name ON sessions(name);
CREATE INDEX idx_sessions_application ON sessions(application);

CREATE TABLE IF NOT EXISTS tasks (
    id              TEXT PRIMARY KEY,
    number          INTEGER NOT NULL,
    session          TEXT NOT NULL,

    input           BLOB,
    output          BLOB,

    creation_time   INTEGER NOT NULL,
    completion_time INTEGER,

    state           INTEGER NOT NULL,

    UNIQUE (session, number)
);
