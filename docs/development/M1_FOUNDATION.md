# M1 — Desktop Foundation

## Goal

Create a stable application shell and persistence foundation without prematurely building the full import feature.

## Implementation status

**Branch:** `feat/m1-foundation`  
**Pull request:** #2  
**Status:** implementation complete for the first foundation pass; Windows runtime/build verification is pending before this milestone can be marked complete.

Do not mark acceptance items complete based only on source review. M1 Definition of Done still requires frontend tests, Rust tests, a Windows Tauri launch, database restart persistence, and a Windows package/build check.

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
- initial tables from `04_DATA_MODEL.md` (may stage less-used tables if documented)
- foreign keys enabled
- repository/service boundaries
- test DB helper

### Diagnostics

Settings/About displays:

- app version
- DB path
- schema version
- optional “Open data folder”

## Tests

- app frontend unit test baseline
- Rust unit test baseline
- migration integration test
- simple DB write/read repository test
- command error serialization test

## Non-goals

- no production Excel importer yet;
- no dashboard analytics yet;
- no pipeline behavior yet;
- no Google/Meta integration.

## Acceptance criteria

- [ ] `npm/pnpm` frontend tests pass.
- [ ] `cargo test` passes.
- [ ] Tauri dev app launches on Windows.
- [ ] packaged/dev app creates/opens SQLite DB in correct app data directory.
- [ ] migrations apply from empty DB.
- [ ] app restarts without recreating/loss of DB.
- [ ] navigation shell works.
- [x] README includes development commands.
