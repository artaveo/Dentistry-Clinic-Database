-- Clinic metadata, settings and the append-only audit log (roadmap 1.3, 1.6).

CREATE TABLE db_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;

CREATE TABLE setting (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    updated_by TEXT
) STRICT;

-- who · what · when · entity · entity_id · old_value · new_value · computer
CREATE TABLE audit_log (
    id         INTEGER PRIMARY KEY,   -- monotonic; gives an unambiguous order
    at         TEXT NOT NULL,         -- UTC ISO-8601 (ADR-07)
    user_id    TEXT,
    username   TEXT,
    action     TEXT NOT NULL,
    entity     TEXT,
    entity_id  TEXT,
    old_value  TEXT,                  -- JSON
    new_value  TEXT,                  -- JSON
    computer   TEXT NOT NULL
) STRICT;

CREATE INDEX idx_audit_at ON audit_log(at);
CREATE INDEX idx_audit_entity ON audit_log(entity, entity_id);

-- Append-only, enforced by the database itself, not only by the UI.
CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log
BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
