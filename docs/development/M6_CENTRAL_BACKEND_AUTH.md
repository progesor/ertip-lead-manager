# M6 — Centralized Backend, PostgreSQL and Authentication

## Status

**IN PROGRESS** — Issue #14, draft PR #15, branch `feat/m6-central-backend-foundation`. Frozen local fallback remains `v0.1.0-local`.

## Target

```text
Windows Tauri ─┐
               ├── HTTPS /api/v1 ── Rust/Axum API/Auth ── private PostgreSQL
Future Web ────┘
```

PostgreSQL is never client-facing. Server sessions establish trusted user/audit identity. Tauri uses opaque bearer sessions; Web uses Secure/HttpOnly cookies. Passwords use Argon2id.

## Implemented

- PostgreSQL schema/migrations/readiness and Coolify container contract;
- bootstrap ADMIN, login/logout/current-session, failed-login lock;
- ADMIN/MANAGER/SALES server-side RBAC;
- personnel/assignment and lead list/detail/status;
- notes, append-only product overrides and follow-ups;
- pipeline/dashboard/analytics;
- real CSV/XLSX manual import preview/commit/history;
- additional-user invitation/activation/password-change/reset lifecycle.

## Credential lifecycle

ADMIN issues a 24-hour one-time `PROVISION` token to active personnel with e-mail and no credentials. Only the SHA-256 token hash is stored. The user activates it and chooses a 12–128 character password.

ADMIN reset immediately marks reset-pending, revokes all target sessions and blocks old-password login, then returns a one-time `RESET` token. Reset activation establishes the replacement password. Self password change retains the current session and revokes other sessions. Credential events are stored in `auth_security_events`.

The final login reset-gate check and session insertion are atomic under a PostgreSQL credential-row lock, eliminating the old-password reset/login race.

Credential administration is ADMIN-only. E-mail delivery is deliberately outside the M6 server lifecycle.

## Manual import

Server parses actual `.csv` / `.xlsx`; client-normalized JSON is never trusted. Preview is read-only. Commit reparses/replans from current PostgreSQL state, serializes concurrent imports, rolls back on blocking identity/row errors, skips exact duplicate external IDs, preserves existing CRM status on repeat submissions and keeps agency `Status` / `İletişime Geçme Tarihi` raw-payload-only. Reimport is submission-idempotent while still creating import-batch history.

## Real staging PASS

`lead-api-staging.progesor.net` has passed:

- rolling deployment / PostgreSQL readiness;
- first ADMIN bootstrap and removal of bootstrap secrets;
- HTTPS login, `/me`, logout and revoked-token 401;
- follow-up lifecycle including stale 409;
- pipeline/dashboard/analytics;
- manual import preview/commit/history;
- exact-file reimport yielding zero new submissions and six exact duplicates with a second committed batch.

## CI

PostgreSQL 17 server suite passes **28/28 tests**, including credential invitation/activation, multi-session self password change, ADMIN reset, session revocation, old-password rejection, reset activation, manual import and all prior CRM/read-model tests.

## Remaining M6 acceptance

1. credential lifecycle real-staging smoke test;
2. PostgreSQL backup/restore runbook + evidence;
3. SQLite schema-v4 → PostgreSQL migration/reconciliation + representative test;
4. secure Tauri token storage before M7 production API switch.

PR #15 remains draft/open until these gates pass.
