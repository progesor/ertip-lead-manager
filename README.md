# Ertip Lead Manager

Windows-first lead management and analytics application for Meta lead exports used by Ertip Medical.

> Status: **M5 Analytics + M5.5 Team Assignment in active development**. M0–M4 are complete and merged to `main`.

## Product summary

Ertip Lead Manager imports manually downloaded `.xlsx` and `.csv` lead files, preserves immutable source data, detects duplicate/repeat submissions, supports legacy free-text and structured multi-select product interests, and provides a Windows desktop workspace for daily lead and sales operations.

The application is intentionally organized around this daily-use hierarchy:

1. **Genel Bakış / Dashboard** — KPI and attention-first work queue.
2. **Pipeline / Kanban** — primary lifecycle/status workspace.
3. **Lead Detail** — per-contact operational workspace.
4. **Leadler** — secondary broad search and audit list.

Current capabilities include:

- real SQLite-backed Dashboard KPIs and attention queues;
- pointer-based full-card Kanban drag/drop with floating preview and audited status changes;
- Kanban search/country/product/repeat/warning plus due-today/overdue quick filters;
- follow-up create/reschedule/complete/cancel with canonical UTC persistence;
- context-aware navigation between Dashboard/Kanban and Lead Detail;
- production Lead Detail layout with a 2/3 operational workspace and 1/3 sticky tabbed history/source panel;
- CRM lifecycle status changes and immutable activity audit;
- editable CRM notes;
- manual contact-level product-interest corrections stored separately from imported source data;
- full immutable submission/source history when needed without dominating the daily workflow;
- real SQLite-backed Lead list with search/filter/sort/pagination;
- dynamic country filtering with Turkish country names;
- platform, repeat and data-quality indicators;
- 10k-contact / 25k-submission workspace smoke coverage;
- M5 analytics development: explicit submission/contact/repeat metrics, trends, funnel and acquisition breakdowns;
- M5.5 local personnel groundwork: stable staff IDs, lead assignment, Kanban assignee visibility/filtering and audit-ready actor fields.

The original local-first SQLite mode remains the development/fallback runtime. The planned multi-user release architecture is a private PostgreSQL database behind an authenticated backend API on Coolify; Tauri and the future Web App will use the same API rather than connecting directly to PostgreSQL. See `docs/development/M5_5_TEAM_MULTIUSER.md`.

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

## Application stack

- Tauri 2
- React 19 + TypeScript
- Vite
- Tailwind CSS 4
- Rust backend commands
- SQLite via SQLx
- Calamine + Rust `csv` for manual lead import
- Vitest + React Testing Library
- Biome for linting/formatting

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

The SQLite database is created below Tauri's Windows application-data directory for the identifier `com.ertipmedical.leadmanager`. The exact resolved path and current schema version are shown under **Ayarlar → Sistem Bilgisi**. Application data is not automatically deleted by retention policy.

## Privacy

Production lead exports contain personal data. Never commit real lead files, customer names, e-mail addresses, phone numbers, database files, logs containing PII, or backups. Parser tests use sanitized fixtures under `fixtures/`.
