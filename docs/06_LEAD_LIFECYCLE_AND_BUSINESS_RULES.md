# 06 — Lead Lifecycle and Business Rules

## 1. Contact vs submission

The UI centers on a lead contact, while the detail view exposes all submissions.

A repeat form submission is useful behavior, not noise. It can indicate renewed interest and should be visible in the timeline.

## 2. Lifecycle statuses

| Status | Meaning |
|---|---|
| `NEW` | Imported and not yet intentionally contacted |
| `CONTACTED` | Outbound contact attempt made |
| `REPLIED` | Prospect responded |
| `QUALIFIED` | Prospect is considered commercially relevant/valid |
| `QUOTE_SENT` | Price/quotation was sent |
| `WON` | Lead converted to a sale/business outcome |
| `LOST` | Lead is no longer actively pursued / lost |
| `INVALID` | Spam, invalid contact, irrelevant, duplicate person intentionally rejected, etc. |

## 3. Status transition policy

Do not hard-block most transitions. Real sales work can move backward or skip steps.

Examples allowed:

- NEW → QUALIFIED
- CONTACTED → QUOTE_SENT
- LOST → CONTACTED (reopened)
- WON → CONTACTED (rare but possible; activity should show it)

Every status change creates an immutable activity event.

## 4. Default status

A newly created contact defaults to `NEW` regardless of the raw exported `lead_status` field.

A new submission linked to an existing contact does **not** reset the existing contact's status automatically. Instead, add a `SUBMISSION_IMPORTED` activity and optionally mark the contact as having a new repeat submission for attention.

## 5. Repeat submission indicators

A contact is considered a repeat contact when `submission_count > 1`.

Useful derived labels:

- `Repeat · 2 submissions`
- `Latest submission today`
- `New repeat submission since last contact` (future refinement)

## 6. Notes

- Notes are CRM data and editable.
- Keep notes concise and chronological.
- Editing/deleting a note produces an activity metadata event.
- Do not copy all note text into the activity payload unnecessarily.

## 7. Follow-ups

A follow-up is a time-based work item attached to a contact.

States:

- `OPEN`
- `COMPLETED`
- `CANCELLED`

Dashboard groups:

- Overdue
- Due today
- Upcoming

If multiple open follow-ups exist, show the earliest as primary and indicate the count.

## 8. Product interests

Product interest is a **set**, not a scalar. One submission/contact may have several product interests simultaneously.

Canonical customer-facing categories:

| Code | Label |
|---|---|
| `FUE_MICROMOTOR_SYSTEMS` | FUE Micromotor Systems |
| `LONG_HAIR_FUE_SOLUTIONS` | Long Hair FUE Solutions |
| `FUE_PUNCHES` | FUE Punches |
| `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS` | Implanters, Forceps & Surgical Instruments |
| `MEDICAL_CHAIRS_CLINIC_FURNITURE` | Medical Chairs & Clinic Furniture |
| `OTHER_GENERAL_INFORMATION` | Other Products / General Information |

`UNKNOWN` is an internal state for ambiguous historical input and is not a form option.

The structured Meta question should be multi-select: **“Which products are you interested in? / Select all that apply.”**

A lead choosing both Micromotor and FUE Punches must retain both interests. There is no canonical “primary product” in V1.

### 8.1 Automatic vs effective contact interests

Submission-level product interests are source-derived data and are immutable after import.

The contact workspace exposes an **effective product-interest set** for CRM use:

1. union all canonical interests from every linked submission;
2. find the latest manual contact override for each product code;
3. latest `ADD` forces that product into the effective set;
4. latest `REMOVE` forces that product out of the effective set;
5. a product with no manual override follows the immutable source-derived union.

Manual overrides are append-only records in `contact_product_interest_overrides`. Re-import never deletes or rewrites them. Each change creates a `PRODUCT_INTEREST_CHANGED` activity event containing product code and include/remove direction, while the original source submission values remain unchanged.

Lead-list product chips and product filters use the same effective-interest rule as lead detail. This prevents list/detail disagreement after a manual correction.

## 9. Legacy product normalization examples

Legacy free-text answers remain supported. Rule concepts include:

```text
contains "micro motor" / "micromotor" → FUE_MICROMOTOR_SYSTEMS
contains "long hair"                   → LONG_HAIR_FUE_SOLUTIONS
contains "punch"                       → FUE_PUNCHES
contains "implanter" / "forceps"      → IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS
contains "chair" / furniture terms    → MEDICAL_CHAIRS_CLINIC_FURNITURE
exact/clear "all products" intent      → OTHER_GENERAL_INFORMATION (preserve raw text)
"yes", "information", "?"           → UNKNOWN unless stronger evidence exists
```

If a legacy answer clearly names more than one category, the normalizer may assign multiple canonical interests. Priorities matter when phrases overlap; for example, `long hair micro motor` may legitimately map to both `LONG_HAIR_FUE_SOLUTIONS` and `FUE_MICROMOTOR_SYSTEMS` rather than forcing one winner. Preserve the raw answer always.

## 10. Data-quality warnings

Warnings are advisory unless they create identity ambiguity or prevent safe parsing.

### `COUNTRY_PHONE_MISMATCH`

A country field and phone prefix appear inconsistent. Do not “fix” either value automatically.

### `INVALID_EMAIL`

Malformed syntactic e-mail.

### `INVALID_PHONE`

No usable phone token can be produced.

### `UNKNOWN_PRODUCT`

Answer cannot be mapped reliably.

### `IDENTITY_CONFLICT`

Normalized identifiers point to different contacts. Requires manual resolution before auto-linking.

### `MISSING_CONTACT_METHOD`

No valid e-mail or phone; may still be usable if name/source information exists, but should be visible.

A manual product correction does not rewrite or delete the original source-quality issue. Issue resolution/dismissal is a separate review decision so the application retains the fact that the original source value was ambiguous.

## 11. Manual identity resolution

V1 may initially keep ambiguous submissions unlinked in a review queue or create a standalone contact flagged for review. Whichever implementation is selected in M2 must avoid destructive merging.

A future merge operation should:

- choose destination contact;
- move/link submissions and CRM child records safely;
- preserve an audit event;
- support undo if feasible.

Contact merge is not required for the earliest V1 milestones.

## 12. Won / lost metadata

First V1 may only require status. If added, optional fields can include:

- lost reason
- sale/quote value
- currency
- won date

Do not add financial complexity before the core lifecycle is stable.
