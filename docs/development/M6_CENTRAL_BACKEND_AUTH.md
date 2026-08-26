# M6 — Centralized Backend, PostgreSQL and Authentication

## Status

**IN PROGRESS**

- Issue: #14
- PR: #15 (draft)
- Branch: `feat/m6-central-backend-foundation`
- Frozen fallback: `release/local-v0.1.0` / `v0.1.0-local`

## Goal

Create the authoritative multi-user backend for Ertip Lead Manager without weakening the CRM identity/source/audit rules proven by the frozen local application.

## Target topology

```text
Windows Tauri ─┐
               ├── HTTPS /api/v1 ── Axum API/Auth ── PostgreSQL
Future Web ────┘                         │
                                        └── Coolify private network
```

PostgreSQL is never client-facing. Tauri and Web receive only HTTPS API/session material.

## Technology and authentication decisions

- Rust + Axum backend, Tokio runtime;
- PostgreSQL through SQLx;
- structured logging through `tracing`;
- `/api/v1` shared API namespace;
- Coolify/Docker deployment;
- local Tauri + SQLite schema-v4 fallback remains frozen independently;
- server-side opaque sessions rather than client-supplied actor IDs;
- Web transport: Secure + HttpOnly + SameSite=Lax cookie;
- Tauri transport: opaque bearer session, with secure OS-backed storage required before M7 production rollout;
- Argon2id credentials;
- only SHA-256 session-token hashes persisted;
- five failed password attempts trigger temporary DB-backed lock;
- one-time first-ADMIN bootstrap from runtime secrets.

Additional personnel created through CRM API are stable identities but credential provisioning/invitation/reset for additional users remains an M6 task.

## Authorization policy

| Capability | ADMIN | MANAGER | SALES |
| --- | --- | --- | --- |
| Read personnel | Yes | Yes | No |
| Mutate personnel | Yes | No | No |
| Read all leads | Yes | Yes | No |
| Read assigned own leads | Yes | Yes | Yes |
| Assign/unassign leads | Yes | Yes | No |
| Status/notes/products/follow-ups | Yes | Yes | Assigned own leads only |
| Pipeline/dashboard/analytics | All | All | Assigned own leads only |
| Manual import | Yes | Yes | No |

SALES scope is enforced inside PostgreSQL-backed services/queries rather than only in UI. Out-of-scope detail/mutations do not disclose another salesperson's lead.

## Identity / source / audit invariants

- `external_lead_id` remains unique submission identity;
- contact matching remains conservative and never merges on name alone;
- immutable source/raw values remain recoverable;
- stable application/personnel IDs remain migration targets;
- agency `Status` and `İletişime Geçme Tarihi` remain raw import fields rather than CRM inputs;
- manual product decisions remain append-only over automatic submission-derived interests;
- centralized activities derive `actor_user_id` from authenticated session;
- status, assignment, notes, products and follow-ups remain auditable.

## Concurrency implementation

Centralized mutable CRM state uses persisted revisions and explicit locking:

- personnel/assignment/status/product aggregate requests use `expectedRevision`;
- notes and follow-ups use resource-level revisions;
- stale writes return HTTP `409` / `STALE_REVISION`;
- scoped note/follow-up writes lock the lead while SALES assignment authorization is checked;
- assignment/product aggregate writes use update locking;
- manual import uses a PostgreSQL transaction advisory lock so concurrent import commits cannot plan against the same identity snapshot.

## M6 slices

### M6.1 — Server foundation

- [x] standalone `server/` Rust crate;
- [x] typed runtime configuration;
- [x] `/health/live`;
- [x] PostgreSQL-backed `/health/ready`;
- [x] structured logging and graceful shutdown;
- [x] JSON API error baseline;
- [x] Coolify-ready Dockerfile;
- [x] PostgreSQL 17 CI while preserving frozen local/frontend gates;
- [x] real Coolify staging deployment.

### M6.2 — PostgreSQL canonical schema

- [x] canonical migrations and integrity constraints;
- [x] authentication/session schema;
- [x] migration tests against real PostgreSQL 17;
- [x] DB readiness check;
- [x] PostgreSQL service/API parity for current CRM, follow-up, read-model and manual-import domains;
- [ ] backup/restore runbook + evidence.

### M6.3 — Authentication / authorization

- [x] first-ADMIN bootstrap;
- [x] login/logout/current-session;
- [x] opaque session expiry/revocation;
- [x] cookie + bearer extraction;
- [x] `app_users` identity binding;
- [x] server-derived actor context;
- [x] ADMIN/MANAGER/SALES policy tests;
- [x] DB-backed temporary account lock;
- [x] foundation/auth staging validation and bootstrap-secret removal;
- [ ] additional-user credential provisioning/invitation/reset;
- [ ] secure Tauri token storage before M7.

### M6.4 — CRM / read-model API parity

- [x] personnel read/create/update/activation;
- [x] lead assignment/unassignment;
- [x] lead list/detail/status;
- [x] notes create/update/delete;
- [x] append-only product overrides;
- [x] follow-up list/create/reschedule/complete/cancel;
- [x] pipeline read model;
- [x] dashboard attention/KPI read model;
- [x] analytics read model;
- [x] SALES assigned-only scope;
- [x] authenticated mutation audit actor;
- [x] optimistic revision conflict handling;
- [x] follow-up staging smoke validation;
- [x] pipeline/dashboard/analytics staging smoke validation.

Current route families include:

```text
/api/v1/personnel
/api/v1/leads
/api/v1/leads/{contactId}/notes
/api/v1/leads/{contactId}/product-interests/{productCode}
/api/v1/leads/{contactId}/follow-ups
/api/v1/pipeline
/api/v1/dashboard/attention
/api/v1/analytics
```

### M6.5 — Manual import parity

- [x] `POST /api/v1/imports/preview`;
- [x] `POST /api/v1/imports/commit`;
- [x] `GET /api/v1/imports/history`;
- [x] actual multipart `.csv` / `.xlsx` input, 20 MiB max;
- [x] canonical required-header / normalization / product / identity rules ported server-side;
- [x] preview is read-only;
- [x] commit reparses/replans against current DB;
- [x] transaction advisory lock serializes concurrent imports;
- [x] conflict/error rows block whole transaction;
- [x] exact duplicate submission skipping and repeat-upload idempotency;
- [x] repeat submission does not overwrite CRM status;
- [x] agency fields preserved only in raw payload;
- [x] authenticated ADMIN/MANAGER import activities;
- [x] SALES import rejected;
- [x] PostgreSQL integration test covering preview → commit → duplicate reimport → history;
- [ ] real Coolify staging import smoke test.

### M6.6 — SQLite → PostgreSQL migration

- [ ] migration/export utility;
- [ ] preserve stable IDs and source/audit timestamps;
- [ ] copy immutable raw payloads exactly;
- [ ] copy personnel/assignment/audit history;
- [ ] reconciliation report;
- [ ] representative schema-v4 migration test.

## API conventions

Error responses use stable code/message objects, for example:

```json
{
  "error": {
    "code": "STALE_REVISION",
    "message": "..."
  }
}
```

Request IDs are generated/propagated by the HTTP layer. PII is minimized in logs and diagnostics prefer application/request IDs.

## Real staging status

Recorded in `docs/development/M6_STAGING_VALIDATION.md`.

**PASS:**

- health/readiness and API→PostgreSQL connectivity;
- first ADMIN bootstrap/login/me/logout/revoked-token behavior;
- persisted ADMIN after bootstrap secrets removed;
- follow-up create/list/reschedule/stale-409/complete;
- pipeline 8-column board and synthetic NEW lead;
- analytics clean zero-submission result/all status buckets;
- dashboard KPI/new-uncontacted synthetic lead result.

**Next staging gate:** manual import preview → commit → history → duplicate reimport using synthetic staging-only CSV data.

## Current CI checkpoint

Manual-import code head `3b7f234c2399f7553a535288b9be7da946627473` passed **27/27** PostgreSQL 17 server tests. This includes all prior auth/RBAC/CRM/follow-up/read-model gates plus canonical import-domain tests and the PostgreSQL manual-import integration test. Frontend and Windows Rust/local checks also remained green at that checkpoint.

## Remaining M6 work

1. manual import real staging smoke validation;
2. additional-user credential provisioning/invitation/reset;
3. PostgreSQL backup/restore operating evidence;
4. SQLite schema-v4 → PostgreSQL migration/reconciliation;
5. secure Tauri token storage before M7.

M6 remains open and PR #15 remains draft. The production Tauri switch is M7; Web UI is M8.
