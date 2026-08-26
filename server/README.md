# Ertip Lead Manager Server

M6 centralized API/auth process.

## Runtime

Required `DATABASE_URL` points to private PostgreSQL. Optional runtime settings: `ELM_BIND_ADDR`, `ELM_DB_MAX_CONNECTIONS`, `ELM_SESSION_TTL_HOURS`, `RUST_LOG`. An empty database may bootstrap one initial ADMIN with temporary `ELM_BOOTSTRAP_ADMIN_*` values; remove them after validation.

The server applies embedded migrations before listening. `/health/ready` includes a PostgreSQL dependency check.

## Authentication

```text
POST /api/v1/auth/login/tauri
POST /api/v1/auth/login/web
POST /api/v1/auth/logout
GET  /api/v1/me
POST /api/v1/auth/activate
POST /api/v1/auth/change-password
POST /api/v1/personnel/{userId}/auth/invitation
POST /api/v1/personnel/{userId}/auth/reset
```

Tauri uses opaque bearer sessions and Web uses Secure/HttpOnly/SameSite=Lax cookies. Only SHA-256 session-token hashes are persisted. Passwords use Argon2id.

ADMIN can issue a 24-hour one-time invitation token to active personnel with an e-mail and no existing credentials. Only the token hash is stored; the user activates it and chooses a 12–128 character password.

ADMIN reset marks credentials reset-pending, blocks old-password login, revokes every active target session and returns a one-time reset token. The replacement password is established through the same activation endpoint. Final login reset-gate checking and session insertion are atomic under a PostgreSQL credential-row lock.

Self password change keeps the current session and revokes all other sessions. Credential events are recorded in `auth_security_events`. M6 does not couple token delivery to an e-mail provider.

## Authorization

- ADMIN: personnel + credential administration and all CRM/read-model/import operations.
- MANAGER: personnel read and global CRM/read-model/import operations; no personnel/credential administration.
- SALES: assigned-own CRM scope only; no personnel, assignment, credential administration or import.

## CRM / read models / imports

Implemented: personnel, lead list/detail/status/assignment, notes, append-only product overrides, follow-ups, pipeline, dashboard, analytics and manual import.

Manual import accepts real `.csv`/`.xlsx` multipart uploads up to 20 MiB. Preview is read-only. Commit reparses/replans against current PostgreSQL state, serializes import transactions, rolls back on blocking identity/row errors, skips exact duplicate external IDs, preserves CRM status on repeat submissions and keeps agency CRM-looking fields raw-payload-only. Reimport is submission-idempotent while retaining batch history.

Mutable CRM resources use revision-based lost-update protection and server-derived audit actors.

## Coolify checkpoint

Real staging PASS: foundation/auth/bootstrap cleanup, follow-ups, pipeline/dashboard/analytics and manual import preview/commit/history/idempotent reimport.

Credential lifecycle is code/CI PASS with **28/28 PostgreSQL server tests** and is the next deliberate staging smoke test.

Remaining after credential staging: PostgreSQL backup/restore evidence, SQLite schema-v4 → PostgreSQL migration/reconciliation and secure Tauri token storage before M7 production API rollout.
