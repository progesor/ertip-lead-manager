# 05 — Excel Import Contract

## 1. Purpose

Defines the supported manual Excel import format and deterministic import behavior.

The reference export inspected on **2026-08-20** contained 19 columns and 13 data rows. It included 12 unique external lead IDs, demonstrating that the same lead ID can appear twice with equivalent timestamps represented in different UTC offsets. The sample also contained contacts who submitted more than once using different external lead IDs.

No real customer PII should be copied into repository fixtures or tests.

## 2. Reference headers

The current known export contains these headers:

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

## 3. Required vs optional headers

### Required for a supported import

- `id`
- `created_time`
- `full_name` (header required; value may be blank with warning)
- `email` (header required; value may be blank)
- `phone_number` (header required; value may be blank)

At least one contact method or name should normally exist per row; a row with no useful identity/contact fields may be imported only if policy explicitly permits it. Initial implementation should classify it as a row error or severe warning and surface it.

### Expected but non-blocking if absent in future exports

- ad/campaign/adset/form fields
- `is_organic`
- `platform`
- form-answer fields
- `country`
- `lead_status`

Unknown additional columns must not break import. Preserve them in `raw_payload_json` where feasible.

## 4. Header matching

- Trim leading/trailing whitespace.
- Compare known machine headers case-sensitively or case-insensitively consistently; recommended: case-insensitive exact after trim.
- Never fuzzy-match identity-critical fields such as `id` without explicit mapping UI.
- If required headers are missing, block commit and show which headers are missing.

## 5. Row parsing

### External lead ID

Current values may have prefixes such as `l:`. Preserve full text exactly.

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

### Lead status

`lead_status` from the export is source metadata only. It must **not** overwrite the application lifecycle status on later imports.

## 6. Form version tolerance

The inspected sample contained at least two form names/IDs with different answer conventions. Import logic must not assume all form versions produce semantically clean answers.

Example product-answer quality classes observed in the sample:

- useful product phrases such as micromotor/grafts/long-hair related text;
- generic affirmative text such as `yes`;
- generic `Information`;
- question marks / non-semantic content;
- `All`.

Therefore raw form answer and normalized product-interest assignments are separate concepts. Historical free-text data must remain importable after the form changes.

## 7. Product-question schema evolution (legacy → multi-select)

The Meta form is being changed from a free-text product question to a **multi-select** product-interest question. This decision is canonical even though the first updated Excel export has not yet been observed.

Customer-facing options:

1. `FUE Micromotor Systems` → `FUE_MICROMOTOR_SYSTEMS`
2. `Long Hair FUE Solutions` → `LONG_HAIR_FUE_SOLUTIONS`
3. `FUE Punches` → `FUE_PUNCHES`
4. `Implanters, Forceps & Surgical Instruments` → `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS`
5. `Medical Chairs & Clinic Furniture` → `MEDICAL_CHAIRS_CLINIC_FURNITURE`
6. `Other Products / General Information` → `OTHER_GENERAL_INFORMATION`

The old known header remains:

`which_product_would_you_like_to_receive_more_information_about?`

The exact machine header generated by the new question and the exact serialization of multiple selected values are **TBD until the first real post-change export is inspected**. Do not hard-code an assumed delimiter. In particular, one option label itself contains commas, so naive comma splitting is forbidden.

Importer strategy:

- maintain a registry of supported product-question header aliases/versions;
- preserve the exact raw cell value regardless of version;
- route legacy free text through deterministic legacy normalization;
- route the verified new multi-select representation through a dedicated parser;
- map every selected option independently to a canonical product code;
- allow one submission to yield multiple normalized product-interest rows;
- unknown/unmapped values create an `UNKNOWN_PRODUCT` warning rather than being silently dropped;
- once the first new export is available, add a sanitized regression fixture and record its header/serialization here before or alongside importer implementation.

The optional free-text detail question, if retained in Meta, should be treated as a separate source answer and must not replace the structured multi-select field.

## 8. Import outcome states per row

Each preview row should be classified into one primary outcome:

- `NEW_CONTACT_NEW_SUBMISSION`
- `REPEAT_CONTACT_NEW_SUBMISSION`
- `EXACT_DUPLICATE_SUBMISSION`
- `IDENTITY_CONFLICT_REVIEW`
- `ROW_ERROR`

Plus zero or more warnings.

## 9. Exact duplicate rule

If `external_lead_id` already exists in DB, the row is an exact duplicate submission.

- Do not insert another submission.
- Do not overwrite previous source values.
- Do not overwrite CRM status/notes/follow-ups.
- Record aggregate duplicate count in the import batch.

If the same import file itself contains the same external ID multiple times and the DB does not yet have it, only one canonical submission should be inserted; duplicates within the file should be surfaced in preview.

## 10. Repeat-contact matching

For a new external ID:

1. Normalize e-mail if usable.
2. Normalize phone if usable.
3. Query existing contacts.
4. If both available and point to same contact => strong repeat match.
5. If one usable identifier matches exactly and the other is blank/non-conflicting => repeat candidate can auto-link according to implementation policy.
6. If e-mail matches contact A and phone matches contact B => identity conflict; do not auto-merge.
7. Name alone never auto-links.

The preview should explain why a row is considered repeat.

## 11. File-level duplicate recognition

Optionally compute SHA-256 for the selected file.

If the exact same file was already committed:

- warn prominently;
- still allow preview;
- rely on external ID uniqueness for data safety.

Do not block solely on identical file hash because users may intentionally inspect/re-import.

## 12. Transactional commit

Commit steps must occur in one DB transaction for inserted records and batch metadata.

A crash/failure must not leave half-linked contacts and submissions.

## 13. Reference fixtures

Use `fixtures/leads_sample_sanitized.csv` for development tests. It intentionally contains:

- an exact duplicate external ID represented in another timezone;
- a repeat contact with a new external ID;
- an unknown product answer;
- a country/phone mismatch warning case.

The current fixture is intentionally **legacy-schema**. Do not fabricate a multi-select Excel fixture until the real post-change export format is observed. After that export arrives, create a second sanitized fixture containing at least one row with two or more selected product interests.
