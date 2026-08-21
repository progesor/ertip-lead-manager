# Sanitized fixtures

Files in this folder must contain only synthetic or sanitized data.

## Legacy fixtures

- `leads_sample_sanitized.csv` mirrors the legacy known column layout and intentionally includes identity/import edge cases.
- `leads_sample_sanitized.xlsx` is the legacy XLSX counterpart used for Calamine integration coverage.

## Verified structured multi-select fixture

`leads_sample_multiselect_sanitized.csv` mirrors the post-change schema observed on **2026-08-21** without containing any real customer data.

It intentionally covers:

- the unchanged product-question header;
- a structured single selection (`fue_punches`);
- a structured three-selection value joined with `|`;
- all six verified machine values in one quoted CSV field;
- the comma-containing implanter/forceps machine token;
- `other_products_/_general_information`;
- agency-added `Status` and `İletişime Geçme Tarihi` columns that must be ignored as CRM inputs;
- source `lead_status=CREATED` which remains raw metadata only;
- a repeat contact with a new external ID;
- an exact duplicate external ID represented with a different timestamp offset.

CSV quoting is intentional: one product token contains commas. Parser tests must use a standards-compliant CSV parser rather than manual comma splitting.

Never replace these fixtures with a real customer export.
