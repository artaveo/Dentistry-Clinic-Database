# Finance rebuild — progress log

Owner of this work: session «finance design» (side session of «Mecanical Test»). Design: `docs/design/finance-standard.md` (v3).
Rules: CI only (no local cargo), rustfmt + contract regeneration through a `fmt/*` branch, push to `main` only when a phase is complete and green.

## Phase 1 — implementation decisions (refine design v3 to `docs/design/db-size-rules.md`)
| # | Decision | Why |
|---|---|---|
| I1 | **Three tables carry all documents and the ledger:** `fin_doc` (one header row per document of any kind), `fin_line` (lines of any document: invoice lines, receipt allocations, bill/voucher lines, plan items), `fin_gl` (the double-entry ledger, one signed amount per row). Plus `fin_account` (chart). | Every table and index is ≥ 1 page; one header table = one numbering index, one void path, one audit path. |
| I2 | Enums are **short lower-case TEXT codes** with CHECK (rule 5), not integers. | db-size rule 5 overrides design ب۶.۰ S2. |
| I3 | `fin_account.id` **is** the account code (`1110`), INTEGER PRIMARY KEY. System accounts store no name: the UI translates `fin.account.<code>`. User accounts store one name. | Rule 4: no stored copies of fixed labels. |
| I4 | Dates in `fin_*` are INTEGER `yyyymmdd`; instants are INTEGER unix seconds. API returns ISO strings. | 4 bytes vs 10/24. |
| I5 | Document numbers: `document_counter(prefix, year)` (existing table), text built on read: `INV-1405-000001`. | No new table. |
| I6 | No journal-voucher numbers for automatic postings: a ledger row is identified by its document; a void writes reversing rows (`rev = 1`) dated the void day. | Fewer numbers, same audit trail. |
| I7 | Suppliers stay in `supplier` (+ `tin`, `license_no`); no table rebuild. Purchases (inventory receipts) create a supplier bill `bl` in the same transaction (`fin_doc.ext_id` = purchase id). | Avoids rebuilding inventory FKs. |
| I8 | Commission rules (`commission_rule`) stay in phase 1 for the doctor-share **preview**; phase 2 replaces them with compensation plans and frozen statements. | Shares are phase 2. |
| I9 | No stored print snapshot: receipts/invoices are rebuilt from immutable rows. | Rule 7 (JSON only where law requires; the rows are the document). |
| I10 | Opening balances, periods/locking, hash chain, official issue → phases 3–4 as planned. | |

## Phase 1 — schema (migration `0021_finance.sql`)
* DROP: `installment`, `credit_note`, `payment`, `invoice_line`, `invoice`, `cash_closing`, `supplier_payment`, `expense`, `expense_category`, `payment_method`; finance rows of `document_counter`; `billing.*` settings except commission ones (renamed `fin.share_*`); `treatment_item.billed_at = NULL`.
* ADD: `supplier.tin`, `supplier.license_no`.
* CREATE: `fin_account`, `fin_doc`, `fin_line`, `fin_gl` (+ immutability triggers), chart seed.

| I11 | Permissions keep their codes in phase 1: `billing.view` = see finance, `billing.edit` = receive and spend, `billing.void` = adjust, refund and void, `billing.override` = change a price. Phase 2/3 add `finance.payroll`, `finance.close`, `finance.lock`. | No role migration in phase 1. |
| I12 | The UI tab id stays `billing` (Shell.tsx is shared with layout sessions); its label is «مالی». | Fewer conflicts. |
| I13 | Finance reports live in `core/src/reports.rs` (report engine); kinds: daily_summary, monthly_summary, receipts, invoices, adjustments, aging, installments, deposits, expenses, money_book, provider_production, service_revenue, doctor_shares, cash_counts, income_statement. | Reuse table/export helpers. |

## Status
- [x] Design v3 committed locally (953cab2)
- [x] Migration 0021 (`core/migrations/0021_finance.sql`, registered in db.rs)
- [x] core/src/fin: util, ledger, accounts, invoices, receipts, patients, spend, cash, docs, shares, overview (written, not compiled yet)
- [x] shared/src/fin.rs (replaces billing.rs), method list `fin.*`, ValidationRule changes, inventory contract (supplier tin/license, purchase account_id, supplier_payments removed), ReportKind finance set + ReportParams.account_id
- [x] reports.rs finance reports, dashboard, notifications, search; lan.rs is_change; restore.rs count
- [ ] api.rs routes; lib.rs module; auth permissions; patient.rs merge/delete; audit.rs
- [ ] inventory.rs (purchase → bill, supplier payments → vouchers, balances from ledger)
- [ ] reports.rs (dashboard, notifications, finance reports moved to fin)
- [ ] tests (hand-computed A/B scenarios, posting rules, dbsize budgets)
- [ ] UI finance pages, patient account, print, inventory tabs, dashboard, i18n, e2e, manual test guide
- [ ] fmt/contract branch, CI green, push main

## Next
Write migration 0021.
