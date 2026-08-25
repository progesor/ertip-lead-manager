# M6 — Centralized Backend, PostgreSQL and Authentication

## Status

**IN PROGRESS**

- Issue: #14
- PR: #15 (draft)
- Branch: `feat/m6-central-backend-foundation`
- Frozen fallback: `release/local-v0.1.0` / `v0.1.0-local`

## Goal

Create the authoritative multi-user backend for Ertip Lead Manager without rewriting or weakening the CRM identity/source/audit rules proven by the local application.

## Target topology

```text
Windows Tauri ─┐
               ├── HTTPS /api/v1 ── Axum API/Auth ── PostgreSQL
Future Web ────┘                         │
                                        └── Coolify private network
```

PostgreSQL is never a public/client-facing service. Tauri and Web receive only HTTPS API credentials/session material.

## Technology decisions

- Backend runtime: Rust.
- HTTP framework: Axum.
- Persistence: PostgreSQL through SQLx.
- Async runtime: Tokio.
- Structured logging: `tracing` / `tracing-subscriber`.
- API namespace: `/api/v1`.
- Container deployment: Dockerfile compatible with Coolify.
- Local fallback remains Tauri + SQLite schema v4 and is frozen independently.

Using Rust on both sides is intentional. Existing local service/domain rules can be extracted toward shared crates incrementally instead of being translated into a second language and drifting.

## Authentication decision

Use **server-side sessions** with opaque random session secrets rather than treating client-provided user IDs as identity.

Implemented transport:

- Web: Secure + HttpOnly + SameSite=Lax session cookie.
- Tauri: opaque bearer session token; secure OS-backed client storage is completed in M7 before production rollout.

Session records live server-side and support expiry/revocation. Only the SHA-256 hash of the raw session token is persisted. The API resolves the authenticated CRM user from the session and derives `actor_user_id` itself.

Password credentials use Argon2id hashes. Five failed password attempts trigger a temporary database-backed account lock. An empty deployment can create one initial ADMIN from Coolify/runtime bootstrap secrets; the bootstrap password should be removed after that account exists.

Additional personnel created through the current CRM API are stable CRM identities but do not yet receive credentials automatically. API personnel DTOs expose `authEnabled`; additional-user credential provisioning/invitation remains an M6 task.

## Authorization policy — implemented baseline

Stable roles:

- `ADMIN`
- `MANAGER`
- `SALES`

Current server policy:

| Capability | ADMIN | MANAGER | SALES |
| --- | --- | --- | --- |
| Read personnel | Yes | Yes | No |
| Create/update/deactivate personnel | Yes | No | No |
| Read all leads | Yes | Yes | No |
| Read assigned own leads | Yes | Yes | Yes |
| Assign/unassign leads | Yes | Yes | No |
| Change lead status | Yes | Yes | Assigned own leads only |

`SALES` lead visibility is enforced by PostgreSQL query scope, not only by frontend filtering. Attempting to request unassigned leads or another person's assignee filter is forbidden. Detail/status access outside the salesperson's assignment scope does not disclose another lead.

The current ADMIN is prevented from demoting or deactivating its own account through the personnel API.

## Identity and audit invariants

These rules survive SQLite → PostgreSQL:

- `external_lead_id` remains unique submission identity.
- contact matching remains conservative; never merge only on name.
- immutable source submission/raw payload values remain recoverable.
- application UUID/stable IDs are preserved during migration.
- personnel IDs remain stable.
- status, notes, product overrides, follow-ups and assignment mutations create auditable activity.
- authenticated audit actor comes from server session, never request JSON.

The implemented assignment and lead-status endpoints already persist `ASSIGNEE_CHANGED` / `STATUS_CHANGED` activities with the authenticated actor ID.

## Concurrency implementation

Centralized mutable CRM state uses persisted `revision` values.

Current personnel update/deactivation, lead assignment and lead-status mutation contracts carry `expectedRevision`. Writes are checked against the current persisted revision and successful changes increment it. A stale request receives HTTP `409` with `STALE_REVISION` instead of overwriting a newer update.

This pattern must be applied to the remaining mutable CRM resources as their API slices are implemented.

## M6 slices

### M6.1 — Server foundation

- [x] standalone `server/` Rust crate;
- [x] typed environment configuration;
- [x] `/health/live`;
- [x] `/health/ready` with PostgreSQL check;
- [x] structured logging;
- [x] standard JSON API error envelope baseline;
- [x] Coolify-ready Dockerfile;
- [x] server CI gate while preserving existing Tauri CI.

### M6.2 — PostgreSQL canonical schema

- [x] PostgreSQL migrations for canonical entities;
- [x] constraints/indexes matching proven SQLite semantics;
- [x] authentication/session tables;
- [x] migration tests on an empty real PostgreSQL 17 database;
- [x] DB readiness check;
- [ ] complete PostgreSQL adapters/API parity for every CRM domain;
- [ ] backup/restore runbook.

### M6.3 — Authentication and authorization

- [x] one-time first-ADMIN credential/bootstrap flow;
- [x] login/logout/current-session;
- [x] opaque session creation/expiry/revocation;
- [x] cookie + bearer extraction;
- [x] `app_users` authenticated identity binding;
- [x] server-derived actor context;
- [x] ADMIN / MANAGER / SALES role-policy tests;
- [x] login abuse baseline through database-backed temporary account lock;
- [ ] additional-user credential provisioning/invitation/reset flow;
- [ ] secure Tauri client token storage (M7 production switch gate).

### M6.4 — CRM API parity

Implemented first slice:

- [x] personnel read/create/update/activation endpoints;
- [x] lead assignment/unassignment endpoint;
- [x] lead list with search/status/country/product/assignee/repeat/warning filters and paging;
- [x] lead detail with source submissions, effective products, quality issues, notes and activity history;
- [x] lead status endpoint;
- [x] server-derived audit actor for assignment/status;
- [x] optimistic revision conflict handling for the implemented mutable resources;
- [x] SALES assigned-only query/detail/status scope;
- [ ] note create/update/delete API;
- [ ] product-interest override API;
- [ ] follow-up API;
- [ ] pipeline + Dashboard API;
- [ ] analytics API;
- [ ] manual import preview/commit API;
- [ ] extend revision/lost-update protection to every remaining mutable CRM resource.

Current HTTP routes:

```text
GET   /api/v1/personnel
POST  /api/v1/personnel
PATCH /api/v1/personnel/{userId}
PATCH /api/v1/personnel/{userId}/active

GET   /api/v1/leads
GET   /api/v1/leads/{contactId}
PUT   /api/v1/leads/{contactId}/assignment
PATCH /api/v1/leads/{contactId}/status
```

Handlers remain thin: they resolve the authenticated actor and delegate authorization/business/persistence behavior to server service code. Database rows are not treated as external API contracts.

### M6.5 — SQLite → PostgreSQL migration

- [ ] migration/export utility;
- [ ] preserve stable IDs and source/audit timestamps;
- [ ] copy immutable raw payloads exactly;
- [ ] copy personnel/assignment/audit history;
- [ ] reconciliation report;
- [ ] representative schema-v4 migration test.

## API conventions

Success/error responses use explicit DTOs. Current errors use a stable code/message object:

```json
{
  "error": {
    "code": "STALE_REVISION",
    "message": "..."
  }
}
```

Request IDs are already generated and propagated through the HTTP layer; including the request ID inside the JSON error body remains a later API-envelope improvement.

PII is minimized in logs. Server diagnostics prefer application IDs/request IDs.

## Deployment contract

Coolify should run at least:

- one private PostgreSQL resource;
- one API application built from `server/Dockerfile`;
- HTTPS reverse proxy/domain for API;
- no public PostgreSQL port;
- persistent PostgreSQL storage/backups.

The API container exposes `/health/ready` for dependency-aware readiness.

## Current validation checkpoint

The following have passed the GitHub CI PostgreSQL 17 lane during M6 development:

- canonical migrations against an empty PostgreSQL database;
- Argon2/session bootstrap-login-resolve-logout integration;
- RBAC policy unit tests;
- PostgreSQL CRM integration covering SALES scope, MANAGER assignment, authenticated status audit and stale revision rejection;
- authenticated HTTP route compilation/tests;
- existing frontend checks and frozen local Tauri Rust tests remain separate gates.

M6 remains open. The next controlled API slice is notes + product-interest overrides, followed by follow-ups and then pipeline/dashboard/analytics/import parity before migration tooling and production deployment closure.
