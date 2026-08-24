# 11 — Roadmap

Milestones are sequential. Keep scope controlled, but the target deployment model changed after M5 based on real operational demand for personnel ownership and multi-user use.

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
- CI build/test baseline.

## M2 — Excel import and identity

Goal: safely turn spreadsheet exports into canonical DB records.

Deliverables:

- `.xlsx` / `.csv` file picker;
- header detection;
- parser/normalizers, including legacy free-text and verified multi-select product schemas;
- preview;
- duplicate/repeat/conflict classification;
- transactional commit;
- import history;
- data-quality issue generation;
- comprehensive domain tests.

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
- full-card drag/drop lifecycle changes;
- follow-up create/reschedule/complete/cancel;
- due/overdue views;
- dashboard attention widgets;
- production Lead Detail workspace;
- persistent Light/Dark theme.

## M5 — Dashboard and analytics

Goal: practical marketing/sales insight.

Status: **COMPLETE**.

Deliverables:

- KPI dashboard;
- date filters;
- trend charts;
- platform/country/campaign/form/ad set/ad/multi-select product-interest breakdowns;
- repeat submission metrics;
- current-status funnel;
- clear unique-contact vs submission definitions;
- 10k-contact / 25k-submission analytics validation.

## M5.5 — Team assignment and multi-user readiness

Goal: introduce personnel ownership without creating a dead-end local-only data model.

Deliverables:

- persistent `app_users` personnel records;
- active/inactive lifecycle and roles;
- current lead assignee;
- Lead Detail assignment controls;
- Kanban assignee display and assignee/unassigned filtering;
- immutable assignment audit;
- nullable future audit actor identity;
- stable user IDs and future auth-subject binding;
- centralized architecture decision documented.

Local SQLite remains the implementation/test database for this milestone. Authentication is not part of M5.5.

## M6 — Centralized backend, PostgreSQL and authentication

Goal: establish the authoritative multi-user backend on Coolify while preserving the existing CRM domain rules.

Target topology:

```text
Tauri Windows ─┐
               ├── HTTPS API + Auth ── private PostgreSQL
Future Web ────┘
```

Deliverables:

- backend HTTP API with explicit versioned contracts;
- PostgreSQL persistence and migrations;
- PostgreSQL kept on Coolify private/internal network;
- authentication/session model;
- `app_users.auth_subject` binding;
- server-derived current-user / `actor_user_id` context;
- ADMIN / MANAGER / SALES authorization rules enforced server-side;
- optimistic concurrency / lost-update protection for mutable CRM state;
- server-side validation equivalent to current local service rules;
- health/readiness endpoints and production logging;
- backup/restore strategy for centralized PostgreSQL;
- SQLite → PostgreSQL migration + reconciliation tooling;
- migration validation preserving stable IDs and immutable source/audit history.

Non-negotiable: Windows/Web clients do **not** receive PostgreSQL credentials and do not connect directly to PostgreSQL.

## M7 — Online multi-user Windows release

Goal: turn the existing Tauri application into the first real multi-user production client.

Deliverables:

- login/logout/session UX;
- Tauri API client layer replacing local persistence calls in production mode;
- current-user and role-aware UI;
- audit actor displayed from authenticated backend data;
- staff assignment against centralized state;
- concurrent-edit conflict UX;
- network/loading/retry/offline-error states;
- secure configuration and API endpoint handling;
- production installer/update strategy;
- internal release checklist;
- centralized backup/recovery test;
- real multi-PC acceptance test.

Local SQLite may remain available for development, migration tests and explicit offline/demo tooling, but it is not the authoritative production database in multi-user mode.

## M8 — Web App

Goal: provide browser access without duplicating CRM business rules.

Deliverables:

- web client using the same M6 backend API/auth contract;
- Dashboard, Pipeline, Lead Detail, personnel assignment and analytics parity for core workflows;
- responsive desktop/tablet layout;
- role-aware navigation and permissions;
- deployment through Coolify;
- shared validation/error semantics with the Windows client.

The Web App must not introduce a second persistence/business-logic implementation.

## Post-online candidates

After real multi-user usage:

- saved filters/views;
- bulk status/assignment actions;
- tags;
- configurable product normalization rules;
- CSV export of filtered leads;
- optional quote/sale value fields;
- contact merge/review tool;
- notifications and personal work queues;
- assignment workload balancing.

## Integration candidates

Only after the centralized API/data model is stable:

- Meta Lead Ads API;
- Meta Ads spend/performance;
- Google Sheet sync if still operationally useful;
- Odoo sales data;
- qualified lead / won-sale attribution;
- CPL / cost per qualified / ROAS;
- WhatsApp or e-mail workflow integrations.

Manual `.xlsx` / `.csv` import remains a supported ingestion/fallback workflow even after centralized deployment.

## Non-roadmap rule

Do not add an integration merely because it is technically easy. Add it when real usage proves the need and the canonical identity/source/audit model can support it without bypassing backend authorization or immutable source-data rules.
