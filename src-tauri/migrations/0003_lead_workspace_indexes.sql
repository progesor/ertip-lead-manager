-- M3 query-path indexes. These do not change CRM/source data semantics.
CREATE INDEX IF NOT EXISTS idx_submissions_contact_created
    ON lead_submissions(lead_contact_id, source_created_at_utc DESC);

CREATE INDEX IF NOT EXISTS idx_quality_contact_status
    ON lead_data_quality_issues(lead_contact_id, status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_activities_contact_occurred
    ON lead_activities(lead_contact_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_notes_contact_created
    ON lead_notes(lead_contact_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_product_overrides_contact_product_created
    ON contact_product_interest_overrides(lead_contact_id, product_code, created_at DESC);
