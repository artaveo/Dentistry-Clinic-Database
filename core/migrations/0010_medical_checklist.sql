-- M1 / OF-025 (Phase 5A): a structured medical-history checklist replaces the free-text fields of
-- roadmap 3.3. Every question is answered yes / no / unknown (no row = not asked yet); some open a
-- detail when the answer is yes. The questions are reference data: the system ones are seeded from
-- core/seeds/medical_questions.psv on every start (codes, labels, alert flags), the clinic can switch
-- them off and add its own. Labels live in their own table like reference_translation.

CREATE TABLE medical_question (
    id           TEXT PRIMARY KEY,                    -- seed_id('medical_question', code) for system questions
    code         TEXT NOT NULL UNIQUE,                -- anticoagulants, pregnant, … (prescriptions check these)
    group_code   TEXT NOT NULL,                       -- cardio, blood, … (see MedicalQuestionInfo)
    sort_order   INTEGER NOT NULL DEFAULT 0,
    detail_kind  TEXT NOT NULL DEFAULT 'none' CHECK (detail_kind IN ('none', 'text', 'choice', 'text_choice', 'months')),
    choices      TEXT,                                -- 'controlled;uncontrolled' for choice kinds
    alert        INTEGER NOT NULL DEFAULT 0 CHECK (alert IN (0, 1)),
    female_only  INTEGER NOT NULL DEFAULT 0 CHECK (female_only IN (0, 1)),
    is_system    INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active    INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at   TEXT NOT NULL,
    created_by   TEXT,
    updated_at   TEXT NOT NULL,
    updated_by   TEXT,
    deleted_at   TEXT,
    version      INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE TABLE medical_question_translation (
    question_id   TEXT NOT NULL REFERENCES medical_question(id),
    language_code TEXT NOT NULL CHECK (language_code IN ('fa', 'ps', 'en')),
    label         TEXT NOT NULL,
    detail_label  TEXT,
    alert_note    TEXT,
    PRIMARY KEY (question_id, language_code)
) STRICT;

-- The patient's current answers. The history row (patient_medical_history) carries the version that
-- guards them all, the «سایر توضیحات» notes and when it was last reviewed; earlier answers are in the
-- append-only audit log (before/after of every save), as for the rest of the record.
CREATE TABLE patient_medical_answer (
    patient_id    TEXT NOT NULL REFERENCES patient(id),
    question_id   TEXT NOT NULL REFERENCES medical_question(id),
    answer        TEXT NOT NULL CHECK (answer IN ('yes', 'no', 'unknown')),
    detail_text   TEXT,
    detail_choice TEXT,
    PRIMARY KEY (patient_id, question_id)
) STRICT;

ALTER TABLE patient_medical_history ADD COLUMN reviewed_at TEXT;
ALTER TABLE patient_medical_history ADD COLUMN reviewed_by TEXT;
UPDATE patient_medical_history SET reviewed_at = updated_at, reviewed_by = updated_by;

-- Nothing typed before is lost: the old free-text fields move into «سایر توضیحات», each under its
-- own heading, ahead of what was already there.
UPDATE patient_medical_history
SET notes = NULLIF(TRIM(
       COALESCE('حساسیت‌ها: ' || NULLIF(TRIM(allergies), '') || char(10), '')
    || COALESCE('داروهای مصرفی: ' || NULLIF(TRIM(current_medications), '') || char(10), '')
    || COALESCE('بیماری‌های مزمن: ' || NULLIF(TRIM(chronic_conditions), '') || char(10), '')
    || COALESCE('سابقه دندان‌پزشکی: ' || NULLIF(TRIM(dental_history), '') || char(10), '')
    || COALESCE('جراحی‌های قبلی: ' || NULLIF(TRIM(previous_surgeries), '') || char(10), '')
    || COALESCE(notes, ''), ' ' || char(10)), '');

ALTER TABLE patient_medical_history DROP COLUMN allergies;
ALTER TABLE patient_medical_history DROP COLUMN current_medications;
ALTER TABLE patient_medical_history DROP COLUMN chronic_conditions;
ALTER TABLE patient_medical_history DROP COLUMN dental_history;
ALTER TABLE patient_medical_history DROP COLUMN previous_surgeries;
