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
| `services.json` | M3 catalog: 11 categories, 53 services and variants, scope and specialty; since v0.6.0 also how each service is drawn on the chart (`symbol`) and the usual days between its sessions (`interval`, default 7); prices are **not** seeded (0 = "not set") | **owner to set prices**; doctor to review symbols and intervals |
| `tooth_conditions.json` | 5.3: 18 existing tooth conditions for the chart (caries, filling, crown, root canal, missing, extracted, implant, …): chart symbol, surfaces or whole tooth, short mark — fa/ps/en | **doctor to review**; Pashto needs native-speaker review |
| `note_templates.json` | 5.6: 9 visit-note templates (first exam, toothache, root canal 1st/last session, extraction, filling, scaling, ortho adjustment, implant surgery), the eight note sections in fa/ps/en | **doctor to review** (clinical wording); Pashto needs native-speaker review |
| `plan_templates.json` | 5.7: 8 ready-made plans (root canal + crown, single implant, extraction + implant, bridge, fixed orthodontics, complete dentures, check-up + cleaning, child prevention) as lists of catalog services with their teeth and phase | **doctor to review** |

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

## Phase 5A and 5B seeds belong to the clinic after the first start

Unlike the reference lists above, the clinical seeds are a **starting point**
the doctor edits in «فهرست‌های بالینی» and «خدمات و تعرفه‌ها»:

* medicines, ready-made prescriptions, catalog categories and services are
  inserted once (`ON CONFLICT DO NOTHING`) — a later start never overwrites a
  doctor's edit, price or switched-off item;
* a document template the clinic edited is marked `customized` and is never
  overwritten by a newer seed text; «بازگشت به متن اصلی» restores the seed;
* tooth conditions and ready-made plans are inserted once as well; a note template the clinic
  edited is marked `customized` like the document templates;
* system checklist questions keep their seeded wording (so alerts and
  prescription warnings stay reliable) but the clinic may switch one off and
  add its own questions.

`.psv` files are UTF-8, `|`-separated, first line is the header, `-` means
empty. Rules are checked by `cargo test -p artaveo-core seeds formulary catalog`.
