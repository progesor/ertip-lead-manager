# M6 — Centralized Backend, PostgreSQL and Authentication

## Status

**IN PROGRESS** — Issue #14, draft PR #15, branch `feat/m6-central-backend-foundation`. Frozen fallback remains `v0.1.0-local`.

## Architecture

```text
Windows Tauri ─┐
               ├── HTTPS /api/v1 ── Rust/Axum Auth + CRM API ── private PostgreSQL
Future Web ────┘
```

PostgreSQL is never client-facing. Web uses Secure/HttpOnly sessions; Tauri uses opaque bearer sessions. Raw session tokens are not persisted. Passwords use Argon2id. Trusted audit actor identity always comes from the server session.

## Implemented M6 surface

- PostgreSQL migrations/constraints/indexes and dependency-aware readiness;
- Coolify Docker deployment contract;
- first-ADMIN bootstrap, login/logout/current session and failed-login lock;
- ADMIN/MANAGER/SALES server-side authorization;
- personnel and assignment;
- lead list/detail/status;
- notes and append-only product overrides;
- follow-up lifecycle;
- pipeline/dashboard/analytics read models;
- server-side CSV/XLSX manual import preview/commit/history;
- additional-user invitation/activation/password-change/reset lifecycle.

SALES access is assigned-own only for CRM/read models. MANAGER has global CRM/read-model/import access but no personnel or credential administration. ADMIN has all current privileges.

## Additional-user credential lifecycle

ADMIN can issue a 24-hour one-time `PROVISION` token for active personnel with an e-mail and no credentials. Only the SHA-256 hash is stored; the raw token is returned once. The user activates the token and chooses a 12–128 character password, which is Argon2id-hashed.

ADMIN reset marks the target credential reset-pending, revokes all target sessions, blocks old-password login, revokes unused one-time tokens and returns a 24-hour `RESET` token. Reset activation establishes the replacement password.

Self password change keeps the current session but revokes all other sessions. Credential security events are stored separately in `auth_security_events`.

The final login reset-gate check and session insertion share one PostgreSQL transaction under the credential-row lock. If reset happens first, login cannot insert a session; if login happens first, the following reset revokes it.

E-mail delivery is intentionally not coupled to this lifecycle in M6.

## Manual import invariants

- actual `.csv` / `.xlsx` uploads; server never trusts client-normalized lead JSON;
- preview is read-only;
- commit reparses/replans using current PostgreSQL state;
- advisory transaction lock serializes concurrent imports;
- identity conflict/row error blocks the entire commit;
- exact duplicate external IDs are skipped;
- repeat submissions do not overwrite CRM status;
- agency `Status` / `İletişime Geçme Tarihi` remain raw-payload-only;
- repeated upload is idempotent for submissions but still creates batch history;
- authenticated ADMIN/MANAGER actor is recorded for imported activities.

## Real staging PASS

At `lead-api-staging.progesor.net`:

- rolling deployment + PostgreSQL readiness;
- first ADMIN bootstrap and later bootstrap-secret removal;
- HTTPS login, `/me`, logout and revoked-token 401;
- follow-up create/list/reschedule/stale-409/complete;
- pipeline/dashboard/analytics;
- manual import preview/first commit/history;
- exact-file reimport producing 0 importable submissions and 6 exact duplicates while recording a second committed batch.

No real customer data or raw secrets are recorded in staging evidence.

## CI checkpoint

The PostgreSQL 17 server suite passes **28/28 tests** with credential invitation/activation, multi-session self password change, ADMIN reset, session revocation, old-password rejection, reset activation, manual import, CRM/follow-up and read-model tests included.

## Remaining M6 acceptance work

1. deliberate real-staging credential lifecycle smoke test;
2. PostgreSQL backup/restore runbook + evidence;
3. SQLite schema-v4 → PostgreSQL migration/reconciliation tooling + representative test;
4. secure Tauri token storage before M7 production API switch.

PR #15 stays draft/open and M6 stays open until these gates pass.
