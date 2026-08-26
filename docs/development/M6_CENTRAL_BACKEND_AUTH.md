# M6 — Centralized Backend, PostgreSQL and Authentication

## Status

**IN PROGRESS** — Issue #14, draft PR #15, branch `feat/m6-central-backend-foundation`; frozen fallback `v0.1.0-local` remains independent.

## Architecture

```text
Tauri / Future Web → HTTPS /api/v1 → Rust/Axum Auth + CRM → private PostgreSQL
```

Server-side sessions provide trusted identity/audit actor context. Tauri uses opaque bearer sessions and Web uses Secure/HttpOnly cookies. Passwords are Argon2id-hashed.

## Implemented

- PostgreSQL migrations/readiness and Coolify container deployment;
- first-ADMIN bootstrap, login/logout/current-session and failed-login lock;
- ADMIN/MANAGER/SALES server-side RBAC;
- personnel/assignment, lead list/detail/status, notes, product overrides;
- follow-ups, pipeline, dashboard and analytics;
- canonical server-side CSV/XLSX import preview/commit/history;
- additional-user invitation/activation/password-change/reset lifecycle.

## Credential lifecycle

ADMIN creates 24-hour one-time `PROVISION` or `RESET` tokens; only SHA-256 token hashes are persisted. Users choose a 12–128 character password at `/api/v1/auth/activate`; passwords are Argon2id-hashed.

ADMIN reset immediately marks reset-pending, revokes all target sessions and blocks old-password login. Self password change keeps the current session and revokes all others. Credential events are stored in `auth_security_events`.

The final login reset-gate check and session insertion are atomic under the PostgreSQL credential-row lock: reset-first blocks login, login-first creates a session that the following reset revokes. Credential administration is ADMIN-only. E-mail delivery is outside this server lifecycle.

## Manual import invariants

Server parses actual source files; preview is read-only and commit reparses/replans against current PostgreSQL state. Concurrent imports are serialized. Blocking identity/row errors roll back the commit. Exact external-ID duplicates are skipped, repeat submissions preserve current CRM status, agency CRM-looking fields remain raw-only, and repeat upload is idempotent for submissions while preserving batch history.

## Real staging PASS

`lead-api-staging.progesor.net` has passed container/PostgreSQL readiness, bootstrap + secret removal, bearer auth/logout/revoke, follow-ups, pipeline/dashboard/analytics and manual import including exact-file all-duplicate reimport.

## CI

The PostgreSQL 17 server suite passes **28/28 tests** with credential lifecycle plus all existing CRM/read-model/import tests.

## Remaining M6 acceptance

1. credential lifecycle real-staging smoke;
2. PostgreSQL backup/restore evidence;
3. SQLite schema-v4 → PostgreSQL migration/reconciliation;
4. secure Tauri token storage before M7 production API switch.

PR #15 stays draft/open until all gates pass.
