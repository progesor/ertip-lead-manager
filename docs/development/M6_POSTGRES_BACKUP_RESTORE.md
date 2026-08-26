# M6 — PostgreSQL Backup / Restore Recoverability Runbook

## Goal

Prove that the authoritative PostgreSQL database can be backed up and restored without writing a restore over the live staging database.

This runbook separates two concerns:

1. **operational backup retention** through Coolify's PostgreSQL backup feature;
2. **recoverability evidence** by restoring a fresh dump into a disposable database and comparing deterministic fingerprints.

A successful backup file alone is not considered restore evidence.

## Safety rules

- Run this against **staging** first.
- Never restore into the source database during this acceptance test.
- Keep Coolify application auto-deploy disabled during the test.
- Avoid CRM/import/credential writes while the fingerprint → dump → restore comparison is running.
- Do not paste database passwords, `DATABASE_URL`, bearer tokens or invitation/reset tokens into logs or repository files.
- The disposable restore database is created with a unique timestamped name and dropped at the end.

## A. Coolify operational backup contract

Coolify's PostgreSQL backup workflow uses custom-format `pg_dump` and supports scheduled backups plus optional S3-compatible storage.

Recommended staging baseline:

- database resource → **Backups**;
- create a scheduled PostgreSQL backup;
- select the Ertip Lead Manager database explicitly when multiple databases exist;
- use a sensible recurring schedule (for example daily during staging);
- configure local retention;
- when an S3-compatible destination is available, keep an off-host copy as well;
- trigger one **Backup Now** execution and confirm the execution completes successfully.

Production rollout should not rely only on a backup stored on the same host as PostgreSQL. An off-host/S3 copy is strongly preferred before the database becomes the only production authority.

Official references:

- https://coolify.io/docs/databases/backups
- https://coolify.io/docs/databases/postgresql

## B. Disposable restore proof

Open the **PostgreSQL resource terminal** in Coolify. A prompt such as `/ #` is the container shell; the commands below are shell commands and intentionally invoke `psql`, `pg_dump`, `pg_restore`, `createdb` and `dropdb` themselves.

The PostgreSQL image normally exposes `POSTGRES_USER` and `POSTGRES_DB`. The script does not print their passwords.

Paste the following block as one operation:

```sh
set -eu

PGUSER="${POSTGRES_USER:-postgres}"
SRC_DB="${POSTGRES_DB:-postgres}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
RESTORE_DB="elm_restore_smoke_$(date -u +%Y%m%d%H%M%S)"
BACKUP="/tmp/elm-${SRC_DB}-${STAMP}.dump"
FP_SQL="/tmp/elm-backup-fingerprint.sql"
SOURCE_FP="/tmp/elm-source-${STAMP}.fp"
RESTORE_FP="/tmp/elm-restore-${STAMP}.fp"

printf 'Source DB: %s\n' "$SRC_DB"
printf 'Restore DB: %s\n' "$RESTORE_DB"
pg_dump --version
pg_restore --version

cat > "$FP_SQL" <<'SQL'
\pset tuples_only on
\pset format unaligned
\set ON_ERROR_STOP on

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
SQL

# Fingerprint the source immediately before the dump.
psql -X -v ON_ERROR_STOP=1 -U "$PGUSER" -d "$SRC_DB" -f "$FP_SQL" > "$SOURCE_FP"

# Custom-format logical backup, matching the Coolify PostgreSQL backup format.
pg_dump \
  --format=custom \
  --no-acl \
  --no-owner \
  --username "$PGUSER" \
  --dbname "$SRC_DB" \
  --file "$BACKUP"

test -s "$BACKUP"
printf '\nBackup file:\n'
ls -lh "$BACKUP"
printf 'Archive entries: '
pg_restore --list "$BACKUP" | wc -l

# Restore only into a new disposable database. Use the known source DB as the
# maintenance connection instead of assuming a database named after PGUSER exists.
createdb -U "$PGUSER" --maintenance-db "$SRC_DB" "$RESTORE_DB"
pg_restore \
  --exit-on-error \
  --no-acl \
  --no-owner \
  --username "$PGUSER" \
  --dbname "$RESTORE_DB" \
  "$BACKUP"

# Fingerprint the restored copy and require byte-for-byte matching fingerprint output.
psql -X -v ON_ERROR_STOP=1 -U "$PGUSER" -d "$RESTORE_DB" -f "$FP_SQL" > "$RESTORE_FP"

printf '\nSource fingerprint:\n'
cat "$SOURCE_FP"
printf '\nRestored fingerprint:\n'
cat "$RESTORE_FP"

printf '\nFingerprint diff (must be empty):\n'
diff -u "$SOURCE_FP" "$RESTORE_FP"

# Additional human-readable restore sanity checks.
printf '\nRestored lead status counts:\n'
psql -X -v ON_ERROR_STOP=1 -U "$PGUSER" -d "$RESTORE_DB" \
  -c "SELECT status, count(*) FROM lead_contacts GROUP BY status ORDER BY status;"

printf '\nRestored migration state:\n'
psql -X -v ON_ERROR_STOP=1 -U "$PGUSER" -d "$RESTORE_DB" \
  -c "SELECT version, description, success FROM _sqlx_migrations ORDER BY version;"

printf '\nBACKUP_RESTORE_SMOKE=PASS\n'

# Cleanup only the disposable restore database and temporary smoke artifacts.
dropdb -U "$PGUSER" --maintenance-db "$SRC_DB" "$RESTORE_DB"
rm -f "$FP_SQL" "$SOURCE_FP" "$RESTORE_FP" "$BACKUP"
```

## Expected acceptance result

The evidence is PASS only when all of the following are true:

- `pg_dump` exits successfully;
- the custom-format dump exists and is non-empty;
- `pg_restore --list` reports archive entries;
- the disposable database is created successfully;
- `pg_restore --exit-on-error` completes successfully;
- source and restored fingerprints are identical (`diff` prints no differences);
- invariant lines report `0` for duplicate external lead IDs, contact submission-count mismatch and failed migrations;
- restored lead/status and migration queries are readable;
- `BACKUP_RESTORE_SMOKE=PASS` is printed;
- the disposable restore database is dropped afterward.

## Evidence to record

Do **not** record raw database contents. Store only:

- test date/time;
- PostgreSQL / pg_dump / pg_restore versions;
- dump file size;
- archive entry count;
- `BACKUP_RESTORE_SMOKE=PASS`;
- the hash/count fingerprint lines (they contain no raw values);
- whether Coolify's scheduled/manual backup execution also completed successfully;
- whether an off-host/S3 backup destination is configured.

## Production note

The disposable restore proof establishes logical recoverability of the current PostgreSQL schema/data. Before the M7 production switch, the operational backup schedule and retention must also be appropriate for the production RPO/RTO, and an off-host copy should be configured when practical.
