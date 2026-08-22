# M2 — Manual File Import and Deduplication

## Goal

Implement the most critical workflow: safe, explainable, transactional `.xlsx` / `.csv` ingestion.

## Implementation status

**Branch:** `feat/m2-import`  
**Pull request:** #4  
**Status:** **PASS**

Verified on 2026-08-21 / 2026-08-22:

- the post-change Meta multi-select export was inspected and its pipe-delimited serialization was locked into the import contract;
- a real 16-row customer export preview produced 15 importable submissions, 12 new contacts, 3 repeats, 1 exact duplicate, 0 identity conflicts and 0 row errors;
- the same 16-row export was committed successfully on the Windows development PC;
- a newer 21-row export was then committed incrementally and the application ended with 20 contacts, demonstrating real-world repeat/dedup behavior across successive exports;
- import history persisted both committed batches;
- agency `Status` and `İletişime Geçme Tarihi` remained source-only fields and did not become CRM lifecycle/follow-up state;
- frontend lint/test/build passed in CI;
- 48 Rust tests passed on Windows CI;
- Windows NSIS debug package build passed in GitHub Actions.

## Verified source contract

- Legacy free-text export behavior is documented.
- Post-change Meta multi-select export inspected on 2026-08-21.
- Product header remains `which_product_would_you_like_to_receive_more_information_about?`.
- Structured multi-select machine values use `|` as the delimiter.
- Verified machine-value mapping is canonical in `docs/05_EXCEL_IMPORT_CONTRACT.md`.
- Agency columns `Status` and `İletişime Geçme Tarihi` are ignored as CRM inputs.
- Source `lead_status` remains raw source metadata only.
- V1 accepts manual `.xlsx` and `.csv` files.

## Delivered work packages

### M2.1 Source adapters and parser foundation

- Windows file picker allowing `.xlsx` and `.csv`;
- common canonical tabular row boundary;
- XLSX adapter with Calamine;
- CSV adapter with Rust `csv` crate;
- UTF-8 / optional BOM CSV handling;
- XLSX sheet/header discovery;
- CSV header discovery;
- row extraction;
- raw payload preservation;
- unsupported file/encoding errors;
- native/serial Excel timestamp fallback restricted to the canonical `created_time` field.

### M2.2 Header and source-field contract

- required/optional header validation;
- unknown-column tolerance;
- explicit ignore rules for agency `Status` and `İletişime Geçme Tarihi`;
- raw `lead_status` preserved without mapping to CRM lifecycle;
- source identifiers preserved as text.

### M2.3 Normalization and product parsing

- e-mail normalization;
- phone normalization;
- country normalization;
- boolean parsing;
- timestamp normalization;
- legacy product normalization;
- structured product parser that:
  - splits only on `|`;
  - maps verified machine values;
  - supports one or many selections;
  - never comma-splits;
  - emits warnings for unknown structured tokens.

### M2.4 Identity engine

Implemented and tested canonical outcomes:

- exact duplicate submission;
- repeat contact matched by e-mail and/or phone;
- same-batch provisional repeat matching;
- identity conflict requiring review;
- blocking row error;
- name alone never used as an identity key.

### M2.5 Preview UI

- summary KPIs;
- row-level decision badges;
- warning/error details;
- file format/name metadata;
- sheet metadata for XLSX;
- native Windows file selection;
- explicit commit confirmation;
- commit disabled for blocking conflict/error rows.

### M2.6 Transactional commit

- import batch persistence;
- source format metadata;
- contact create/link;
- submission insert;
- normalized multi-value product-interest rows;
- data-quality issue creation;
- activity creation;
- commit-time revalidation;
- full transaction rollback on blocking/failure;
- exact duplicates skipped without duplicating submissions;
- repeat import does not overwrite existing CRM status.

### M2.7 Import history

- recent batch list;
- counts/outcomes;
- file name, persisted source format, worksheet/date and app version;
- committed-batch visibility in the UI.

`file_sha256` remains nullable/optional and is deferred; exact submission identity is enforced by the unique external lead ID, while source rows remain losslessly preserved in `raw_payload_json`.

## Critical test coverage

### Adapter parity

- equivalent sanitized XLSX and CSV rows produce equivalent canonical values;
- CSV quoted fields containing commas parse correctly;
- UTF-8 BOM CSV parses correctly;
- unsupported CSV encoding fails clearly;
- unknown additional columns do not fail schema validation;
- native Excel date serials normalize deterministically when encountered in `created_time`.

### Identity

- same external ID twice in same file;
- same external ID already in DB;
- timezone-equivalent timestamps;
- new ID + same e-mail;
- new ID + same phone;
- new ID + e-mail/phone both same contact;
- e-mail points A, phone points B;
- malformed e-mail/phone;
- same-batch provisional contact linking.

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
- re-import does not overwrite existing CRM status.

### Integrity

- missing optional/unknown columns tolerated;
- transaction rollback;
- import history persisted with source format;
- idempotent re-import of known external IDs;
- incremental later export adds only genuinely new submissions/contacts.

## Acceptance criteria

- [x] Sanitized legacy `.xlsx` and `.csv` fixtures parse predictably.
- [x] Sanitized structured multi-select `.csv` maps every verified selected option to canonical interest rows without dropping values.
- [x] XLSX adapter feeds the same structured parser and passes single/multi-select integration coverage.
- [x] CSV and XLSX equivalent source rows produce equivalent canonical values after adapter parsing.
- [x] Agency `Status` / `İletişime Geçme Tarihi` never mutate CRM state.
- [x] Re-import creates zero additional submissions for known external IDs.
- [x] Existing CRM lifecycle state remains unchanged after re-import.
- [x] Repeat contact creates/links a second submission, not a second contact, in unambiguous cases.
- [x] Conflicting identifiers never auto-merge.
- [x] Import failure cannot leave partial batch records.
- [x] Import history is persisted.
- [x] Real Windows preview and commit flow succeeds against the supplied Meta export.
- [x] Successive 16-row then 21-row real exports import incrementally without duplicating existing contacts.
- [x] Windows frontend, Rust tests and NSIS debug package gate pass in CI.

## Exit

M2 is complete. Development continues in M3 with the Lead Workspace: daily lead list, search/filter/sort, lead detail, linked submissions, lifecycle status changes, notes/activity, product interests and data-quality warnings.
