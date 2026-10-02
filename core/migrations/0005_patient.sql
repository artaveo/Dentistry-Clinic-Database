-- Patients & medical records (roadmap Phase 3). Reference lookups (gender,
-- province/district, referral_source, relationship) point at the generic
-- reference-data tables from 0003_reference.sql — no new lookup tables needed.

CREATE TABLE counter (
    name  TEXT PRIMARY KEY,
    value INTEGER NOT NULL DEFAULT 0
) STRICT;
INSERT INTO counter(name, value) VALUES ('patient_number', 0);

CREATE TABLE patient (
    id                                 TEXT PRIMARY KEY,         -- UUIDv7
    patient_number                     TEXT NOT NULL,            -- P-000123 (ADR-05)
    full_name                          TEXT NOT NULL,
    father_name                        TEXT,
    preferred_language                 TEXT CHECK (preferred_language IN ('fa', 'ps', 'en')),
    gender_id                          TEXT REFERENCES reference_item(id),
    date_of_birth                      TEXT,                     -- ISO date; a patient may have this or approximate_age
    approximate_age                    INTEGER,
    phone                              TEXT,
    secondary_phone                    TEXT,
    province_id                        TEXT REFERENCES province(id),
    district_id                        TEXT REFERENCES district(id),
    address                            TEXT,
    emergency_contact_name             TEXT,
    emergency_contact_phone            TEXT,
    emergency_contact_relationship_id  TEXT REFERENCES reference_item(id),
    referral_source_id                 TEXT REFERENCES reference_item(id),
    notes                              TEXT,
    registration_date                  TEXT NOT NULL,            -- ISO date
    status                             TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    merged_into_id                     TEXT REFERENCES patient(id), -- set when this record lost a merge (3.1)
    created_at                         TEXT NOT NULL,
    created_by                         TEXT,
    updated_at                         TEXT NOT NULL,
    updated_by                         TEXT,
    deleted_at                         TEXT,
    version                            INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE UNIQUE INDEX ux_patient_number ON patient(patient_number) WHERE deleted_at IS NULL;
CREATE INDEX idx_patient_phone ON patient(phone) WHERE deleted_at IS NULL;
CREATE INDEX idx_patient_father_name ON patient(father_name) WHERE deleted_at IS NULL;
CREATE INDEX idx_patient_name ON patient(full_name) WHERE deleted_at IS NULL;

-- Full-text search (roadmap 3.4): maintained explicitly in Rust (patient.rs),
-- not by triggers, so normalization (core::normalize) happens in one place.
-- `search_text` holds the normalized, space-joined searchable fields.
CREATE VIRTUAL TABLE patient_fts USING fts5(patient_id UNINDEXED, search_text, tokenize = 'unicode61');

-- Medical history (3.2/3.3): one current row per patient; the change history
-- required by 3.3 comes from the existing append-only audit log (filtered by
-- entity_id), not a separate versioned table.
CREATE TABLE patient_medical_history (
    id                  TEXT PRIMARY KEY,
    patient_id          TEXT NOT NULL UNIQUE REFERENCES patient(id),
    allergies           TEXT,
    current_medications TEXT,
    chronic_conditions  TEXT,
    dental_history      TEXT,
    previous_surgeries  TEXT,
    notes               TEXT,
    created_at          TEXT NOT NULL,
    created_by          TEXT,
    updated_at          TEXT NOT NULL,
    updated_by          TEXT,
    deleted_at          TEXT,
    version             INTEGER NOT NULL DEFAULT 1
) STRICT;

-- Attachments (3.5, ADR-12): metadata only here; bytes live hash-named under
-- the app data folder (core/src/attachment.rs), encrypted like the database.
CREATE TABLE patient_attachment (
    id                TEXT PRIMARY KEY,
    patient_id        TEXT NOT NULL REFERENCES patient(id),
    kind              TEXT NOT NULL CHECK (kind IN ('xray', 'photo', 'document', 'scan', 'consent_form', 'other')),
    file_name         TEXT NOT NULL,
    mime_type         TEXT NOT NULL,
    sha256            TEXT NOT NULL,
    size_bytes        INTEGER NOT NULL,
    tooth             TEXT,
    description       TEXT,
    has_thumbnail     INTEGER NOT NULL DEFAULT 0 CHECK (has_thumbnail IN (0, 1)),
    captured_at       TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    created_by        TEXT,
    updated_at        TEXT NOT NULL,
    updated_by        TEXT,
    deleted_at        TEXT,
    version           INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_patient_attachment_patient ON patient_attachment(patient_id) WHERE deleted_at IS NULL;
