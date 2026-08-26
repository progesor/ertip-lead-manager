# M6 — Coolify Staging Validation Record

## Status

**PASS:** foundation/auth/bootstrap, follow-ups, pipeline/dashboard/analytics, manual imports.  
**CODE + CI PASS / STAGING PENDING:** additional-user credential lifecycle.

Validation date: 2026-08-26  
Environment: Coolify staging  
Public API hostname: `lead-api-staging.progesor.net`  
Database: private PostgreSQL 17  
Source branch: `feat/m6-central-backend-foundation`

This evidence intentionally excludes passwords, connection strings, raw bearer tokens, raw activation/reset tokens and real customer PII.

## Proven live-staging checkpoints

- Cloudflare HTTPS → Coolify Axum container → private PostgreSQL 17;
- rolling deploy + custom `/health/ready` PostgreSQL dependency check;
- first ADMIN bootstrap, HTTPS bearer login, `/me`, logout 204, revoked-token 401;
- bootstrap ADMIN environment variables removed, healthy redeploy, persisted ADMIN login;
- synthetic follow-up create/list/reschedule/stale-409/complete;
- pipeline all eight columns with `perColumnLimit=100` and synthetic lead in `NEW`;
- analytics expected zero-submission result with all eight funnel buckets;
- dashboard total/new KPI = 1 and synthetic lead in `newUncontacted`.

## Manual import live-staging checkpoint

A generated six-row synthetic UTF-8 CSV was uploaded over HTTPS.

First preview/commit:

```text
totalRows             = 6
importableSubmissions = 5
newContacts           = 4
repeatSubmissions     = 1
exactDuplicates       = 1
identityConflicts     = 0
rowErrors             = 0
warningCount          = 0
```

The first commit recorded five submissions and a `COMMITTED` batch. Re-submitting the identical file returned zero importable submissions and six exact duplicates, while history recorded a second `COMMITTED` batch. Live import idempotency and batch-history preservation are PASS.

## Credential lifecycle pre-staging gate

Implemented and PostgreSQL-CI validated:

- ADMIN-only 24-hour `PROVISION` / `RESET` tokens;
- only SHA-256 token hashes persisted;
- single-use activation/reset;
- user-selected Argon2id password;
- self password change retaining current session and revoking other sessions;
- ADMIN reset immediately blocking old-password login and revoking all target sessions;
- reset activation establishing the replacement password;
- separate credential security-event audit;
- atomic login reset-gate/session insertion under credential-row locking.

Server suite: **28/28 PASS**. Real credential staging smoke is next.

## Secret / deployment policy

- PostgreSQL remains private/internal;
- runtime secrets are not build-time variables;
- bootstrap ADMIN secrets remain removed;
- Coolify auto-deploy remains OFF during active M6;
- frozen local Tauri release is not pointed at staging.

## Remaining M6 validation

1. additional-user credential lifecycle staging smoke;
2. PostgreSQL backup/restore evidence;
3. SQLite schema-v4 → PostgreSQL migration/reconciliation;
4. secure Tauri token storage before M7 production rollout.

PR #15 remains draft/open until all M6 acceptance gates pass.
