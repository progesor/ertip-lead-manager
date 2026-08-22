# M3 — Lead Workspace

## Goal

Make the application useful for daily lead review without Excel.

## Implementation status

**Branch:** `feat/m3-lead-workspace`  
**Pull request:** #6  
**Status:** **PASS**

Verified on Windows on 2026-08-22:

- real M2-imported contacts are visible in the lead list;
- search, sorting, product/status filters, dynamic country selector, warning details and platform chips behave correctly;
- lead detail opens from the list and shows every linked submission plus immutable raw Meta values;
- CRM status changes persist and create immutable activity events;
- notes can be created, edited and deleted with audit events;
- manual contact-level product corrections coexist with immutable submission interests and are used consistently by detail/list/filter views;
- product overrides survive re-import by automated integration coverage;
- 10,000 contacts / 25,000 submissions pass the workspace query smoke test;
- schema version 3 adds indexes for the M3 query paths;
- frontend lint/test/build, Windows Rust tests and Windows NSIS debug packaging all pass on the final code head.

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

### Performance and reliability

- schema version 3 query-path indexes
- deterministic single-connection `:memory:` test database helper
- 10k-contact / 25k-submission list/search smoke coverage
- CI concurrency cancels superseded branch/PR runs

## Acceptance criteria

- [x] 10k synthetic contacts remain usable.
- [x] Search returns expected contact by name/e-mail/phone/external ID.
- [x] Combined filters are deterministic.
- [x] Opening a repeat contact shows all submissions.
- [x] Raw values are not editable.
- [x] A lead with multiple product interests displays all interests without forcing a primary category.
- [x] Product filter semantics are deterministic and documented (contains-any for the selected effective product).
- [x] Status persists after restart and re-import architecture keeps CRM state separate.
- [x] Notes persist and are not overwritten by import architecture.
- [x] Manual product-interest override is integrated into the Windows lead-detail workflow and protected by automated re-import coverage.
- [x] Final frontend, Windows Rust and Windows NSIS package gates pass.

## Exit

M3 is complete. Development continues in M4 on lifecycle pipeline, follow-ups and dashboard attention queues.
