# M6 — SQLite schema-v4 → PostgreSQL Migration / Reconciliation

## Purpose

Move the frozen local Tauri schema-v4 CRM dataset into the centralized PostgreSQL model without weakening identity, immutable source or audit guarantees.

The local Windows database is created under Tauri's application data directory as:

```text
ertip-lead-manager.sqlite3
```

The migration is a **one-time administrative CLI**, not an HTTP endpoint.

## Safety model

The CLI fails closed unless all of these conditions hold:

- source file exists;
- source SQLx schema version is exactly `4`;
- SQLite `PRAGMA foreign_key_check` is clean;
- target PostgreSQL schema contains the required centralized migrations;
- every target CRM/domain table is empty;
- no source `app_users` ID, e-mail or non-null `auth_subject` collides with an existing centralized user.

A PostgreSQL advisory transaction lock plus `ACCESS EXCLUSIVE` domain-table locks prevent a concurrent migration/write window.

The target may already contain centralized/bootstrap authentication users. They are retained if they do not collide with stable local personnel identities.

## Data preserved

The migration copies the following local v4 resources with the same stable IDs:

- `app_users`;
- `lead_contacts` and `assigned_user_id`;
- `import_batches`;
- immutable `lead_submissions`, including exact `raw_payload_json`;
- `submission_product_interests`;
- append-only `contact_product_interest_overrides`;
- `lead_notes`;
- `lead_activities`, including `actor_user_id` and audit timestamps;
- `follow_ups`;
- `lead_data_quality_issues`.

SQLite integer booleans such as `is_organic` are converted to PostgreSQL booleans. RFC3339 text timestamps are converted to `TIMESTAMPTZ` while preserving the represented instant.

## Centralized revision initialization

Local schema v4 predates centralized optimistic-concurrency revisions. Therefore migrated rows begin at:

```text
app_users.revision      = 0
lead_contacts.revision  = 0
lead_notes.revision     = 0
follow_ups.revision     = 0
```

For legacy follow-ups, PostgreSQL `updated_at` is initialized from `completed_at` when present, otherwise `created_at`.

These values represent the first centralized version of the preserved local state; they do not rewrite the historic local audit stream.

## Migration executable

The verified server image contains:

```text
/usr/local/bin/migrate_sqlite_v4
```

The CLI requires an explicit destructive-intent switch:

```text
migrate_sqlite_v4 --execute <path-to-schema-v4-sqlite-file>
```

It reads the target from `DATABASE_URL`. No client receives PostgreSQL credentials.

Example administrative execution pattern:

```sh
DATABASE_URL='<private target URL>' \
/usr/local/bin/migrate_sqlite_v4 --execute /migration/ertip-lead-manager.sqlite3
```

Do not paste the actual `DATABASE_URL` into tickets, chat logs or repository files.

## Reconciliation report

On success the CLI emits a JSON report with:

- source/target schema versions;
- source personnel count;
- source-vs-target row counts for every migrated domain table;
- SHA-256 comparison for migrated source user IDs;
- SHA-256 comparison for all stable domain IDs;
- SHA-256 comparison for exact raw submission payloads;
- SHA-256 comparison for activity/audit tuples;
- SHA-256 comparison for contact assignment tuples;
- final `allChecksPassed`.

A successful process exit requires all reconciliation checks to match.

The digest output is designed as migration evidence without printing raw customer payloads.

## Representative CI proof

The PostgreSQL 17 CI lane constructs an actual local database by applying the real Tauri migration chain:

```text
0001_initial.sql
0002_import_batch_format.sql
0003_lead_workspace_indexes.sql
0004_team_assignment.sql
```

The representative fixture includes:

- existing centralized ADMIN retained in PostgreSQL;
- two stable local personnel identities;
- assigned lead;
- committed import batch;
- two immutable submissions;
- Unicode/raw JSON payloads;
- product interests and manual override;
- note;
- audit activity with actor identity;
- follow-up;
- data-quality issue.

The integration test then verifies:

- all table counts reconcile;
- source user IDs reconcile;
- all stable domain IDs reconcile;
- raw payload hashes reconcile;
- audit history reconciles;
- assignments reconcile;
- local `is_organic` integer becomes PostgreSQL boolean;
- new centralized revisions start at zero;
- existing non-colliding centralized user remains;
- a second migration attempt fails closed because target domain data is no longer empty.

CI test:

```text
representative_schema_v4_migrates_with_stable_ids_raw_payloads_audit_and_assignments
```

Status at implementation checkpoint: **PASS** against PostgreSQL 17.

## Cutover procedure

The actual production migration should be performed only after the M6 backup/restore gate is proven and during a controlled write freeze:

1. stop/disable local CRM writes;
2. make a copy of the local `ertip-lead-manager.sqlite3` source file;
3. create/verify a PostgreSQL backup before migration;
4. ensure target CRM/domain tables are empty and expected centralized users are known;
5. run the migration CLI against the copied SQLite file;
6. save only the reconciliation JSON/digests as evidence, not database secrets;
7. require `allChecksPassed = true`;
8. perform API-level post-migration smoke reads and representative CRM mutations;
9. keep the frozen local DB/release as rollback evidence until M7 acceptance.

Do not run the CLI against the current staging CRM database while it contains synthetic smoke/import rows; the tool will reject that target by design.

## Rollback boundary

This utility never deletes source SQLite data. If migration fails, PostgreSQL transaction rollback protects the attempted copy. For a real cutover, PostgreSQL backup/restore remains the authoritative database rollback mechanism after any subsequent accepted centralized writes.
