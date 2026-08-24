-- M5.5 multi-user readiness. Local mode keeps auth_subject / actor_user_id nullable.
CREATE TABLE app_users (
    id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    email TEXT COLLATE NOCASE UNIQUE,
    role TEXT NOT NULL DEFAULT 'SALES' CHECK (role IN ('ADMIN', 'MANAGER', 'SALES')),
    is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
    auth_subject TEXT UNIQUE,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

ALTER TABLE lead_contacts
ADD COLUMN assigned_user_id TEXT REFERENCES app_users(id) ON DELETE SET NULL;

ALTER TABLE lead_activities
ADD COLUMN actor_user_id TEXT REFERENCES app_users(id) ON DELETE SET NULL;

CREATE INDEX idx_app_users_active_name
    ON app_users(is_active, display_name COLLATE NOCASE, id);

CREATE INDEX idx_lead_contacts_assigned_user
    ON lead_contacts(assigned_user_id, latest_submission_at DESC);

CREATE INDEX idx_activities_actor_occurred
    ON lead_activities(actor_user_id, occurred_at DESC);
