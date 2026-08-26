# Ertip Lead Manager Server

M6 centralized Rust/Axum API backed by private PostgreSQL.

## Runtime

Required: `DATABASE_URL`. Optional: `ELM_BIND_ADDR`, `ELM_DB_MAX_CONNECTIONS`, `ELM_SESSION_TTL_HOURS`, `RUST_LOG`. Empty databases may temporarily use `ELM_BOOTSTRAP_ADMIN_*`; remove those values after the initial ADMIN is validated.

`/health/ready` performs a PostgreSQL readiness check. Coolify exposes only the HTTPS API; PostgreSQL remains private.

## Authentication / credentials

Tauri uses opaque bearer sessions; Web uses Secure/HttpOnly/SameSite=Lax cookies. Only SHA-256 session-token hashes are persisted. Passwords use Argon2id.

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

ADMIN issues 24-hour one-time invitation/reset tokens; only their SHA-256 hashes are stored. Users choose their own 12–128 character passwords at activation. ADMIN reset immediately blocks old-password login and revokes all target sessions. Self password change keeps the current session and revokes all others. Login reset-gate checking and session insertion are atomic under the PostgreSQL credential-row lock. Credential security events are persisted separately.

## Authorization

- ADMIN: personnel/credential administration + all CRM/read-model/import operations.
- MANAGER: personnel read + global CRM/read-model/import operations; no personnel/credential mutation.
- SALES: assigned-own CRM scope only; no assignment, credential administration or import.

## CRM / read models / import

Implemented: personnel, lead list/detail/status/assignment, notes, product overrides, follow-ups, pipeline, dashboard, analytics and manual CSV/XLSX import.

Manual import preview is read-only. Commit reparses/replans against current PostgreSQL state, serializes concurrent imports, rolls back blocking identity/row errors, skips exact duplicate external IDs, preserves CRM status on repeat submissions and remains submission-idempotent while recording batch history.

Mutable CRM state uses revision conflict protection and server-derived audit actors.

## M6 checkpoint

Real staging PASS: foundation/auth/bootstrap cleanup, follow-ups, pipeline/dashboard/analytics and manual import including exact-file duplicate reimport.

Credential lifecycle is code/CI PASS with **28/28 PostgreSQL server tests** and is the next deliberate staging gate.

Remaining: credential staging, backup/restore evidence, SQLite-v4 → PostgreSQL migration/reconciliation, and secure Tauri token storage before M7 production API switch.
