# Seed data

CSV files embedded into the Core at build time and synchronised into every
clinic database on start-up (idempotent upsert by `code`). IDs are
deterministic UUIDv7-layout values derived from the code (see `seeds.rs`), so
the same province/district/reference item has the same ID in every clinic.

| File | Content | Status |
|---|---|---|
| `reference.csv` | gender, blood_group, marital_status, payment_method — fa/ps/en | complete |
| `provinces.csv` | 34 provinces, ISO 3166-2:AF codes — fa/ps/en | complete; Pashto spellings need native-speaker review |
| `districts.csv` | districts per province — fa/ps/en | **header only** — needs an authoritative source (see below) |

## Districts

District names must come from an authoritative list (OCHA COD-AB
`afg_admbnda_adm2` or the NSIA administrative list), not be typed from memory:
~400 names × 3 languages is too error-prone. The importer is ready:

```
province_code,code,sort,fa,ps,en
AF-KAB,AF-KAB-01,1,<دری>,<پښتو>,<English>
```

Rules enforced at build/start: `province_code` must exist in `provinces.csv`,
`code` unique, all three labels non-empty. Add the rows and the next start
syncs them into existing clinic databases (`cargo test -p artaveo-core seeds`
validates the file).
