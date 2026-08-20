# 04 — Data Model

## 1. Core model

The model intentionally separates a **contact** from one or more **source submissions**.

```text
lead_contacts 1 ────── * lead_submissions
      │                         │
      │                         └──── * submission_product_interests
      │
      ├──────── * lead_notes
      ├──────── * lead_activities
      ├──────── * follow_ups
      ├──────── * contact_product_interest_overrides
      └──────── * lead_data_quality_issues

import_batches 1 ───── * lead_submissions
```

## 2. `lead_contacts`

Represents the application-level prospect/contact.

Suggested fields:

| Field | Type | Notes |
|---|---|---|
| `id` | text UUID | primary key |
| `display_name` | text nullable | application display name |
| `primary_email` | text nullable | current normalized/display value |
| `normalized_email` | text nullable | indexed identity candidate |
| `primary_phone` | text nullable | user-friendly display value |
| `normalized_phone` | text nullable | indexed identity candidate |
| `country_code` | text nullable | normalized ISO alpha-2 where possible |
| `status` | text enum | canonical lifecycle status |
| `created_at` | UTC timestamp | local record creation |
| `updated_at` | UTC timestamp | last app-managed update |
| `latest_submission_at` | UTC timestamp nullable | denormalized for fast sorting; update transactionally |
| `submission_count` | integer | optional denormalized count; may instead be queried |

Do not treat display name as an identity key.

## 3. `lead_submissions`

One row per unique external Meta lead ID.

| Field | Type | Notes |
|---|---|---|
| `id` | text UUID | local primary key |
| `lead_contact_id` | FK | linked contact |
| `import_batch_id` | FK | originating import |
| `external_lead_id` | text UNIQUE | canonical exact-duplicate key |
| `source_created_at_utc` | timestamp nullable | parsed canonical time |
| `source_created_at_raw` | text | exact imported timestamp |
| `ad_id` | text nullable | raw external ID |
| `ad_name` | text nullable | raw name |
| `adset_id` | text nullable | raw external ID |
| `adset_name` | text nullable | raw name |
| `campaign_id` | text nullable | raw external ID |
| `campaign_name` | text nullable | raw name |
| `form_id` | text nullable | raw external ID |
| `form_name` | text nullable | raw name |
| `is_organic` | boolean nullable | parsed source value |
| `platform` | text nullable | e.g. `ig`, `fb` |
| `raw_procedure_answer` | text nullable | source answer |
| `raw_product_answer` | text nullable | exact source representation; legacy free text or future multi-select serialization |
| `raw_full_name` | text nullable | source value |
| `raw_email` | text nullable | source value |
| `raw_phone` | text nullable | source value |
| `raw_country` | text nullable | source value |
| `raw_lead_status` | text nullable | source value, not app lifecycle |
| `normalized_email` | text nullable | import-derived helper |
| `normalized_phone` | text nullable | import-derived helper |
| `raw_payload_json` | text JSON | lossless mapped row payload / future fields |
| `created_at` | UTC timestamp | insertion time |

`raw_*` fields must not be modified by normal CRM operations.

## 4. Product-interest relations

Product interest is many-to-many. Do **not** add a single `normalized_product` column back to `lead_contacts` or `lead_submissions`.

### 4.1 `submission_product_interests`

Normalized interests derived from a specific imported submission.

| Field | Type | Notes |
|---|---|---|
| `id` | UUID | primary key |
| `lead_submission_id` | FK | source submission |
| `product_code` | text enum | one of the six canonical codes, or `UNKNOWN` where needed for legacy data |
| `origin` | enum | `DIRECT_MULTI_SELECT`, `LEGACY_NORMALIZED`, or similar stable origin |
| `confidence` | enum nullable | optional `HIGH`/`LOW`; useful for legacy rules only |
| `created_at` | timestamp | UTC |

Recommended uniqueness: `(lead_submission_id, product_code)`.

A new multi-select form submission can therefore create several rows here. Raw source text remains on `lead_submissions`.

### 4.2 `contact_product_interest_overrides`

Application-managed corrections/additions at contact level. This is separate from imported source evidence.

| Field | Type | Notes |
|---|---|---|
| `id` | UUID | primary key |
| `lead_contact_id` | FK | contact |
| `product_code` | text enum | canonical code |
| `action` | enum | `ADD` or `REMOVE` |
| `created_at` | timestamp | UTC |

The effective contact interest set may be derived as the union of normalized submission interests plus explicit contact-level additions minus explicit removals. Implementation may simplify the override mechanism in the first UI milestone, but it must preserve the distinction between source-derived and user-managed data.

### 4.3 Canonical product codes

- `FUE_MICROMOTOR_SYSTEMS`
- `LONG_HAIR_FUE_SOLUTIONS`
- `FUE_PUNCHES`
- `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS`
- `MEDICAL_CHAIRS_CLINIC_FURNITURE`
- `OTHER_GENERAL_INFORMATION`
- `UNKNOWN` (internal only)

## 5. `import_batches`

| Field | Type | Notes |
|---|---|---|
| `id` | UUID | primary key |
| `file_name` | text | original base name |
| `file_size` | integer nullable | bytes |
| `file_sha256` | text nullable | useful for same-file recognition |
| `sheet_name` | text | imported worksheet |
| `started_at` | timestamp | UTC |
| `completed_at` | timestamp nullable | UTC |
| `status` | enum | PREVIEWED/COMMITTED/FAILED/CANCELLED as needed |
| `total_rows` | integer | parsed data rows |
| `new_submissions` | integer | committed/preview count |
| `exact_duplicates` | integer | same external ID |
| `repeat_candidates` | integer | linked/candidate repeats |
| `warning_count` | integer | non-blocking issues |
| `error_count` | integer | blocking/row failures |
| `app_version` | text | diagnostics |

## 6. `lead_notes`

| Field | Type |
|---|---|
| `id` | UUID |
| `lead_contact_id` | FK |
| `body` | text |
| `created_at` | timestamp |
| `updated_at` | timestamp |

V1 single-user means no required author entity. A future user ID can be added through migration.

## 7. `lead_activities`

Immutable/audit-oriented events.

Suggested `activity_type` values:

- `LEAD_CREATED`
- `SUBMISSION_IMPORTED`
- `STATUS_CHANGED`
- `NOTE_ADDED`
- `NOTE_EDITED`
- `NOTE_DELETED`
- `FOLLOW_UP_SET`
- `FOLLOW_UP_COMPLETED`
- `FOLLOW_UP_RESCHEDULED`
- `PRODUCT_CLASSIFIED`
- `IDENTITY_LINKED`
- `IDENTITY_UNLINKED`

Fields:

| Field | Type |
|---|---|
| `id` | UUID |
| `lead_contact_id` | FK |
| `activity_type` | text enum |
| `occurred_at` | UTC timestamp |
| `payload_json` | JSON text |

Do not store full sensitive source payloads redundantly in activities.

## 8. `follow_ups`

Prefer history-preserving follow-up rows rather than a single mutable date if implementation cost is acceptable.

| Field | Type |
|---|---|
| `id` | UUID |
| `lead_contact_id` | FK |
| `due_at` | UTC timestamp |
| `status` | `OPEN`, `COMPLETED`, `CANCELLED` |
| `note` | text nullable |
| `created_at` | timestamp |
| `completed_at` | timestamp nullable |

For dashboard purposes, the nearest open due follow-up is the actionable one.

## 9. `lead_data_quality_issues`

| Field | Type |
|---|---|
| `id` | UUID |
| `lead_contact_id` | FK nullable |
| `lead_submission_id` | FK nullable |
| `issue_type` | enum/text |
| `severity` | INFO/WARNING/ERROR |
| `details_json` | JSON text |
| `status` | OPEN/DISMISSED/RESOLVED |
| `created_at` | timestamp |
| `resolved_at` | timestamp nullable |

Potential issue types:

- `INVALID_EMAIL`
- `INVALID_PHONE`
- `COUNTRY_PHONE_MISMATCH`
- `UNKNOWN_PRODUCT`
- `IDENTITY_CONFLICT`
- `MISSING_CONTACT_METHOD`
- `INVALID_TIMESTAMP`

## 10. Normalization rules table (optional V1.x)

`product_normalization_rules`

- `id`
- `pattern`
- `match_type` (`EXACT`, `CONTAINS`, possibly `REGEX` later)
- `product_code`
- `priority`
- `enabled`

A legacy free-text answer may emit one or more canonical product codes when the text clearly contains multiple interests. Initial V1 may seed rules in code/config and migrate to table management later.

## 11. Indexes

At minimum evaluate indexes for:

- `lead_submissions.external_lead_id` UNIQUE
- `lead_submissions.normalized_email`
- `lead_submissions.normalized_phone`
- `lead_submissions.source_created_at_utc`
- `lead_contacts.normalized_email`
- `lead_contacts.normalized_phone`
- `lead_contacts.status`
- `lead_contacts.latest_submission_at`
- `submission_product_interests(product_code, lead_submission_id)`
- `contact_product_interest_overrides(lead_contact_id, product_code)`
- `follow_ups(status, due_at)`
- marketing dimensions used frequently in analytics

## 12. Deletion policy

V1 should avoid hard-deleting contacts/submissions through normal UI.

- Source submissions: never hard-delete through normal workflow.
- Contacts: archive/hide can be added if needed.
- Notes: user deletion is allowed but should create an activity entry.
- Import rollback after commit is intentionally not a casual operation; if provided, it must protect CRM data and linked submissions.
