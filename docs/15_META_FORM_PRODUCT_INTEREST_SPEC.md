# 15 — Meta Form Product-Interest Specification

**Decision date:** 2026-08-20  
**Status:** Accepted product decision; Meta form update pending/being applied  
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

A prospect may select any useful combination. Examples:

- Micromotor + FUE Punches
- Long Hair FUE Solutions + Micromotor + FUE Punches
- Implanters/Forceps + Furniture

The application must preserve all selected categories. There is no required primary product in V1.

## 6. Optional free-text detail field

If the form allows an additional optional text question, recommended wording is:

**Is there a specific product, model or information you are looking for?**  
*Optional*

This field is supplementary detail only. It must not replace or override the structured product-interest multi-select.

## 7. Legacy compatibility

Historical exports use the known free-text header:

`which_product_would_you_like_to_receive_more_information_about?`

Examples include `micromotor`, `Long hair micro motor`, `Hair grafts`, `All`, `yes`, `Information`, and non-semantic question marks. These historical rows remain valid source records.

Legacy normalization is deterministic and conservative. A legacy phrase may map to more than one canonical category when that meaning is clear. Raw source text is never changed.

## 8. Pending verification after form deployment

The following must be captured from the first real Excel export produced by the updated Meta form:

- exact machine/header name of the new multi-select question;
- exact representation of one selected value;
- exact representation of two or more selected values;
- escaping/quoting behavior;
- behavior when `Other Products / General Information` is selected;
- whether the optional free-text detail field receives its own stable header.

**Do not assume comma-separated values.** One canonical label contains commas (`Implanters, Forceps & Surgical Instruments`), so naive comma splitting is explicitly prohibited.

Once verified, update `docs/05_EXCEL_IMPORT_CONTRACT.md` and add a sanitized multi-select `.xlsx` fixture before finalizing M2 parsing logic.

## 9. Analytics semantics

Product categories are non-mutually-exclusive. If a submission selects three categories, it contributes one membership to each of those three categories. As a result, product-category totals can exceed total submissions. UI labels/tooltips must make this clear.
