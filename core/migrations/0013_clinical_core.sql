-- Phase 5B (v0.6.0): the dental clinical core — dental chart (5.2–5.4, M5), treatment plans and
-- treatments (5.7, 5.8) with multi-session courses linked to appointments (M4), clinical notes with
-- sign-off and addenda (5.6), periodontal chart (5.5); the patient timeline (5.11) reads them all.
-- Clinical rows are never hard-deleted (ADR-10); prices are AFN × 100 (ADR-06); tooth numbers are FDI.

-- M5: how each service is drawn on the chart, and (M4) the usual days between two of its sessions.
ALTER TABLE service ADD COLUMN chart_symbol TEXT NOT NULL DEFAULT 'none' CHECK (chart_symbol IN (
    'none', 'surface', 'sealant', 'veneer', 'crown', 'root_canal', 'pulpotomy', 'extraction', 'missing', 'implant',
    'abutment', 'implant_crown', 'bridge', 'denture', 'bracket', 'fracture', 'impacted', 'lesion', 'scaling', 'marker'));
ALTER TABLE service ADD COLUMN session_interval_days INTEGER NOT NULL DEFAULT 7
    CHECK (session_interval_days BETWEEN 1 AND 365);

-- The built-in catalog of v0.5.0 gets its drawing and interval (new clinics get them from services.json).
UPDATE service SET chart_symbol = 'surface'      WHERE is_system = 1 AND (code LIKE 'RST-%');
UPDATE service SET chart_symbol = 'root_canal'   WHERE is_system = 1 AND (code LIKE 'END-%');
UPDATE service SET chart_symbol = 'extraction'   WHERE is_system = 1 AND (code LIKE 'SRG-%');
UPDATE service SET chart_symbol = 'crown'        WHERE is_system = 1 AND (code LIKE 'PRS-01%' OR code = 'PED-02');
UPDATE service SET chart_symbol = 'bridge'       WHERE is_system = 1 AND (code LIKE 'PRS-02%');
UPDATE service SET chart_symbol = 'denture'      WHERE is_system = 1 AND (code LIKE 'PRS-03%');
UPDATE service SET chart_symbol = 'implant'      WHERE is_system = 1 AND (code LIKE 'IMP-01%');
UPDATE service SET chart_symbol = 'abutment'     WHERE is_system = 1 AND (code = 'IMP-02');
UPDATE service SET chart_symbol = 'implant_crown' WHERE is_system = 1 AND (code = 'IMP-03');
UPDATE service SET chart_symbol = 'bracket'      WHERE is_system = 1 AND (code LIKE 'ORT-01%');
UPDATE service SET chart_symbol = 'scaling'      WHERE is_system = 1 AND (code LIKE 'PER-%');
UPDATE service SET chart_symbol = 'veneer'       WHERE is_system = 1 AND (code = 'COS-02');
UPDATE service SET chart_symbol = 'marker'       WHERE is_system = 1 AND (code = 'COS-03');
UPDATE service SET chart_symbol = 'pulpotomy'    WHERE is_system = 1 AND (code = 'PED-01');
UPDATE service SET chart_symbol = 'sealant'      WHERE is_system = 1 AND (code = 'PED-03');
UPDATE service SET session_interval_days = 30    WHERE is_system = 1 AND (code LIKE 'ORT-01%' OR code = 'ORT-02');

-- 5.3: the conditions a tooth can have — seeded from core/seeds/tooth_conditions.json, the clinic adds its own.
CREATE TABLE tooth_condition (
    id             TEXT PRIMARY KEY,
    code           TEXT NOT NULL UNIQUE,
    name_fa        TEXT NOT NULL,
    name_ps        TEXT NOT NULL,
    name_en        TEXT NOT NULL,
    symbol         TEXT NOT NULL,
    needs_surface  INTEGER NOT NULL DEFAULT 0 CHECK (needs_surface IN (0, 1)),
    abbreviation   TEXT,
    sort_order     INTEGER NOT NULL DEFAULT 0,
    is_system      INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active      INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at     TEXT NOT NULL,
    created_by     TEXT,
    updated_at     TEXT NOT NULL,
    updated_by     TEXT,
    deleted_at     TEXT,
    version        INTEGER NOT NULL DEFAULT 1
) STRICT;

-- Which teeth a patient's chart shows (a UI choice; the default follows the patient's age).
CREATE TABLE dental_chart (
    patient_id     TEXT PRIMARY KEY REFERENCES patient(id),
    dentition      TEXT NOT NULL CHECK (dentition IN ('permanent', 'primary', 'mixed')),
    primary_slots  TEXT NOT NULL DEFAULT '',          -- '13,14,15,…' (mixed dentition)
    created_at     TEXT NOT NULL,
    created_by     TEXT,
    updated_at     TEXT NOT NULL,
    updated_by     TEXT,
    version        INTEGER NOT NULL DEFAULT 1
) STRICT;

-- Existing conditions found at an examination (the chart's blue layer); removed = soft delete with who/when.
CREATE TABLE tooth_finding (
    id             TEXT PRIMARY KEY,
    patient_id     TEXT NOT NULL REFERENCES patient(id),
    tooth          INTEGER NOT NULL CHECK (tooth BETWEEN 11 AND 85),
    surfaces       TEXT NOT NULL DEFAULT '',
    condition_id   TEXT NOT NULL REFERENCES tooth_condition(id),
    note           TEXT,
    recorded_on    TEXT NOT NULL,                     -- clinic-local date
    created_at     TEXT NOT NULL,
    created_by     TEXT,
    updated_at     TEXT NOT NULL,
    updated_by     TEXT,
    deleted_at     TEXT,
    deleted_by     TEXT,
    version        INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_tooth_finding_patient ON tooth_finding(patient_id, tooth) WHERE deleted_at IS NULL;

-- 5.7: a patient's treatment plans; several proposed ones are alternatives for the patient to choose from.
CREATE TABLE treatment_plan (
    id             TEXT PRIMARY KEY,
    patient_id     TEXT NOT NULL REFERENCES patient(id),
    title          TEXT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'proposed'
        CHECK (status IN ('proposed', 'accepted', 'in_progress', 'completed', 'cancelled')),
    notes          TEXT,
    accepted_at    TEXT,
    completed_at   TEXT,
    cancelled_at   TEXT,
    cancel_reason  TEXT,
    superseded_by  TEXT REFERENCES treatment_plan(id),  -- the alternative the patient chose instead
    created_at     TEXT NOT NULL,
    created_by     TEXT,
    updated_at     TEXT NOT NULL,
    updated_by     TEXT,
    deleted_at     TEXT,
    version        INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_treatment_plan_patient ON treatment_plan(patient_id, created_at) WHERE deleted_at IS NULL;

-- A procedure of a plan (5.7) or, without a plan, a treatment recorded directly (5.8). Once completed it
-- is a treatment done, ready for an invoice (Phase 6 sets billed_at).
CREATE TABLE treatment_item (
    id                TEXT PRIMARY KEY,
    plan_id           TEXT REFERENCES treatment_plan(id),
    patient_id        TEXT NOT NULL REFERENCES patient(id),
    service_id        TEXT NOT NULL REFERENCES service(id),
    teeth             TEXT NOT NULL DEFAULT '',        -- '36' / '14,15,16' (FDI)
    surfaces          TEXT NOT NULL DEFAULT '',        -- 'MOD'
    doctor_id         TEXT REFERENCES doctor(id),
    unit_price        INTEGER NOT NULL DEFAULT 0 CHECK (unit_price >= 0),
    units             INTEGER NOT NULL DEFAULT 1 CHECK (units BETWEEN 1 AND 64),
    phase             INTEGER NOT NULL DEFAULT 1 CHECK (phase BETWEEN 1 AND 5),
    status            TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'in_progress', 'completed', 'cancelled')),
    sessions_planned  INTEGER NOT NULL DEFAULT 1 CHECK (sessions_planned BETWEEN 1 AND 60),
    details           TEXT,
    notes             TEXT,
    est_start         TEXT,
    est_end           TEXT,
    completed_at      TEXT,
    cancelled_at      TEXT,
    cancel_reason     TEXT,
    billed_at         TEXT,
    sort_order        INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL,
    created_by        TEXT,
    updated_at        TEXT NOT NULL,
    updated_by        TEXT,
    deleted_at        TEXT,
    version           INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_treatment_item_patient ON treatment_item(patient_id, status) WHERE deleted_at IS NULL;
CREATE INDEX idx_treatment_item_plan ON treatment_item(plan_id, phase, sort_order) WHERE deleted_at IS NULL;

-- M4: each session done of an item, at a visit (appointment) or recorded by hand.
CREATE TABLE treatment_session (
    id              TEXT PRIMARY KEY,
    item_id         TEXT NOT NULL REFERENCES treatment_item(id),
    patient_id      TEXT NOT NULL REFERENCES patient(id),
    session_no      INTEGER NOT NULL CHECK (session_no BETWEEN 1 AND 99),
    appointment_id  TEXT REFERENCES appointment(id),
    doctor_id       TEXT REFERENCES doctor(id),
    performed_on    TEXT NOT NULL,                     -- clinic-local date
    note            TEXT,
    created_at      TEXT NOT NULL,
    created_by      TEXT
) STRICT;

CREATE INDEX idx_treatment_session_item ON treatment_session(item_id, session_no);
CREATE UNIQUE INDEX ux_treatment_session_appointment ON treatment_session(item_id, appointment_id) WHERE appointment_id IS NOT NULL;

-- M4: an appointment may be a session of a plan item ("root canal 36 — session 2 of 3").
ALTER TABLE appointment ADD COLUMN treatment_item_id TEXT REFERENCES treatment_item(id);
ALTER TABLE appointment ADD COLUMN session_no INTEGER;
CREATE INDEX idx_appointment_item ON appointment(treatment_item_id) WHERE treatment_item_id IS NOT NULL AND deleted_at IS NULL;

-- Ready-made plans (seeded from core/seeds/plan_templates.json once, then the clinic's).
CREATE TABLE plan_template (
    id          TEXT PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    name_fa     TEXT NOT NULL,
    name_ps     TEXT NOT NULL,
    name_en     TEXT NOT NULL,
    items_json  TEXT NOT NULL,                         -- PlanTemplateItem[]
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_system   INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

-- 5.6: a visit's clinical note. A signed note is locked; later changes are addenda.
CREATE TABLE clinical_note (
    id               TEXT PRIMARY KEY,
    patient_id       TEXT NOT NULL REFERENCES patient(id),
    appointment_id   TEXT REFERENCES appointment(id),
    item_id          TEXT REFERENCES treatment_item(id),
    doctor_id        TEXT REFERENCES doctor(id),
    visit_date       TEXT NOT NULL,
    chief_complaint  TEXT NOT NULL DEFAULT '',
    examination      TEXT NOT NULL DEFAULT '',
    diagnosis        TEXT NOT NULL DEFAULT '',
    findings         TEXT NOT NULL DEFAULT '',
    procedure        TEXT NOT NULL DEFAULT '',
    notes            TEXT NOT NULL DEFAULT '',
    follow_up        TEXT NOT NULL DEFAULT '',
    recommendations  TEXT NOT NULL DEFAULT '',
    template_id      TEXT,
    status           TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'signed')),
    signed_at        TEXT,
    signed_by        TEXT,
    created_at       TEXT NOT NULL,
    created_by       TEXT,
    updated_at       TEXT NOT NULL,
    updated_by       TEXT,
    deleted_at       TEXT,
    version          INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_clinical_note_patient ON clinical_note(patient_id, visit_date DESC) WHERE deleted_at IS NULL;

CREATE TABLE clinical_note_addendum (
    id          TEXT PRIMARY KEY,
    note_id     TEXT NOT NULL REFERENCES clinical_note(id),
    text        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    created_by  TEXT
) STRICT;

CREATE INDEX idx_clinical_note_addendum ON clinical_note_addendum(note_id, created_at);

-- Ready-made clinical notes (core/seeds/note_templates.json); a built-in one the clinic rewrote is `customized`.
CREATE TABLE note_template (
    id          TEXT PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    name_fa     TEXT NOT NULL,
    name_ps     TEXT NOT NULL,
    name_en     TEXT NOT NULL,
    body_json   TEXT NOT NULL,                          -- NoteTemplateTexts
    sort_order  INTEGER NOT NULL DEFAULT 0,
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

-- 5.5: periodontal exams (six sites a tooth), compared with each other.
CREATE TABLE perio_exam (
    id          TEXT PRIMARY KEY,
    patient_id  TEXT NOT NULL REFERENCES patient(id),
    exam_date   TEXT NOT NULL,
    doctor_id   TEXT REFERENCES doctor(id),
    data_json   TEXT NOT NULL,                          -- PerioTooth[]
    notes       TEXT,
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_perio_exam_patient ON perio_exam(patient_id, exam_date DESC) WHERE deleted_at IS NULL;

-- 5.7: the printed treatment plan / cost estimate is a clinical document of its own series (TP-1405-000001).
-- SQLite cannot widen a CHECK, so the document table is rebuilt (nothing references it).
CREATE TABLE clinical_document_new (
    id              TEXT PRIMARY KEY,
    number          TEXT NOT NULL UNIQUE,
    kind            TEXT NOT NULL CHECK (kind IN ('prescription', 'consent', 'post_op', 'referral', 'imaging_request',
                                                  'certificate', 'lab_order', 'record_summary', 'treatment_plan')),
    patient_id      TEXT NOT NULL REFERENCES patient(id),
    doctor_id       TEXT REFERENCES doctor(id),
    appointment_id  TEXT REFERENCES appointment(id),
    language        TEXT NOT NULL CHECK (language IN ('fa', 'ps', 'en')),
    paper           TEXT NOT NULL CHECK (paper IN ('a4', 'a5', 'a6')),
    patient_json    TEXT NOT NULL,
    doctor_json     TEXT,
    content_json    TEXT NOT NULL,
    issued_at       TEXT NOT NULL,
    print_count     INTEGER NOT NULL DEFAULT 0,
    last_printed_at TEXT,
    attachment_id   TEXT REFERENCES patient_attachment(id),
    status          TEXT NOT NULL DEFAULT 'issued' CHECK (status IN ('issued', 'void')),
    void_reason     TEXT,
    created_at      TEXT NOT NULL,
    created_by      TEXT,
    updated_at      TEXT NOT NULL,
    updated_by      TEXT,
    deleted_at      TEXT,
    version         INTEGER NOT NULL DEFAULT 1
) STRICT;

INSERT INTO clinical_document_new(id, number, kind, patient_id, doctor_id, appointment_id, language, paper, patient_json,
    doctor_json, content_json, issued_at, print_count, last_printed_at, attachment_id, status, void_reason, created_at,
    created_by, updated_at, updated_by, deleted_at, version)
SELECT id, number, kind, patient_id, doctor_id, appointment_id, language, paper, patient_json, doctor_json, content_json,
    issued_at, print_count, last_printed_at, attachment_id, status, void_reason, created_at, created_by, updated_at,
    updated_by, deleted_at, version
FROM clinical_document;

DROP TABLE clinical_document;
ALTER TABLE clinical_document_new RENAME TO clinical_document;
CREATE INDEX idx_clinical_document_patient ON clinical_document(patient_id, issued_at DESC) WHERE deleted_at IS NULL;
