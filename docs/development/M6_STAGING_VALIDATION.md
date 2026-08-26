# M6 — Coolify Staging Validation Record

## Status

**STAGING FOUNDATION / AUTH / FOLLOW-UP / READ MODELS / MANUAL IMPORT: PASS**  
**ADDITIONAL-USER CREDENTIAL LIFECYCLE: CODE/CI PASS, STAGING PENDING**

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

- rolling update starts a new container before retiring the old container;
- Dockerfile custom healthcheck is detected by Coolify;
- `/health/ready` passes after the configured start period;
- readiness identifies `ertip-lead-manager-server` version `0.1.0` with status `ready`;
- HTTPS `/health/ready` succeeds through the staging hostname;
- readiness includes a real PostgreSQL dependency check and therefore validates API → PostgreSQL connectivity.

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

A synthetic staging-only lead was used; no customer data was required.

Validated:

- authenticated ADMIN access to follow-up routes;
- create follow-up in `OPEN` state;
- list follow-ups for a lead;
- reschedule with revision increment;
- stale `expectedRevision` rejection with HTTP `409`;
- complete transition to `COMPLETED`;
- final list reflects the terminal state.

## Pipeline / dashboard / analytics staging validation

A deliberate green-checkpoint deployment validated the PostgreSQL read models with the synthetic staging lead.

Validated:

- pipeline returned all eight status columns (`NEW`, `CONTACTED`, `REPLIED`, `QUALIFIED`, `QUOTE_SENT`, `WON`, `LOST`, `INVALID`);
- `perColumnLimit` was `100` and the synthetic lead appeared in `NEW`;
- analytics returned zero submissions/unique/repeat rows as expected because the synthetic lead had no source submission;
- analytics still returned all eight current-status funnel buckets;
- dashboard returned one total contact and one NEW contact;
- the synthetic lead appeared in `newUncontacted`;
- follow-up/repeat/quality attention groups were empty as expected for that fixture.

## Manual import staging validation

Manual import was validated through the public HTTPS API using a generated staging-only UTF-8 CSV containing six synthetic rows and no real customer data.

Preview result:

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

The first commit returned the same plan summary and recorded a `COMMITTED` import batch with five imported submissions, one exact duplicate, one repeat submission and zero warnings/errors.

The exact same file was then submitted a second time. The second commit returned:

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

Import history then contained two committed batches for the synthetic file: the original five-submission import and the zero-submission all-duplicate reimport. This validates live staging idempotency while preserving import-batch history.

## Credential lifecycle CI checkpoint

The additional-user credential lifecycle is implemented but has not yet been deployed to staging.

PostgreSQL 17 integration currently validates:

- ADMIN-only one-time invitation token issuance;
- SHA-256 token-hash persistence rather than raw token storage;
- one-use 24-hour activation token semantics;
- user-chosen Argon2id password activation;
- multiple sessions followed by self password change;
- current-session retention and other-session revocation after self password change;
- ADMIN reset revoking all sessions and blocking old-password login immediately;
- reset activation with a new password;
- credential security-event persistence;
- atomic reset-gate recheck and login session creation to prevent reset/login races.

The server suite passes 28/28 tests with this lifecycle included. Real staging remains the next credential gate.

## Secret hygiene

Current staging policy:

- `DATABASE_URL` remains runtime-only;
- bootstrap ADMIN name/e-mail/password variables are no longer present after first-account validation;
- real database credentials, login passwords, bearer tokens and activation/reset tokens are not committed;
- PostgreSQL remains private/internal to Coolify;
- Coolify auto-deploy is disabled during active M6 development so only deliberate green checkpoints are deployed;
- the installed frozen local Tauri application is not pointed at staging during M6.

## Remaining staging validation

M6 still requires staged validation/evidence for:

- additional-user invitation/activation/password-change/reset lifecycle;
- PostgreSQL backup/restore;
- SQLite schema-v4 → PostgreSQL migration/reconciliation.

Passing these checkpoints does not by itself authorize switching the production Tauri client to API mode. That remains an M7 action after the complete M6 acceptance gate.
