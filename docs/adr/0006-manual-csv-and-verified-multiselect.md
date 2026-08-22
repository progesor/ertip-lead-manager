# ADR 0006 — Manual CSV Support and Verified Meta Multi-Select Serialization

**Status:** Accepted  
**Date:** 2026-08-21

## Context

V1 was initially scoped to manual `.xlsx` ingestion. A real post-change Meta lead workbook was inspected after the product-interest question was converted to multi-select. The same workflow can also provide CSV exports, and supporting CSV materially improves portability/testing without introducing live integrations.

The inspected workbook also contained two agency-maintained columns (`Status` and `İletişime Geçme Tarihi`) appended after the Meta/source fields. These values belong to the agency's own lightweight tracking workflow rather than Ertip Lead Manager's CRM state.

## Decision

1. V1 supports manual `.xlsx` and `.csv` imports.
2. `.xlsx` is parsed with Calamine; `.csv` is parsed with the Rust `csv` crate.
3. Both adapters feed one canonical row/import pipeline. Identity, normalization, product mapping, preview and commit rules are not duplicated per format.
4. CSV V1 input is UTF-8 with optional UTF-8 BOM.
5. The Meta product header remains `which_product_would_you_like_to_receive_more_information_about?` after the form change.
6. New structured product answers use machine-value tokens and join multiple selections with `|`.
7. Structured product parsing splits only on `|`; commas are not delimiters.
8. Verified machine-value mappings are recorded in `docs/05_EXCEL_IMPORT_CONTRACT.md` and `docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md`.
9. Agency `Status` and `İletişime Geçme Tarihi` are ignored as CRM inputs. They may remain in raw payload metadata.
10. Source `lead_status` is a distinct source field and is preserved raw only; it never overwrites application lifecycle status.

## Consequences

- M2 needs two format adapters but only one business-rule pipeline.
- CSV provides a simple deterministic fixture format for structured product tests.
- The application can import agency-shared files without inheriting the agency's mutable status/follow-up state.
- Legacy free-text and new structured values coexist under one source header, so product parsing must distinguish them by verified value rules.
