-- Phase 6: invoices, payments and receipts, installments, doctor commission, clinic expenses, the daily
-- cash closing. Money is INTEGER AFN × 100 (ADR-06). An issued invoice and every payment row are
-- never edited (ADR-11): a mistake is a void (invoice), a credit note, or a refund, each with a reason.
-- Numbers (INV / RC / CU / RV / CN-<Shamsi year>-<serial>) come from `document_counter`, inside the
-- same transaction that writes the row, so none is skipped or repeated.

CREATE TABLE payment_method (
    id          TEXT PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,
    name_fa     TEXT NOT NULL,
    name_ps     TEXT NOT NULL,
    name_en     TEXT NOT NULL,
    is_cash     INTEGER NOT NULL DEFAULT 0 CHECK (is_cash IN (0, 1)),
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

INSERT INTO payment_method(id, code, name_fa, name_ps, name_en, is_cash, sort_order, is_system, created_at, updated_at)
VALUES
    ('pm-cash', 'cash', 'نقد', 'نغدي', 'Cash', 1, 10, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    ('pm-bank', 'bank_transfer', 'حواله بانکی', 'بانکي حواله', 'Bank transfer', 0, 20, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));

CREATE TABLE invoice (
    id              TEXT PRIMARY KEY,
    number          TEXT NOT NULL UNIQUE,
    patient_id      TEXT NOT NULL REFERENCES patient(id),
    status          TEXT NOT NULL DEFAULT 'issued' CHECK (status IN ('issued', 'void')),
    issued_at       TEXT NOT NULL,
    local_date      TEXT NOT NULL,
    discount        INTEGER NOT NULL DEFAULT 0 CHECK (discount >= 0),
    note            TEXT,
    installment_base INTEGER NOT NULL DEFAULT 0,        -- received before the installment plan was set
    void_reason     TEXT,
    voided_at       TEXT,
    print_count     INTEGER NOT NULL DEFAULT 0,
    last_printed_at TEXT,
    created_at      TEXT NOT NULL,
    created_by      TEXT,
    updated_at      TEXT NOT NULL,
    updated_by      TEXT,
    deleted_at      TEXT,
    version         INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_invoice_patient ON invoice(patient_id, issued_at) WHERE deleted_at IS NULL;
CREATE INDEX idx_invoice_date ON invoice(local_date) WHERE deleted_at IS NULL;

CREATE TABLE invoice_line (
    id                 TEXT PRIMARY KEY,
    invoice_id         TEXT NOT NULL REFERENCES invoice(id),
    treatment_item_id  TEXT REFERENCES treatment_item(id),
    service_id         TEXT REFERENCES service(id),
    names_json         TEXT NOT NULL,                   -- {"fa":…,"ps":…,"en":…} as invoiced
    teeth              TEXT NOT NULL DEFAULT '',
    surfaces           TEXT NOT NULL DEFAULT '',
    doctor_id          TEXT REFERENCES doctor(id),
    quantity           INTEGER NOT NULL CHECK (quantity BETWEEN 1 AND 1000),
    unit_price         INTEGER NOT NULL CHECK (unit_price >= 0),
    listed_price       INTEGER NOT NULL DEFAULT 0,
    line_total         INTEGER NOT NULL CHECK (line_total >= 0),
    sort_order         INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX idx_invoice_line_invoice ON invoice_line(invoice_id, sort_order);
CREATE INDEX idx_invoice_line_item ON invoice_line(treatment_item_id) WHERE treatment_item_id IS NOT NULL;
CREATE INDEX idx_invoice_line_doctor ON invoice_line(doctor_id) WHERE doctor_id IS NOT NULL;

CREATE TABLE credit_note (
    id          TEXT PRIMARY KEY,
    number      TEXT NOT NULL UNIQUE,
    invoice_id  TEXT NOT NULL REFERENCES invoice(id),
    patient_id  TEXT NOT NULL REFERENCES patient(id),
    amount      INTEGER NOT NULL CHECK (amount > 0),
    reason      TEXT NOT NULL,
    issued_at   TEXT NOT NULL,
    local_date  TEXT NOT NULL,
    created_by  TEXT
) STRICT;

CREATE INDEX idx_credit_note_invoice ON credit_note(invoice_id);

-- Every money movement of a patient: a payment received (for an invoice, or an advance with no
-- invoice), credit put towards an invoice, or a refund of an earlier row.
CREATE TABLE payment (
    id                 TEXT PRIMARY KEY,
    number             TEXT NOT NULL UNIQUE,
    kind               TEXT NOT NULL CHECK (kind IN ('payment', 'credit', 'refund')),
    patient_id         TEXT NOT NULL REFERENCES patient(id),
    invoice_id         TEXT REFERENCES invoice(id),
    method_id          TEXT REFERENCES payment_method(id),
    amount             INTEGER NOT NULL CHECK (amount > 0),
    paid_at            TEXT NOT NULL,
    local_date         TEXT NOT NULL,
    note               TEXT,
    refund_of          TEXT REFERENCES payment(id),
    returns_to_credit  INTEGER NOT NULL DEFAULT 0 CHECK (returns_to_credit IN (0, 1)),
    reason             TEXT,
    receipt_json       TEXT NOT NULL,                   -- ReceiptSnapshot as issued
    print_count        INTEGER NOT NULL DEFAULT 0,
    last_printed_at    TEXT,
    created_at         TEXT NOT NULL,
    created_by         TEXT,
    version            INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_payment_patient ON payment(patient_id, paid_at);
CREATE INDEX idx_payment_invoice ON payment(invoice_id) WHERE invoice_id IS NOT NULL;
CREATE INDEX idx_payment_date ON payment(local_date);
CREATE INDEX idx_payment_refund_of ON payment(refund_of) WHERE refund_of IS NOT NULL;

CREATE TABLE installment (
    id          TEXT PRIMARY KEY,
    invoice_id  TEXT NOT NULL REFERENCES invoice(id),
    no          INTEGER NOT NULL,
    due_date    TEXT NOT NULL,
    amount      INTEGER NOT NULL CHECK (amount > 0),
    UNIQUE (invoice_id, no)
) STRICT;

CREATE INDEX idx_installment_due ON installment(due_date);

CREATE TABLE expense_category (
    id          TEXT PRIMARY KEY,
    name_fa     TEXT NOT NULL,
    name_ps     TEXT NOT NULL,
    name_en     TEXT NOT NULL,
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

INSERT INTO expense_category(id, name_fa, name_ps, name_en, sort_order, is_system, created_at, updated_at)
SELECT id, fa, ps, en, sort, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM (
    SELECT 'ec-rent' AS id, 'اجاره' AS fa, 'کرایه' AS ps, 'Rent' AS en, 10 AS sort
    UNION ALL SELECT 'ec-salary', 'معاش', 'معاش', 'Salaries', 20
    UNION ALL SELECT 'ec-utilities', 'برق، آب و انترنت', 'برېښنا، اوبه او انټرنیټ', 'Electricity, water, internet', 30
    UNION ALL SELECT 'ec-materials', 'خرید مواد', 'د توکو پیرود', 'Materials purchase', 40
    UNION ALL SELECT 'ec-lab', 'لابراتوار', 'لابراتوار', 'Dental lab', 50
    UNION ALL SELECT 'ec-maintenance', 'تعمیر و نگهداری', 'ترمیم او ساتنه', 'Repair and maintenance', 60
    UNION ALL SELECT 'ec-other', 'سایر', 'نور', 'Other', 90
);

CREATE TABLE expense (
    id             TEXT PRIMARY KEY,
    category_id    TEXT NOT NULL REFERENCES expense_category(id),
    amount         INTEGER NOT NULL CHECK (amount > 0),
    local_date     TEXT NOT NULL,
    description    TEXT NOT NULL,
    payee          TEXT,
    from_cash      INTEGER NOT NULL DEFAULT 1 CHECK (from_cash IN (0, 1)),
    delete_reason  TEXT,
    created_at     TEXT NOT NULL,
    created_by     TEXT,
    updated_at     TEXT NOT NULL,
    updated_by     TEXT,
    deleted_at     TEXT,
    version        INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_expense_date ON expense(local_date) WHERE deleted_at IS NULL;

-- One closing per clinic day (6.7).
CREATE TABLE cash_closing (
    id          TEXT PRIMARY KEY,
    local_date  TEXT NOT NULL UNIQUE,
    expected    INTEGER NOT NULL,
    counted     INTEGER NOT NULL CHECK (counted >= 0),
    difference  INTEGER NOT NULL,
    reason      TEXT,
    closed_at   TEXT NOT NULL,
    closed_by   TEXT
) STRICT;

CREATE TABLE commission_rule (
    id          TEXT PRIMARY KEY,
    doctor_id   TEXT REFERENCES doctor(id),
    service_id  TEXT REFERENCES service(id),
    kind        TEXT NOT NULL CHECK (kind IN ('percent', 'fixed')),
    value       INTEGER NOT NULL CHECK (value >= 0),
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE UNIQUE INDEX ux_commission_rule_scope
    ON commission_rule(COALESCE(doctor_id, ''), COALESCE(service_id, '')) WHERE deleted_at IS NULL;
