-- Staff, doctors, appointments & reception workflow (roadmap Phase 4).
-- Clinic-local days are Saturday = 0 … Friday = 6 (Afghan week, ADR-21);
-- times of day inside a weekly schedule are minutes after local midnight;
-- appointment instants are UTC ISO-8601 (ADR-07) with the clinic-local date
-- kept alongside so "the appointments of day D" is an index lookup.

-- Custom roles (4.1): system roles keep translated names in the UI, custom
-- roles carry the label the clinic typed.
ALTER TABLE role ADD COLUMN label TEXT;

-- Attachment thumbnails are made by the Core when a file is saved (OF-018, ADR-12): a small
-- JPEG stored encrypted under its own content hash, so the list never decodes the full image.
ALTER TABLE patient_attachment ADD COLUMN thumbnail_sha256 TEXT;

-- The patient list is newest-registered first: without this index every page sorted all patients
-- (374 ms at 100 000 rows, OF-016).
CREATE INDEX idx_patient_registered ON patient(registration_date DESC, patient_number DESC) WHERE deleted_at IS NULL;

CREATE TABLE doctor (
    id          TEXT PRIMARY KEY,                    -- UUIDv7
    user_id     TEXT REFERENCES app_user(id),        -- optional login of this doctor
    full_name   TEXT NOT NULL,
    specialty   TEXT,
    color       TEXT NOT NULL,                       -- calendar colour, #rrggbb
    status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE UNIQUE INDEX ux_doctor_user ON doctor(user_id) WHERE user_id IS NOT NULL AND deleted_at IS NULL;

CREATE TABLE chair (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,                       -- "Chair 1", "Room A", …
    status      TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'inactive')),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE UNIQUE INDEX ux_chair_name ON chair(name COLLATE NOCASE) WHERE deleted_at IS NULL;

-- Weekly working hours (several intervals per day allowed) and breaks. The
-- whole schedule of a doctor is replaced together (doctors.set_schedule), so
-- these rows carry no version of their own; the doctor's version guards them.
CREATE TABLE doctor_hours (
    id         TEXT PRIMARY KEY,
    doctor_id  TEXT NOT NULL REFERENCES doctor(id),
    day        INTEGER NOT NULL CHECK (day BETWEEN 0 AND 6),
    start_min  INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
    end_min    INTEGER NOT NULL CHECK (end_min BETWEEN 1 AND 1440),
    CHECK (end_min > start_min)
) STRICT;
CREATE INDEX idx_doctor_hours_doctor ON doctor_hours(doctor_id, day);

CREATE TABLE doctor_break (
    id         TEXT PRIMARY KEY,
    doctor_id  TEXT NOT NULL REFERENCES doctor(id),
    day        INTEGER NOT NULL CHECK (day BETWEEN 0 AND 6),
    start_min  INTEGER NOT NULL CHECK (start_min BETWEEN 0 AND 1439),
    end_min    INTEGER NOT NULL CHECK (end_min BETWEEN 1 AND 1440),
    CHECK (end_min > start_min)
) STRICT;
CREATE INDEX idx_doctor_break_doctor ON doctor_break(doctor_id, day);

-- Chairs a doctor may use. No rows = any chair.
CREATE TABLE doctor_chair (
    doctor_id  TEXT NOT NULL REFERENCES doctor(id),
    chair_id   TEXT NOT NULL REFERENCES chair(id),
    PRIMARY KEY (doctor_id, chair_id)
) STRICT;

-- Leave / unavailable days, both ends inclusive (clinic-local ISO dates).
CREATE TABLE doctor_leave (
    id          TEXT PRIMARY KEY,
    doctor_id   TEXT NOT NULL REFERENCES doctor(id),
    start_date  TEXT NOT NULL,
    end_date    TEXT NOT NULL,
    reason      TEXT,
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1,
    CHECK (end_date >= start_date)
) STRICT;
CREATE INDEX idx_doctor_leave_doctor ON doctor_leave(doctor_id, start_date) WHERE deleted_at IS NULL;

CREATE TABLE appointment (
    id                    TEXT PRIMARY KEY,
    patient_id            TEXT NOT NULL REFERENCES patient(id),
    doctor_id             TEXT NOT NULL REFERENCES doctor(id),
    chair_id              TEXT REFERENCES chair(id),
    start_at              TEXT NOT NULL,             -- UTC
    end_at                TEXT NOT NULL,             -- UTC
    local_date            TEXT NOT NULL,             -- clinic-local ISO date of start_at
    start_min             INTEGER NOT NULL,          -- clinic-local minutes after midnight (0–1439)
    end_min               INTEGER NOT NULL,          -- … (1–1440)
    reason                TEXT,
    notes                 TEXT,
    status                TEXT NOT NULL DEFAULT 'scheduled'
        CHECK (status IN ('scheduled', 'confirmed', 'checked_in', 'in_treatment', 'completed',
                          'cancelled', 'no_show', 'rescheduled')),
    is_walk_in            INTEGER NOT NULL DEFAULT 0 CHECK (is_walk_in IN (0, 1)),
    queue_number          INTEGER,                   -- per local day, assigned at check-in (4.5)
    checked_in_at         TEXT,
    treatment_started_at  TEXT,
    completed_at          TEXT,
    cancelled_at          TEXT,                      -- when it was cancelled / marked no-show / rescheduled
    cancel_reason         TEXT,
    rescheduled_from_id   TEXT REFERENCES appointment(id),
    rescheduled_to_id     TEXT REFERENCES appointment(id),
    created_at            TEXT NOT NULL,
    created_by            TEXT,
    updated_at            TEXT NOT NULL,
    updated_by            TEXT,
    deleted_at            TEXT,
    version               INTEGER NOT NULL DEFAULT 1,
    CHECK (end_at > start_at)
) STRICT;

CREATE INDEX idx_appointment_date ON appointment(local_date, start_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_appointment_doctor ON appointment(doctor_id, start_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_appointment_chair ON appointment(chair_id, start_at) WHERE deleted_at IS NULL AND chair_id IS NOT NULL;
CREATE INDEX idx_appointment_patient ON appointment(patient_id, start_at) WHERE deleted_at IS NULL;

-- Follow-up & recall (4.6): "this patient should come back around <due_date>".
CREATE TABLE recall (
    id                     TEXT PRIMARY KEY,
    patient_id             TEXT NOT NULL REFERENCES patient(id),
    kind                   TEXT NOT NULL DEFAULT 'checkup'
        CHECK (kind IN ('checkup', 'cleaning', 'follow_up', 'no_show', 'other')),
    due_date               TEXT NOT NULL,            -- clinic-local ISO date
    repeat_months          INTEGER CHECK (repeat_months IS NULL OR repeat_months BETWEEN 1 AND 60),
    note                   TEXT,
    status                 TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'contacted', 'booked', 'done', 'dismissed')),
    appointment_id         TEXT REFERENCES appointment(id),        -- the visit booked for this recall
    source_appointment_id  TEXT REFERENCES appointment(id),        -- the visit that created it
    last_contacted_at      TEXT,
    contact_note           TEXT,
    created_at             TEXT NOT NULL,
    created_by             TEXT,
    updated_at             TEXT NOT NULL,
    updated_by             TEXT,
    deleted_at             TEXT,
    version                INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_recall_due ON recall(due_date) WHERE deleted_at IS NULL AND status IN ('pending', 'contacted');
CREATE INDEX idx_recall_patient ON recall(patient_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_recall_appointment ON recall(appointment_id) WHERE appointment_id IS NOT NULL;
