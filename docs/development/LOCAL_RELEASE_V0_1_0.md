# Ertip Lead Manager — Local Release v0.1.0

## Purpose

Freeze the completed local SQLite/Tauri product as a recoverable Windows fallback before the project moves to the centralized API/PostgreSQL/Auth architecture.

## Release identity

- App version: `0.1.0`
- Release label: `v0.1.0-local`
- Target: Windows 10/11 x64
- Installer: Tauri 2 NSIS, current-user install
- Persistence: local SQLite, schema version 4
- Source baseline: M5 + M5.5 complete

## Included product scope

- manual XLSX/CSV Meta lead import and deduplication;
- immutable source submissions and raw payload preservation;
- lead workspace/search/filter/detail;
- lifecycle status and activity audit;
- product-interest normalization and manual CRM correction;
- Kanban pipeline with pointer drag/drop;
- follow-ups, due/overdue workflow and Dashboard attention queues;
- analytics/reporting with date filters and marketing dimensions;
- Light/Dark theme;
- local personnel management;
- lead assignee management, Kanban and Lead List assignee visibility/filtering.

## Build procedure

Canonical release build:

```powershell
npm install --no-audit --no-fund
npm run check
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:build
```

Expected installer output:

```text
src-tauri/target/release/bundle/nsis/*.exe
```

The dedicated GitHub Actions local-release workflow also publishes the installer and SHA-256 checksum as a workflow artifact.

## Data safety before install/update test

The application database is intentionally not bundled into the installer.

Before testing an installer against an existing working profile:

1. Close Ertip Lead Manager completely.
2. Open Settings and note the displayed SQLite database path.
3. Copy `ertip-lead-manager.sqlite3` to a separate backup folder.
4. Keep that backup until installer/update testing is accepted.

Database downgrade is not assumed. Restore means closing the app and restoring the backed-up SQLite file to the same app-data location.

## Acceptance smoke test

### Fresh install / startup

- [ ] NSIS installer completes for current Windows user.
- [ ] Application starts without development tools or Node.js installed.
- [ ] Settings shows app version `0.1.0` and schema version `4`.
- [ ] Light/Dark mode works after restart.

### Existing database upgrade/profile test

- [ ] Existing SQLite database opens without data loss.
- [ ] Existing contacts/submissions/import history are intact.
- [ ] Existing notes, follow-ups and activities are intact.
- [ ] Existing personnel and lead assignments are intact.

### Daily workflow

- [ ] Dashboard loads and attention queues work.
- [ ] Kanban loads, drag/drop works and assignee names/filters work.
- [ ] Lead Detail loads; status, assignee, follow-up and note mutations persist.
- [ ] Lead List shows the Sorumlu column and personnel/Atanmamış filtering works.
- [ ] Import preview + commit works using a sanitized/test file.
- [ ] Analytics loads and date/source breakdown filters work.

### Installer sanity

- [ ] Start-menu entry launches correctly.
- [ ] Closing/reopening preserves local data and theme.
- [ ] Uninstall/reinstall behavior is understood before using uninstall on the production profile.

## Signing note

This internal fallback build is currently unsigned. Windows SmartScreen/reputation warnings may appear on a new machine. Code signing should be added before broad external distribution, but it is not required for this internal fallback release candidate.

## Freeze rule

After this candidate passes the manual smoke test, create/retain a stable `v0.1.0-local` source point and keep the installer artifact/checksum independently of the upcoming M6 cloud/API work.
