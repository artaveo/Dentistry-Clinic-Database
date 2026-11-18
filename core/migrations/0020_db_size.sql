-- v0.12.5 database size (docs/design/db-size-rules.md). Measured on a new clinic database:
-- province/district labels were 1314 rows (geo_translation and its primary-key index); they are
-- served from the embedded seed files now, so the table goes. idx_district_province only served
-- the district list, which no longer reads the database.
DROP TABLE geo_translation;
DROP INDEX idx_district_province;

-- The record history filters on entity_id alone, which idx_audit_entity (entity, entity_id) cannot
-- serve; the narrower index can, and costs less per audit row.
DROP INDEX idx_audit_entity;
CREATE INDEX idx_audit_entity_id ON audit_log(entity_id) WHERE entity_id IS NOT NULL;

-- Patient search goes through patient_fts (and LIKE '%…%' in billing/recall lists cannot use an
-- index): these three were written on every patient save and never read.
DROP INDEX idx_patient_phone;
DROP INDEX idx_patient_father_name;
DROP INDEX idx_patient_name;
