CREATE TABLE IF NOT EXISTS applications (
    id                  TEXT NOT NULL PRIMARY KEY,
    name                TEXT NOT NULL UNIQUE,
    shim                INTEGER NOT NULL,
    description         TEXT,
    labels              TEXT,

    image               TEXT,
    command             TEXT,
    arguments           TEXT,
    environments        TEXT,
    working_directory   TEXT,
    schema              TEXT,

    max_instances       INTEGER NOT NULL,
    delay_release       INTEGER NOT NULL,
    creation_time       INTEGER NOT NULL,

    state INTEGER       NOT NULL
);

CREATE INDEX idx_applications_name ON applications(name);
