-- Phase 8: inventory (items, categories, units, locations, batches with expiry), stock operations
-- (receive, consume, adjust, return, transfer), purchases, suppliers and what the clinic owes them,
-- and the dental lab (a lab is a supplier of kind 'lab'; a Lab Case is the work sent to it).
-- Quantities are INTEGER hundredths of the item's unit (2.5 boxes = 250). Money is AFN × 100 (ADR-06).
-- Stock is kept per batch (lot + expiry + place); every change is one signed row of the movement
-- ledger, which is never edited (ADR-11): a mistake is a new adjustment or a void with a reason.
-- Numbers (PU / SP / LC-<Shamsi year>-<serial>) come from `document_counter`.

CREATE TABLE stock_category (
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

CREATE TABLE stock_unit (
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

CREATE TABLE stock_location (
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

INSERT INTO stock_category(id, name_fa, name_ps, name_en, sort_order, is_system, created_at, updated_at)
SELECT id, fa, ps, en, sort, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM (
    SELECT 'sc-disposable' AS id, 'مصرفی و حفاظتی' AS fa, 'مصرفي او ساتونکي' AS ps, 'Disposables and protection' AS en, 10 AS sort
    UNION ALL SELECT 'sc-anesthetic', 'بی‌حسی', 'بې‌حسي', 'Anesthetics', 20
    UNION ALL SELECT 'sc-restorative', 'ترمیمی و پرکردگی', 'ترمیمي او ډکونه', 'Restorative', 30
    UNION ALL SELECT 'sc-endo', 'عصب‌کشی', 'عصب‌کشي', 'Endodontic', 40
    UNION ALL SELECT 'sc-surgical', 'جراحی', 'جراحي', 'Surgical', 50
    UNION ALL SELECT 'sc-impression', 'قالب‌گیری', 'قالب‌ګیري', 'Impression', 60
    UNION ALL SELECT 'sc-medicine', 'دوا', 'درمل', 'Medicines', 70
    UNION ALL SELECT 'sc-other', 'سایر', 'نور', 'Other', 90
);

INSERT INTO stock_unit(id, name_fa, name_ps, name_en, sort_order, is_system, created_at, updated_at)
SELECT id, fa, ps, en, sort, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM (
    SELECT 'su-piece' AS id, 'عدد' AS fa, 'دانه' AS ps, 'Piece' AS en, 10 AS sort
    UNION ALL SELECT 'su-box', 'بسته', 'بکس', 'Box', 20
    UNION ALL SELECT 'su-pack', 'پاکت', 'پاکټ', 'Pack', 30
    UNION ALL SELECT 'su-tube', 'تیوب', 'تیوب', 'Tube', 40
    UNION ALL SELECT 'su-vial', 'ویال', 'ویال', 'Vial', 50
    UNION ALL SELECT 'su-bottle', 'بوتل', 'بوتل', 'Bottle', 60
    UNION ALL SELECT 'su-ml', 'میلی‌لیتر', 'ملي‌لیتر', 'ml', 70
    UNION ALL SELECT 'su-g', 'گرام', 'ګرام', 'g', 80
);

INSERT INTO stock_location(id, name_fa, name_ps, name_en, sort_order, is_system, created_at, updated_at)
SELECT id, fa, ps, en, sort, 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM (
    SELECT 'sl-main' AS id, 'گدام اصلی' AS fa, 'اصلي ګدام' AS ps, 'Main store' AS en, 10 AS sort
    UNION ALL SELECT 'sl-room', 'اتاق معاینه', 'د معاینې خونه', 'Treatment room', 20
);

-- A supplier of goods, or a dental laboratory (kind 'lab').
CREATE TABLE supplier (
    id              TEXT PRIMARY KEY,
    kind            TEXT NOT NULL DEFAULT 'supplier' CHECK (kind IN ('supplier', 'lab')),
    name            TEXT NOT NULL,
    contact_person  TEXT,
    phone           TEXT,
    address         TEXT,
    note            TEXT,
    is_active       INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at      TEXT NOT NULL,
    created_by      TEXT,
    updated_at      TEXT NOT NULL,
    updated_by      TEXT,
    deleted_at      TEXT,
    version         INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_supplier_kind ON supplier(kind, name) WHERE deleted_at IS NULL;

CREATE TABLE stock_item (
    id          TEXT PRIMARY KEY,
    code        TEXT,
    name_fa     TEXT NOT NULL,
    name_ps     TEXT NOT NULL,
    name_en     TEXT NOT NULL,
    category_id TEXT NOT NULL REFERENCES stock_category(id),
    unit_id     TEXT NOT NULL REFERENCES stock_unit(id),
    min_qty     INTEGER NOT NULL DEFAULT 0 CHECK (min_qty >= 0),
    note        TEXT,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_stock_item_category ON stock_item(category_id) WHERE deleted_at IS NULL;

CREATE TABLE purchase (
    id           TEXT PRIMARY KEY,
    number       TEXT NOT NULL UNIQUE,
    supplier_id  TEXT NOT NULL REFERENCES supplier(id),
    local_date   TEXT NOT NULL,
    total        INTEGER NOT NULL CHECK (total >= 0),
    note         TEXT,
    status       TEXT NOT NULL DEFAULT 'received' CHECK (status IN ('received', 'void')),
    void_reason  TEXT,
    voided_at    TEXT,
    created_at   TEXT NOT NULL,
    created_by   TEXT,
    version      INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_purchase_supplier ON purchase(supplier_id, local_date);
CREATE INDEX idx_purchase_date ON purchase(local_date);

CREATE TABLE purchase_line (
    id           TEXT PRIMARY KEY,
    purchase_id  TEXT NOT NULL REFERENCES purchase(id),
    item_id      TEXT NOT NULL REFERENCES stock_item(id),
    qty          INTEGER NOT NULL CHECK (qty > 0),
    unit_cost    INTEGER NOT NULL CHECK (unit_cost >= 0),
    line_total   INTEGER NOT NULL CHECK (line_total >= 0),
    lot_no       TEXT,
    expiry_date  TEXT,
    location_id  TEXT NOT NULL REFERENCES stock_location(id),
    sort_order   INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX idx_purchase_line_purchase ON purchase_line(purchase_id, sort_order);
CREATE INDEX idx_purchase_line_item ON purchase_line(item_id);

-- Stock on the shelf: one row per lot at one place. qty_left only ever changes through a movement.
CREATE TABLE stock_batch (
    id           TEXT PRIMARY KEY,
    item_id      TEXT NOT NULL REFERENCES stock_item(id),
    location_id  TEXT NOT NULL REFERENCES stock_location(id),
    lot_no       TEXT,
    expiry_date  TEXT,
    qty_left     INTEGER NOT NULL CHECK (qty_left >= 0),
    unit_cost    INTEGER NOT NULL DEFAULT 0 CHECK (unit_cost >= 0),
    received_on  TEXT NOT NULL,
    supplier_id  TEXT REFERENCES supplier(id),
    purchase_id  TEXT REFERENCES purchase(id),
    created_at   TEXT NOT NULL,
    created_by   TEXT
) STRICT;

CREATE INDEX idx_stock_batch_item ON stock_batch(item_id, expiry_date) WHERE qty_left > 0;
CREATE INDEX idx_stock_batch_expiry ON stock_batch(expiry_date) WHERE qty_left > 0 AND expiry_date IS NOT NULL;
CREATE INDEX idx_stock_batch_purchase ON stock_batch(purchase_id) WHERE purchase_id IS NOT NULL;

-- The ledger. qty is signed (+ in, − out). `credit` is what a supplier credits the clinic for a return.
CREATE TABLE stock_movement (
    id                 TEXT PRIMARY KEY,
    item_id            TEXT NOT NULL REFERENCES stock_item(id),
    batch_id           TEXT NOT NULL REFERENCES stock_batch(id),
    location_id        TEXT NOT NULL REFERENCES stock_location(id),
    kind               TEXT NOT NULL CHECK (kind IN ('purchase', 'receive', 'consume', 'adjust', 'return',
                                                     'transfer_out', 'transfer_in', 'void_purchase')),
    qty                INTEGER NOT NULL CHECK (qty <> 0),
    unit_cost          INTEGER NOT NULL DEFAULT 0,
    credit             INTEGER NOT NULL DEFAULT 0 CHECK (credit >= 0),
    supplier_id        TEXT REFERENCES supplier(id),
    purchase_id        TEXT REFERENCES purchase(id),
    patient_id         TEXT REFERENCES patient(id),
    treatment_item_id  TEXT REFERENCES treatment_item(id),
    reason             TEXT,
    local_date         TEXT NOT NULL,
    created_at         TEXT NOT NULL,
    created_by         TEXT
) STRICT;

CREATE INDEX idx_stock_movement_item ON stock_movement(item_id, created_at);
CREATE INDEX idx_stock_movement_date ON stock_movement(local_date);
CREATE INDEX idx_stock_movement_treatment ON stock_movement(treatment_item_id) WHERE treatment_item_id IS NOT NULL;
CREATE INDEX idx_stock_movement_supplier ON stock_movement(supplier_id) WHERE supplier_id IS NOT NULL;

-- 8.2: the materials a service uses; consuming them for a treatment is optional.
CREATE TABLE service_material (
    service_id  TEXT NOT NULL REFERENCES service(id),
    item_id     TEXT NOT NULL REFERENCES stock_item(id),
    qty         INTEGER NOT NULL CHECK (qty > 0),
    PRIMARY KEY (service_id, item_id)
) STRICT;

-- 8.5: money paid to a supplier or lab. When it comes out of a drawer or account it is also a clinic
-- expense (so the cash closing and the profit report stay right); `expense_id` links the two.
CREATE TABLE supplier_payment (
    id           TEXT PRIMARY KEY,
    number       TEXT NOT NULL UNIQUE,
    supplier_id  TEXT NOT NULL REFERENCES supplier(id),
    amount       INTEGER NOT NULL CHECK (amount > 0),
    method_id    TEXT REFERENCES payment_method(id),
    local_date   TEXT NOT NULL,
    note         TEXT,
    expense_id   TEXT REFERENCES expense(id),
    status       TEXT NOT NULL DEFAULT 'paid' CHECK (status IN ('paid', 'void')),
    void_reason  TEXT,
    created_at   TEXT NOT NULL,
    created_by   TEXT,
    version      INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_supplier_payment_supplier ON supplier_payment(supplier_id, local_date);

-- 8.6: work sent to a lab for one patient (crown, bridge, denture, …).
CREATE TABLE lab_case (
    id                 TEXT PRIMARY KEY,
    number             TEXT NOT NULL UNIQUE,
    patient_id         TEXT NOT NULL REFERENCES patient(id),
    doctor_id          TEXT REFERENCES doctor(id),
    treatment_item_id  TEXT REFERENCES treatment_item(id),
    lab_id             TEXT NOT NULL REFERENCES supplier(id),
    teeth              TEXT NOT NULL DEFAULT '',
    work_type          TEXT NOT NULL,
    shade              TEXT,
    status             TEXT NOT NULL DEFAULT 'planned'
        CHECK (status IN ('planned', 'sent', 'received', 'delivered', 'cancelled')),
    sent_date          TEXT,
    expected_date      TEXT,
    received_date      TEXT,
    delivered_date     TEXT,
    cost               INTEGER NOT NULL DEFAULT 0 CHECK (cost >= 0),
    note               TEXT,
    cancel_reason      TEXT,
    created_at         TEXT NOT NULL,
    created_by         TEXT,
    updated_at         TEXT NOT NULL,
    updated_by         TEXT,
    version            INTEGER NOT NULL DEFAULT 1
) STRICT;

CREATE INDEX idx_lab_case_patient ON lab_case(patient_id, created_at);
CREATE INDEX idx_lab_case_lab ON lab_case(lab_id, status);
CREATE INDEX idx_lab_case_open ON lab_case(expected_date) WHERE status IN ('planned', 'sent');
CREATE INDEX idx_lab_case_treatment ON lab_case(treatment_item_id) WHERE treatment_item_id IS NOT NULL;
