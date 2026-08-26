CREATE TABLE app_users (
    id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL CHECK (length(trim(display_name)) >= 2),
    email TEXT,
    role TEXT NOT NULL DEFAULT 'SALES' CHECK (role IN ('ADMIN', 'MANAGER', 'SALES')),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    auth_subject TEXT UNIQUE,
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE UNIQUE INDEX ux_app_users_email_lower
    ON app_users(lower(email))
    WHERE email IS NOT NULL AND trim(email) <> '';

CREATE INDEX idx_app_users_active_name
    ON app_users(is_active, lower(display_name), id);

CREATE TABLE lead_contacts (
    id TEXT PRIMARY KEY,
    display_name TEXT,
    primary_email TEXT,
    normalized_email TEXT,
    primary_phone TEXT,
    normalized_phone TEXT,
    country_code TEXT,
    status TEXT NOT NULL DEFAULT 'NEW' CHECK (
        status IN ('NEW', 'CONTACTED', 'REPLIED', 'QUALIFIED', 'QUOTE_SENT', 'WON', 'LOST', 'INVALID')
    ),
    assigned_user_id TEXT REFERENCES app_users(id) ON DELETE SET NULL,
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    latest_submission_at TIMESTAMPTZ,
    submission_count INTEGER NOT NULL DEFAULT 0 CHECK (submission_count >= 0)
);

CREATE TABLE import_batches (
    id TEXT PRIMARY KEY,
    file_name TEXT NOT NULL,
    file_size BIGINT CHECK (file_size IS NULL OR file_size >= 0),
    file_sha256 TEXT,
    file_format TEXT NOT NULL DEFAULT 'UNKNOWN' CHECK (file_format IN ('XLSX', 'CSV', 'UNKNOWN')),
    sheet_name TEXT NOT NULL,
    started_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ,
    status TEXT NOT NULL CHECK (status IN ('PREVIEWED', 'COMMITTED', 'FAILED', 'CANCELLED')),
    total_rows INTEGER NOT NULL DEFAULT 0 CHECK (total_rows >= 0),
    new_submissions INTEGER NOT NULL DEFAULT 0 CHECK (new_submissions >= 0),
    exact_duplicates INTEGER NOT NULL DEFAULT 0 CHECK (exact_duplicates >= 0),
    repeat_candidates INTEGER NOT NULL DEFAULT 0 CHECK (repeat_candidates >= 0),
    warning_count INTEGER NOT NULL DEFAULT 0 CHECK (warning_count >= 0),
    error_count INTEGER NOT NULL DEFAULT 0 CHECK (error_count >= 0),
    app_version TEXT NOT NULL
);

CREATE TABLE lead_submissions (
    id TEXT PRIMARY KEY,
    lead_contact_id TEXT NOT NULL REFERENCES lead_contacts(id) ON DELETE RESTRICT,
    import_batch_id TEXT NOT NULL REFERENCES import_batches(id) ON DELETE RESTRICT,
    external_lead_id TEXT NOT NULL UNIQUE,
    source_created_at_utc TIMESTAMPTZ,
    source_created_at_raw TEXT NOT NULL,
    ad_id TEXT,
    ad_name TEXT,
    adset_id TEXT,
    adset_name TEXT,
    campaign_id TEXT,
    campaign_name TEXT,
    form_id TEXT,
    form_name TEXT,
    is_organic BOOLEAN,
    platform TEXT,
    raw_procedure_answer TEXT,
    raw_product_answer TEXT,
    raw_full_name TEXT,
    raw_email TEXT,
    raw_phone TEXT,
    raw_country TEXT,
    raw_lead_status TEXT,
    normalized_email TEXT,
    normalized_phone TEXT,
    raw_payload_json TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE submission_product_interests (
    id TEXT PRIMARY KEY,
    lead_submission_id TEXT NOT NULL REFERENCES lead_submissions(id) ON DELETE CASCADE,
    product_code TEXT NOT NULL CHECK (
        product_code IN (
            'FUE_MICROMOTOR_SYSTEMS',
            'LONG_HAIR_FUE_SOLUTIONS',
            'FUE_PUNCHES',
            'IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS',
            'MEDICAL_CHAIRS_CLINIC_FURNITURE',
            'OTHER_GENERAL_INFORMATION',
            'UNKNOWN'
        )
    ),
    origin TEXT NOT NULL,
    confidence TEXT CHECK (confidence IN ('HIGH', 'LOW') OR confidence IS NULL),
    created_at TIMESTAMPTZ NOT NULL,
    UNIQUE (lead_submission_id, product_code)
);

CREATE TABLE contact_product_interest_overrides (
    id TEXT PRIMARY KEY,
    lead_contact_id TEXT NOT NULL REFERENCES lead_contacts(id) ON DELETE CASCADE,
    product_code TEXT NOT NULL CHECK (
        product_code IN (
            'FUE_MICROMOTOR_SYSTEMS',
            'LONG_HAIR_FUE_SOLUTIONS',
            'FUE_PUNCHES',
            'IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS',
            'MEDICAL_CHAIRS_CLINIC_FURNITURE',
            'OTHER_GENERAL_INFORMATION',
            'UNKNOWN'
        )
    ),
    action TEXT NOT NULL CHECK (action IN ('ADD', 'REMOVE')),
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE lead_notes (
    id TEXT PRIMARY KEY,
    lead_contact_id TEXT NOT NULL REFERENCES lead_contacts(id) ON DELETE CASCADE,
    body TEXT NOT NULL,
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE lead_activities (
    id TEXT PRIMARY KEY,
    lead_contact_id TEXT NOT NULL REFERENCES lead_contacts(id) ON DELETE CASCADE,
    actor_user_id TEXT REFERENCES app_users(id) ON DELETE SET NULL,
    activity_type TEXT NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}'
);

CREATE TABLE follow_ups (
    id TEXT PRIMARY KEY,
    lead_contact_id TEXT NOT NULL REFERENCES lead_contacts(id) ON DELETE CASCADE,
    due_at TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('OPEN', 'COMPLETED', 'CANCELLED')),
    note TEXT,
    revision BIGINT NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);

CREATE TABLE lead_data_quality_issues (
    id TEXT PRIMARY KEY,
    lead_contact_id TEXT REFERENCES lead_contacts(id) ON DELETE CASCADE,
    lead_submission_id TEXT REFERENCES lead_submissions(id) ON DELETE CASCADE,
    issue_type TEXT NOT NULL,
    severity TEXT NOT NULL CHECK (severity IN ('INFO', 'WARNING', 'ERROR')),
    details_json TEXT NOT NULL DEFAULT '{}',
    status TEXT NOT NULL DEFAULT 'OPEN' CHECK (status IN ('OPEN', 'DISMISSED', 'RESOLVED')),
    created_at TIMESTAMPTZ NOT NULL,
    resolved_at TIMESTAMPTZ,
    CHECK (lead_contact_id IS NOT NULL OR lead_submission_id IS NOT NULL)
);

CREATE INDEX idx_lead_contacts_normalized_email ON lead_contacts(normalized_email);
CREATE INDEX idx_lead_contacts_normalized_phone ON lead_contacts(normalized_phone);
CREATE INDEX idx_lead_contacts_status ON lead_contacts(status);
CREATE INDEX idx_lead_contacts_latest_submission ON lead_contacts(latest_submission_at DESC);
CREATE INDEX idx_lead_contacts_assigned_user ON lead_contacts(assigned_user_id, latest_submission_at DESC);

CREATE INDEX idx_submissions_normalized_email ON lead_submissions(normalized_email);
CREATE INDEX idx_submissions_normalized_phone ON lead_submissions(normalized_phone);
CREATE INDEX idx_submissions_source_created ON lead_submissions(source_created_at_utc DESC);
CREATE INDEX idx_submissions_contact_created ON lead_submissions(lead_contact_id, source_created_at_utc DESC);
CREATE INDEX idx_submissions_campaign ON lead_submissions(campaign_id, source_created_at_utc DESC);
CREATE INDEX idx_submissions_form ON lead_submissions(form_id, source_created_at_utc DESC);
CREATE INDEX idx_submissions_platform ON lead_submissions(platform, source_created_at_utc DESC);

CREATE INDEX idx_submission_products_code ON submission_product_interests(product_code, lead_submission_id);
CREATE INDEX idx_contact_product_overrides ON contact_product_interest_overrides(lead_contact_id, product_code);
CREATE INDEX idx_product_overrides_contact_product_created
    ON contact_product_interest_overrides(lead_contact_id, product_code, created_at DESC);

CREATE INDEX idx_notes_contact_created ON lead_notes(lead_contact_id, created_at DESC);
CREATE INDEX idx_activities_contact_occurred ON lead_activities(lead_contact_id, occurred_at DESC);
CREATE INDEX idx_activities_actor_occurred ON lead_activities(actor_user_id, occurred_at DESC);
CREATE INDEX idx_follow_ups_status_due ON follow_ups(status, due_at);
CREATE INDEX idx_quality_status_type ON lead_data_quality_issues(status, issue_type);
CREATE INDEX idx_quality_contact_status ON lead_data_quality_issues(lead_contact_id, status, created_at DESC);
