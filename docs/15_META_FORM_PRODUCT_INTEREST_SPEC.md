# 15 — Meta Form Product-Interest Specification

**Decision date:** 2026-08-20  
**Export verification date:** 2026-08-21  
**Status:** Accepted and verified against a real post-change Meta lead export  
**Scope:** Product-interest question used by the lead form and the importer's canonical mapping

## 1. Goal

Replace the low-quality free-text product question with structured data while still allowing a prospect to express interest in more than one product group.

## 2. Canonical question

Recommended English wording:

**Which products are you interested in?**  
*Select all that apply.*

Field type: **multi-select / multiple selection**.

## 3. Canonical customer-facing options

| Stable internal code | Form label |
|---|---|
| `FUE_MICROMOTOR_SYSTEMS` | FUE Micromotor Systems |
| `LONG_HAIR_FUE_SOLUTIONS` | Long Hair FUE Solutions |
| `FUE_PUNCHES` | FUE Punches |
| `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS` | Implanters, Forceps & Surgical Instruments |
| `MEDICAL_CHAIRS_CLINIC_FURNITURE` | Medical Chairs & Clinic Furniture |
| `OTHER_GENERAL_INFORMATION` | Other Products / General Information |

The application uses the stable internal codes. Display labels may be localized later without changing stored codes.

## 4. Internal-only state

`UNKNOWN` is allowed internally for legacy or malformed input. It must **not** be offered as a customer-facing form choice.

## 5. Multi-select semantics

A prospect may select any useful combination. The application preserves all selected categories. There is no required primary product in V1.

## 6. Verified export representation

The first real post-change `.xlsx` export was inspected on **2026-08-21**.

### Header behavior

The form change did **not** create a new product-question header. The export still uses:

`which_product_would_you_like_to_receive_more_information_about?`

This means legacy free-text and new structured answers coexist under the same source header.

### Structured machine values

| Form label | Verified source machine value | Canonical code |
|---|---|---|
| FUE Micromotor Systems | `fue_micromotor_systems` | `FUE_MICROMOTOR_SYSTEMS` |
| Long Hair FUE Solutions | `long_hair_fue_solutions` | `LONG_HAIR_FUE_SOLUTIONS` |
| FUE Punches | `fue_punches` | `FUE_PUNCHES` |
| Implanters, Forceps & Surgical Instruments | `implanters,_forceps_&_surgical_instruments` | `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS` |
| Medical Chairs & Clinic Furniture | `medical_chairs_&_clinic_furniture` | `MEDICAL_CHAIRS_CLINIC_FURNITURE` |
| Other Products / General Information | `other_products_/_general_information` | `OTHER_GENERAL_INFORMATION` |

### Multiple-selection serialization

Multiple selected values are joined using the pipe character:

`|`

Example:

```text
fue_micromotor_systems|fue_punches|long_hair_fue_solutions
```

A real exported row demonstrated all six selections in one value, confirming that pipe separation is stable across several tokens.

### Important delimiter rule

Do **not** comma-split product answers. The machine value:

`implanters,_forceps_&_surgical_instruments`

contains commas inside a single valid selection.

Structured product parsing therefore splits only on `|`, then maps each complete token.

## 7. Optional free-text detail field

If the form later adds an additional optional text question, recommended wording remains:

**Is there a specific product, model or information you are looking for?**  
*Optional*

This field is supplementary detail only. It must not replace or override the structured product-interest multi-select.

The 2026-08-21 export used for verification did not establish a separate optional-detail header, so no such field is canonical yet.

## 8. Legacy compatibility

Historical exports use the same product header and include values such as `micromotor`, `Long hair micro motor`, `Hair grafts`, `All`, `yes`, `Information`, and non-semantic question marks. These historical rows remain valid source records.

Legacy normalization is deterministic and conservative. A legacy phrase may map to more than one canonical category when that meaning is clear. Raw source text is never changed.

The importer must distinguish recognized structured machine tokens from legacy free text by value rules, not by header alone.

## 9. Analytics semantics

Product categories are non-mutually-exclusive. If a submission selects three categories, it contributes one membership to each of those three categories. As a result, product-category totals can exceed total submissions. UI labels/tooltips must make this clear.
