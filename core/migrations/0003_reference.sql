-- Reference data (roadmap 1.5): records store only IDs; labels come from
-- the translation table in the current UI language.

CREATE TABLE reference_type (
    id   TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE                     -- gender, blood_group, …
) STRICT;

CREATE TABLE reference_item (
    id          TEXT PRIMARY KEY,                 -- UUIDv7 (deterministic for system seeds)
    type_id     TEXT NOT NULL REFERENCES reference_type(id),
    code        TEXT NOT NULL,
    sort_order  INTEGER NOT NULL DEFAULT 0,
    is_active   INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    is_system   INTEGER NOT NULL DEFAULT 0 CHECK (is_system IN (0, 1)),
    created_at  TEXT NOT NULL,
    created_by  TEXT,
    updated_at  TEXT NOT NULL,
    updated_by  TEXT,
    deleted_at  TEXT,
    version     INTEGER NOT NULL DEFAULT 1,
    UNIQUE (type_id, code)
) STRICT;

CREATE TABLE reference_translation (
    reference_item_id TEXT NOT NULL REFERENCES reference_item(id),
    language_code     TEXT NOT NULL CHECK (language_code IN ('fa', 'ps', 'en')),
    label             TEXT NOT NULL,
    PRIMARY KEY (reference_item_id, language_code)
) STRICT;

-- Geography with a real hierarchy.
CREATE TABLE province (
    id          TEXT PRIMARY KEY,
    code        TEXT NOT NULL UNIQUE,             -- ISO 3166-2:AF, e.g. AF-KAB
    sort_order  INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE TABLE district (
    id          TEXT PRIMARY KEY,
    province_id TEXT NOT NULL REFERENCES province(id),
    code        TEXT NOT NULL UNIQUE,
    sort_order  INTEGER NOT NULL DEFAULT 0
) STRICT;

CREATE INDEX idx_district_province ON district(province_id);

CREATE TABLE geo_translation (
    entity_type   TEXT NOT NULL CHECK (entity_type IN ('province', 'district')),
    entity_id     TEXT NOT NULL,
    language_code TEXT NOT NULL CHECK (language_code IN ('fa', 'ps', 'en')),
    label         TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id, language_code)
) STRICT;
