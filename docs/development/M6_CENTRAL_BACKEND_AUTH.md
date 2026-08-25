# M6 — Centralized Backend, PostgreSQL and Authentication

## Status

**IN PROGRESS**

- Issue: #14
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

Planned transport:

- Web: Secure + HttpOnly session cookie.
- Tauri: opaque bearer session token; secure OS-backed client storage is completed in M7 before production rollout.

Session records live server-side and support expiry/revocation. The API resolves the authenticated CRM user from the session and derives `actor_user_id` itself.

Password credentials, when enabled, use Argon2id hashes. Passwords and raw session tokens are never stored in logs or plaintext database columns.

## Authorization baseline

Roles already exist in the local schema and remain stable:

- `ADMIN`
- `MANAGER`
- `SALES`

Server-side authorization is mandatory even when the UI hides an action.

Initial policy direction:

- ADMIN: personnel/auth administration and all CRM operations.
- MANAGER: team CRM operations, assignment and reporting; no security-critical server administration.
- SALES: normal CRM work, primarily own/assigned lead workflows; exact visibility/edit policy is finalized with API endpoint implementation.

## Identity and audit invariants

These rules survive SQLite → PostgreSQL:

- `external_lead_id` remains unique submission identity.
- contact matching remains conservative; never merge only on name.
- immutable source submission/raw payload values remain recoverable.
- application UUID IDs are preserved during migration.
- personnel IDs remain stable.
- status, notes, product overrides, follow-ups and assignment mutations create auditable activity.
- authenticated audit actor comes from server session, never request JSON.

## Concurrency direction

Local SQLite had one interactive user. Centralized CRM state needs lost-update protection.

Mutable aggregate records will expose a revision/version or equivalent precondition. API mutations must reject stale writes with an explicit conflict response rather than silently overwriting another user's change.

Exact implementation is selected per resource during M6 API parity work.

## M6 slices

### M6.1 — Server foundation

- [ ] standalone `server/` Rust crate;
- [ ] typed environment configuration;
- [ ] `/health/live`;
- [ ] `/health/ready` with PostgreSQL check;
- [ ] structured logging;
- [ ] standard API error envelope;
- [ ] Coolify-ready Dockerfile;
- [ ] server CI gate while preserving existing Tauri CI.

### M6.2 — PostgreSQL canonical schema

- [ ] PostgreSQL migrations for canonical entities;
- [ ] constraints/indexes matching proven SQLite semantics;
- [ ] authentication/session tables;
- [ ] migration tests on an empty PostgreSQL database;
- [ ] backup/restore runbook.

### M6.3 — Authentication and authorization

- [ ] credential/bootstrap flow;
- [ ] login/logout/current-session;
- [ ] opaque session creation/expiry/revocation;
- [ ] cookie + bearer extraction;
- [ ] `app_users` binding;
- [ ] server-derived actor context;
- [ ] role-policy tests;
- [ ] login rate-limit baseline.

### M6.4 — CRM API parity

Move existing behavior behind versioned HTTP contracts in controlled groups:

1. personnel + assignments;
2. leads/list/detail/status;
3. notes + product interests;
4. follow-ups;
5. pipeline + Dashboard;
6. analytics;
7. manual import preview/commit.

Handlers remain thin. Business rules stay in application/domain services and persistence stays behind repositories.

### M6.5 — SQLite → PostgreSQL migration

- [ ] migration/export utility;
- [ ] preserve stable IDs and source/audit timestamps;
- [ ] copy immutable raw payloads exactly;
- [ ] copy personnel/assignment/audit history;
- [ ] reconciliation report;
- [ ] representative schema-v4 migration test.

## API conventions

Success/error responses use explicit DTOs; database rows are not API contracts.

Error shape direction:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "User-readable message",
    "requestId": "..."
  }
}
```

PII is minimized in logs. Server diagnostics prefer application IDs/request IDs.

## Environment contract — foundation

Initial server environment:

```text
ELM_BIND_ADDR=0.0.0.0:8080
DATABASE_URL=postgres://...
RUST_LOG=info,ertip_lead_manager_server=debug
```

Secrets remain Coolify runtime secrets and are never committed.

Auth-specific secrets/config are added only with M6.3.

## Deployment contract

Coolify should run at least:

- one private PostgreSQL resource;
- one API application built from `server/Dockerfile`;
- HTTPS reverse proxy/domain for API;
- no public PostgreSQL port;
- persistent PostgreSQL storage/backups.

The API container must expose a readiness endpoint suitable for Coolify health checks.

## M6.1 acceptance

- server crate compiles/tests in CI;
- `/health/live` returns 200 without requiring a successful DB round-trip;
- `/health/ready` reports database availability and fails non-200 when DB is unavailable;
- invalid environment configuration fails fast with a readable startup error;
- container image can be built from repository source;
- existing local Tauri frontend/Rust/package CI remains green.
