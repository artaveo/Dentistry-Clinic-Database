-- OF-020 (rule 14): unsent form drafts, one per user and form. Kept in the encrypted
-- database (ADR-03), never in browser storage, so a locked screen, a minimised window or a
-- closed app does not lose what was typed.

CREATE TABLE form_draft (
    user_id    TEXT NOT NULL REFERENCES app_user(id),
    form_key   TEXT NOT NULL,
    data_json  TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (user_id, form_key)
) STRICT;
