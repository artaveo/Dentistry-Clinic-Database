# Seed data

CSV files embedded into the Core at build time and synchronised into every
clinic database on start-up (idempotent upsert by `code`). IDs are
deterministic UUIDv7-layout values derived from the code (see `seeds.rs`), so
the same province/district/reference item has the same ID in every clinic.

| File | Content | Status |
|---|---|---|
| `reference.csv` | gender, blood_group, marital_status, payment_method — fa/ps/en | complete |
| `provinces.csv` | 34 provinces, ISO 3166-2:AF codes — fa/ps/en | complete; Dari spellings confirmed by the product owner; Pashto needs native-speaker review |
| `districts.csv` | 404 districts incl. 34 provincial centres (`AF-XXX-CENTER`, listed first) — fa/ps/en | Dari from the product owner's list; Pashto and English transliterated, **need native-speaker review** |

## Districts

Dari names come from the product owner's list (compiled from Wikipedia's
district category, AAN's district map, geo-ref.net and others). Codes are
`<province>-<SLUG>` from the English name, so they stay stable when the list
is reordered. Normalisations applied to the Dari text:

* hyphens inside names → ZWNJ (`ده-سبز` → `ده‌سبز`), izafe marks removed (`دشتِ‌ارچی` → `دشت ارچی`, `قلعهٔ‌زال` → `قلعه زال`)
* `یکاولنگ (نمبر ۱ و نمبر ۲)` split into two districts
* a district identical to its provincial centre merged with it (Daykundi: نیلی, Kunar: اسدآباد/اسعدآباد)

Names to confirm (not in the usual district lists, or a known district seems
missing): Badakhshan `حامی` and missing Yamgan; Paktya `سمکنی` next to
`چم کنی` and missing Zurmat / Laja Mangal; Takhar missing Darqad; Ghor missing
Charsada; Helmand missing Nawzad. Editing the CSV and restarting the app is
enough — the seed sync upserts by code and IDs never change.

Rules enforced by `cargo test -p artaveo-core seeds`: known province codes,
unique codes, three non-empty labels, one centre per province, Persian ی/ک in
Dari.
