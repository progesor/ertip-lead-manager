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

The e-mail and password variables must be configured together. The password must be 12–128 characters. Bootstrap applies only when `app_users` is empty and never resets an existing user's password. Remove all bootstrap ADMIN variables after the initial ADMIN has been created and validated.

Run:

```bash
cargo run --manifest-path server/Cargo.toml
```

At startup the server validates configuration, creates the PostgreSQL pool, applies embedded SQLx migrations, performs the optional first-ADMIN bootstrap and starts the HTTP listener.

## Health and authentication endpoints

```text
GET  /health/live
GET  /health/ready
GET  /api/v1
POST /api/v1/auth/login/tauri
POST /api/v1/auth/login/web
POST /api/v1/auth/logout
GET  /api/v1/me

POST /api/v1/personnel/{userId}/auth/invitation
POST /api/v1/personnel/{userId}/auth/reset
POST /api/v1/auth/activate
POST /api/v1/auth/change-password
```

`/health/live` proves the process/router is alive. `/health/ready` performs a PostgreSQL `SELECT 1` and returns a non-200 response when the database is unavailable.

Tauri login returns an opaque session token sent as `Authorization: Bearer <token>`. Only its SHA-256 hash is stored server-side. Web login uses the same credentials but returns the session through a `Secure; HttpOnly; SameSite=Lax` cookie. Passwords are stored as Argon2id hashes and repeated failed logins trigger the database-backed temporary lock policy.

### Additional-user credential lifecycle

Personnel records are CRM identities first. Creating personnel does not invent or expose a permanent password.

Only `ADMIN` may start credential invitation or reset. Both operations accept the latest personnel `expectedRevision`.

Invitation:

```text
POST /api/v1/personnel/{userId}/auth/invitation
{
  "expectedRevision": 0
}
```

The target must be active, have an e-mail address and not already have credentials. The server returns a random one-time `PROVISION` token with a 24-hour expiry. Only the SHA-256 token hash is persisted. The raw token is returned once and should be delivered through a trusted channel; M6 does not send e-mail itself.

The user establishes their password through:

```text
POST /api/v1/auth/activate
{
  "token": "<one-time token>",
  "password": "<12-128 character password>"
}
```

The token is single-use. Activation Argon2id-hashes the chosen password, enables credential login, records a security event and increments personnel revision.

ADMIN reset:

```text
POST /api/v1/personnel/{userId}/auth/reset
{
  "expectedRevision": <current personnel revision>
}
```

Starting a reset immediately closes the old-password login gate, clears failed-login lock state, revokes every active session for the target user, revokes prior unused one-time tokens, increments personnel revision and returns a new 24-hour `RESET` token. The user completes the reset through `/api/v1/auth/activate` with a new password.

The final login reset-gate recheck and session creation are atomic inside one PostgreSQL transaction that locks the credential row. If login wins the lock, a following reset revokes that new session; if reset wins, login sees reset-pending and cannot create a session. This closes the old-password reset/login race.

Authenticated users may change their own password:

```text
POST /api/v1/auth/change-password
{
  "currentPassword": "...",
  "newPassword": "..."
}
```

A successful self-change keeps the current session valid but revokes every other active session for that user. It also revokes unused activation/reset tokens. Security events are stored separately from CRM lead activities.

## Current `/api/v1` CRM surface

All endpoints require an authenticated session unless explicitly documented otherwise. Trusted actor identity is always derived from the server-side session and is never accepted from a request body.

```text
GET   /api/v1/personnel?includeInactive=false
POST  /api/v1/personnel
PATCH /api/v1/personnel/{userId}
PATCH /api/v1/personnel/{userId}/active

GET    /api/v1/leads
GET    /api/v1/leads/{contactId}
PUT    /api/v1/leads/{contactId}/assignment
PATCH  /api/v1/leads/{contactId}/status
POST   /api/v1/leads/{contactId}/notes
PATCH  /api/v1/leads/{contactId}/notes/{noteId}
DELETE /api/v1/leads/{contactId}/notes/{noteId}?expectedRevision={revision}
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
GET  /api/v1/imports/history?limit=20
```

### Authorization policy

- `ADMIN`: personnel administration, credential lifecycle, all current CRM operations/read models and manual imports.
- `MANAGER`: personnel read, CRM operations/read models across all leads and manual imports; cannot mutate personnel or administer credentials.
- `SALES`: CRM reads and edits only for leads currently assigned to their own user ID. SALES cannot manage personnel, reassign leads, administer credentials or run manual imports.

SALES scope is enforced inside PostgreSQL queries/mutations rather than only in UI filtering. Out-of-scope lead detail/mutations do not disclose another salesperson's lead.

### Optimistic concurrency and audit actor

Mutable personnel, assignment, lead-status and product-interest requests carry `expectedRevision`; notes and follow-ups use their own resource revision. Stale writes return HTTP `409` with `error.code = STALE_REVISION`.

Activities created by centralized CRM mutations use the authenticated session user as `actor_user_id`. Credential invitation/reset/change events are stored in `auth_security_events` with target and actor identities where applicable.

Current stable lead statuses are `NEW`, `CONTACTED`, `REPLIED`, `QUALIFIED`, `QUOTE_SENT`, `WON`, `LOST`, `INVALID`.

### Product interests

Automatic interests remain submission-derived. Manual product decisions remain append-only in `contact_product_interest_overrides`; the latest decision overrides the automatic result. No-op requests do not create redundant override/activity rows.

### Follow-ups

Follow-ups preserve the local `OPEN` → `COMPLETED` / `CANCELLED` lifecycle, RFC3339-to-UTC normalization, revision protection and authenticated audit events. SALES can mutate only follow-ups belonging to their currently assigned leads.

### Pipeline, dashboard and analytics

The PostgreSQL read models preserve the proven local semantics:

- pipeline active columns `NEW` through `QUOTE_SENT`, optional terminal `WON`/`LOST`/`INVALID`, bounded per-column cards, effective product interests, warnings/repeat/assignee/product/country/search and open follow-up `TODAY`/`OVERDUE` filters;
- dashboard KPI/attention groups for NEW leads, due-today/overdue follow-ups, recent repeats, open quality issues and a bounded submission summary window;
- analytics lower-inclusive/upper-exclusive submission windows, repeat-submission semantics, current-status funnel and country/platform/raw-product/campaign/form/adset/ad breakdowns.

For SALES callers all three read models are scoped server-side to leads currently assigned to that salesperson.

## Manual import API

Manual import accepts real source files; clients do not submit pre-normalized lead JSON. The server parses and validates the upload using the canonical import rules.

Supported input:

- `.csv` — UTF-8, BOM tolerated;
- `.xlsx` — scans worksheets/header rows using the same required-column rules as the local importer;
- maximum upload size: 20 MiB;
- multipart field name: `file`.

Required lead headers remain:

```text
id
created_time
full_name
email
phone_number
```

Preview is read-only:

```text
POST /api/v1/imports/preview
Content-Type: multipart/form-data
file=<CSV or XLSX>
```

Commit uses the same multipart contract:

```text
POST /api/v1/imports/commit
```

Important commit guarantees:

- the file is parsed and the identity plan is rebuilt at commit time; preview output is never trusted as commit authority;
- PostgreSQL transaction-level advisory locking serializes manual imports;
- identity conflicts or row errors roll back the whole commit;
- exact duplicate external submission IDs are skipped;
- importing the same file again is idempotent for submissions while still recording import-batch history;
- repeat submissions do not overwrite current CRM status;
- `Status` and `İletişime Geçme Tarihi` agency columns remain in immutable raw payload only;
- raw source payload, normalized identities, product interests and data-quality warnings are preserved;
- `LEAD_CREATED` and `SUBMISSION_IMPORTED` activities use the authenticated ADMIN/MANAGER actor;
- contact aggregate revision increments when an import updates contact-derived fields/submission count.

`GET /api/v1/imports/history` exposes recent committed batch summaries. SALES callers are rejected for preview, commit and history.

## Docker / Coolify

Build from repository root:

```bash
docker build -f server/Dockerfile -t ertip-lead-manager-server .
```

Recommended Coolify setup:

- Dockerfile: `server/Dockerfile`
- Build context: repository root
- Container port: `8080`
- Health/readiness path: `/health/ready`
- PostgreSQL: private/internal Coolify network only
- Public endpoint: API container via HTTPS reverse proxy

Do not expose PostgreSQL credentials to Tauri/Web clients.

## Current M6 boundary

Foundation, PostgreSQL schema, authentication/RBAC, personnel/assignment, lead CRM operations, notes, product overrides, follow-ups, pipeline/dashboard/analytics, manual import parity and the additional-user credential lifecycle are implemented. Foundation/auth/follow-up, pipeline/dashboard/analytics and manual import have passed real Coolify staging checkpoints. The additional-user credential lifecycle still requires its deliberate staging smoke test. Remaining M6 work after that includes PostgreSQL backup/restore evidence, SQLite schema-v4 → PostgreSQL migration/reconciliation and secure Tauri token storage before the M7 production API switch. The frozen local Tauri build remains independent throughout M6.
