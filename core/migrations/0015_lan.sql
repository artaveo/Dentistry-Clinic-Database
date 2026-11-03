-- LAN mode (roadmap Phase 7). The clinic server's identity and the paired computers live in the
-- encrypted database, so a Backup/Restore moves them to a new server computer together with the
-- clinic data (7.6) and the paired computers keep trusting it (ADR-18: the certificate is kept).

-- The self-signed TLS certificate and its private key (a single row).
CREATE TABLE lan_identity (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    cert_der    BLOB NOT NULL,
    key_der     BLOB NOT NULL,
    created_at  TEXT NOT NULL
) STRICT;

-- One row per paired computer. Only the SHA-256 of the device token is stored.
CREATE TABLE lan_device (
    id            TEXT PRIMARY KEY,               -- random 64-bit hex (also sent to the device)
    name          TEXT NOT NULL,
    token_hash    TEXT NOT NULL,
    paired_at     TEXT NOT NULL,
    last_seen_at  TEXT,
    last_user     TEXT,
    last_address  TEXT,
    revoked_at    TEXT
) STRICT;

CREATE INDEX idx_lan_device_active ON lan_device(revoked_at);
