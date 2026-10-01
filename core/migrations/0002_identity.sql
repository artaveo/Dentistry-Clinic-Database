-- Users, roles and granular permissions (roadmap 1.6).
-- Permission codes and the system roles are defined in Rust (auth.rs) and
-- synchronised on every start, so code and database cannot drift.

CREATE TABLE permission (
    code TEXT PRIMARY KEY
) STRICT;

CREATE TABLE role (
    id          TEXT PRIMARY KEY,                 -- UUIDv7
    code        TEXT NOT NULL UNIQUE,
    is_system   INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE role_permission (
    role_id         TEXT NOT NULL REFERENCES role(id),
    permission_code TEXT NOT NULL REFERENCES permission(code),
    PRIMARY KEY (role_id, permission_code)
) STRICT;

CREATE TABLE app_user (
    id                  TEXT PRIMARY KEY,         -- UUIDv7
    username            TEXT NOT NULL,
    display_name        TEXT NOT NULL,
    password_hash       TEXT NOT NULL,            -- Argon2id PHC string
    role_id             TEXT NOT NULL REFERENCES role(id),
    is_active           INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    failed_attempts     INTEGER NOT NULL DEFAULT 0,
    locked_until        TEXT,
    password_changed_at TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    created_by          TEXT,
    updated_at          TEXT NOT NULL,
    updated_by          TEXT,
    deleted_at          TEXT,
    version             INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE UNIQUE INDEX ux_app_user_username ON app_user(username COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE INDEX idx_app_user_role ON app_user(role_id);
