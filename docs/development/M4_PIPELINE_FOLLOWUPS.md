# M4 — Pipeline and Follow-ups

## Goal

Turn stored leads into an actionable sales workflow.

## Deliverables

### Pipeline

- lifecycle columns
- lead cards
- drag/drop status change
- non-drag status control for accessibility
- filters
- optional hide/show terminal statuses

### Follow-ups

- create follow-up
- due date/time
- optional short note
- complete
- reschedule
- cancel
- due today / overdue filters

### Dashboard attention area

- uncontacted NEW leads
- due today
- overdue
- recent repeat submissions
- open identity/data-quality issues

## Acceptance criteria

- [ ] Pipeline status update and lead-detail status update use the same backend service.
- [ ] Every status change creates activity.
- [ ] Failed drag/drop update visibly rolls back.
- [ ] Follow-up persists after restart.
- [ ] Overdue calculation uses current local display time correctly while persisted timestamps are UTC.
