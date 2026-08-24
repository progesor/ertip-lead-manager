# M4 — Pipeline and Follow-ups

## Goal

Turn stored leads into a production-ready daily sales workflow centered on **Dashboard + Kanban**, with Lead Detail as the operational workspace and Lead List as a secondary search/review surface.

## Implementation status

**Issue:** #7  
**Pull request:** #8  
**Status:** **PASS — COMPLETE**

## Daily-use hierarchy

1. **Genel Bakış / Dashboard** — attention-first work queue.
2. **Pipeline / Kanban** — primary lifecycle/status workspace.
3. **Lead Detail** — notes, follow-ups, product correction and customer context.
4. **Leadler** — secondary query/list screen for broad search and auditing.

## Delivered

### Pipeline / Kanban

- SQLite-backed projection grouped by lifecycle status;
- NEW / CONTACTED / REPLIED / QUALIFIED / QUOTE_SENT active columns;
- optional WON / LOST / INVALID terminal columns;
- effective product interests, platform, repeat, quality-warning and follow-up context on cards;
- phone and country promoted as primary customer context, with e-mail secondary;
- search, country, product, repeat and warning filters;
- **Gecikmiş** and **Bugün Takip** quick filters calculated in SQLite with real totals;
- full-card pointer interaction tuned for Tauri/WebView2;
- floating mouse-attached drag preview, source placeholder fade and target-column highlight;
- normal click opens Lead Detail; drag threshold prevents accidental navigation;
- Kanban and Lead Detail use the same audited M3 `change_lead_status` backend command;
- failed status mutations roll optimistic moves back;
- Kanban filter state survives Lead Detail round-trips.

### Follow-ups

- create follow-up with date/time and optional note;
- reschedule, complete and cancel OPEN follow-ups;
- all mutations transactional with immutable activity events;
- RFC3339 timestamps normalize to canonical UTC milliseconds in Rust;
- UI edits local date/time and converts at the application boundary;
- overdue / due-today / upcoming semantics;
- completed/cancelled follow-ups remain in history;
- Pipeline cards show earliest OPEN follow-up and open-follow-up count.

### Dashboard / attention workspace

- real SQLite KPI values: total, NEW, QUALIFIED, QUOTE_SENT and WON;
- overdue follow-ups;
- remaining follow-ups due today;
- NEW / uncontacted leads;
- recent repeat submissions;
- open data-quality issues;
- phone + country shown directly in attention rows;
- rows open Lead Detail and return to Dashboard;
- direct Pipeline action.

### Lead Detail production workspace

- context-aware return: Pipeline → Pipeline, Dashboard → Dashboard, Lead List → Lead List;
- compact customer identity/status hero;
- main **2/3 operational column** for product interests, data-quality warnings, follow-ups and CRM notes;
- right **1/3 sticky background panel**;
- tabs: **Aktivite / Submission / Kaynak**;
- audit/submission/raw Meta data removed from the primary vertical workflow but kept fully accessible;
- manual product corrections stay separated from immutable source interests.

### Readability and theme

- daily-use typography increased across Dashboard, Pipeline, Lead Detail, Lead List, import and settings screens;
- persistent Light / Dark theme with top-bar toggle;
- theme preference survives restart and first run follows system preference;
- theme is applied before React render to avoid light-theme flash;
- dark-theme contrast audit covers Dashboard headings, Lead names, tables, forms, Kanban, follow-ups, Lead Detail history/submissions/source, import empty state and badges;
- final import empty-state heading (`Manuel lead dosyası seçin`) uses theme-safe contrast.

## Acceptance / final smoke

- [x] Pipeline status and Lead Detail status use the same backend service.
- [x] Real status changes create activity through the shared M3 service.
- [x] Failed Kanban mutation rolls back.
- [x] Full-card pointer drag + floating preview validated through the Windows UX review loop.
- [x] Kanban → Lead Detail → Kanban return context and filters validated.
- [x] Dashboard → Lead Detail → Dashboard return context validated.
- [x] Follow-up CRUD persists in SQLite and is covered by backend integration tests.
- [x] UTC/local-time follow-up boundary behavior is covered by backend and UI workflow tests.
- [x] Dashboard attention groups use shared follow-up/repeat/quality semantics.
- [x] Pipeline due/overdue quick filters use backend query windows and real totals.
- [x] Production 2/3 + 1/3 Lead Detail layout validated through iterative Windows UX review.
- [x] Light/Dark theme and readability reviewed interactively; final leaked contrast states fixed.
- [x] Existing import/dedup incremental workflow remains intact.
- [x] 10k contacts / 25k submissions performance smoke remains green.
- [x] Final frontend lint + unit tests + production build PASS.
- [x] Final Windows Rust suite PASS.
- [x] Final Windows Tauri debug NSIS package PASS.

## Final validation reference

Final code candidate before documentation closeout: `17d04c3388538b0b7ea45dde364fa6f3b3fc86dd`.

GitHub Actions run `32703217989`:

- Frontend checks — **PASS**
- Rust tests (Windows) — **PASS**
- Tauri debug package (Windows / NSIS) — **PASS**

M4 is complete and ready for squash merge to `main`.
