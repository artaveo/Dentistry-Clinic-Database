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
| `medical_questions.psv` | M1 checklist: 34 system questions (group, detail field, alert, women-only) — fa/ps/en, pipe-separated | **doctor to review**; Pashto needs native-speaker review |
| `drugs.psv` | 24-medicine starter formulary (Latin name, form, strength, warning classes, default dose) | **doctor to review** |
| `rx_templates.json` | 7 ready-made prescriptions pointing at `drugs.psv` codes | **doctor to review** |
| `document_templates.json` | 6 consent forms + 5 after-treatment sheets, A4, with `{patient}`, `{doctor}`, `{clinic}`, `{teeth}`, `{procedure}`, `{date}` placeholders — fa/ps/en | **doctor to review** (legal/clinical wording) |
| `services.json` | M3 catalog: 11 categories, 53 services and variants, scope and specialty; prices are **not** seeded (0 = "not set") | **owner to set prices** |

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

## Phase 5A seeds belong to the clinic after the first start

Unlike the reference lists above, the clinical seeds are a **starting point**
the doctor edits in «فهرست‌های بالینی» and «خدمات و تعرفه‌ها»:

* medicines, ready-made prescriptions, catalog categories and services are
  inserted once (`ON CONFLICT DO NOTHING`) — a later start never overwrites a
  doctor's edit, price or switched-off item;
* a document template the clinic edited is marked `customized` and is never
  overwritten by a newer seed text; «بازگشت به متن اصلی» restores the seed;
* system checklist questions keep their seeded wording (so alerts and
  prescription warnings stay reliable) but the clinic may switch one off and
  add its own questions.

`.psv` files are UTF-8, `|`-separated, first line is the header, `-` means
empty. Rules are checked by `cargo test -p artaveo-core seeds formulary catalog`.
