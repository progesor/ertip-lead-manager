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
└───────────────┬─────────────────────┬────────┘
                │                     │
         ┌──────▼──────┐      ┌──────▼────────────────────┐
         │   SQLite    │      │ Manual file source adapters│
         │    SQLx     │      │ XLSX / Calamine            │
         └─────────────┘      │ CSV / rust-csv             │
                              └─────────────────────────────┘
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
- parsing XLSX or CSV;
- enforcing transactional import rules;
- canonical analytics calculations.

### Rust application/domain layer

Responsible for:

- business rules;
- import validation and preview;
- timestamp/contact normalization;
- identity matching;
- product-interest schema parsing;
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
      │  ├─ mod.rs
      │  ├─ source.rs
      │  ├─ xlsx.rs
      │  ├─ csv.rs
      │  ├─ headers.rs
      │  └─ product_interest.rs
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

## 5. Manual file import architecture

Import is split into a source-adapter phase and two application phases.

### 5.1 Source adapters

`XlsxFileSource` and `CsvFileSource` convert their format into the same canonical tabular representation before business rules run.

```text
.xlsx ──> XlsxFileSource ─┐
                          ├─> RawSubmissionRow[] ─> canonical import pipeline
.csv  ──> CsvFileSource ──┘
```

Rules:

- `.xlsx` uses Calamine and discovers the first supported worksheet/header row.
- `.csv` uses the Rust `csv` crate, not manual comma splitting.
- CSV V1 encoding is UTF-8 with optional UTF-8 BOM.
- File-format adapters may report format-specific metadata/errors, but must not implement identity, deduplication, CRM-state, or product-normalization business rules independently.
- Unknown additional columns remain available to `raw_payload_json` where feasible.

### 5.2 Phase A — Preview

1. Open selected file read-only.
2. Select the adapter by validated extension/content expectations.
3. Locate/map supported headers.
4. Parse rows into `RawSubmissionRow`.
5. Validate required fields.
6. Normalize timestamp/e-mail/phone/country and derive zero-or-more product-interest candidates according to the detected source schema.
7. Ignore agency-maintained `Status` and `İletişime Geçme Tarihi` as CRM inputs; keep them only in raw payload metadata if present.
8. Preserve source `lead_status` separately as raw source metadata.
9. Check external ID against DB.
10. Check contact identity candidates.
11. Produce per-row outcome and aggregate summary.
12. Return preview without writing business records.

### 5.3 Phase B — Commit

1. Begin SQLite transaction.
2. Create import batch record including source format.
3. Revalidate assumptions that matter for uniqueness.
4. Insert only new submissions.
5. Create/link contacts using conservative identity rules.
6. Seed default application state for newly created contacts.
7. Create normalized product-interest rows.
8. Create import/activity/data-quality metadata.
9. Commit transaction.
10. Return committed counts.

If any integrity error invalidates the batch, roll back.

## 6. Verified Meta multi-select parser boundary

The post-change export observed on **2026-08-21** keeps the header:

`which_product_would_you_like_to_receive_more_information_about?`

New structured answers use lower-case machine values. Multiple selected values are joined with the pipe character (`|`). A single selection has no delimiter. The parser must:

1. preserve the complete raw cell string;
2. split structured post-change values only on `|`;
3. trim each token;
4. map each complete token through the verified machine-value table;
5. emit one normalized product-interest membership per mapped token;
6. raise `UNKNOWN_PRODUCT` for unknown tokens without dropping the raw value.

Never split structured product answers on commas. One verified token is `implanters,_forceps_&_surgical_instruments`, which contains commas internally.

Legacy free-text rows using the same header remain routed through the legacy normalization path. Detection must therefore be schema/value-aware and deterministic.

## 7. Database conventions

- UUID/ULID local identifiers are application IDs.
- External Meta IDs are stored as text and remain unchanged.
- Application-managed UTC timestamps use RFC 3339 text; raw source timestamps remain preserved separately.
- Enable SQLite foreign keys.
- File databases use WAL mode with `NORMAL` synchronous mode as accepted in ADR 0005.
- Add indexes based on actual query paths, especially normalized e-mail/phone, latest submission date, status, follow-up due date, campaign/form/platform dimensions.

## 8. Error model

Backend errors should be typed into categories:

- `ValidationError`
- `ImportSchemaError`
- `ImportRowError`
- `UnsupportedFileType`
- `CsvEncodingError`
- `IdentityConflict`
- `NotFound`
- `DatabaseError`
- `BackupError`
- `UnexpectedError`

User-facing errors must be clear and non-technical while logs retain enough technical detail for debugging without leaking unnecessary PII.

## 9. Future integration seams

The architecture allows later adapters:

```text
LeadSourceAdapter
├─ XlsxFileSource      (V1)
├─ CsvFileSource       (V1)
├─ GoogleSheetsSource  (future)
└─ MetaLeadApiSource   (future)

SalesDataAdapter
├─ LocalManualSales    (possible V1.x)
└─ OdooSalesSource     (future)
```

Future adapters must feed the same canonical submission/contact domain rather than bypassing it.

### Product schema adapter rule

Product-question parsing must be version-aware. Legacy free text and the verified Meta multi-select representation are different source schemas that converge on the same canonical many-valued product-interest model. Do not encode form-specific parsing rules in React components or analytics queries.
