# M6 — Coolify Staging Validation Record

## Status

**STAGING FOUNDATION / AUTH / FOLLOW-UP / READ MODELS / MANUAL IMPORT: PASS**  
**ADDITIONAL-USER CREDENTIAL LIFECYCLE: CODE/CI PASS, STAGING PENDING**

Validation date: 2026-08-26  
Environment: Coolify staging  
Public API hostname: `lead-api-staging.progesor.net`  
Database: private PostgreSQL 17  
Source branch: `feat/m6-central-backend-foundation`

This record intentionally excludes passwords, raw session tokens, raw activation/reset tokens, database connection strings, personal e-mail addresses and other secrets/PII.

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

The staging application is deployed from the M6 branch through `server/Dockerfile`. Rolling deploy, custom readiness healthcheck, HTTPS routing and API → PostgreSQL connectivity are PASS.

## Bootstrap ADMIN validation

Validated:

- initial ADMIN bootstrap on an empty PostgreSQL database;
- HTTPS Tauri login and `/me`;
- logout HTTP 204 and revoked-token HTTP 401;
- removal of all bootstrap ADMIN environment variables;
- healthy redeploy without bootstrap secrets;
- persisted ADMIN login after redeploy.

This proves bootstrap environment values are initial-provisioning material only.

## Follow-up API staging validation

Using a synthetic staging-only lead, authenticated ADMIN create/list/reschedule/stale-409/complete behavior passed. No customer data was required.

## Pipeline / dashboard / analytics staging validation

Validated with the synthetic staging lead:

- pipeline returned all eight status columns and `perColumnLimit=100`;
- the synthetic lead appeared in `NEW`;
- analytics returned the expected zero-submission result plus all eight funnel buckets;
- dashboard returned total/new KPI = 1 and the synthetic lead in `newUncontacted`;
- other attention groups were empty as expected for the fixture.

## Manual import staging validation

Manual import was validated through the public HTTPS API using a generated staging-only UTF-8 CSV containing six synthetic rows.

Preview:

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

The first commit recorded five imported submissions, one exact duplicate and one repeat submission with zero warnings/errors.

Submitting the exact same file again produced:

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

Import history contained two committed batches: the original five-submission import and the zero-submission all-duplicate reimport. This validates live idempotency while preserving batch history.

## Credential lifecycle CI checkpoint

The additional-user credential lifecycle is implemented but not yet deployed to staging.

PostgreSQL 17 integration validates:

- ADMIN-only invitation and reset token issuance;
- SHA-256 token-hash persistence rather than raw token storage;
- single-use 24-hour activation/reset tokens;
- user-chosen Argon2id password activation;
- multiple sessions followed by self password change;
- current-session retention and other-session revocation after self password change;
- ADMIN reset revoking every active session and immediately blocking old-password login;
- reset activation with a new password;
- credential security-event persistence;
- atomic login reset-gate recheck and session creation under the credential-row lock.

The server suite passes 28/28 tests with this lifecycle included. Real staging is the next credential gate.

## Secret hygiene

- `DATABASE_URL` is runtime-only;
- bootstrap ADMIN variables have been removed;
- PostgreSQL remains private/internal to Coolify;
- credentials/tokens are not committed or included in validation evidence;
- Coolify auto-deploy remains disabled so only deliberate green checkpoints are deployed;
- the frozen local Tauri application is not pointed at staging during M6.

## Remaining staging / M6 validation

- additional-user invitation/activation/password-change/reset lifecycle;
- PostgreSQL backup/restore evidence;
- SQLite schema-v4 → PostgreSQL migration/reconciliation evidence;
- secure Tauri token storage before M7 production API rollout.

These checkpoints do not authorize an early PR merge or production Tauri API switch.
