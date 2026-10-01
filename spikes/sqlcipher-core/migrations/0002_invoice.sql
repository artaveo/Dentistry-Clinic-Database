CREATE TABLE invoice (
    id          TEXT PRIMARY KEY,
    patient_id  TEXT NOT NULL REFERENCES patient(id),
    total_minor INTEGER NOT NULL CHECK (total_minor >= 0),  -- AFN × 100 (ADR-06)
    issued_at   TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_invoice_patient ON invoice(patient_id) WHERE deleted_at IS NULL;
