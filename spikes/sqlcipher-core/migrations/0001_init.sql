-- Spike schema: proves STRICT tables, FK enforcement, soft delete, UUIDv7 text PKs
-- and INTEGER money columns (ADR-03, ADR-05, ADR-06, ADR-10).
CREATE TABLE db_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE patient (
    id          TEXT PRIMARY KEY,           -- UUIDv7
    number      TEXT NOT NULL UNIQUE,       -- human number, e.g. P-000123
    full_name   TEXT NOT NULL,
    phone       TEXT,                       -- stored with Latin digits (ADR-09)
    created_at  TEXT NOT NULL,              -- UTC ISO-8601 (ADR-07)
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;
