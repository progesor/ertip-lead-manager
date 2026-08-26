# M6 — Coolify Staging Validation Record

## Status

**PASS:** foundation/auth/bootstrap, follow-ups, pipeline/dashboard/analytics, manual imports.  
**CODE + CI PASS / STAGING PENDING:** additional-user credential lifecycle.

Environment: `lead-api-staging.progesor.net` on Coolify + private PostgreSQL 17.  
Source branch: `feat/m6-central-backend-foundation`.

This record excludes passwords, database connection strings, raw bearer tokens, raw invitation/reset tokens and real customer PII.

## Live staging evidence

Validated:

- Cloudflare HTTPS → Coolify API → private PostgreSQL;
- rolling deploy and PostgreSQL-backed `/health/ready`;
- first ADMIN bootstrap;
- HTTPS bearer login + `/me`;
- logout 204 + revoked-token 401;
- bootstrap environment variables removed;
- healthy redeploy and persisted ADMIN login without bootstrap secrets;
- synthetic follow-up create/list/reschedule/stale-409/complete;
- pipeline all 8 statuses, `perColumnLimit=100`, synthetic lead in `NEW`;
- analytics expected zero-submission result with all 8 funnel buckets;
- dashboard total/new KPI = 1 and synthetic lead in `newUncontacted`.

## Manual import live staging

A six-row synthetic UTF-8 CSV produced on preview and first commit:

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

The first commit wrote five submissions and a committed batch. Re-submitting the exact same file produced zero importable submissions and six exact duplicates. Import history contained a second committed batch with zero imported submissions. Live import idempotency and batch-history preservation are PASS.

## Credential lifecycle pre-staging

Implemented and PostgreSQL-CI validated:

- ADMIN-only 24-hour one-time `PROVISION` / `RESET` tokens;
- SHA-256 token hashes only;
- user-chosen Argon2id password activation;
- single-use activation/reset;
- self password change retaining current session while revoking all others;
- ADMIN reset immediately blocking old-password login and revoking all target sessions;
- reset activation with replacement password;
- credential security-event audit;
- atomic login reset-gate/session insertion under credential-row locking.

PostgreSQL 17 server suite: **28/28 PASS**. Credential staging smoke is next.

## Deployment / secret policy

- PostgreSQL stays private/internal;
- runtime secrets are not build-time values;
- bootstrap ADMIN secrets stay removed;
- Coolify auto-deploy stays OFF during M6;
- frozen local Tauri remains independent.

## Remaining M6 validation

1. credential lifecycle staging smoke;
2. PostgreSQL backup/restore evidence;
3. SQLite schema-v4 → PostgreSQL migration/reconciliation;
4. secure Tauri token storage before M7 production rollout.

PR #15 remains draft/open until all acceptance gates pass.
