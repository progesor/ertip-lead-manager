# 10 — Test Strategy

## 1. Testing goals

Most important risks:

1. importing the wrong rows;
2. duplicate insertion;
3. incorrect contact merges;
4. losing CRM state on re-import;
5. broken timestamp/phone/e-mail normalization;
6. analytics counts that mix contacts and submissions incorrectly;
7. database migration/data-loss defects.

Tests should concentrate on these before visual details.

## 2. Test layers

### Rust unit tests

High priority pure functions:

- header mapping
- boolean parsing
- timestamp parsing and UTC conversion
- e-mail normalization
- phone normalization
- country normalization
- legacy product normalization and multi-select product parsing
- identity decision matrix
- import row classification

### Rust integration tests

Using temporary SQLite DB:

- migrations apply from empty DB;
- external lead ID unique constraint;
- first import creates contact/submission;
- exact duplicate import inserts nothing new;
- new external ID + same e-mail links repeat contact;
- new external ID + same phone links repeat contact;
- e-mail/phone conflict does not auto-merge;
- re-import does not change status/note/follow-up;
- import transaction rolls back on forced failure;
- analytics query counts known fixtures correctly;
- backup/restore round trip.

### Frontend unit/component tests

- filter controls serialize correct query;
- status badge/control states;
- import preview summary;
- warning rendering;
- lead detail renders multiple submissions;
- follow-up due/overdue states;
- error and empty states.

### End-to-end / smoke

At minimum manual release smoke checklist; automated Tauri E2E may be added when foundation is stable.

## 3. Canonical identity decision tests

| Existing e-mail | Existing phone | Expected |
|---|---|---|
| none | none | new contact |
| A | none | link A |
| none | A | link A |
| A | A | link A |
| A | B | identity conflict; no auto-merge |
| name match only | none | new contact |

Also test missing identifiers and malformed values.

## 4. Timestamp duplicate test

Fixture should include two rows with the same `external_lead_id` where:

```text
2026-08-20T04:37:27-05:00
2026-08-20T12:37:27+03:00
```

They represent the same instant but the key reason they are duplicates is the external ID. Preview should classify one insertion and one duplicate if neither existed previously.

## 5. Repeat submission test

Two rows:

- different external IDs;
- same normalized e-mail and/or phone;
- different submission dates.

Expected: one contact, two submissions.

## 6. Product-interest tests

### Legacy free-text normalization

Seed examples:

- `Long hair micro motor` → may yield both `LONG_HAIR_FUE_SOLUTIONS` and `FUE_MICROMOTOR_SYSTEMS`
- `micromotor` → `FUE_MICROMOTOR_SYSTEMS`
- `FUE punch` → `FUE_PUNCHES`
- `forceps` → `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS`
- `medical chair` → `MEDICAL_CHAIRS_CLINIC_FURNITURE`
- `All` → `OTHER_GENERAL_INFORMATION` only when rule confidence is intentional; raw value remains available
- `yes`, `Information`, `?????` → `UNKNOWN` unless stronger context exists

Unknown/generic values must not be confidently mislabeled.

### New structured multi-select

After the first real post-change Excel export is available:

- add a sanitized fixture using the exact exported header and cell serialization;
- test one selected option;
- test two or more selected options;
- test all six options if the source permits it;
- test unknown future option text;
- assert no interest is dropped or duplicated;
- assert a multi-select row creates multiple canonical memberships;
- assert raw source cell text remains unchanged;
- specifically test that commas inside `Implanters, Forceps & Surgical Instruments` are not treated as an assumed delimiter.

## 7. Import regression fixtures

Repository fixtures must be synthetic/sanitized and small enough to understand manually.

`fixtures/leads_sample_sanitized.csv` is the first canonical **legacy-schema** fixture. An `.xlsx` equivalent may be generated for parser integration testing. A separate post-change multi-select fixture must be added only after the exact Meta export format has been observed.

## 8. Performance tests

Generate synthetic datasets for:

- 10k contacts
- 25k submissions

Measure:

- list query/filter response
- dashboard metrics
- import preview of 1k/5k rows

Do not use production PII for performance tests.

## 9. Release smoke checklist

Before internal release:

- clean install on Windows x64;
- app starts without network;
- import sanitized `.xlsx`;
- preview counts correct;
- commit succeeds;
- re-import produces duplicates not new rows;
- repeat contact is linked;
- status/note/follow-up persist after restart;
- filters/search work;
- dashboard counts match fixture;
- backup created;
- restore tested;
- app uninstalls without silently deleting user data unless installer explicitly says so.

## 10. Definition of test completeness

A feature is not complete if its business-critical failure path has no test. Snapshot-heavy UI tests are lower priority than domain and persistence correctness.
