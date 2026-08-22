# M3 — Lead Workspace

## Goal

Make the application useful for daily lead review without Excel.

## Implementation status

**Branch:** `feat/m3-lead-workspace`  
**Pull request:** #6  
**Status:** IN PROGRESS

Locally validated on Windows so far:

- real M2-imported contacts are visible in the lead list;
- search, filters, dynamic country selector, warning details and platform chips behave correctly;
- read-only lead detail opens from the list;
- CRM status changes persist and appear in activity history;
- notes can be created, edited and deleted with audit events.

## Deliverables

### Leads list

- backend query with pagination
- search by name/e-mail/phone/external lead ID
- deterministic sorting
- status/product/country filters
- searchable country options derived from actual DB values
- repeat/warning quick filters
- platform chips
- repeat/warning indicators
- contact-level effective product interests

### Lead detail

- contact overview
- all submissions
- read-only raw source values
- current status
- effective multi-value product-interest view
- automatic source product interests remain immutable
- manual product ADD/REMOVE overrides stored separately and survive re-import
- notes
- activity timeline
- data-quality issues

### Lifecycle

- status change
- immutable `STATUS_CHANGED` activity
- status filtering

### Notes

- add/edit/delete
- timestamps
- activity metadata without copying note body into activity payload

### Product correction

- source submission interests remain immutable
- contact-level manual overrides are append-only
- latest override per product determines the manual decision
- effective product interests are automatic interests with latest ADD/REMOVE overrides applied
- list display and product filter use the same effective-interest rule as lead detail
- every manual change creates `PRODUCT_INTEREST_CHANGED` activity

## Acceptance criteria

- [ ] 10k synthetic contacts remain usable.
- [x] Search returns expected contact by name/e-mail/phone/external ID.
- [x] Combined filters are deterministic.
- [x] Opening a repeat contact shows all submissions.
- [x] Raw values are not editable.
- [x] A lead with multiple product interests displays all interests without forcing a primary category.
- [x] Product filter semantics are deterministic and documented (contains-any for the selected effective product).
- [x] Status persists after restart and re-import architecture keeps CRM state separate.
- [x] Notes persist and are not overwritten by import architecture.
- [ ] Manual product-interest override validated on the Windows development DB.
- [ ] Final Windows CI + NSIS packaging gate passes after all M3 changes.

## Remaining before M3 PASS

1. Validate manual product add/remove in the real Windows lead detail UI and confirm list/filter reflection.
2. Pass the 10k-contact / 25k-submission automated workspace smoke test.
3. Run final frontend, Windows Rust and NSIS package gates.
4. Mark PR #6 ready and squash merge to `main`.
