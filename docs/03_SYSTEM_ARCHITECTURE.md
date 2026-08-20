# 03 — System Architecture

## 1. Architecture style

A local desktop application with a thin React presentation layer and a Rust domain/application backend inside Tauri.

```text
┌──────────────────────────────────────────────┐
│               React / TypeScript             │
│                                              │
│ Dashboard | Leads | Pipeline | Analytics     │
│ Imports   | Settings | Detail Workspace      │
└──────────────────────┬───────────────────────┘
                       │ typed Tauri commands/events
┌──────────────────────▼───────────────────────┐
│                  Rust backend                │
│                                              │
│ Import Service                               │
│ Lead Service                                 │
│ Follow-up Service                            │
│ Analytics Service                            │
│ Backup Service                               │
│ Domain normalization / identity rules        │
└───────────────┬─────────────────┬────────────┘
                │                 │
         ┌──────▼──────┐   ┌──────▼───────────┐
         │   SQLite    │   │ .xlsx / Calamine │
         │    SQLx     │   │ read-only import │
         └─────────────┘   └──────────────────┘
```

## 2. Layer responsibilities

### Frontend

Responsible for:

- presentation and interaction;
- UI state;
- query/filter controls;
- client-side form validation for immediate feedback;
- invoking typed backend commands;
- rendering data returned from backend.

Not responsible for:

- deciding duplicate/repeat identity;
- writing raw SQL;
- parsing Excel;
- enforcing transactional import rules;
- canonical analytics calculations.

### Rust application/domain layer

Responsible for:

- business rules;
- import validation and preview;
- timestamp/contact normalization;
- identity matching;
- status transitions and activity creation;
- follow-up operations;
- analytics query composition;
- backup/restore orchestration.

### Persistence layer

Repository abstractions backed by SQLite/SQLx.

Responsibilities:

- schema migrations;
- transactional writes;
- indexed queries;
- unique/foreign-key constraints;
- persistence mapping.

## 3. Proposed repository structure

```text
/
├─ README.md
├─ AGENTS.md
├─ docs/
├─ fixtures/
├─ package.json
├─ src/
│  ├─ app/
│  ├─ components/
│  ├─ features/
│  │  ├─ dashboard/
│  │  ├─ imports/
│  │  ├─ leads/
│  │  ├─ pipeline/
│  │  ├─ analytics/
│  │  └─ settings/
│  ├─ lib/
│  ├─ routes/
│  └─ types/
└─ src-tauri/
   ├─ Cargo.toml
   ├─ migrations/
   └─ src/
      ├─ commands/
      ├─ domain/
      ├─ services/
      ├─ repositories/
      ├─ import/
      ├─ db/
      └─ lib.rs
```

Exact folder names may evolve, but business logic must not collapse into command handlers or React components.

## 4. Command boundary

Frontend communicates with Rust through explicit commands, e.g. conceptual API:

- `preview_import(path)`
- `commit_import(preview_token/options)`
- `list_imports(query)`
- `search_leads(filter, pagination)`
- `get_lead_detail(lead_id)`
- `update_lead_status(lead_id, status)`
- `add_note(lead_id, text)`
- `set_follow_up(lead_id, due_at)`
- `get_dashboard_metrics(range)`
- `get_analytics_breakdown(range, dimension)`
- `create_backup(destination)`
- `restore_backup(path)`

Use generated/shared DTO types where practical; do not expose database row shapes directly to the UI.

## 5. Excel import architecture

Import is split into two phases.

### Phase A — Preview

1. Open file read-only.
2. Locate supported sheet/header row.
3. Map headers.
4. Parse rows into `RawSubmissionRow`.
5. Validate required fields.
6. Normalize timestamp/e-mail/phone/country and derive zero-or-more product-interest candidates according to the detected form schema version.
7. Check external ID against DB.
8. Check contact identity candidates.
9. Produce per-row outcome and aggregate summary.
10. Return preview without writing business records.

### Phase B — Commit

1. Begin SQLite transaction.
2. Create import batch record.
3. Revalidate assumptions that matter for uniqueness.
4. Insert only new submissions.
5. Create/link contacts using conservative identity rules.
6. Seed default application state for newly created contacts.
7. Create import/activity metadata.
8. Commit transaction.
9. Return committed counts.

If any integrity error invalidates the batch, roll back.

## 6. Database conventions

- UUID/ULID local identifiers are application IDs.
- External Meta IDs are stored as text and remain unchanged.
- Store UTC timestamps in ISO text or integer epoch consistently; choose one during M1 and document it.
- Enable SQLite foreign keys.
- Use WAL mode if verified safe with the backup strategy.
- Add indexes based on actual query paths, especially normalized e-mail/phone, latest submission date, status, follow-up due date, campaign/form/platform dimensions.

## 7. Error model

Backend errors should be typed into categories:

- `ValidationError`
- `ImportSchemaError`
- `ImportRowError`
- `IdentityConflict`
- `NotFound`
- `DatabaseError`
- `BackupError`
- `UnexpectedError`

User-facing errors must be clear and non-technical while logs retain enough technical detail for debugging without leaking unnecessary PII.

## 8. Future integration seams

The architecture should allow later adapters:

```text
LeadSourceAdapter
├─ XlsxFileSource      (V1)
├─ GoogleSheetsSource  (future)
└─ MetaLeadApiSource   (future)

SalesDataAdapter
├─ LocalManualSales    (possible V1.x)
└─ OdooSalesSource     (future)
```

Future adapters must feed the same canonical submission/contact domain rather than bypassing it.

### Product schema adapter rule

Product-question parsing must be version-aware. Legacy free-text and the new Meta multi-select question are different source schemas that converge on the same canonical many-valued product-interest model. Do not encode form-specific parsing rules in React components or analytics queries.
