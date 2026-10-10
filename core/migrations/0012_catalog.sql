-- Phase 5A, M2 and M3 of docs/specs/clinical-workflow-v1.md.
--
-- M2: doctors have one or more specialties (reference data of type `specialty`, seeded with nine and
-- extendable by the clinic) and a licence number for their prescriptions' letterhead. A free-text
-- specialty typed before v0.5.0 is matched to the list on start-up (specialty::link_legacy); what
-- matches nothing stays in doctor.specialty and is still shown.
ALTER TABLE doctor ADD COLUMN license_number TEXT;

CREATE TABLE doctor_specialty (
    doctor_id     TEXT NOT NULL REFERENCES doctor(id),
    specialty_id  TEXT NOT NULL REFERENCES reference_item(id),
    PRIMARY KEY (doctor_id, specialty_id)
) STRICT;

-- M3: a three-level service catalog — category → service → variant (a service row whose parent_id is
-- set). Seeded once from core/seeds/services.json, then entirely the clinic's to edit (prices, names,
-- codes). Treatment plans and treatments (5B) and invoices (Phase 6) point at service rows.
CREATE TABLE service_category (
    id            TEXT PRIMARY KEY,
    code          TEXT NOT NULL UNIQUE,
    name_fa       TEXT NOT NULL,
    name_ps       TEXT NOT NULL,
    name_en       TEXT NOT NULL,
    specialty_id  TEXT REFERENCES reference_item(id),
    sort_order    INTEGER NOT NULL DEFAULT 0,
    is_system     INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active     INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at    TEXT NOT NULL,
    created_by    TEXT,
    updated_at    TEXT NOT NULL,
    updated_by    TEXT,
    deleted_at    TEXT,
    version       INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE service (
    id                   TEXT PRIMARY KEY,
    code                 TEXT NOT NULL,                  -- the clinic's short code, e.g. END-02
    category_id          TEXT NOT NULL REFERENCES service_category(id),
    parent_id            TEXT REFERENCES service(id),    -- set = a variant of that service
    name_fa              TEXT NOT NULL,
    name_ps              TEXT NOT NULL,
    name_en              TEXT NOT NULL,
    price                INTEGER NOT NULL DEFAULT 0 CHECK (price >= 0),   -- AFN × 100 (ADR-06)
    specialty_id         TEXT REFERENCES reference_item(id),
    tooth_scope          TEXT NOT NULL CHECK (tooth_scope IN ('tooth', 'teeth', 'quadrant', 'arch', 'mouth', 'none')),
    needs_surface        INTEGER NOT NULL DEFAULT 0 CHECK (needs_surface IN (0, 1)),
    sessions             INTEGER NOT NULL DEFAULT 1 CHECK (sessions BETWEEN 1 AND 60),
    lab_required         INTEGER NOT NULL DEFAULT 0 CHECK (lab_required IN (0, 1)),
    consent_template_id  TEXT REFERENCES document_template(id),
    post_op_template_id  TEXT REFERENCES document_template(id),
    sort_order           INTEGER NOT NULL DEFAULT 0,
    is_system            INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active            INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at           TEXT NOT NULL,
    created_by           TEXT,
    updated_at           TEXT NOT NULL,
    updated_by           TEXT,
    deleted_at           TEXT,
    version              INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE UNIQUE INDEX ux_service_code ON service(code COLLATE NOCASE) WHERE deleted_at IS NULL;
CREATE INDEX idx_service_category ON service(category_id, parent_id, sort_order) WHERE deleted_at IS NULL;

-- M2/M3: what a visit is for, when chosen when booking (doctors of the service's specialty first).
ALTER TABLE appointment ADD COLUMN service_id TEXT REFERENCES service(id);
