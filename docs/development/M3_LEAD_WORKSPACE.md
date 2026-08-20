# M3 — Lead Workspace

## Goal

Make the application useful for daily lead review without Excel.

## Deliverables

### Leads list

- backend query with pagination/limit-offset or cursor strategy
- search
- sorting
- common filters
- advanced marketing filters
- repeat/warning/follow-up indicators

### Lead detail

- contact overview
- all submissions
- read-only raw source values
- current status
- multi-value product-interest chips + manual add/remove/correction
- notes
- activity timeline
- data-quality issues

### Lifecycle

- status change
- activity creation
- status filtering

### Notes

- add/edit/delete
- timestamps
- activity metadata

## Acceptance criteria

- [ ] 10k synthetic contacts remain usable.
- [ ] Search returns expected contact by name/e-mail/phone/external ID.
- [ ] Combined filters are deterministic.
- [ ] Opening a repeat contact shows all submissions.
- [ ] Raw values are not editable.
- [ ] A lead with multiple product interests displays all interests without forcing a primary category.
- [ ] Product filter semantics are deterministic and documented (contains-any by default).
- [ ] Status persists after restart and re-import.
- [ ] Notes persist and are not overwritten by import.
