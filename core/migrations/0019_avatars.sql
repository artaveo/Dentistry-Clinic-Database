-- v0.12.5: pictures for people (owner feedback OF-049, OF-050).
-- A patient's or doctor's optional photo is a small JPEG kept in the encrypted attachment store
-- (`attachment::store`, content address = SHA-256 of the bytes); the row only points at it.
-- No photo = the UI draws the default illustration (by sex and age) or the initials.
ALTER TABLE patient ADD COLUMN photo_sha256 TEXT;

-- The doctor's sex picks the default illustration. Existing doctors stay unset (neutral picture)
-- until the clinic chooses; nothing is guessed from the name.
ALTER TABLE doctor ADD COLUMN sex TEXT CHECK (sex IN ('male', 'female'));
ALTER TABLE doctor ADD COLUMN photo_sha256 TEXT;
