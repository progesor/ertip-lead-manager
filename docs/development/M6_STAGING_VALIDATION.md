# M6 — Coolify Staging Validation Record

## Status

**STAGING FOUNDATION / AUTH / FOLLOW-UP / READ MODELS: PASS**

Validation date: 2026-08-26  
Environment: Coolify staging  
Public API hostname: `lead-api-staging.progesor.net`  
Database: private PostgreSQL 17  
Source branch: `feat/m6-central-backend-foundation`

This record intentionally excludes passwords, raw session tokens, database connection strings, personal e-mail addresses and other secrets/PII.

## Validated deployment path

```text
Cloudflare / HTTPS
        ↓
lead-api-staging.progesor.net
        ↓
Coolify rolling deployment
        ↓
Rust / Axum API container
        ↓
private PostgreSQL 17
```

## Deployment / health evidence

The staging application is deployed from the M6 branch through `server/Dockerfile`.

Validated:

- rolling update starts a new container;
- Dockerfile custom healthcheck is detected by Coolify;
- `/health/ready` passes after the configured start period;
- readiness identifies `ertip-lead-manager-server` version `0.1.0` with status `ready`;
- the new container becomes healthy before the previous container is removed;
- HTTPS `/health/ready` succeeds through the staging hostname;
- readiness includes a real PostgreSQL dependency check, validating API → PostgreSQL connectivity as well as process liveness.

## Bootstrap ADMIN validation

First empty-database deployment used the temporary bootstrap ADMIN environment contract.

Validated:

1. initial ADMIN identity was created in PostgreSQL;
2. Tauri login over HTTPS succeeded;
3. returned authenticated user had role `ADMIN`;
4. opaque bearer session authenticated `GET /api/v1/me`;
5. logout returned HTTP `204`;
6. the logged-out bearer token subsequently returned HTTP `401`;
7. all bootstrap ADMIN environment variables were removed from Coolify;
8. the API was redeployed without bootstrap secrets;
9. the container returned healthy after redeploy;
10. the existing ADMIN could still log in successfully after redeploy.

This proves the bootstrap variables are initial-provisioning material only. The persisted PostgreSQL user/credential remains authoritative after bootstrap secrets are removed.

## Follow-up API staging validation

The PostgreSQL-backed follow-up HTTP slice was redeployed after its CI gate passed. A synthetic staging-only lead was used; no customer data was required.

Validated:

- authenticated ADMIN access to follow-up routes;
- create follow-up in `OPEN` state;
- list follow-ups for a lead;
- reschedule with revision increment;
- stale `expectedRevision` rejection with HTTP `409`;
- complete transition to `COMPLETED`;
- final list reflects the terminal state;
- staging remained healthy through deployment and smoke test.

This closes the M6 follow-up API slice across service tests, HTTP wiring, PostgreSQL 17 CI and real Coolify staging.

## Pipeline / dashboard / analytics staging validation

A deliberate green checkpoint was deployed after the PostgreSQL 17, Windows/local, frontend, server-image and Tauri packaging gates passed. The existing synthetic lead `staging-smoke-lead-001` was used to verify read-only behavior.

### Pipeline

`GET /api/v1/pipeline?includeTerminal=true` returned successfully and preserved the expected board model:

- eight columns: `NEW`, `CONTACTED`, `REPLIED`, `QUALIFIED`, `QUOTE_SENT`, `WON`, `LOST`, `INVALID`;
- `perColumnLimit = 100`;
- `visibleTotal = 1`;
- `staging-smoke-lead-001` appeared in `NEW`;
- terminal columns were present and empty;
- card fields for assignment, repeat state, product interests, platforms, warnings and open follow-up summary were returned without error.

### Analytics

`GET /api/v1/analytics` returned successfully. The synthetic smoke lead has no imported submission, therefore the expected result was an empty submission range and zero submission aggregates rather than a fabricated analytics record.

Validated:

- `submissions = 0`;
- `uniqueContacts = 0`;
- `repeatSubmissions = 0`;
- all eight current-status funnel buckets were returned;
- country/platform/product/campaign/form/adset/ad breakdown arrays returned cleanly as empty arrays.

### Dashboard

`GET /api/v1/dashboard/attention` returned successfully for explicit UTC day/recent/analytics windows.

Validated:

- `totalContacts = 1`;
- `newContacts = 1`;
- `staging-smoke-lead-001` appeared in `newUncontacted`;
- the 30-day submission summary remained zero as expected for a lead without a submission;
- due-today, overdue, recent-repeat and open-quality groups returned valid empty results.

This closes pipeline/dashboard/analytics across PostgreSQL integration tests and live Coolify staging read-only smoke validation.

## Manual import checkpoint

Server-side manual import parity is implemented after the read-model staging checkpoint. It accepts real CSV/XLSX multipart uploads and applies the canonical local import rules on the server rather than trusting client-normalized JSON.

The PostgreSQL 17 integration gate validates:

- preview is read-only;
- ADMIN/MANAGER import permission and SALES rejection;
- same-file plan with 4 new contacts, 1 repeat submission and 1 exact duplicate;
- first commit writes 5 unique submissions;
- second commit writes no duplicate submissions while recording batch history;
- repeat import does not overwrite an independently changed CRM status;
- agency `Status` and `İletişime Geçme Tarihi` values remain in raw payload only;
- authenticated actor is stored on `LEAD_CREATED` / `SUBMISSION_IMPORTED` activities;
- import history is persisted.

Manual import is **not yet marked staging PASS**. The next deliberate deployment must exercise preview → commit → history → exact-duplicate reimport over the public staging API with staging-only synthetic data.

## Secret hygiene

Current staging policy:

- `DATABASE_URL` remains runtime-only;
- bootstrap ADMIN name/e-mail/password variables are no longer present after first-account validation;
- real database credentials, login passwords and bearer tokens are not committed;
- PostgreSQL remains private/internal to Coolify;
- Coolify auto-deploy is disabled during active M6 development so only deliberate green checkpoints are deployed;
- the installed frozen local Tauri application is not pointed at staging during M6.

## Remaining staging validation

Foundation/auth/follow-up/read-model validation is complete. M6 still requires staged validation/evidence for:

- manual import preview/commit/history and duplicate reimport;
- representative personnel/role-policy and SALES assigned-only behavior where not already covered by server integration tests;
- additional-user credential lifecycle when implemented;
- PostgreSQL backup/restore;
- SQLite schema-v4 → PostgreSQL migration/reconciliation.

Passing these checkpoints does not close M6 or authorize switching the production Tauri client to API mode. That remains an M7 action after the complete M6 acceptance gate.
