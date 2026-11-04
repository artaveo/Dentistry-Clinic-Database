-- Phase 9: what a user has already read in the notification centre, and the indexes the reports and
-- the search lean on (all reports must finish within 3 seconds over a year of a busy clinic's data).

CREATE TABLE notification_read (
    user_id  TEXT NOT NULL REFERENCES app_user(id),
    key      TEXT NOT NULL,
    read_at  TEXT NOT NULL,
    PRIMARY KEY (user_id, key)
) STRICT;

CREATE INDEX idx_treatment_item_completed ON treatment_item(completed_at)
    WHERE status = 'completed' AND deleted_at IS NULL;
CREATE INDEX idx_tooth_finding_date ON tooth_finding(recorded_on) WHERE deleted_at IS NULL;
CREATE INDEX idx_audit_action ON audit_log(action, at);
CREATE INDEX idx_audit_user ON audit_log(username, at);
CREATE INDEX idx_expense_category ON expense(category_id, local_date) WHERE deleted_at IS NULL;
