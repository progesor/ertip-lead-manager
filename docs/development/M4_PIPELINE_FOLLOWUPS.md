# M4 — Pipeline and Follow-ups

## Goal

Turn stored leads into an actionable sales workflow.

## Implementation status

**Branch:** `feat/m4-pipeline-followups`  
**Issue:** #7  
**Pull request:** #8  
**Status:** IN PROGRESS

### Pipeline slice

- SQLite-backed pipeline projection grouped by lifecycle status;
- active lifecycle columns shown by default;
- optional WON / LOST / INVALID terminal columns;
- lead cards reuse the effective product-interest rules from M3;
- search, country, product, repeat and warning filters;
- native drag/drop status change;
- accessible per-card status select as non-drag fallback;
- both controls call the existing M3 `change_lead_status` backend command;
- optimistic drag/drop visually rolls back if the backend rejects/fails the update;
- columns are capped at 100 visible cards while retaining the real total count;
- each card shows the earliest OPEN follow-up and count when available.

### Follow-up slice

- existing `follow_ups` schema reused; source/import data remains untouched;
- create follow-up with due date/time and optional short note;
- reschedule OPEN follow-up;
- complete OPEN follow-up;
- cancel OPEN follow-up;
- all mutations run transactionally and write immutable activity events;
- incoming RFC3339 timestamps are normalized to canonical UTC milliseconds in Rust;
- lead-detail follow-up panel edits dates in browser local time and sends UTC to backend;
- overdue / due-today / upcoming labels are derived from current local display time;
- completed/cancelled follow-ups remain visible in expandable history;
- follow-up mutations remount/reload lead detail so activity history stays current.

## Deliverables

### Pipeline

- lifecycle columns
- lead cards
- drag/drop status change
- non-drag status control for accessibility
- filters
- optional hide/show terminal statuses
- next-follow-up context on cards

### Follow-ups

- create follow-up
- due date/time
- optional short note
- complete
- reschedule
- cancel
- due today / overdue semantics

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
- [x] Follow-up CRUD is persisted in SQLite and audited by backend tests.
- [ ] Follow-up create/reschedule/complete/cancel validated on the real Windows development DB.
- [x] Backend canonicalizes follow-up timestamps to UTC; UI converts at local-time boundaries.
- [ ] Dashboard attention groups use the same follow-up/repeat/quality semantics as detail views.
- [ ] Final Windows CI + NSIS packaging gate passes.

## Next

1. Validate pipeline drag/drop/status selector and lead-detail follow-up panel on the real Windows DB.
2. Add due-today / overdue attention filters and dashboard groups.
3. Complete final M4 Windows/CI/package acceptance.
