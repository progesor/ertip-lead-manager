# Ertip Lead Manager Server

M6 centralized API/auth process.

## Local development contract

Required environment:

```text
DATABASE_URL=postgres://user:password@localhost:5432/ertip_lead_manager
```

Optional runtime settings:

```text
ELM_BIND_ADDR=0.0.0.0:8080
ELM_DB_MAX_CONNECTIONS=10
ELM_SESSION_TTL_HOURS=12
RUST_LOG=info,ertip_lead_manager_server=debug,tower_http=info
```

First empty-database deployment may bootstrap one initial ADMIN:

```text
ELM_BOOTSTRAP_ADMIN_NAME=Ertip Admin
ELM_BOOTSTRAP_ADMIN_EMAIL=admin@example.com
ELM_BOOTSTRAP_ADMIN_PASSWORD=<long random secret>
```

The e-mail and password variables must be configured together. The password must be 12–128 characters. Bootstrap applies only when `app_users` is empty and never resets an existing user's password. Remove the bootstrap password secret after the initial ADMIN has been created successfully.

Run:

```bash
cargo run --manifest-path server/Cargo.toml
```

At startup the server:

1. validates configuration;
2. creates a lazy PostgreSQL pool;
3. applies embedded SQLx migrations;
4. performs first-ADMIN bootstrap only when the user table is empty and bootstrap configuration is present;
5. starts the HTTP listener.

## Health and authentication endpoints

```text
GET  /health/live
GET  /health/ready
GET  /api/v1
POST /api/v1/auth/login/tauri
POST /api/v1/auth/login/web
POST /api/v1/auth/logout
GET  /api/v1/me
```

`/health/live` proves the process/router is alive. `/health/ready` performs a PostgreSQL `SELECT 1` and returns a non-200 response when the database is not available.

### Tauri login

```http
POST /api/v1/auth/login/tauri
Content-Type: application/json

{
  "email": "admin@example.com",
  "password": "..."
}
```

The response includes an opaque session `token`. Send it as:

```http
Authorization: Bearer <token>
```

Only the SHA-256 hash of the session token is stored server-side.

### Web login

`POST /api/v1/auth/login/web` uses the same credentials but returns the session through a `Secure; HttpOnly; SameSite=Lax` cookie. The token is not included in the JSON body.

`GET /api/v1/me`, CRM endpoints and logout accept either the Tauri bearer token or the Web session cookie. Passwords are stored only as Argon2id hashes. Five failed password attempts trigger a temporary database-backed account lock.

## CRM API — current M6 slice

All CRM endpoints below require an authenticated session. `actor_user_id` is derived from that session and is never accepted as trusted request input.

```text
GET   /api/v1/personnel?includeInactive=false
POST  /api/v1/personnel
PATCH /api/v1/personnel/{userId}
PATCH /api/v1/personnel/{userId}/active

GET   /api/v1/leads
GET   /api/v1/leads/{contactId}
PUT   /api/v1/leads/{contactId}/assignment
PATCH /api/v1/leads/{contactId}/status
```

### Authorization policy

- `ADMIN`: all current personnel and CRM operations.
- `MANAGER`: may read personnel, read all leads, assign/unassign leads and change lead status; cannot create/update/deactivate personnel.
- `SALES`: may read only leads currently assigned to their own user ID and may change status only on those leads. They cannot browse other/unassigned leads, manage personnel or reassign leads.

A `SALES` caller requesting another assignee or the unassigned-only list is rejected server-side. Lead detail outside the caller's assignment scope returns not found so the API does not disclose the existence of another salesperson's lead.

### Optimistic concurrency

Mutable personnel, assignment and lead-status requests carry an `expectedRevision` value obtained from the latest API response. Successful writes increment the persisted revision. A stale write returns:

```text
HTTP 409
error.code = STALE_REVISION
```

This prevents a second client from silently overwriting a newer CRM change.

Example assignment body:

```json
{
  "assignedUserId": "user-uuid-or-stable-id",
  "expectedRevision": 3
}
```

Use `null` for `assignedUserId` to unassign a lead.

Example status body:

```json
{
  "status": "CONTACTED",
  "expectedRevision": 4
}
```

Current stable lead statuses remain `NEW`, `CONTACTED`, `REPLIED`, `QUALIFIED`, `QUOTE_SENT`, `WON`, `LOST`, `INVALID`.

### Personnel authentication state

Creating a personnel record in the current CRM slice creates the stable `app_users` identity but does not automatically issue a password. Personnel responses expose `authEnabled` so an ADMIN can distinguish a CRM identity from a credential-enabled login identity. Credential provisioning/invitation for additional users remains a later M6 authentication slice; the first ADMIN is still created through the one-time bootstrap contract above.

## Docker / Coolify

Build from repository root:

```bash
docker build -f server/Dockerfile -t ertip-lead-manager-server .
```

The container listens on port `8080` by default. Supply `DATABASE_URL` and authentication bootstrap/runtime secrets through Coolify environment configuration.

Recommended Coolify setup:

- Dockerfile: `server/Dockerfile`
- Build context: repository root
- Container port: `8080`
- Health/readiness path: `/health/ready`
- PostgreSQL: private/internal Coolify network only
- Public endpoint: API container via HTTPS reverse proxy

Do not expose PostgreSQL credentials to Tauri/Web clients.

## Current M6 boundary

The server foundation, PostgreSQL schema, server-side session authentication, RBAC policy and first PostgreSQL-backed CRM API slice (personnel, assignment, lead list/detail/status) are implemented and covered by the PostgreSQL 17 CI lane. Notes/product interests, follow-ups, pipeline/dashboard/analytics, manual import parity, additional-user credential provisioning, backup/restore operations and SQLite→PostgreSQL migration/reconciliation remain in M6. The frozen local Tauri build remains independent until M7 switches the Windows production client to the API.
