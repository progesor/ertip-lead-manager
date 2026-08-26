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

An empty database may bootstrap one initial ADMIN with temporary `ELM_BOOTSTRAP_ADMIN_*` runtime variables. The bootstrap password must be 12–128 characters. Bootstrap runs only when `app_users` is empty; remove all bootstrap variables after the initial account is validated.

The server applies embedded SQLx migrations at startup before accepting traffic.

## Core auth routes

```text
POST /api/v1/auth/login/tauri
POST /api/v1/auth/login/web
POST /api/v1/auth/logout
GET  /api/v1/me

POST /api/v1/personnel/{userId}/auth/invitation
POST /api/v1/personnel/{userId}/auth/reset
POST /api/v1/auth/activate
POST /api/v1/auth/change-password
```

Tauri uses an opaque bearer token. Web uses a `Secure; HttpOnly; SameSite=Lax` cookie. Raw session tokens are never stored; only SHA-256 hashes are persisted. Passwords use Argon2id.

### Additional-user credentials

Personnel are stable CRM identities and gain login credentials through a separate lifecycle.

`ADMIN` may issue a 24-hour one-time `PROVISION` token for an active personnel record with an e-mail address and no existing credentials. The request carries the current personnel `expectedRevision`. Only a SHA-256 hash of the token is persisted; the raw token is returned once.

The user chooses a 12–128 character password through `POST /api/v1/auth/activate`. The token is single-use and the chosen password is Argon2id-hashed.

`ADMIN` may later issue a one-time `RESET` token. Starting reset immediately marks the credential reset-pending, clears failed-login lock state, revokes every active session for the target user, revokes previous unused one-time tokens and increments personnel revision. Old-password login is blocked until the reset token is activated with a new password.

The final login reset-gate check and session insertion occur atomically in a PostgreSQL transaction that locks the credential row. Whichever operation wins the row lock determines the outcome: reset-first prevents session creation; login-first creates a session that the following reset then revokes.

Authenticated users may call `POST /api/v1/auth/change-password`. A successful self-change keeps the current session and revokes all other sessions. Credential events are persisted in `auth_security_events`. E-mail delivery is intentionally not part of M6; token delivery remains provider/UI independent.

## CRM / read-model / import routes

```text
GET   /api/v1/personnel
POST  /api/v1/personnel
PATCH /api/v1/personnel/{userId}
PATCH /api/v1/personnel/{userId}/active

GET    /api/v1/leads
GET    /api/v1/leads/{contactId}
PUT    /api/v1/leads/{contactId}/assignment
PATCH  /api/v1/leads/{contactId}/status
POST   /api/v1/leads/{contactId}/notes
PATCH  /api/v1/leads/{contactId}/notes/{noteId}
DELETE /api/v1/leads/{contactId}/notes/{noteId}
PUT    /api/v1/leads/{contactId}/product-interests/{productCode}

GET   /api/v1/leads/{contactId}/follow-ups
POST  /api/v1/leads/{contactId}/follow-ups
PATCH /api/v1/leads/{contactId}/follow-ups/{followUpId}
POST  /api/v1/leads/{contactId}/follow-ups/{followUpId}/complete
POST  /api/v1/leads/{contactId}/follow-ups/{followUpId}/cancel

GET /api/v1/pipeline
GET /api/v1/dashboard/attention
GET /api/v1/analytics

POST /api/v1/imports/preview
POST /api/v1/imports/commit
GET  /api/v1/imports/history
```

Authorization:

- `ADMIN`: personnel mutation, credential lifecycle, all CRM/read-model/import operations.
- `MANAGER`: personnel read, all lead CRM/read-model operations and manual imports; no personnel or credential administration.
- `SALES`: only currently assigned own leads; no personnel, assignment, credential administration or manual import.

Mutable centralized CRM resources use revision-based lost-update protection. Server-derived session identity is the trusted audit actor.

## Manual import

The API accepts real `.csv` and `.xlsx` multipart uploads, maximum 20 MiB, field name `file`. Server-side parsing uses canonical normalization/product/identity rules. Preview is read-only. Commit reparses/replans against current PostgreSQL state and uses a transaction-level advisory lock so concurrent imports cannot race the same identity snapshot.

Identity conflicts or row errors roll back the complete commit. Exact duplicate external submission IDs are skipped. Re-importing the same file is idempotent for submissions while recording a new import-batch history row. Repeat submissions do not overwrite current CRM status. Agency `Status` and `İletişime Geçme Tarihi` fields remain raw-payload-only.

## Health / Coolify

```text
GET /health/live
GET /health/ready
```

`/health/ready` includes a real PostgreSQL check.

Build:

```bash
docker build -f server/Dockerfile -t ertip-lead-manager-server .
```

Recommended Coolify properties: private PostgreSQL, public HTTPS API only, container port 8080, `/health/ready` custom healthcheck, runtime-only secrets.

## Current M6 boundary

Implemented: foundation, PostgreSQL schema, auth/RBAC, personnel/assignment, lead CRM, notes, product overrides, follow-ups, pipeline/dashboard/analytics, manual imports and additional-user credential lifecycle.

Real Coolify staging PASS: foundation/auth/bootstrap cleanup, follow-ups, pipeline/dashboard/analytics and manual-import preview/commit/history/idempotent reimport. Credential lifecycle is code/CI PASS (28/28 PostgreSQL server tests) and is the next deliberate staging smoke test.

Remaining after credential staging: PostgreSQL backup/restore evidence, SQLite schema-v4 → PostgreSQL migration/reconciliation and secure Tauri token storage before M7 switches the production client to API mode.
