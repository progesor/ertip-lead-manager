# Ertip Lead Manager — Development Start Prompt

Use this prompt when starting the implementation conversation/agent after the repository is created.

---

We are starting development of **Ertip Lead Manager**.

Treat the repository documentation as canonical. Read these files first, in this order:

1. `AGENTS.md`
2. `docs/00_PROJECT_CANON.md`
3. `docs/project-canon.yaml`
4. `docs/02_PRODUCT_REQUIREMENTS.md`
5. `docs/03_SYSTEM_ARCHITECTURE.md`
6. `docs/04_DATA_MODEL.md`
7. `docs/05_EXCEL_IMPORT_CONTRACT.md`
8. `docs/06_LEAD_LIFECYCLE_AND_BUSINESS_RULES.md`
9. `docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md`
10. `docs/07_UI_UX_DESIGN_SYSTEM.md`
11. `docs/10_TEST_STRATEGY.md`
12. `docs/11_ROADMAP.md`
13. `docs/development/M0_DISCOVERY_CHECKLIST.md`

Product summary:

- Windows-first desktop lead manager for Ertip Medical.
- Tauri 2 + React/TypeScript frontend + Rust backend + SQLite.
- V1 is local-first, single-user, and offline-capable.
- V1 imports Meta lead exports manually from `.xlsx`.
- Do not implement Google Sheets sync, Meta API, cloud DB, authentication, WhatsApp API, or Odoo integration in V1.
- Source submission data is immutable.
- CRM state is separate.
- `external_lead_id` uniquely identifies a submission.
- Same contact with a new external ID is a repeat submission, not an exact duplicate.
- Never auto-merge on name alone.
- Product interest is multi-select/many-valued; do not implement a single `normalized_product` field.
- Canonical customer-facing product interests are the six values in `docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md`.
- Existing legacy free-text product data must remain supported.
- The exact new Meta Excel multi-select serialization is pending the first post-change export; do not guess it.

Start with M0. Inspect the repository, report any conflict between the actual repository and the canon, then complete the M0 checklist items that can be resolved from the repo. Do not begin M1 until M0 decisions are clear. When M1 starts, implement in small, testable steps and keep documentation synchronized with behavior.
