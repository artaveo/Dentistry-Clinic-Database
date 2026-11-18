# Database size rules (Artaveo Dental)

Date: 2026-11-18 · Applies to every schema change from v0.12.5 on, including the finance redesign
(`docs/design/finance-standard.md`, ~40 new tables).

## Rules for every new table and migration (read this first)

1. **Pages, not rows, decide the size of a small clinic.** Every table and every index is a b-tree that
   takes at least one whole page, even when empty. A new clinic uses `db::PAGE_SIZE` = 1024-byte pages;
   when it grows past `db::GROW_AT_BYTES` (5 MB, the measured crossover) it is rewritten once at start
   with `db::LARGE_PAGE_SIZE` = 4096, because 300-600-byte rows pack better into big pages
   (`db::target_page_size`, `db::fit_page_size`). Never open a clinic database without `db::connect` /
   `db::opens_with` (SQLCipher does not store the page size), and never use `VACUUM INTO` (it writes the
   copy with the default page size, which cannot be opened again): use `db::vacuum_into`.
2. **Cost of an empty table** = 1 page for the table + 1 page per index + 1 page per automatic index.
   A `TEXT PRIMARY KEY` and every `UNIQUE` constraint create an automatic index. A new table with a
   TEXT id, one UNIQUE and two indexes costs 4 KB empty. Budget for the finance work: **at most 2 explicit
   indexes per table on average** (40 tables ≈ +160 KB on a new clinic), and an index only for a query
   that exists and filters on that column's leading part (an index on `(a, b)` does not serve
   `WHERE b = ?`; `LIKE '%x%'` never uses an index).
2a. **Rows near 300 bytes waste small pages**: a 1024-byte page holds 936 usable bytes, so two 310-byte
   rows fill it. Keep hot, many-row tables (journal lines, payments) narrow: no snapshot JSON in the
   row that is read in lists, ids only once, amounts as INTEGER.
3. **Comments go above `CREATE TABLE`, never inside the parentheses.** SQLite stores the text inside
   the statement in `sqlite_schema` in every clinic database forever (it was 62 KB of the old schema).
4. **No copies of the same text.** Labels in three languages for system data that never changes
   (provinces, districts, …) are served from the seed files in the binary, not stored. Rows store ids
   and codes; names come from one place.
5. **Enums are short codes.** Measured: an enum as a 9-letter word instead of a 1-byte integer costs
   ~8 bytes per row (0.8 KB per 100 rows), far less than one 36-character UUID column. Keep the short
   TEXT codes that the CHECK constraints and the contract use; do not add a second "label" column.
   New enums: lower-case codes of at most ~12 letters, or an INTEGER when the set is numeric by nature.
6. **Every foreign key column costs 36 bytes per row** (UUID text) plus its index. Do not add a
   foreign key "just in case"; do not repeat a parent id that can be reached through the parent.
7. **Snapshots and JSON**: store a JSON snapshot only when the law requires the document as issued
   (receipts, invoices). Leave out `null` fields (`audit::compact_json` does this for the audit log).
8. **Migrations that drop or rewrite tables** are fine: `Core::install` runs `VACUUM` after a migration
   when more than 1/8 of the file is free pages, then `wal_checkpoint(TRUNCATE)`.
9. **Budgets are tests** (`core/src/tests_dbsize.rs`): a new clinic database must stay within
   `FRESH_BUDGET`, 100 patients within `PER_100_PATIENTS_BUDGET`. A migration that breaks a budget
   must lower something else or raise the budget with a measured reason in the same commit.
10. **Measure, do not guess:** `cargo test -p artaveo-core dbsize_report -- --ignored --nocapture`
    prints page counts per table and index (SQLite `dbstat`), layer sizes by page size and growth
    per 100 patients/appointments/invoices.

## After (v0.12.5 work, CI run 44324518265)

| | before (v0.12.4) | after |
|---|---|---|
| new clinic after setup | 1524 KB (4096-byte pages) | 607 KB (1024-byte pages) |
| WAL right after setup | 2.1 MB | 13 KB, cut to 0 at checkpoint; never above 512 KB after one |
| geo labels (1314 rows + PK index) | 180 KB | 0 (served from `core/seeds/*.csv`) |
| unused indexes | 3 on patient, `idx_audit_entity` (unusable for `entity_id = ?`), `idx_district_province` | dropped; `idx_audit_entity_id` added |
| audit JSON | every `null` field stored | `null` fields left out |
| pre-migration copies | piled up, never removed | newest 2 kept |
| backup of a new clinic | ~1.5 MB | ~0.6 MB (backups are compact copies of the database) |
| 100 000-patient gate (OF-016) | passes | passes (name search 63 ms with a 64 MB page cache; 256 ms with the 2 MB default) |

Same data, compact copy, by page size: new clinic 599 / 838 / 1296 KB (1024 / 2048 / 4096); after
+100 patients with a visit, a treatment, an invoice and a payment each: 1538 / 1654 / 2084 KB.
So 1024 wins until about 460 such patients (~5 MB), then 4096 wins; hence the one-time switch.

Not changed, measured as minor: enums as text (`sex`, `status`: ~8 bytes per row), FTS5 index
(~29 KB per 100 patients). Not changed, measured as big but out of scope: 36-character UUID text ids
and ISO timestamps in every row and index (the main cost per row), `payment.receipt_json` (~600 bytes
per payment; the finance redesign replaces these tables).

## Why (measured, CI run 44317256660, v0.12.4 schema)

| | old (4096-byte pages) |
|---|---|
| empty schema (19 migrations: 84 tables, 75 indexes, 99 automatic indexes = 257 b-trees) | 1108 KB |
| seed rows (districts in 3 languages, templates, services, stock items, …) | +416 KB |
| new clinic after setup | 1524 KB (239 of 258 b-trees one page) |
| WAL right after setup | 2.1 MB beside a 4 KB `clinic.db`; never shrank (`journal_size_limit` unset) |
| +100 patients / +100 appointments / +100 treatment+invoice+payment | +164 / +196 / +504 KB |
| audit log | ~760 bytes per row with its 4 indexes |
| pre-migration safety copies | one full database per update, never removed |
