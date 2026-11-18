-- Finance rebuild, phase 1 (docs/design/finance-standard.md v3, docs/design/finance-progress.md).
-- The old billing tables are replaced; their data was test data and is discarded (owner decision
-- 1405/08/27). Every document lives in fin_doc, its lines in fin_line, and its double-entry posting
-- in fin_gl (one signed amount per row: + debit, − credit; each document's rows add up to 0).
-- Money is AFN × 100 (ADR-06). Dates in fin_* are INTEGER yyyymmdd; instants are unix seconds.
DROP TABLE installment;
DROP TABLE credit_note;
DROP TABLE payment;
DROP TABLE invoice_line;
DROP TABLE invoice;
DROP TABLE cash_closing;
DROP TABLE supplier_payment;
DROP TABLE expense;
DROP TABLE expense_category;
DROP TABLE payment_method;
DELETE FROM document_counter WHERE prefix IN ('INV', 'RC', 'CU', 'RV', 'CN', 'SP');
DELETE FROM setting WHERE key = 'billing.receipt_footer';
UPDATE setting SET key = 'fin.share_enabled' WHERE key = 'billing.commission_enabled';
UPDATE setting SET key = 'fin.share_basis' WHERE key = 'billing.commission_basis';
UPDATE treatment_item SET billed_at = NULL WHERE billed_at IS NOT NULL;

ALTER TABLE supplier ADD COLUMN tin TEXT;
ALTER TABLE supplier ADD COLUMN license_no TEXT;

-- The chart of accounts. id is the account code (1110 = cash drawer). System accounts carry no
-- name: the UI shows its own label for the code. money: cash / bank / mobile for money accounts.
-- party: whose sub-ledger the account keeps (patient, supplier, employee, provider, owner).
CREATE TABLE fin_account (
    id         INTEGER PRIMARY KEY,
    parent_id  INTEGER,
    type       TEXT NOT NULL CHECK (type IN ('asset', 'liability', 'equity', 'income', 'contra', 'expense')),
    name       TEXT,
    money      TEXT CHECK (money IN ('cash', 'bank', 'mobile')),
    party      TEXT CHECK (party IN ('patient', 'supplier', 'employee', 'provider', 'owner')),
    bank_name  TEXT,
    bank_no    TEXT,
    is_system  INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    is_active  INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    version    INTEGER NOT NULL DEFAULT 1
) STRICT;

INSERT INTO fin_account(id, parent_id, type, money, party, is_system) VALUES
    (1100, NULL, 'asset', NULL, NULL, 1),
    (1110, 1100, 'asset', 'cash', NULL, 1),
    (1210, 1100, 'asset', 'bank', NULL, 1),
    (1310, NULL, 'asset', NULL, 'patient', 1),
    (1320, NULL, 'asset', NULL, 'employee', 1),
    (1330, NULL, 'asset', NULL, 'provider', 1),
    (1340, NULL, 'asset', NULL, 'supplier', 1),
    (1510, NULL, 'asset', NULL, NULL, 1),
    (2110, NULL, 'liability', NULL, 'supplier', 1),
    (2210, NULL, 'liability', NULL, 'patient', 1),
    (2310, NULL, 'liability', NULL, 'employee', 1),
    (2320, NULL, 'liability', NULL, 'provider', 1),
    (2410, NULL, 'liability', NULL, NULL, 1),
    (2420, NULL, 'liability', NULL, NULL, 1),
    (2430, NULL, 'liability', NULL, NULL, 1),
    (2510, NULL, 'liability', NULL, NULL, 1),
    (3110, NULL, 'equity', NULL, NULL, 1),
    (3120, NULL, 'equity', NULL, 'owner', 1),
    (3210, NULL, 'equity', NULL, NULL, 1),
    (3900, NULL, 'equity', NULL, NULL, 1),
    (4110, NULL, 'income', NULL, NULL, 1),
    (4190, NULL, 'income', NULL, NULL, 1),
    (4910, NULL, 'contra', NULL, NULL, 1),
    (4920, NULL, 'contra', NULL, NULL, 1),
    (5000, NULL, 'expense', NULL, NULL, 1),
    (5110, 5000, 'expense', NULL, NULL, 1),
    (5120, 5000, 'expense', NULL, NULL, 1),
    (5210, 5000, 'expense', NULL, NULL, 1),
    (5220, 5000, 'expense', NULL, NULL, 1),
    (5230, 5000, 'expense', NULL, NULL, 1),
    (5240, 5000, 'expense', NULL, NULL, 1),
    (5250, 5000, 'expense', NULL, NULL, 1),
    (5260, 5000, 'expense', NULL, NULL, 1),
    (5270, 5000, 'expense', NULL, NULL, 1),
    (5280, 5000, 'expense', NULL, NULL, 1),
    (5810, NULL, 'expense', NULL, NULL, 1),
    (5910, NULL, 'expense', NULL, NULL, 1);

-- One row per document of any kind. kind: inv invoice, rc receipt, al allocation of a deposit to
-- invoices (or back), rf refund, cn price reduction, wo write-off, pp installment plan, bl supplier
-- bill, pv payment voucher, or other receipt, tr transfer, cc cash count. The printed number is
-- PREFIX-year-serial. party_id is the patient (or the supplier for bl/pv); account_id the money
-- account (to: account2_id for tr); ref_id the invoice (cn, wo, pp) or the replaced plan;
-- ext_id the inventory purchase, lab case or return a bill was made from. Only status, the void
-- columns (voided_at, voided_by, void_reason), print_count and party_id (patient merge) ever change.
CREATE TABLE fin_doc (
    id          INTEGER PRIMARY KEY,
    kind        TEXT NOT NULL CHECK (kind IN ('inv', 'rc', 'al', 'rf', 'cn', 'wo', 'pp', 'bl', 'pv', 'or', 'tr', 'cc')),
    year        INTEGER NOT NULL,
    serial      INTEGER NOT NULL,
    doc_date    INTEGER NOT NULL,
    status      TEXT NOT NULL DEFAULT 'posted' CHECK (status IN ('posted', 'void', 'superseded')),
    party       TEXT CHECK (party IN ('patient', 'supplier')),
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

CREATE INDEX idx_fin_doc_party ON fin_doc(party_id, kind) WHERE party_id IS NOT NULL;
CREATE INDEX idx_fin_doc_date ON fin_doc(doc_date, kind);
CREATE INDEX idx_fin_doc_ref ON fin_doc(ref_id) WHERE ref_id IS NOT NULL;

-- Lines of a document. inv: one service line (amount = gross = qty × unit_price, discount taken
-- off it). rc/al: an allocation to the invoice ref_id (al may be negative: money back to the
-- deposit). bl/pv: an account and its gross amount (pv: withheld tax, entered by hand). pp: one
-- installment (due_date, amount).
CREATE TABLE fin_line (
    doc_id      INTEGER NOT NULL,
    no          INTEGER NOT NULL,
    account_id  INTEGER,
    amount      INTEGER NOT NULL,
    ref_id      INTEGER,
    qty         INTEGER,
    unit_price  INTEGER,
    list_price  INTEGER,
    discount    INTEGER,
    item_id     TEXT,
    service_id  TEXT,
    provider_id TEXT,
    teeth       TEXT,
    name        TEXT,
    due_date    INTEGER,
    withheld    INTEGER,
    wh_kind     TEXT CHECK (wh_kind IN ('rent', 'contractor')),
    PRIMARY KEY (doc_id, no)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_fin_line_ref ON fin_line(ref_id) WHERE ref_id IS NOT NULL;

-- The general ledger. Each posted document writes rows that add up to 0; a void writes the same
-- rows with the opposite sign (rev = 1), dated the void day. party_id keeps the sub-ledger of
-- control accounts (patients, suppliers).
CREATE TABLE fin_gl (
    doc_id     INTEGER NOT NULL,
    no         INTEGER NOT NULL,
    date       INTEGER NOT NULL,
    account_id INTEGER NOT NULL,
    amount     INTEGER NOT NULL CHECK (amount <> 0),
    party_id   TEXT,
    rev        INTEGER NOT NULL DEFAULT 0 CHECK (rev IN (0, 1)),
    PRIMARY KEY (doc_id, no)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_fin_gl_account ON fin_gl(account_id, date);
CREATE INDEX idx_fin_gl_party ON fin_gl(party_id) WHERE party_id IS NOT NULL;

-- Issued documents and their postings are never rewritten (ADR-11).
CREATE TRIGGER fin_gl_no_delete BEFORE DELETE ON fin_gl
BEGIN SELECT RAISE(ABORT, 'fin_gl is append-only'); END;
CREATE TRIGGER fin_gl_no_update BEFORE UPDATE ON fin_gl
WHEN NEW.doc_id <> OLD.doc_id OR NEW.no <> OLD.no OR NEW.date <> OLD.date
    OR NEW.account_id <> OLD.account_id OR NEW.amount <> OLD.amount OR NEW.rev <> OLD.rev
BEGIN SELECT RAISE(ABORT, 'fin_gl is append-only'); END;
CREATE TRIGGER fin_line_no_delete BEFORE DELETE ON fin_line
BEGIN SELECT RAISE(ABORT, 'fin_line is append-only'); END;
CREATE TRIGGER fin_line_no_update BEFORE UPDATE ON fin_line
BEGIN SELECT RAISE(ABORT, 'fin_line is append-only'); END;
CREATE TRIGGER fin_doc_no_delete BEFORE DELETE ON fin_doc
BEGIN SELECT RAISE(ABORT, 'fin_doc is append-only'); END;
CREATE TRIGGER fin_doc_no_update BEFORE UPDATE ON fin_doc
WHEN NEW.kind <> OLD.kind OR NEW.year <> OLD.year OR NEW.serial <> OLD.serial OR NEW.doc_date <> OLD.doc_date
    OR NEW.amount <> OLD.amount OR NEW.account_id IS NOT OLD.account_id OR NEW.account2_id IS NOT OLD.account2_id
    OR NEW.ref_id IS NOT OLD.ref_id OR NEW.created_at <> OLD.created_at
    OR (OLD.status <> 'posted' AND NEW.status <> OLD.status)
BEGIN SELECT RAISE(ABORT, 'an issued finance document is never rewritten'); END;
