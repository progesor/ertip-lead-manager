# M4 — Pipeline and Follow-ups

## Goal

Turn stored leads into an actionable sales workflow.

## Implementation status

**Branch:** `feat/m4-pipeline-followups`  
**Issue:** #7  
**Status:** IN PROGRESS

### Current slice

- SQLite-backed pipeline projection grouped by lifecycle status;
- active lifecycle columns shown by default;
- optional WON / LOST / INVALID terminal columns;
- lead cards reuse the effective product-interest rules from M3;
- search, country, product, repeat and warning filters;
- native drag/drop status change;
- accessible per-card status select as non-drag fallback;
- both controls call the existing M3 `change_lead_status` backend command;
- optimistic drag/drop visually rolls back if the backend rejects/fails the update;
- columns are capped at 100 visible cards while retaining the real total count.

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

- [x] Pipeline status update and lead-detail status update use the same backend service.
- [x] Every real status change continues through the M3 service that creates activity.
- [x] Failed drag/drop update visibly rolls back in the pipeline UI implementation.
- [ ] Pipeline board validated on the real Windows development DB.
- [ ] Follow-up persists after restart.
- [ ] Overdue calculation uses current local display time correctly while persisted timestamps are UTC.
- [ ] Dashboard attention groups use the same follow-up/repeat/quality semantics as detail views.
- [ ] Final Windows CI + NSIS packaging gate passes.
