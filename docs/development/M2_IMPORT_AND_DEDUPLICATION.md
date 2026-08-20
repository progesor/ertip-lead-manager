# M2 — Excel Import and Deduplication

## Goal

Implement the most critical workflow: safe, explainable, transactional `.xlsx` ingestion.

## Work packages

### M2.1 Parser

- file picker
- workbook/sheet open with Calamine
- header discovery
- row extraction
- type coercion handling
- raw payload preservation

### M2.2 Normalization

- e-mail
- phone
- country
- booleans
- timestamps
- legacy product normalization + verified multi-select product parsing

### M2.3 Identity engine

Implement and test decision matrix from canonical docs.

Outputs must explain:

- exact duplicate reason;
- repeat-match identifiers;
- conflict identifiers;
- warnings/errors.

### M2.4 Preview UI

- summary KPIs
- tabs/groups by outcome
- warning/error details
- file/sheet metadata
- commit/cancel

### M2.5 Transactional commit

- import batch
- contact create/link
- submission insert
- data-quality issue creation
- activities
- rollback on failure

### M2.6 Import history

- list batches
- counts/outcome
- file hash/name/date
- detail view basic

## Critical tests

- same external ID twice in same file;
- same external ID already in DB;
- timezone-equivalent duplicate rows;
- new ID + same e-mail;
- new ID + same phone;
- new ID + e-mail/phone both same contact;
- e-mail points A, phone points B;
- malformed e-mail/phone;
- legacy unknown product;
- legacy answer mapping to more than one product interest;
- new structured multi-select with 1 selection;
- new structured multi-select with 2+ selections;
- option labels containing commas must not be naively comma-split;
- missing optional campaign columns;
- unknown additional columns;
- transaction rollback.

## Acceptance criteria

- [ ] Sanitized legacy reference `.xlsx` preview counts exactly as expected.
- [ ] Once post-change export evidence exists, sanitized multi-select `.xlsx` maps every selected option to canonical interest rows without dropping values.
- [ ] Re-import creates zero additional submissions for known external IDs.
- [ ] CRM state remains unchanged after re-import.
- [ ] Repeat contact creates/link a second submission, not second contact, in unambiguous case.
- [ ] Conflicting identifiers never auto-merge.
- [ ] Import failure cannot leave partial batch records.
- [ ] Import history is persisted.
