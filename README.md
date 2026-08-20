# Ertip Lead Manager

Windows-first, local-first lead management and analytics application for Meta lead exports used by Ertip Medical.

> Status: **M1 — Desktop Foundation in progress** on `feat/m1-foundation`.

## Product summary

Ertip Lead Manager imports manually downloaded `.xlsx` lead files, preserves the original source data, detects duplicate/repeat submissions, supports legacy free-text and new multi-select product interests, and provides a fast Windows desktop workspace for reviewing, qualifying, following up, and analyzing leads.

V1 deliberately does **not** connect to Google Sheets, Meta APIs, WhatsApp APIs, cloud databases, or multi-user authentication. Core workflows remain usable offline.

## Canonical documents

Read these before changing product behavior or architecture:

1. `docs/00_PROJECT_CANON.md`
2. `docs/project-canon.yaml`
3. `docs/02_PRODUCT_REQUIREMENTS.md`
4. `docs/03_SYSTEM_ARCHITECTURE.md`
5. `docs/04_DATA_MODEL.md`
6. `docs/05_EXCEL_IMPORT_CONTRACT.md`
7. `docs/06_LEAD_LIFECYCLE_AND_BUSINESS_RULES.md`
8. `docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md`
9. `docs/07_UI_UX_DESIGN_SYSTEM.md`
10. `docs/10_TEST_STRATEGY.md`
11. `docs/11_ROADMAP.md`

`AGENTS.md` contains implementation rules for AI-assisted development.

## M1 stack

- Tauri 2
- React 19 + TypeScript
- Vite
- Tailwind CSS 4
- Rust backend commands
- SQLite via SQLx
- Vitest + React Testing Library
- Biome for linting/formatting

Feature-specific libraries such as TanStack Table, Recharts, Zod and Calamine are added in the milestone where they are first used rather than preloaded into the M1 shell.

## Windows development prerequisites

Install these once on the development PC:

- Node.js 22 or newer
- Rust stable using `rustup`
- Microsoft Visual Studio Build Tools with **Desktop development with C++**
- Microsoft Edge WebView2 Runtime (normally already available on current Windows 10/11)

## Development commands

From the repository root:

```powershell
npm install
npm run tauri:dev
```

Frontend-only development:

```powershell
npm run dev
```

Validation:

```powershell
npm run lint
npm run test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

Windows installer build:

```powershell
npm run tauri:build
```

The internal V1 packaging target is an unsigned NSIS per-user installer until code-signing policy is introduced.

## Local data

The SQLite database is created below Tauri's Windows application-data directory for the identifier `com.ertipmedical.leadmanager`. The exact resolved path is shown under **Ayarlar → Sistem Bilgisi**. Application data is not automatically deleted by retention policy.

## Privacy

Production lead exports contain personal data. Never commit real lead files, customer names, e-mail addresses, phone numbers, database files, logs containing PII, or backups. Parser tests use sanitized fixtures under `fixtures/`.
