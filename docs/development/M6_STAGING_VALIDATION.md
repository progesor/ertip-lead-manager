# M6 — Coolify Staging Validation Record

## Status

**PASS:** foundation/auth/bootstrap, follow-ups, pipeline/dashboard/analytics, manual imports.  
**CODE + CI PASS / STAGING PENDING:** additional-user credential lifecycle.

Validation date: 2026-08-26  
Environment: Coolify staging  
Public API hostname: `lead-api-staging.progesor.net`  
Database: private PostgreSQL 17  
Source branch: `feat/m6-central-backend-foundation`

No passwords, database connection strings, raw bearer tokens, raw invitation/reset tokens, personal e-mail addresses or other secrets/PII are recorded here.

## Deployment / foundation

Validated through Cloudflare HTTPS → Coolify Axum container → private PostgreSQL 17:

- rolling deployment;
- Docker custom `/health/ready` healthcheck;
- real PostgreSQL readiness dependency check;
- HTTPS public health endpoint;
- first ADMIN bootstrap;
- Tauri bearer login + `/me`;
- logout 204 + revoked-token 401;
- removal of all bootstrap ADMIN environment variables;
- healthy redeploy and persisted ADMIN login without bootstrap secrets.

## Follow-up staging

A synthetic staging-only lead validated authenticated create/list/reschedule/stale-409/complete behavior.

## Pipeline / dashboard / analytics staging

Using the synthetic lead:

- pipeline returned all eight status columns with `perColumnLimit=100` and the lead in `NEW`;
- analytics returned the expected zero-submission result and all eight funnel buckets;
- dashboard returned total/new KPI = 1 and the lead in `newUncontacted`;
- other attention groups were empty as expected.

## Manual import staging

A generated staging-only UTF-8 CSV with six synthetic rows was uploaded through the public HTTPS API.

Preview / first commit:

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

The first commit created five submissions and a committed batch. Re-submitting the exact same file produced:

```text
totalRows             = 6
importableSubmissions = 0
newContacts           = 0
repeatSubmissions     = 0
exactDuplicates       = 6
identityConflicts     = 0
rowErrors             = 0
warningCount          = 0
```

History contained both committed batches: the original import and the zero-submission all-duplicate reimport. Live staging idempotency and batch-history preservation are therefore PASS.

## Credential lifecycle pre-staging gate

Implemented server lifecycle:

- ADMIN-only 24-hour one-time `PROVISION` / `RESET` token issuance;
- only SHA-256 token hashes persisted;
- user-selected Argon2id password activation;
- single-use token semantics;
- authenticated self password change;
- self-change keeps current session and revokes all other sessions;
- ADMIN reset immediately blocks old-password login and revokes all target sessions;
- reset activation establishes the new password;
- credential security events persisted separately;
- login reset-gate and session insertion atomic under PostgreSQL credential-row locking.

PostgreSQL 17 server suite: **28/28 PASS**. Real Coolify staging is the next gate for this slice.

## Secret hygiene

- `DATABASE_URL` is runtime-only;
- bootstrap ADMIN variables are removed;
- PostgreSQL is private/internal;
- Coolify auto-deploy is disabled during active M6 work;
- the frozen local Tauri release is not pointed at staging.

## Remaining M6 validation

1. additional-user credential staging smoke test;
2. PostgreSQL backup/restore evidence;
3. SQLite schema-v4 → PostgreSQL migration/reconciliation;
4. secure Tauri token storage before M7 production API rollout.

PR #15 remains draft/open until the full M6 acceptance gate passes.
