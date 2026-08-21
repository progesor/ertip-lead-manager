# 05 — Manual File Import Contract

> File name retained for compatibility: `05_EXCEL_IMPORT_CONTRACT.md`. V1 now supports both `.xlsx` and `.csv`.

## 1. Purpose

Defines the supported manual file formats and deterministic import behavior.

The legacy reference export inspected on **2026-08-20** contained 19 columns and demonstrated both exact duplicate external IDs and repeat contacts with new external IDs.

A newer `.xlsx` export inspected on **2026-08-21** contained 21 columns and 16 data rows. It confirmed the deployed Meta multi-select product format and also contained two agency-maintained columns appended after the Meta/source fields.

No real customer PII should be copied into repository fixtures or tests.

## 2. Supported file formats

V1 supports:

- `.xlsx` — parsed read-only with Calamine;
- `.csv` — parsed with the Rust `csv` crate.

Both adapters must produce the same canonical row representation before validation, normalization, identity matching, preview classification, and transactional commit.

### CSV encoding

V1 supports UTF-8 CSV with optional UTF-8 BOM. Unsupported/non-UTF-8 input must fail with a clear file/encoding error rather than silently corrupting names or headers.

Never parse CSV by manually splitting lines or commas. RFC-style quoting must be respected because fields can contain commas.

## 3. Reference headers

The known Meta/source fields are:

```text
id
created_time
ad_id
ad_name
adset_id
adset_name
campaign_id
campaign_name
form_id
form_name
is_organic
platform
do_you_perform_hair_transplant_procedures?
which_product_would_you_like_to_receive_more_information_about?
full_name
email
phone_number
country
lead_status
```

The 2026-08-21 agency-provided workbook also appended:

```text
Status
İletişime Geçme Tarihi
```

These two appended columns are not treated as application CRM fields in V1.

## 4. Required vs optional headers

### Required for a supported import

- `id`
- `created_time`
- `full_name` (header required; value may be blank with warning)
- `email` (header required; value may be blank)
- `phone_number` (header required; value may be blank)

At least one useful identity/contact field should normally exist per row. A row with no useful identity/contact fields is a row error or severe warning according to the final M2 policy.

### Expected but non-blocking if absent

- ad/campaign/adset/form fields
- `is_organic`
- `platform`
- form-answer fields
- `country`
- `lead_status`

Unknown additional columns must not break import. Preserve them in `raw_payload_json` where feasible.

## 5. Header matching

- Trim leading/trailing whitespace.
- Match known machine headers case-insensitively after trim.
- Never fuzzy-match identity-critical fields such as `id` without explicit mapping UI.
- If required headers are missing, block commit and list the missing headers.
- Header order is not canonical; mapping is by header name.

## 6. Agency-maintained columns vs source/application state

### `lead_status`

`lead_status` is a known lower-case source field. Newer Meta rows observed on 2026-08-21 contain values such as `CREATED`.

Rules:

- preserve raw value;
- do not map it to the application's lifecycle status;
- re-import must never overwrite CRM status from `lead_status`.

### `Status`

Capitalized `Status` is an agency-maintained workbook column, not the application lifecycle status.

### `İletişime Geçme Tarihi`

This is also an agency-maintained workbook column.

For both agency fields:

- ignore as V1 CRM inputs;
- do not set application status, activity or follow-up from them;
- preserve in raw payload metadata if present;
- their presence or absence must not affect schema support.

This distinction is intentional to avoid accidentally importing another tool's mutable CRM state into Ertip Lead Manager.

## 7. Row parsing

### External lead ID

Values may have prefixes such as `l:`. Preserve full text exactly.

Normalization for uniqueness: trim surrounding whitespace only unless future evidence requires more.

### IDs

Ad/adset/campaign/form IDs may have textual prefixes (`ag:`, `as:`, `c:`, `f:`). Store as text; never coerce to integers.

### Boolean

`is_organic` may arrive as boolean or text (`true` / `false`). Parse tolerant common forms but preserve raw payload.

### Platform

Known examples: `ig`, `fb`. Normalize for display but preserve raw value. Unknown values remain valid source text.

### Phone

Known exports may prefix values with `p:`. Parsing steps:

1. preserve raw value;
2. trim whitespace;
3. remove only known source prefix `p:` for normalization;
4. normalize obvious punctuation/spaces;
5. retain leading `+` if present;
6. if country/number is insufficient to confidently produce E.164, keep a conservative digits/+ normalized token and raise a warning when appropriate.

Do not invent a country code silently.

### E-mail

- trim whitespace;
- lowercase for identity comparison;
- preserve raw casing/text;
- basic syntactic validation only; do not attempt live mailbox verification.

### Country

Known format is two-letter country code. Normalize uppercase if valid ISO alpha-2. Unknown values remain raw with warning.

### Timestamp

`created_time` may include ISO-8601 UTC offsets.

- Parse the provided offset.
- Convert to UTC for canonical comparisons.
- Preserve exact raw string.
- Two rows with identical external lead ID are duplicates even if timestamp strings differ by timezone representation.

For CSV, timestamp input remains text. For XLSX, the adapter must tolerate text cells and should surface unsupported/unexpected numeric date representation deterministically rather than silently changing source meaning.

## 8. Form version tolerance

Historical form versions used free-text answers in the product field. Examples observed include useful product phrases, generic `yes`, `Information`, question marks, and `All`.

The deployed structured form reuses the **same known header**:

`which_product_would_you_like_to_receive_more_information_about?`

Therefore header alone is not enough to distinguish legacy free text from structured machine values. Product parsing must be value/schema aware and deterministic.

## 9. Verified Meta multi-select serialization

Verified from the real post-change `.xlsx` export on **2026-08-21**.

### Header

Unchanged:

`which_product_would_you_like_to_receive_more_information_about?`

### Single selection examples

```text
fue_punches
other_products_/_general_information
```

### Multiple selection representation

Multiple selected machine values are joined with the pipe delimiter:

```text
fue_micromotor_systems|fue_punches|long_hair_fue_solutions
```

A real row also demonstrated all six values serialized in one cell:

```text
fue_micromotor_systems|other_products_/_general_information|medical_chairs_&_clinic_furniture|implanters,_forceps_&_surgical_instruments|fue_punches|long_hair_fue_solutions
```

### Verified machine-value mapping

| Source machine value | Canonical code |
|---|---|
| `fue_micromotor_systems` | `FUE_MICROMOTOR_SYSTEMS` |
| `long_hair_fue_solutions` | `LONG_HAIR_FUE_SOLUTIONS` |
| `fue_punches` | `FUE_PUNCHES` |
| `implanters,_forceps_&_surgical_instruments` | `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS` |
| `medical_chairs_&_clinic_furniture` | `MEDICAL_CHAIRS_CLINIC_FURNITURE` |
| `other_products_/_general_information` | `OTHER_GENERAL_INFORMATION` |

### Structured parser rules

1. Preserve the exact full raw product cell.
2. If the value is recognized as structured machine-value syntax, split only on `|`.
3. Trim each token.
4. Do not split tokens on commas, underscores, ampersands or `/`.
5. Map each whole token using the verified table above.
6. De-duplicate repeated product tokens within a single submission.
7. Unknown structured tokens produce `UNKNOWN_PRODUCT` warning while preserving the raw token/value.
8. One submission may create zero, one or many `submission_product_interests` rows.

Naive comma splitting is forbidden. The verified implanter/forceps machine value itself contains commas.

## 10. Legacy product normalization

Legacy free-text values under the same header remain supported indefinitely.

Rules:

- preserve raw source answer;
- route clear known phrases through deterministic legacy rules;
- allow one legacy phrase to emit more than one canonical interest where meaning is clearly multi-category;
- ambiguous/non-semantic values remain `UNKNOWN` and create a warning;
- never reinterpret a recognized structured machine token as free text.

## 11. Import outcome states per row

Each preview row has one primary outcome:

- `NEW_CONTACT_NEW_SUBMISSION`
- `REPEAT_CONTACT_NEW_SUBMISSION`
- `EXACT_DUPLICATE_SUBMISSION`
- `IDENTITY_CONFLICT_REVIEW`
- `ROW_ERROR`

Plus zero or more warnings.

## 12. Exact duplicate rule

If `external_lead_id` already exists in DB, the row is an exact duplicate submission.

- Do not insert another submission.
- Do not overwrite previous source values.
- Do not overwrite CRM status/notes/follow-ups.
- Record aggregate duplicate count in the import batch.

If the same file itself contains the same external ID multiple times and the DB does not yet have it, only one canonical submission is inserted; duplicates within the file are surfaced in preview.

## 13. Repeat-contact matching

For a new external ID:

1. Normalize e-mail if usable.
2. Normalize phone if usable.
3. Query existing contacts.
4. If both available and point to same contact => strong repeat match.
5. If one usable identifier matches exactly and the other is blank/non-conflicting => repeat candidate can auto-link according to implementation policy.
6. If e-mail matches contact A and phone matches contact B => identity conflict; do not auto-merge.
7. Name alone never auto-links.

The preview explains why a row is considered repeat.

## 14. File-level duplicate recognition

Compute SHA-256 when practical.

If the exact same file was already committed:

- warn prominently;
- still allow preview;
- rely on external ID uniqueness for data safety.

Do not block solely on identical file hash because users may intentionally inspect/re-import.

## 15. Transactional commit

Commit steps occur in one DB transaction for inserted records and batch metadata.

A crash/failure must not leave half-linked contacts and submissions.

## 16. Reference fixtures

Repository fixtures must remain synthetic/sanitized.

Required fixture coverage for M2:

- legacy CSV fixture;
- legacy XLSX fixture;
- verified-style structured multi-select CSV fixture using `|` serialization;
- parser/unit tests for the same structured values through the XLSX adapter;
- agency-column ignore case (`Status`, `İletişime Geçme Tarihi`);
- exact duplicate external ID;
- repeat contact with a new external ID;
- unknown product and country/phone mismatch cases.

The real customer export inspected on 2026-08-21 is evidence only and must never be committed.
