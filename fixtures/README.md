# Sanitized fixtures

Files in this folder must contain only synthetic or sanitized data.

## Legacy fixtures

- `leads_sample_sanitized.csv`
- `leads_sample_sanitized.xlsx`

These mirror the legacy free-text product-answer layout and contain identity/import edge cases.

## Verified multi-select fixtures

- `leads_sample_multiselect_sanitized.csv`
- `leads_sample_multiselect_sanitized.xlsx`

These mirror the post-change Meta export observed on 2026-08-21 using synthetic data only. They intentionally cover:

- a legacy free-text product answer;
- one structured product selection;
- three structured selections separated by `|`;
- all six structured selections, including the machine value containing commas;
- agency-maintained `Status` / `İletişime Geçme Tarihi` columns;
- a repeat contact with a new external lead ID;
- an exact duplicate external lead ID represented with a different timezone offset.

The XLSX fixture stores source timestamps as text so Calamine receives the same ISO-8601 representation as the real export contract rather than spreadsheet date serials.

Never replace these fixtures with a real customer export.
