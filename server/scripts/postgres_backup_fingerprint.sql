\pset tuples_only on
\pset format unaligned
\set ON_ERROR_STOP on

-- M6 staging recoverability fingerprint.
-- Run against the source database immediately before pg_dump and again against
-- the disposable restored database. The outputs must match exactly.
--
-- Digests contain hashes only; no raw credentials, session tokens or customer
-- values are emitted.

SELECT 'app_users|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM app_users t;
SELECT 'app_credentials|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY user_id)), md5('')) FROM app_credentials t;
SELECT 'auth_sessions|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM auth_sessions t;
SELECT 'auth_one_time_tokens|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM auth_one_time_tokens t;
SELECT 'auth_security_events|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM auth_security_events t;
SELECT 'lead_contacts|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM lead_contacts t;
SELECT 'import_batches|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM import_batches t;
SELECT 'lead_submissions|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM lead_submissions t;
SELECT 'submission_product_interests|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM submission_product_interests t;
SELECT 'contact_product_interest_overrides|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM contact_product_interest_overrides t;
SELECT 'lead_notes|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM lead_notes t;
SELECT 'lead_activities|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM lead_activities t;
SELECT 'follow_ups|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM follow_ups t;
SELECT 'lead_data_quality_issues|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY id)), md5('')) FROM lead_data_quality_issues t;
SELECT '_sqlx_migrations|' || count(*) || '|' || coalesce(md5(string_agg(md5(to_jsonb(t)::text), '' ORDER BY version)), md5('')) FROM _sqlx_migrations t;

SELECT 'invariant|duplicate_external_lead_id|' || count(*)
FROM (
    SELECT external_lead_id
    FROM lead_submissions
    GROUP BY external_lead_id
    HAVING count(*) > 1
) duplicate_ids;

SELECT 'invariant|submission_count_mismatch|' || count(*)
FROM lead_contacts c
WHERE c.submission_count <> (
    SELECT count(*)::integer
    FROM lead_submissions s
    WHERE s.lead_contact_id = c.id
);

SELECT 'invariant|failed_migrations|' || count(*)
FROM _sqlx_migrations
WHERE success IS NOT TRUE;
