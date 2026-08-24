# M4 — Pipeline and Follow-ups

## Goal

Turn stored leads into a production-ready daily sales workflow centered on **Dashboard + Kanban**, with Lead Detail as the operational workspace and Lead List as a secondary search/review surface.

## Implementation status

**Branch:** `feat/m4-pipeline-followups`  
**Issue:** #7  
**Pull request:** #8  
**Status:** IN PROGRESS

## Daily-use hierarchy

1. **Genel Bakış / Dashboard** — attention-first work queue.
2. **Pipeline / Kanban** — primary lifecycle/status workspace.
3. **Lead Detail** — notes, follow-ups, product correction and customer context.
4. **Leadler** — secondary query/list screen for broad search and auditing.

## Implemented

### Pipeline / Kanban

- SQLite-backed projection grouped by lifecycle status;
- NEW / CONTACTED / REPLIED / QUALIFIED / QUOTE_SENT active columns;
- optional WON / LOST / INVALID terminal columns;
- effective product interests, platform, repeat, quality-warning and follow-up context on cards;
- search, country, product, repeat and warning filters;
- **Gecikmiş** and **Bugün Takip** quick filters are calculated in SQLite, not client-side;
- full-card pointer-based drag interaction tuned for Tauri/WebView2;
- floating mouse-attached drag preview, source placeholder fade and target-column highlight;
- normal click opens Lead Detail; drag threshold prevents accidental navigation;
- status dropdown removed from cards: Kanban itself is the primary status control;
- Kanban and Lead Detail both call the same M3 `change_lead_status` backend command;
- failed status mutation rolls the optimistic move back;
- current Kanban filters are preserved when opening a lead and returning;
- columns retain real counts when display limits apply.

### Follow-ups

- existing `follow_ups` schema reused; source/import data remains untouched;
- create follow-up with date/time and optional note;
- reschedule, complete and cancel OPEN follow-ups;
- all mutations are transactional and write immutable activity events;
- incoming RFC3339 timestamps normalize to canonical UTC milliseconds in Rust;
- UI edits dates in local time and converts only at the application boundary;
- overdue / due-today / upcoming labels use the current local display time;
- completed/cancelled follow-ups remain available in history;
- Pipeline cards show earliest OPEN follow-up and open-follow-up count.

### Dashboard / attention workspace

- real SQLite KPI values: total, NEW, QUALIFIED, QUOTE_SENT and WON;
- overdue follow-ups;
- remaining follow-ups due today;
- NEW / uncontacted leads;
- recent repeat submissions;
- open data-quality issues;
- rows open Lead Detail and return context points back to Dashboard;
- direct Pipeline action from Dashboard.

### Lead Detail production workspace

- context-aware return behavior: Pipeline → Pipeline, Dashboard → Dashboard, Lead List → Lead List;
- compact customer identity/status hero;
- main **2/3 operational column** for product interests, data-quality warnings, follow-ups and CRM notes;
- right **1/3 sticky background panel** for non-primary information;
- right panel tabs: **Aktivite / Submission / Kaynak**;
- audit history removed from the main vertical flow;
- submission history removed from the main vertical flow;
- Meta campaign/adset/ad/form IDs and raw payload remain available without dominating the screen;
- manual product corrections remain separated from immutable source interests;
- Lead Detail can refresh after follow-up mutations without breaking return context.

## Acceptance criteria

- [x] Pipeline status update and Lead Detail status update use the same backend service.
- [x] Every real status change creates activity through the shared M3 service.
- [x] Failed Kanban mutation visibly rolls back.
- [x] Follow-up CRUD persists in SQLite and is audited by backend tests.
- [x] Backend canonicalizes follow-up timestamps to UTC; UI converts at local-time boundaries.
- [x] Dashboard attention groups use the same follow-up/repeat/quality semantics as the rest of the app.
- [x] Pipeline due/overdue quick filters use backend query windows and real totals.
- [x] Lead Detail technical history is separated from the primary operational workflow.
- [ ] Floating Kanban drag preview validated on the real Windows development DB.
- [ ] Dashboard attention/follow-up workflow validated on the real Windows development DB.
- [ ] New production Lead Detail layout validated on the real Windows development DB.
- [ ] Final Windows Rust + NSIS packaging gate passes on the final M4 head.

## Remaining before M4 PASS

1. Real Windows UX validation of Dashboard, Kanban drag preview and context-aware navigation.
2. Real Windows UX validation of the new 2/3 + 1/3 Lead Detail workspace.
3. Real follow-up create/reschedule/complete/cancel smoke test.
4. Final frontend + Windows Rust + NSIS package gate.
5. Mark PR #8 ready and squash merge to `main`.
