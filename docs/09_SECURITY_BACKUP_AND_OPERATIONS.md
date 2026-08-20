# 09 — Security, Backup and Operations

## 1. Threat model for V1

V1 is a local single-user desktop tool. Primary risks are:

- accidental loss/corruption of the SQLite database;
- accidental import of malformed data;
- committing/exporting PII into source control;
- unauthorized access to the Windows user profile/computer;
- leakage through logs/backups.

V1 is not designed as a hardened multi-tenant security boundary.

## 2. Data location

Use the Tauri/app-specific Windows application data directory, not the executable directory.

Store at minimum:

- SQLite DB
- local preferences
- logs (if enabled)
- temporary import metadata

The Settings page should display the path and offer “Open data folder”.

## 3. Database safety

- Versioned migrations.
- Foreign keys enabled.
- Import writes wrapped in transactions.
- Consider SQLite WAL mode after verifying backup behavior.
- Never copy a live DB naïvely if WAL makes the copy inconsistent; use SQLite backup API/checkpoint strategy where necessary.

## 4. Backup

User-initiated backup should produce a consistent database copy with metadata such as:

```text
ertip-lead-manager-backup_2026-08-20_1545.db
```

Optional manifest:

- app version
- schema version
- backup UTC time

Do not include real backups in Git.

## 5. Restore

Restore flow:

1. Select backup.
2. Validate file/schema compatibility.
3. Warn that current local data will be replaced.
4. Create a safety backup of current DB where possible.
5. Close DB connections.
6. Restore atomically/safely.
7. Reopen and run compatibility checks.
8. Show result.

## 6. Logs

Logs should avoid full PII where not required.

Good:

```text
Import row 17: INVALID_EMAIL
Import batch abc: 41 inserted, 339 duplicates
```

Avoid:

```text
Failed to parse john@example.com +90...
```

Use external/local IDs for diagnostics where possible.

## 7. Import file handling

The application reads the selected Excel file; it does not need to copy it permanently into app data in V1.

If a temporary copy is required for parsing, delete it after operation.

## 8. Repository privacy controls

`.gitignore` blocks common spreadsheet and DB files. This is not sufficient by itself: developers must never force-add real exports.

Sanitized fixtures only.

## 9. Update strategy

Automatic updater is not required for first internal release. Version the app semantically and document database migrations.

Possible later:

- GitHub Releases
- signed installer
- Tauri updater

## 10. Crash handling

Unexpected errors should:

- show a user-readable failure;
- avoid partial import commits;
- write a PII-minimized diagnostic log;
- preserve existing DB.
