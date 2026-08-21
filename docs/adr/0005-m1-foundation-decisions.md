# ADR 0005 — M1 Foundation Defaults

**Status:** Accepted  
**Date:** 2026-08-20

## Context

M0 left several operational defaults open before implementation could begin. The application is an internal Windows tool used by a Turkish-speaking sales/operations team, while stored domain keys must remain stable and language-neutral.

## Decision

1. **UI language:** V1 interface labels are Turkish. Stable domain enums, database values and code identifiers remain English/language-neutral.
2. **Windows target:** Windows 10/11 x64 is the only supported V1 platform.
3. **Installer:** Internal releases use an NSIS current-user installer. An unsigned installer is acceptable for internal testing until code signing is introduced.
4. **Application data:** Resolve storage using Tauri `app_data_dir()` rather than hard-coding a Windows path. SQLite lives below that directory and the resolved path is visible in Settings.
5. **Retention:** The application performs no automatic deletion of leads, submissions, notes, activities, imports, database files, or backups.
6. **Backup policy:** V1 backups stay local and user-controlled. Restore safety-copy behavior is implemented in M6.
7. **Timestamp persistence:** Application-managed UTC timestamps are stored as RFC 3339 text. Raw source timestamp strings remain preserved separately.
8. **SQLite:** Foreign keys are enabled. File databases use WAL mode and `NORMAL` synchronous mode after initialization.
9. **Dependency policy:** M1 pins exact top-level npm versions. Rust dependencies use compatible stable major/minor requirements and are fully resolved by `Cargo.lock` when generated on a Rust-enabled development environment.
10. **Feature dependencies:** Calamine, TanStack Table, Recharts and Zod are introduced when their owning milestone first needs them; M1 does not preload unused production dependencies.

## Consequences

- Turkish UI wording can evolve without database migrations.
- Future localization remains possible because domain values are not localized.
- Local storage follows Windows/Tauri conventions and is not tied to a developer-specific folder.
- The first internal installer does not require a signing certificate.
- M2 remains blocked from finalizing the new Meta multi-select parser until a real post-change export confirms its header and serialization.
