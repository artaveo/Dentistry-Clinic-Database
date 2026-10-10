-- Phase 5A: the shared document and print engine (ADR-13). Every printed clinical document — prescription
-- (5.9), consent form (5.10), after-treatment instructions, referral, imaging/lab request, medical
-- certificate, lab work order, record summary (5.10b) — is one row here with its own number series
-- (`RX-1405-000001`). What was printed is kept as issued (patient, doctor and content snapshots), so a
-- reprint is identical; a mistake is corrected by voiding it with a reason and issuing a new one.

CREATE TABLE document_counter (
    prefix TEXT NOT NULL,                             -- RX, CF, PO, RF, IR, MC, LO, MR
    year   INTEGER NOT NULL,                          -- Shamsi year of issue
    value  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (prefix, year)
) STRICT;

CREATE TABLE clinical_document (
    id              TEXT PRIMARY KEY,
    number          TEXT NOT NULL UNIQUE,
    kind            TEXT NOT NULL CHECK (kind IN ('prescription', 'consent', 'post_op', 'referral', 'imaging_request',
                                                  'certificate', 'lab_order', 'record_summary')),
    patient_id      TEXT NOT NULL REFERENCES patient(id),
    doctor_id       TEXT REFERENCES doctor(id),
    appointment_id  TEXT REFERENCES appointment(id),
    language        TEXT NOT NULL CHECK (language IN ('fa', 'ps', 'en')),
    paper           TEXT NOT NULL CHECK (paper IN ('a4', 'a5', 'a6')),
    patient_json    TEXT NOT NULL,                    -- DocumentPatient as printed
    doctor_json     TEXT,                             -- DocumentDoctor as printed
    content_json    TEXT NOT NULL,                    -- DocumentContent
    issued_at       TEXT NOT NULL,
    print_count     INTEGER NOT NULL DEFAULT 0,
    last_printed_at TEXT,
    attachment_id   TEXT REFERENCES patient_attachment(id), -- the signed copy, scanned back in
    status          TEXT NOT NULL DEFAULT 'issued' CHECK (status IN ('issued', 'void')),
    void_reason     TEXT,
    created_at      TEXT NOT NULL,
    created_by      TEXT,
    updated_at      TEXT NOT NULL,
    updated_by      TEXT,
    deleted_at      TEXT,
    version         INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_clinical_document_patient ON clinical_document(patient_id, issued_at DESC) WHERE deleted_at IS NULL;

-- The clinic's formulary (5.9): seeded once from core/seeds/drugs.psv (common dental medicines of the
-- essential medicines list), then the clinic's own to edit. `classes` drive the prescription warnings.
CREATE TABLE drug (
    id             TEXT PRIMARY KEY,
    code           TEXT NOT NULL UNIQUE,
    name           TEXT NOT NULL,                     -- Latin scientific/brand name, as the pharmacy reads it
    form           TEXT NOT NULL,
    strength       TEXT,
    classes        TEXT,                              -- 'penicillin;…'
    quantity       TEXT,
    dose           TEXT,
    times_per_day  INTEGER,
    timing         TEXT CHECK (timing IN ('after_food', 'before_food', 'with_food', 'morning', 'bedtime')),
    days           INTEGER,
    as_needed      INTEGER NOT NULL DEFAULT 0 CHECK (as_needed IN (0, 1)),
    is_system      INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active      INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at     TEXT NOT NULL,
    created_by     TEXT,
    updated_at     TEXT NOT NULL,
    updated_by     TEXT,
    deleted_at     TEXT,
    version        INTEGER NOT NULL DEFAULT 1
) STRICT;

-- Ready-made prescriptions ("after an extraction"), seeded once from core/seeds/rx_templates.json.
CREATE TABLE rx_template (
    id          TEXT PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    name_fa     TEXT NOT NULL,
    name_ps     TEXT NOT NULL,
    name_en     TEXT NOT NULL,
    items_json  TEXT NOT NULL,                        -- RxItem[]
    is_system   INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

-- Consent forms and after-treatment instructions (5.10, 5.10b) in three languages. Built-in ones follow
-- core/seeds/document_templates.json until the clinic rewrites them (`customized`), audited.
CREATE TABLE document_template (
    id          TEXT PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('consent', 'post_op')),
    code        TEXT NOT NULL UNIQUE,
    paper       TEXT NOT NULL CHECK (paper IN ('a4', 'a5', 'a6')),
    is_system   INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    customized  INTEGER NOT NULL DEFAULT 0 CHECK (customized IN (0, 1)),
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE document_template_text (
    template_id   TEXT NOT NULL REFERENCES document_template(id),
    language_code TEXT NOT NULL CHECK (language_code IN ('fa', 'ps', 'en')),
    title         TEXT NOT NULL,
    body          TEXT NOT NULL,
    PRIMARY KEY (template_id, language_code)
) STRICT;
