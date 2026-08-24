# M5.5 — Team Assignment and Multi-user Readiness

## Goal

Add personnel ownership to the current local CRM without creating a dead-end schema, then use the same domain identities when the product moves to a centralized authenticated release.

## Immediate local scope

- personnel records (`app_users`) with stable UUID IDs;
- active/inactive lifecycle, no hard delete;
- optional personnel e-mail and role (`ADMIN`, `MANAGER`, `SALES`);
- one current assignee per lead contact;
- Lead Detail assignment control;
- Kanban assignee display and assignee / unassigned filters;
- Lead List current-assignee display and assignee / unassigned filters;
- assignment changes recorded as immutable `ASSIGNEE_CHANGED` activity events;
- nullable `lead_activities.actor_user_id` groundwork for authenticated audit attribution.

Imported submission/source data remains immutable. Assignment is CRM state only.

## Audit semantics

Local single-user development mode has no authenticated actor, therefore `actor_user_id` is intentionally `NULL` for local actions.

When authentication is introduced, `actor_user_id` MUST be derived from the authenticated server-side session. A client-supplied actor/user ID must never be trusted as audit identity.

Assignment activity payload stores both stable IDs and display-name snapshots for the previous/new assignee. This keeps historical activity readable even if a person's current name changes or the person is later deactivated.

## Personnel lifecycle

Personnel records are not hard-deleted in normal application flows.

- Active users can receive new lead assignments.
- Inactive users remain visible on historical/current assignments.
- An inactive user cannot receive a new assignment.
- A lead assigned to an inactive user may be reassigned to an active user or returned to Unassigned.

## Planned centralized architecture

The Windows application must **not** connect directly to an internet-exposed PostgreSQL database.

Target Coolify deployment:

```text
Tauri Windows Client ─┐
                     ├── HTTPS API / Auth Service ── private PostgreSQL
Future Web Client ───┘
```

Recommended deployment properties:

- PostgreSQL on Coolify private/internal network;
- backend API is the only component allowed to perform application DB operations;
- TLS/reverse proxy at the Coolify edge;
- authentication/session validation in the backend;
- permissions/authorization enforced by the backend, not UI hiding;
- Tauri becomes an API client for online mode;
- future Web App uses the exact same API contract;
- `app_users.id` remains the stable CRM user identity;
- `app_users.auth_subject` binds the CRM user to the chosen authentication identity/provider;
- audit actor is resolved by the backend session.

## SQLite → PostgreSQL migration principle

Current SQLite remains useful for development and migration validation. The centralized release should migrate data, not redefine domain semantics.

Stable IDs for contacts, submissions, notes, activities, follow-ups and app users should be retained when copying to PostgreSQL. Source/raw payloads and activity history must remain intact.

## Suggested next architecture milestone after M5/M5.5

Before an internal multi-user release:

1. extract/reuse current Rust service/domain rules behind an HTTP API;
2. introduce PostgreSQL persistence adapter/migrations;
3. add authentication and server-derived current-user context;
4. add authorization roles;
5. migrate local SQLite data into PostgreSQL with reconciliation report;
6. switch Tauri persistence calls from local commands to API client calls;
7. validate concurrent edits and audit attribution;
8. then build the Web App against the same API.

## Validation strategy

M5 is merged. M5.5 is based directly on `main` and validated through the repository pull-request workflow. CI covers frontend lint/tests/build, Windows Rust tests/migrations and a debug NSIS package.

## Acceptance criteria — local M5.5

- [x] staff can be created and edited from Settings;
- [x] staff can be deactivated/reactivated without deleting history;
- [x] lead can be assigned/unassigned from Lead Detail;
- [x] assignment changes create immutable activity records;
- [x] Kanban cards show current assignee;
- [x] Kanban filters by active/inactive assignee and Unassigned;
- [x] Lead List shows current assignee;
- [x] Lead List filters by active/inactive assignee and Unassigned;
- [x] Pipeline filter state survives Lead Detail round-trip;
- [x] current inactive assignee remains readable but cannot receive new assignments;
- [x] activity response can expose future actor identity without breaking local NULL actor mode;
- [x] Light/Dark UI is readable;
- [ ] final latest-head Windows Rust + frontend + NSIS gate passes.
