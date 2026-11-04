-- Phase 10: Backup Manager (roadmap 10.2-10.5). Licensing lives in files beside the database,
-- not here, so restoring an old backup can never bring back an expired trial.

-- A USB stick or external disk the Owner chose with "Use for Backup" (ADR-22).
CREATE TABLE backup_device (
    id               TEXT PRIMARY KEY,        -- the device_id written in the marker file
    vendor           TEXT NOT NULL,
    product          TEXT NOT NULL,
    hardware_serial  TEXT,                    -- NULL when the device has no credible serial
    label            TEXT,
    trusted_at       TEXT NOT NULL,
    trusted_by       TEXT,
    last_seen_at     TEXT,
    deleted_at       TEXT
) STRICT;

-- Where each backup was copied after the local copy: the USB device and the cloud-synced folder.
CREATE TABLE backup_copy (
    backup_id   TEXT NOT NULL REFERENCES backup_log(id),
    target      TEXT NOT NULL CHECK (target IN ('usb', 'cloud')),
    status      TEXT NOT NULL CHECK (status IN ('done', 'pending', 'failed', 'skipped')),
    device_id   TEXT,
    updated_at  TEXT NOT NULL,
    detail      TEXT,
    PRIMARY KEY (backup_id, target)
) STRICT;

CREATE INDEX idx_backup_copy_status ON backup_copy(target, status);
