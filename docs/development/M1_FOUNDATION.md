# M1 — Desktop Foundation

## Goal

Create a stable application shell and persistence foundation without prematurely building the full import feature.

## Implementation status

**Branch:** `feat/m1-foundation`  
**Pull request:** #2  
**Status:** **PASS**

Verified on 2026-08-21:

- frontend lint/test/build passed locally and in CI;
- Rust migration/repository/error tests passed locally and on Windows CI;
- Tauri dev application launched successfully on the Windows development PC;
- Settings diagnostics returned app version `0.1.0`, schema version `1`, and a stable SQLite path;
- application restart reused the same local database path/schema;
- Windows NSIS debug package build passed in GitHub Actions.

## Deliverables

### Project bootstrap

- Tauri 2
- React + TypeScript
- Vite
- lint/format scripts
- test scripts
- Windows dev/build instructions in README

### UI shell

- sidebar navigation
- routes/pages: Dashboard, Leads, Pipeline, Analytics, Imports, Settings
- common layout
- design tokens/theme primitives
- empty-state placeholders

### Backend foundation

- Rust module structure
- Tauri command registration pattern
- typed error mapping
- application data directory resolution
- logging policy

### Database

- SQLx SQLite connection
- migration runner
- initial tables from `04_DATA_MODEL.md`
- foreign keys enabled
- repository/service boundaries
- test DB helper

### Diagnostics

Settings/About displays:

- app version
- DB path
- schema version

## Tests

- app frontend unit test baseline
- Rust unit test baseline
- migration integration test
- simple DB write/read repository test
- command error serialization test
- Windows CI package build

## Non-goals

- no production lead file importer yet;
- no dashboard analytics yet;
- no pipeline behavior yet;
- no Google/Meta integration.

## Acceptance criteria

- [x] `npm` frontend tests pass.
- [x] `cargo test` passes.
- [x] Tauri dev app launches on Windows.
- [x] packaged/dev app creates/opens SQLite DB in correct app data directory.
- [x] migrations apply from empty DB.
- [x] app restarts without recreating/loss of DB.
- [x] navigation shell works.
- [x] README includes development commands.
- [x] Windows NSIS debug package build passes in CI.

## Exit

M1 is complete. Development continues in M2 on manual `.xlsx` / `.csv` import, normalization and deduplication.
