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

PostgreSQL is never a public/client-facing service. Tauri and Web receive only HTTPS API session material.

## Technology and authentication decisions

- Backend: Rust + Axum + Tokio.
- Persistence: PostgreSQL + SQLx.
- Structured logging: `tracing`.
- API namespace: `/api/v1`.
- Container deployment: Coolify-compatible Dockerfile.
- Local fallback: frozen Tauri + SQLite schema v4.
- Web sessions: Secure + HttpOnly + SameSite=Lax cookie.
- Tauri sessions: opaque bearer token; only SHA-256 token hash persisted server-side.
- Passwords: Argon2id.
- Five failed password attempts trigger the database-backed temporary lock policy.
- Trusted `actor_user_id` always comes from the authenticated server session.

The first empty database can bootstrap one ADMIN from temporary runtime secrets. Real staging proved the bootstrap variables can be removed after the ADMIN is persisted without affecting later login.

## Additional-user credential lifecycle

Personnel remain stable CRM identities. Authentication credentials are enabled separately.

Implemented flow:

1. ADMIN issues a one-time `PROVISION` token for an active personnel record with an e-mail address.
2. Only a SHA-256 hash of that token is stored; the raw token is returned once and expires after 24 hours.
3. The user calls `POST /api/v1/auth/activate` and chooses their own 12–128 character password.
4. The server Argon2id-hashes the password, enables credential login and consumes the token.
5. ADMIN may later issue a `RESET` token using the current personnel revision.
6. Reset immediately blocks old-password login and revokes every active session for the target user.
7. The reset token is completed through the same activation endpoint with a new password.
8. Authenticated users may change their own password; the current session remains valid while every other active session is revoked.

Credential administration is ADMIN-only. MANAGER/SALES cannot issue invitation/reset tokens.

Login/reset concurrency is protected: after password verification, the reset-gate recheck and session creation occur in one PostgreSQL transaction while the credential row is locked. If login wins first, a later reset revokes that session; if reset wins first, login sees reset-pending and cannot create a session.

Credential lifecycle events are persisted separately in `auth_security_events`; no plaintext password or raw one-time token is stored there.

M6 intentionally does not implement an e-mail delivery provider. Invitation/reset token delivery is a future UI/integration concern; the server lifecycle itself is provider-independent.

## Authorization policy

Stable roles: `ADMIN`, `MANAGER`, `SALES`.

| Capability | ADMIN | MANAGER | SALES |
| --- | --- | --- | --- |
| Read personnel | Yes | Yes | No |
| Create/update/deactivate personnel | Yes | No | No |
| Provision/reset credentials | Yes | No | No |
| Read all leads | Yes | Yes | No |
| Read assigned own leads | Yes | Yes | Yes |
| Assign/unassign leads | Yes | Yes | No |
| Change lead status | Yes | Yes | Assigned own only |
| Notes/product interests/follow-ups | Yes | Yes | Assigned own only |
| Pipeline/dashboard/analytics | Global | Global | Assigned own only |
| Manual import | Yes | Yes | No |

Authorization is enforced server-side, inside service/query scope rather than only through UI visibility.

## Identity, audit and concurrency invariants

- `external_lead_id` remains unique submission identity.
- contact matching remains conservative; never merge only on name.
- source/raw submission values remain immutable/recoverable.
- stable application IDs survive migration.
- CRM mutations use authenticated audit actors.
- manual product overrides remain append-only.
- mutable centralized CRM resources use revision conflict protection.
- stale writes return HTTP `409` / `STALE_REVISION`.
- scoped writes use contact-row locking where assignment changes could race authorization.

## Implemented M6 API slices

### Foundation / PostgreSQL

- [x] standalone server crate;
- [x] typed configuration/logging/graceful shutdown;
- [x] `/health/live` and PostgreSQL-backed `/health/ready`;
- [x] canonical PostgreSQL migrations/constraints/indexes;
- [x] Coolify-ready non-root image/custom healthcheck;
- [x] PostgreSQL 17 CI gate while retaining frontend/frozen-Tauri gates.

### Auth / RBAC

- [x] Tauri/Web login;
- [x] logout/current session;
- [x] server-side expiry/revocation;
- [x] first-ADMIN bootstrap;
- [x] temporary failed-login lock;
- [x] ADMIN/MANAGER/SALES policy;
- [x] additional-user invitation/activation;
- [x] self password change + other-session revoke;
- [x] ADMIN reset + all-session revoke + old-password gate;
- [x] one-time hash-only 24h activation/reset tokens;
- [x] credential security-event audit;
- [x] atomic reset/login session gate;
- [ ] deliberate real-staging credential lifecycle smoke test;
- [ ] secure Tauri client token storage before M7 production switch.

### CRM / read models

- [x] personnel read/create/update/activation;
- [x] lead list/detail/status/assignment;
- [x] notes;
- [x] append-only product interests;
- [x] follow-ups;
- [x] pipeline;
- [x] dashboard attention/KPI;
- [x] analytics;
- [x] SALES assigned-only scope;
- [x] revision/lost-update protection for current mutable CRM resources.

### Manual import parity

- [x] server-side CSV/XLSX multipart parsing;
- [x] canonical normalization/product/identity planning;
- [x] read-only preview;
- [x] commit-time reparse/replan;
- [x] transaction-level advisory import lock;
- [x] whole-transaction block for identity conflicts/row errors;
- [x] exact-duplicate skip and repeat-upload idempotency;
- [x] raw agency fields preserved without overwriting CRM state;
- [x] authenticated import audit actor;
- [x] real Coolify staging preview → commit → history → all-duplicate reimport validation.

Current auth/import routes include:

```text
POST /api/v1/personnel/{userId}/auth/invitation
POST /api/v1/personnel/{userId}/auth/reset
POST /api/v1/auth/activate
POST /api/v1/auth/change-password

POST /api/v1/imports/preview
POST /api/v1/imports/commit
GET  /api/v1/imports/history
```

## Real staging checkpoint

PASS on `lead-api-staging.progesor.net`:

- container/PostgreSQL readiness;
- first ADMIN bootstrap and bootstrap-secret removal;
- HTTPS bearer login, `/me`, logout and revoked-token 401;
- follow-up lifecycle including stale 409;
- pipeline/dashboard/analytics read models;
- manual import preview/commit/history;
- exact same import re-submission producing zero new submissions and six exact duplicates while recording a second batch.

No passwords, raw bearer tokens or real customer data are recorded in repository evidence.

## CI checkpoint

Credential lifecycle code passes the PostgreSQL 17 server gate with **28/28 tests**, including invitation, activation, multi-session password change, ADMIN reset, session revocation, old-password rejection and reset activation. Existing manual-import, CRM, follow-up and read-model integration tests remain green in the same suite.

## Remaining M6 acceptance work

1. real staging credential lifecycle smoke validation;
2. PostgreSQL backup/restore operating runbook + evidence;
3. SQLite schema-v4 → PostgreSQL migration/reconciliation tooling and representative test;
4. secure Tauri token storage before the M7 production switch.

M6 remains open. PR #15 remains draft and must not be merged solely because individual staging slices pass.
