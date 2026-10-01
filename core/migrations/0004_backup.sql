-- Basic local backup register (roadmap 1.7; full Backup Manager in Phase 10).
CREATE TABLE backup_log (
    id          TEXT PRIMARY KEY,                 -- UUIDv7
    file_name   TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    size_bytes  INTEGER NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('manual', 'daily', 'pre_migration')),
    verified    INTEGER NOT NULL CHECK (verified IN (0, 1)),
    deleted_at  TEXT                              -- set by retention cleanup
) STRICT;

CREATE INDEX idx_backup_log_created ON backup_log(created_at);
