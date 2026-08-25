# Ertip Lead Manager — Local Release v0.1.0

## Status

**FROZEN / PASS** — validated on a real Windows installation before M6 centralized-backend work.

## Purpose

Freeze the completed local SQLite/Tauri product as a recoverable Windows fallback before the project moves to the centralized API/PostgreSQL/Auth architecture.

## Release identity

- App version: `0.1.0`
- Release label: `v0.1.0-local`
- Target: Windows 10/11 x64
- Installer: Tauri 2 NSIS, current-user install
- Persistence: local SQLite, schema version 4
- Source baseline: M5 + M5.5 complete
- Frozen source branch: `release/local-v0.1.0`
- Release build commit: `049bcee39a719f422d663df10f143b09dff367db`
- Release workflow run: `32724166743`
- Artifact name: `ertip-lead-manager-v0.1.0-local-win-x64`
- Installer file: `Ertip Lead Manager_0.1.0_x64-setup.exe`
- Installer SHA-256: `32acf3db650be9887beb33190ce467af400bc680b99e2bfa78ef6448b8f951a7`

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

The dedicated GitHub Actions local-release workflow publishes the installer, release metadata and SHA-256 checksum as a workflow artifact.

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

- [x] NSIS installer completes for current Windows user.
- [x] Application starts from the installed package rather than the development runtime.
- [x] Application opens successfully with local SQLite schema version 4.
- [x] Existing Light/Dark UI remains functional in the installed build.

### Existing database/profile test

- [x] Existing local profile opens successfully after installation.
- [x] Existing CRM data remains available.
- [x] Personnel and assignment-enabled build runs against the existing profile.

### Daily workflow

- [x] Dashboard opens in the installed build.
- [x] Kanban and Lead workspace open in the installed build.
- [x] Lead Detail/personnel-enabled local workflow is operational.
- [x] Lead List ownership-enabled build is operational.
- [x] Analytics opens in the installed build.

The user completed the installer-based smoke pass on 2026-08-25 and reported the installed application working correctly. Detailed feature behavior had already been accepted during M5/M5.5 development smoke testing.

### Installer sanity

- [x] Installed application launches successfully.
- [x] Release-mode NSIS build completed in GitHub Actions.
- [x] SHA-256 checksum generated and independently verified from the downloaded installer.

## Signing note

This internal fallback build is currently unsigned. Windows SmartScreen/reputation warnings may appear on a new machine. Code signing should be added before broad external distribution, but it is not required for this internal fallback release.

## Freeze rule

`release/local-v0.1.0` is now a frozen fallback source point. Do not continue M6/M7/M8 development on that branch. The installer and checksum should be retained independently of future centralized-architecture work.
