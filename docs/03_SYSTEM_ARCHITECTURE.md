# 03 — System Architecture

## 1. Architecture state

Ertip Lead Manager now has two deliberately separated architecture states:

1. **Frozen local fallback** — the accepted `v0.1.0-local` Tauri + SQLite application.
2. **Target centralized production architecture** — Tauri/Web clients using one authenticated HTTPS API backed by private PostgreSQL.

The local fallback is preserved; M6 does not mutate it into a half-online hybrid.

## 2. Frozen local architecture

```text
React / TypeScript
        │ typed Tauri commands
        ▼
Rust application/domain services
        │
        ├── SQLite / SQLx
        └── XLSX / CSV source adapters
```

This remains the fallback/development/migration-reference implementation and is frozen at schema version 4.

## 3. Centralized target architecture

```text
┌─────────────────────────┐        ┌─────────────────────────┐
│ Windows Tauri Client    │        │ Future Web Client       │
│ React + TypeScript      │        │ React + TypeScript      │
└────────────┬────────────┘        └────────────┬────────────┘
             │ HTTPS /api/v1                    │ HTTPS /api/v1
             └────────────────┬──────────────────┘
                              ▼
                 ┌────────────────────────────┐
                 │ Rust Axum API/Auth Server  │
                 │                            │
                 │ HTTP handlers              │
                 │ auth/session context       │
                 │ authorization policy       │
                 │ application/domain services│
                 │ repository interfaces      │
                 └──────────────┬─────────────┘
                                │ SQLx
                                ▼
                 ┌────────────────────────────┐
                 │ PostgreSQL                 │
                 │ Coolify private network    │
                 └────────────────────────────┘
```

Clients never receive PostgreSQL credentials.

## 4. Layer responsibilities

### Client presentation layer

Responsible for:

- presentation and interaction;
- UI state;
- query/filter controls;
- immediate client-side validation;
- session-aware navigation;
- invoking typed API contracts;
- rendering server-returned data and explicit conflict/auth/network states.

Not responsible for:

- deciding duplicate/repeat identity;
- enforcing authorization;
- writing SQL;
- manufacturing audit actor IDs;
- canonical analytics calculations;
- accepting stale writes silently.

### HTTP/API layer

Responsible for:

- route/version contract;
- request deserialization;
- authenticated session extraction;
- server-derived actor context;
- authorization checks;
- DTO/error mapping;
- request IDs / tracing context;
- delegating to application services.

Handlers remain thin and do not contain raw persistence/business logic.

### Application/domain layer

Responsible for:

- business rules;
- import validation/preview;
- timestamp/contact normalization;
- identity matching;
- product-interest parsing;
- status transitions and activity creation;
- personnel/assignment rules;
- follow-up operations;
- analytics semantics;
- concurrency/precondition rules;
- audit-event semantics.

Existing local Rust rules should be extracted/reused incrementally where practical instead of reimplemented independently in HTTP handlers.

### Persistence layer

M6 production repository adapters are backed by PostgreSQL/SQLx.

Responsibilities:

- schema migrations;
- transactional writes;
- indexed queries;
- unique/foreign-key/check constraints;
- optimistic concurrency persistence;
- persistence mapping;
- session/auth persistence.

SQLite adapters remain available only for the frozen local build, tests, migration tooling and explicit development scenarios.

## 5. Repository direction

Target structure evolves toward:

```text
/
├─ src/                         # existing Tauri/Web-compatible React UI
├─ src-tauri/                   # frozen/local Tauri host and local services
├─ server/                      # M6 Axum API process
│  ├─ Cargo.toml
│  ├─ Dockerfile
│  └─ src/
├─ crates/                      # introduced incrementally when reuse is proven
│  ├─ domain/                   # canonical pure rules/types
│  └─ api-contract/             # optional shared DTO contract
├─ docs/
└─ fixtures/
```

Do not perform a large speculative extraction before server behavior exists. Extract shared domain code in controlled slices with parity tests.

## 6. API boundary

Production API namespace begins at:

```text
/api/v1
```

Conceptual route groups:

```text
/health/live
/health/ready
/api/v1/auth/*
/api/v1/me
/api/v1/staff/*
/api/v1/leads/*
/api/v1/follow-ups/*
/api/v1/pipeline
/api/v1/dashboard
/api/v1/analytics/*
/api/v1/imports/*
```

Database row shapes are not public API contracts. DTOs are explicit and versioned.

Standard error direction:

```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "...",
    "requestId": "..."
  }
}
```

## 7. Authentication/session boundary

M6 uses server-side sessions with opaque session secrets.

- Web: Secure + HttpOnly cookie.
- Tauri: opaque bearer session token; secure OS-backed storage is completed before M7 production release.
- Session storage/expiry/revocation is authoritative server-side.
- `app_users.id` remains stable CRM identity.
- Authentication identity binds to the CRM user rather than replacing it.
- `actor_user_id` comes from authenticated server context only.
- Request JSON must never supply the trusted audit actor.

Passwords, raw session tokens and secrets are excluded from logs.

## 8. Authorization boundary

Stable roles:

- `ADMIN`
- `MANAGER`
- `SALES`

Authorization is a server concern. React may hide unavailable controls for UX but cannot be the security boundary.

Policy tests must cover endpoint/service actions as role rules are introduced.

## 9. PostgreSQL conventions

- Stable application IDs remain UUID text/UUID-compatible identifiers and survive SQLite migration.
- External Meta IDs remain exact source identifiers.
- Canonical timestamps use timezone-aware PostgreSQL timestamp semantics or an explicitly documented equivalent while raw source strings remain preserved.
- Unique constraints preserve exact submission identity.
- Foreign keys/check constraints enforce canonical relationships/status/role values where appropriate.
- Indexes follow real lead/pipeline/follow-up/analytics query paths.
- PostgreSQL behavior is performance-tested independently; SQLite query assumptions are not blindly copied.
- PostgreSQL runs on Coolify private/internal networking and is not internet-exposed.

## 10. Manual file import architecture

Manual XLSX/CSV import remains supported after centralization.

Canonical source flow remains:

```text
.xlsx ──> XlsxFileSource ─┐
                          ├─> RawSubmissionRow[] ─> canonical import pipeline
.csv  ──> CsvFileSource ──┘
```

The verified Meta product multi-select rule remains unchanged: structured values split only on `|`, never on comma.

Centralized import must run through authenticated API/application services and the authoritative PostgreSQL transaction. It must not allow a client to bypass identity/deduplication/source-preservation rules.

## 11. Import phases

### Preview

1. Parse supported file/source format.
2. Locate/map headers.
3. Preserve raw source values.
4. Normalize timestamp/e-mail/phone/country/product interests.
5. Check external submission identity.
6. Check conservative contact candidates/conflicts.
7. Return a preview without committing canonical business records.

### Commit

1. Authenticate/authorize the actor.
2. Start PostgreSQL transaction.
3. Revalidate uniqueness/identity assumptions.
4. Insert only new submissions.
5. Create/link contacts conservatively.
6. Persist normalized product-interest memberships.
7. Persist import/data-quality/activity metadata with server-derived actor context where applicable.
8. Commit atomically.

## 12. Concurrency boundary

Centralized mutable CRM state must protect against lost updates.

Approach direction:

- mutable aggregate DTOs expose revision/version or equivalent precondition metadata;
- update commands include the client's observed revision when required;
- stale updates return a typed conflict (HTTP 409 direction);
- UI surfaces the conflict and reload/retry choice;
- immutable source/audit rows are append-only and do not use overwrite semantics.

## 13. Health and operations

Server exposes:

- `/health/live`: process is running; no dependency round-trip required.
- `/health/ready`: critical dependencies, initially PostgreSQL, are available.

Coolify readiness should use `/health/ready`.

Production logs use structured tracing and request IDs while minimizing PII.

## 14. SQLite → PostgreSQL migration boundary

Migration is a data move, not a semantic redesign.

Must preserve:

- contact/submission IDs;
- external lead IDs;
- raw source payloads;
- normalized product memberships;
- notes/follow-ups;
- personnel and assignments;
- activity/audit chronology;
- timestamps/source raw strings.

Migration tooling produces reconciliation counts/key checks before centralized production acceptance.

## 15. Future integration seams

New lead/sales integrations must terminate at the centralized backend/application layer:

```text
LeadSourceAdapter
├─ XlsxFileSource
├─ CsvFileSource
├─ MetaLeadApiSource       (future)
└─ GoogleSheetsSource      (future if still useful)

SalesDataAdapter
├─ Local/Manual metadata
└─ OdooSalesSource         (future)
```

No future adapter may bypass canonical identity/source/audit/authorization rules.
