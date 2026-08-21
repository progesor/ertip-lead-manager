# M2 — Manual File Import and Deduplication

## Goal

Implement the most critical workflow: safe, explainable, transactional `.xlsx` / `.csv` ingestion.

## Verified inputs available before implementation

- Legacy free-text export behavior is documented.
- Post-change Meta multi-select export inspected on 2026-08-21.
- Product header remains `which_product_would_you_like_to_receive_more_information_about?`.
- Structured multi-select machine values use `|` as the delimiter.
- Verified machine-value mapping is canonical in `docs/05_EXCEL_IMPORT_CONTRACT.md`.
- Agency columns `Status` and `İletişime Geçme Tarihi` must be ignored as CRM inputs.
- Source `lead_status` remains raw source metadata only.
- V1 accepts manual `.xlsx` and `.csv` files.

## Work packages

### M2.1 Source adapters and parser foundation

- Windows file picker allowing `.xlsx` and `.csv`
- common `LeadSourceAdapter` / canonical tabular row boundary
- XLSX adapter with Calamine
- CSV adapter with Rust `csv` crate
- UTF-8 / optional BOM CSV handling
- XLSX sheet/header discovery
- CSV header discovery
- row extraction
- raw payload preservation
- unsupported file/encoding errors

### M2.2 Header and source-field contract

- required/optional header validation
- unknown-column tolerance
- explicit ignore rules for agency `Status` and `İletişime Geçme Tarihi`
- preserve raw `lead_status` without mapping to CRM lifecycle
- preserve source identifiers as text

### M2.3 Normalization and product parsing

- e-mail
- phone
- country
- booleans
- timestamps
- legacy product normalization
- verified structured product parser:
  - split only on `|`
  - map verified machine values
  - support 1 or many selections
  - never comma-split
  - unknown structured token warning

### M2.4 Identity engine

Implement and test decision matrix from canonical docs.

Outputs must explain:

- exact duplicate reason;
- repeat-match identifiers;
- conflict identifiers;
- warnings/errors.

### M2.5 Preview UI

- summary KPIs
- tabs/groups by outcome
- warning/error details
- file format/name metadata
- sheet metadata for XLSX
- commit/cancel

### M2.6 Transactional commit

- import batch
- source format metadata
- contact create/link
- submission insert
- normalized product-interest memberships
- data-quality issue creation
- activities
- rollback on failure

### M2.7 Import history

- list batches
- counts/outcome
- file hash/name/format/date
- detail view basic

## Critical tests

### Adapter parity

- equivalent sanitized XLSX and CSV rows produce equivalent canonical `RawSubmissionRow` values;
- CSV quoted fields containing commas parse correctly;
- UTF-8 BOM CSV parses correctly;
- unsupported CSV encoding fails clearly;
- unknown additional columns do not fail schema validation.

### Identity

- same external ID twice in same file;
- same external ID already in DB;
- timezone-equivalent duplicate rows;
- new ID + same e-mail;
- new ID + same phone;
- new ID + e-mail/phone both same contact;
- e-mail points A, phone points B;
- malformed e-mail/phone.

### Product parsing

- legacy unknown product;
- legacy answer mapping to more than one product interest;
- structured single `fue_punches`;
- structured `other_products_/_general_information`;
- structured 2+ selections joined by `|`;
- all six verified machine values in one field;
- `implanters,_forceps_&_surgical_instruments` remains one token despite commas;
- unknown structured machine token produces warning and preserves raw value.

### Source/agency state isolation

- `lead_status=CREATED` is preserved raw but does not set application status;
- agency `Status` does not set application status;
- `İletişime Geçme Tarihi` does not create a follow-up/activity;
- re-import never overwrites CRM status/notes/follow-ups.

### Integrity

- missing optional campaign columns;
- transaction rollback;
- import history persisted with file format.

## Acceptance criteria

- [ ] Sanitized legacy `.xlsx` and `.csv` fixtures parse predictably.
- [ ] Sanitized structured multi-select `.csv` maps every verified selected option to canonical interest rows without dropping values.
- [ ] XLSX adapter feeds the same structured parser and passes single/multi-select integration coverage.
- [ ] CSV and XLSX equivalent source rows produce equivalent canonical values after adapter parsing.
- [ ] Agency `Status` / `İletişime Geçme Tarihi` never mutate CRM state.
- [ ] Re-import creates zero additional submissions for known external IDs.
- [ ] CRM state remains unchanged after re-import.
- [ ] Repeat contact creates/links a second submission, not a second contact, in unambiguous case.
- [ ] Conflicting identifiers never auto-merge.
- [ ] Import failure cannot leave partial batch records.
- [ ] Import history is persisted.
