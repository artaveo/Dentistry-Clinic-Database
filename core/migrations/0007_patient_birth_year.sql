-- OF-019: an estimated age is stored as the estimated birth year, so the age
-- shown stays right as years pass (age = this year − birth year). An exact
-- birth date (`date_of_birth`) always wins; the two are never both set.
-- Clinic time is Asia/Kabul (+04:30, no daylight saving), so "this year" is
-- computed with the same offset everywhere.

ALTER TABLE patient ADD COLUMN approximate_birth_year INTEGER;

UPDATE patient
   SET approximate_birth_year = CAST(strftime('%Y', 'now', '+4 hours', '+30 minutes') AS INTEGER) - approximate_age
 WHERE approximate_age IS NOT NULL AND date_of_birth IS NULL;

UPDATE patient SET approximate_age = NULL WHERE approximate_birth_year IS NOT NULL;
