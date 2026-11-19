-- Finance phase 2 (docs/design/finance-standard.md, phase 2): employees, monthly payroll and
-- frozen doctor-share statements. Two new document kinds in fin_doc (pr payroll, ds share
-- statement) and two new parties (employee, doctor): SQLite cannot change a CHECK, so fin_doc is
-- rebuilt with its rows, indexes and triggers. Nothing references fin_doc by foreign key.

CREATE TABLE fin_doc_new (
    id          INTEGER PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('inv', 'rc', 'al', 'rf', 'cn', 'wo', 'pp', 'bl', 'pv', 'or', 'tr', 'cc', 'pr', 'ds')),
    year        INTEGER NOT NULL,
    serial      INTEGER NOT NULL,
    doc_date    INTEGER NOT NULL,
    status      TEXT NOT NULL DEFAULT 'posted' CHECK (status IN ('posted', 'void', 'superseded')),
    party       TEXT CHECK (party IN ('patient', 'supplier', 'employee', 'doctor')),
    party_id    TEXT,
    account_id  INTEGER,
    account2_id INTEGER,
    ref_id      INTEGER,
    ext_id      TEXT,
    amount      INTEGER NOT NULL DEFAULT 0,
    purpose     TEXT,
    name        TEXT,
    reference   TEXT,
    reason      TEXT,
    note        TEXT,
    print_count INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    created_by  TEXT,
    voided_at   INTEGER,
    voided_by   TEXT,
    void_reason TEXT,
    UNIQUE (kind, year, serial)
) STRICT;

INSERT INTO fin_doc_new SELECT id, kind, year, serial, doc_date, status, party, party_id, account_id, account2_id,
    ref_id, ext_id, amount, purpose, name, reference, reason, note, print_count, created_at, created_by, voided_at,
    voided_by, void_reason FROM fin_doc;
DROP TABLE fin_doc;
ALTER TABLE fin_doc_new RENAME TO fin_doc;

CREATE INDEX idx_fin_doc_party ON fin_doc(party_id, kind) WHERE party_id IS NOT NULL;
CREATE INDEX idx_fin_doc_date ON fin_doc(doc_date, kind);
CREATE INDEX idx_fin_doc_ref ON fin_doc(ref_id) WHERE ref_id IS NOT NULL;

CREATE TRIGGER fin_doc_no_delete BEFORE DELETE ON fin_doc
BEGIN SELECT RAISE(ABORT, 'fin_doc is append-only'); END;
CREATE TRIGGER fin_doc_no_update BEFORE UPDATE ON fin_doc
WHEN NEW.kind <> OLD.kind OR NEW.year <> OLD.year OR NEW.serial <> OLD.serial OR NEW.doc_date <> OLD.doc_date
    OR NEW.amount <> OLD.amount OR NEW.account_id IS NOT OLD.account_id OR NEW.account2_id IS NOT OLD.account2_id
    OR NEW.ref_id IS NOT OLD.ref_id OR NEW.created_at <> OLD.created_at
    OR (OLD.status <> 'posted' AND NEW.status <> OLD.status)
BEGIN SELECT RAISE(ABORT, 'an issued finance document is never rewritten'); END;

-- People the clinic pays a salary. A doctor paid only by share needs no row here.
-- Dates are yyyymmdd; base_salary is AFN × 100 a month.
CREATE TABLE fin_employee (
    id          TEXT PRIMARY KEY,
    full_name   TEXT NOT NULL,
    father_name TEXT,
    tin         TEXT,
    national_id TEXT,
    phone       TEXT,
    position    TEXT NOT NULL,
    doctor_id   TEXT,
    hire_date   INTEGER NOT NULL,
    end_date    INTEGER,
    base_salary INTEGER NOT NULL DEFAULT 0 CHECK (base_salary >= 0),
    note        TEXT,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

-- One employee's line of a payroll (fin_doc kind 'pr'): gross = base + extra + commission,
-- net = gross − advance − tax. Written once with the payroll, never changed.
CREATE TABLE fin_pay_line (
    doc_id      INTEGER NOT NULL,
    employee_id TEXT NOT NULL,
    base        INTEGER NOT NULL,
    extra       INTEGER NOT NULL DEFAULT 0,
    commission  INTEGER NOT NULL DEFAULT 0,
    advance     INTEGER NOT NULL DEFAULT 0,
    tax         INTEGER NOT NULL DEFAULT 0,
    note        TEXT,
    PRIMARY KEY (doc_id, employee_id)
) STRICT, WITHOUT ROWID;

CREATE TRIGGER fin_pay_line_no_change BEFORE UPDATE ON fin_pay_line
BEGIN SELECT RAISE(ABORT, 'fin_pay_line is append-only'); END;
CREATE TRIGGER fin_pay_line_no_delete BEFORE DELETE ON fin_pay_line
BEGIN SELECT RAISE(ABORT, 'fin_pay_line is append-only'); END;
