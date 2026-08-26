# Ertip Lead Manager Server

M6 centralized API/auth process.

## Runtime contract

Required:

```text
DATABASE_URL=postgres://user:password@localhost:5432/ertip_lead_manager
```

Optional:

```text
ELM_BIND_ADDR=0.0.0.0:8080
ELM_DB_MAX_CONNECTIONS=10
ELM_SESSION_TTL_HOURS=12
RUST_LOG=info,ertip_lead_manager_server=debug,tower_http=info
```

An empty database may bootstrap one initial ADMIN with temporary `ELM_BOOTSTRAP_ADMIN_*` runtime variables. Bootstrap applies only while `app_users` is empty. Remove all bootstrap variables after validation.

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

Tauri uses opaque bearer sessions; Web uses Secure/HttpOnly/SameSite=Lax cookies. Only SHA-256 session-token hashes are persisted. Passwords use Argon2id.

Additional personnel are CRM identities first. ADMIN may issue a 24-hour one-time invitation token to active personnel with an e-mail address and no credentials. Only the token hash is stored. The user activates it and chooses a 12–128 character password.

ADMIN reset immediately marks the credential reset-pending, revokes all active target sessions, blocks old-password login and issues a new 24-hour one-time reset token. The same activation endpoint establishes the replacement password. The final login reset-gate check and session insertion are atomic under the PostgreSQL credential-row lock, closing the reset/login race.

Authenticated self password change keeps the current session but revokes all other sessions. Credential events are written to `auth_security_events`. M6 does not send invitation e-mail; delivery remains a later UI/integration concern.

Authorization: ADMIN administers personnel/credentials and all CRM functions; MANAGER has personnel read plus global CRM/read-model/import access but no personnel/credential mutation; SALES sees/edits only assigned own leads and cannot run imports or credential administration.

## CRM / read models / imports

Implemented routes cover personnel, lead list/detail/status/assignment, notes, product overrides, follow-ups, pipeline, dashboard, analytics and manual import.

Manual import accepts real `.csv`/`.xlsx` multipart uploads up to 20 MiB. Preview is read-only. Commit reparses/replans against current PostgreSQL state, serializes imports with an advisory transaction lock, rolls back on identity conflicts/row errors, skips exact duplicate external IDs, preserves current CRM status on repeat submissions and keeps agency `Status` / `İletişime Geçme Tarihi` raw-payload-only. Re-import is idempotent for submissions while retaining batch history.

Mutable centralized CRM resources use revision-based conflict protection and server-derived session actors.

## Health / Coolify

```text
GET /health/live
GET /health/ready
```

`/health/ready` includes a real PostgreSQL check.

```bash
docker build -f server/Dockerfile -t ertip-lead-manager-server .
```

Coolify: private PostgreSQL, public HTTPS API only, port 8080, `/health/ready`, runtime-only secrets.

## Current M6 boundary

Implemented: foundation, PostgreSQL schema, auth/RBAC, personnel/assignment, lead CRM, notes, product overrides, follow-ups, pipeline/dashboard/analytics, manual imports and additional-user credential lifecycle.

Real staging PASS: foundation/auth/bootstrap cleanup, follow-ups, pipeline/dashboard/analytics and manual-import preview/commit/history/idempotent reimport. Credential lifecycle is code/CI PASS with 28/28 PostgreSQL server tests and is the next staging gate.

Remaining after credential staging: PostgreSQL backup/restore evidence, SQLite schema-v4 → PostgreSQL migration/reconciliation and secure Tauri token storage before M7 switches the production client to API mode.
