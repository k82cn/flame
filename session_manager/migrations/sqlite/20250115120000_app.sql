CREATE TABLE IF NOT EXISTS applications (
    name TEXT NOT NULL,
    image TEXT,
    url TEXT,
    command TEXT,
    arguments TEXT,
    environments TEXT,
    working_directory TEXT,
    description TEXT,
    labels TEXT,
    schema TEXT,
    shim INTEGER NOT NULL,
    max_instances INTEGER NOT NULL,
    delay_release INTEGER NOT NULL,
    creation_time INTEGER NOT NULL,
    state INTEGER NOT NULL,
    PRIMARY KEY (name)
);