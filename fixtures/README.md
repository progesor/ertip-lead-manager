# Sanitized fixtures

Files in this folder must contain only synthetic or sanitized data.

`leads_sample_sanitized.csv` mirrors the **legacy known column layout** and intentionally includes identity/import edge cases. Convert/generate an `.xlsx` equivalent during M0/M2 for Calamine integration tests.

The Meta product question is being changed to a six-option multi-select. Do not invent a future multi-select fixture yet: the exact exported header and multiple-value serialization have not been observed. After the first real post-change export is available, inspect it, document the format in `docs/05_EXCEL_IMPORT_CONTRACT.md`, then create a second sanitized fixture with at least one 2+ selection row.

Never replace these fixtures with a real customer export.
