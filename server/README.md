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
```

`/health/live` proves the process/router is alive. `/health/ready` performs a PostgreSQL `SELECT 1` and returns a non-200 response when the database is not available.

Tauri login returns an opaque session token that is sent as `Authorization: Bearer <token>`. Only its SHA-256 hash is stored server-side. Web login uses the same credentials but returns the session through a `Secure; HttpOnly; SameSite=Lax` cookie. Passwords are stored as Argon2id hashes and repeated failed logins trigger the database-backed temporary lock policy.

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

- `ADMIN`: personnel administration, all current CRM operations/read models and manual imports.
- `MANAGER`: personnel read, CRM operations/read models across all leads and manual imports; cannot create/update/deactivate personnel.
- `SALES`: CRM reads and edits only for leads currently assigned to their own user ID. SALES cannot manage personnel, reassign leads or run manual imports.

SALES scope is enforced inside PostgreSQL queries/mutations rather than only in UI filtering. Out-of-scope lead detail/mutations do not disclose another salesperson's lead.

### Optimistic concurrency and audit actor

Mutable personnel, assignment, lead-status and product-interest requests carry `expectedRevision`; notes and follow-ups use their own resource revision. Stale writes return HTTP `409` with `error.code = STALE_REVISION`.

Activities created by centralized mutations use the authenticated session user as `actor_user_id`. Scoped writes hold the contact row as needed so assignment changes cannot race between authorization and mutation.

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

Manual import parity accepts real source files; clients do not submit pre-normalized lead JSON. The server parses and validates the uploaded source using the canonical import rules.

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

Preview example:

```text
POST /api/v1/imports/preview
Content-Type: multipart/form-data
file=<CSV or XLSX>
```

Preview is read-only. It returns row decisions and totals for new contacts, repeat submissions, exact duplicate submissions, identity conflicts, row errors and normalization warnings.

Commit uses the same multipart contract:

```text
POST /api/v1/imports/commit
```

Important commit guarantees:

- the file is parsed and the identity plan is rebuilt at commit time; preview output is never trusted as commit authority;
- PostgreSQL transaction-level advisory locking serializes manual imports so concurrent users cannot race the same identity/duplicate snapshot;
- identity conflicts or row errors block and roll back the whole commit;
- exact duplicate external submission IDs are skipped;
- importing the same file again is idempotent for submissions while still recording import-batch history;
- repeat submissions do not overwrite the lead's current CRM status;
- `Status` and `İletişime Geçme Tarihi` agency columns remain in immutable raw payload only and are not treated as CRM status/contact-date inputs;
- raw source payload, normalized identities, product interests and data-quality warnings are preserved in PostgreSQL;
- `LEAD_CREATED` and `SUBMISSION_IMPORTED` activities use the authenticated ADMIN/MANAGER actor;
- contact aggregate revision increments when an import updates contact-derived fields/submission count.

`GET /api/v1/imports/history` exposes recent committed batch summaries. SALES callers are rejected for preview, commit and history.

## Personnel authentication state

Creating a personnel record creates the stable `app_users` CRM identity but does not yet automatically issue a password. Personnel responses expose `authEnabled` so ADMIN can distinguish CRM identity from credential-enabled login identity. Additional-user credential provisioning/invitation/reset remains a later M6 authentication slice.

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

Foundation, PostgreSQL schema, authentication/RBAC, personnel/assignment, lead CRM operations, notes, product overrides, follow-ups, pipeline/dashboard/analytics and manual import server parity are implemented. Foundation/auth/follow-up and pipeline/dashboard/analytics have passed real Coolify staging checkpoints; manual import still requires its deliberate staging smoke test. Remaining M6 work includes additional-user credential provisioning, PostgreSQL backup/restore evidence, SQLite schema-v4 → PostgreSQL migration/reconciliation and secure Tauri token storage before the M7 production API switch. The frozen local Tauri build remains independent throughout M6.
