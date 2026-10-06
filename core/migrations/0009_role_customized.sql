-- OF-028: built-in roles except the owner can be changed. `customized` marks a built-in role the clinic
-- has changed, so a start-up never overwrites that choice; "back to defaults" clears it again.
ALTER TABLE role ADD COLUMN customized INTEGER NOT NULL DEFAULT 0 CHECK (customized IN (0, 1));
