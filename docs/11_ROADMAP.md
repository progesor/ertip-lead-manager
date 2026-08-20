# 11 — Roadmap

Milestones are sequential. Keep V1 narrow.

## M0 — Repository and canon

Goal: establish source of truth and development conventions.

Deliverables:

- documentation package committed;
- repository created;
- branch strategy selected;
- issue/project workflow optional;
- technology assumptions validated;
- sanitized fixture accepted.

Gate: `docs/development/M0_DISCOVERY_CHECKLIST.md` complete.

## M1 — Desktop foundation

Goal: runnable Windows Tauri application with database foundation.

Deliverables:

- Tauri 2 + React + TypeScript app;
- app shell/sidebar/routes;
- design tokens/basic components;
- SQLite connection;
- migrations;
- typed command pattern;
- settings/data directory diagnostics;
- CI build/test baseline if practical.

No real import workflow yet beyond skeleton.

## M2 — Excel import and identity

Goal: safely turn spreadsheet exports into canonical DB records.

Deliverables:

- `.xlsx` file picker;
- header detection;
- parser/normalizers, including legacy free-text and verified multi-select product schemas;
- preview;
- duplicate/repeat/conflict classification;
- transactional commit;
- import history;
- data-quality issue generation;
- comprehensive domain tests.

This is the highest-risk milestone.

## M3 — Lead workspace

Goal: replace Excel for daily lead review.

Deliverables:

- leads table;
- search/filter/sort;
- lead detail;
- all linked submissions;
- status changes;
- notes/activity timeline;
- multi-value product-interest display and manual correction;
- warning display.

## M4 — Pipeline and follow-ups

Goal: make the application operational for sales follow-up.

Deliverables:

- Kanban pipeline;
- drag/drop + accessible status change;
- follow-up create/reschedule/complete;
- due/overdue views;
- dashboard attention widgets.

## M5 — Dashboard and analytics

Goal: practical marketing/sales insight.

Deliverables:

- KPI dashboard;
- date filters;
- trend charts;
- platform/country/campaign/form/multi-select product-interest breakdowns;
- repeat submission metrics;
- current-status funnel;
- clear unique-contact vs submission definitions.

## M6 — Data safety and internal release

Goal: reliable internal Windows release.

Deliverables:

- backup/restore;
- performance validation;
- error/logging polish;
- empty/loading/error states;
- installer configuration;
- internal release checklist;
- documentation update.

## V1.1 candidates

After real usage:

- saved filters/views;
- bulk status actions;
- tags;
- configurable product normalization rules;
- CSV export of filtered leads;
- optional quote/sale value fields;
- contact merge/review tool.

## V1.5 — Google Sheet sync candidate

Only after manual import is stable:

- Google authentication/connection strategy;
- incremental sync cursor/state;
- same canonical import pipeline adapter;
- sync status and conflict logs.

Manual Excel import remains as fallback.

## V2 — Paid media / business integrations

Candidates:

- Meta Lead Ads API
- Meta Ads spend/performance
- Odoo sales data
- qualified lead / won-sale attribution
- CPL / cost per qualified / ROAS

## Non-roadmap rule

Do not add an integration merely because it is technically easy. Add it when manual V1 usage proves the need and the canonical data model can support it without bypassing import/identity rules.
