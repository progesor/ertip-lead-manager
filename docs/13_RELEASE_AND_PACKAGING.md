# 13 — Release and Packaging

## 1. Versioning

Use semantic versioning:

- `0.x` during internal development
- `1.0.0` when V1 acceptance criteria are complete

Database schema version is tracked through migrations, not inferred only from app version.

## 2. Windows target

Primary target: Windows x64.

Installer format will be selected based on Tauri 2 tooling and internal deployment preference during M6. Avoid committing installer binaries to main Git history; use GitHub Releases when distribution begins.

## 3. Build metadata

Expose in Settings/About:

- app version
- commit/build identifier if available
- database schema/migration version
- data path

Useful for support/debugging.

## 4. Release checklist

- all tests pass;
- clean build;
- migrations tested from prior supported DB;
- smoke checklist completed;
- no real PII fixtures in repo;
- backup/restore verified;
- release notes written;
- installer tested on a machine/user profile other than the development environment where possible.

## 5. Rollback

For internal releases, keep previous installer and require a backup before schema-breaking upgrade risk. Migrations should be forward-safe; database downgrade is not assumed unless explicitly implemented.
