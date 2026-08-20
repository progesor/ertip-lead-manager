# Ertip Lead Manager

Windows-first, local-first lead management and analytics application for Meta lead exports used by Ertip Medical.

> Status: **Documentation / repository bootstrap**. No production code has been implemented yet.

## Product summary

Ertip Lead Manager imports manually downloaded `.xlsx` lead files, preserves the original source data, detects duplicate/repeat submissions, supports legacy free-text and new multi-select product interests, and provides a fast Windows desktop workspace for reviewing, qualifying, following up, and analyzing leads.

The initial release deliberately does **not** connect to Google Sheets, Meta APIs, WhatsApp APIs, cloud databases, or multi-user authentication. Those are future integrations. V1 must remain useful fully offline.

## Canonical documents

Read these before changing product behavior or architecture:

1. [`docs/00_PROJECT_CANON.md`](docs/00_PROJECT_CANON.md)
2. [`docs/project-canon.yaml`](docs/project-canon.yaml)
3. [`docs/02_PRODUCT_REQUIREMENTS.md`](docs/02_PRODUCT_REQUIREMENTS.md)
4. [`docs/03_SYSTEM_ARCHITECTURE.md`](docs/03_SYSTEM_ARCHITECTURE.md)
5. [`docs/04_DATA_MODEL.md`](docs/04_DATA_MODEL.md)
6. [`docs/05_EXCEL_IMPORT_CONTRACT.md`](docs/05_EXCEL_IMPORT_CONTRACT.md)
7. [`docs/06_LEAD_LIFECYCLE_AND_BUSINESS_RULES.md`](docs/06_LEAD_LIFECYCLE_AND_BUSINESS_RULES.md)
8. [`docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md`](docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md)
9. [`docs/07_UI_UX_DESIGN_SYSTEM.md`](docs/07_UI_UX_DESIGN_SYSTEM.md)
10. [`docs/10_TEST_STRATEGY.md`](docs/10_TEST_STRATEGY.md)
11. [`docs/11_ROADMAP.md`](docs/11_ROADMAP.md)

`AGENTS.md` contains implementation rules for AI-assisted development.

Repository publishing instructions: [`docs/14_GITHUB_REPOSITORY_SETUP.md`](docs/14_GITHUB_REPOSITORY_SETUP.md).

## Proposed stack

- **Desktop shell:** Tauri 2
- **Frontend:** React + TypeScript + Vite
- **UI:** Tailwind CSS + accessible headless primitives
- **Table:** TanStack Table
- **Charts:** Recharts
- **Validation:** Zod
- **Backend:** Rust commands exposed through Tauri
- **Excel parser:** Calamine
- **Database:** SQLite via SQLx
- **Testing:** Vitest + React Testing Library + Rust unit/integration tests

Exact dependency versions are chosen during M1 and pinned in the repository.

## Initial workflow

```text
Meta / Google Sheet
      ↓
Manual .xlsx download
      ↓
Ertip Lead Manager → Import Preview
      ↓
Validate / deduplicate / normalize
      ↓
SQLite local database
      ↓
Leads / Pipeline / Follow-ups / Analytics
```

## Development start

After creating the GitHub repository from this package:

1. Complete [`docs/development/M0_DISCOVERY_CHECKLIST.md`](docs/development/M0_DISCOVERY_CHECKLIST.md).
2. Begin [`docs/development/M1_FOUNDATION.md`](docs/development/M1_FOUNDATION.md).
3. Do not implement later milestones early unless the canonical scope is explicitly updated.

## Privacy note

Production lead exports contain personal data. Do not commit real lead files, customer names, e-mail addresses, or phone numbers. Use only sanitized fixtures under `fixtures/`.
