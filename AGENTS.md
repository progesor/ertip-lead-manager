# AGENTS.md — Ertip Lead Manager

This repository is designed for AI-assisted development. Treat this file and the canonical documents as implementation constraints.

## Source-of-truth order

If documents conflict, use this precedence:

1. `docs/00_PROJECT_CANON.md`
2. `docs/project-canon.yaml`
3. Explicitly accepted ADRs under `docs/adr/`
4. `docs/02_PRODUCT_REQUIREMENTS.md`
5. `docs/03_SYSTEM_ARCHITECTURE.md`
6. `docs/04_DATA_MODEL.md`
7. `docs/05_EXCEL_IMPORT_CONTRACT.md`
8. `docs/06_LEAD_LIFECYCLE_AND_BUSINESS_RULES.md`
9. `docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md`
10. Milestone documents under `docs/development/`
11. Other documentation

When changing a canonical decision, update the relevant documentation in the same change.

## Product constraints

- Windows-first desktop application.
- V1 is local-first and single-user.
- V1 uses manual `.xlsx` and `.csv` import only.
- Do not add Google Sheets sync, Meta API sync, cloud DB, authentication, WhatsApp API, or e-mail sending in V1.
- Source data imported from supported files is immutable after import.
- User-managed CRM data is stored separately from source submission data.
- Never auto-merge contacts on name alone.
- `external_lead_id` identifies a submission, not necessarily a unique person.
- A repeated e-mail/phone with a new external lead ID is a repeat submission, not a duplicate row.
- Product interest is many-valued. Never model it as a single product column.
- The six customer-facing product-interest codes are canonical; `UNKNOWN` is internal only.
- Preserve legacy free-text product answers and support them alongside the new structured multi-select form.
- The verified Meta multi-select machine values are pipe-delimited (`|`) inside the product-answer field. Do not comma-split product selections.
- Unknown extra columns must not break import. Agency-added `Status` and `İletişime Geçme Tarihi` are not application lifecycle inputs and are ignored by default while remaining preservable in raw payload metadata.
- The source `lead_status` field is separate from the agency-added `Status` column and remains raw source metadata only.
- All destructive or merge-like actions must be reversible or explicitly confirmed.

## Engineering rules

- Prefer small, testable modules over large components or command handlers.
- Business rules belong in domain/services, not in React components.
- Database access must be behind repositories/services.
- Database migrations are versioned and never edited after release; add a new migration instead.
- File parsing and import decisions must be deterministic and test-covered.
- XLSX and CSV adapters must converge into one canonical row/import pipeline rather than duplicate business rules.
- CSV parsing must use a standards-compliant parser; never split CSV rows manually on commas.
- UI must remain usable with 10,000+ lead submissions without rendering the entire dataset at once.
- Use UTC for persisted timestamps; preserve original source timestamp strings when available.
- Avoid storing derived analytics values when they can be calculated reliably from canonical records.
- Log meaningful user actions as activities when required by the lifecycle spec.

## Privacy / repository hygiene

- Never commit real lead exports.
- Never commit real names, e-mails, phone numbers, tokens, API keys, or production DB files.
- Database files, backups, exports, and logs containing PII must be gitignored.
- Test fixtures must be synthetic or sanitized.

## Definition of done for a milestone

A milestone is complete only when:

- its acceptance criteria pass;
- relevant unit/integration tests pass;
- no known data-loss defect exists;
- documentation reflects implemented behavior;
- the milestone checklist is updated;
- the app builds on Windows x64.
