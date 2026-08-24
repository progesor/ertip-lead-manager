# M5 — Dashboard and Analytics

## Goal

Turn the local lead/submission history into practical marketing and sales insight without inventing data that V1 does not store.

**Branch:** `feat/m5-analytics-reporting`  
**Issue:** #9  
**Status:** IN PROGRESS

## Metric contract

### Submission

A row in `lead_submissions` with a unique `external_lead_id`. Exact duplicate imports are rejected before insertion and therefore are not counted as submissions.

### Unique contact

A distinct `lead_contact_id` that has at least one submission inside the selected analytics date window.

### Repeat submission

A real submission for a contact that already has an earlier real submission. This is intentionally independent of exact duplicates.

### Analytics date

Acquisition/trend analytics use `source_created_at_utc`; `created_at` is only a fallback for historical rows with no canonical source timestamp. Date filters are half-open UTC windows `[from, to)` generated from local calendar-day boundaries in the UI.

### Current-status funnel

The funnel/distribution uses the **current CRM status** of contacts that had at least one submission in the selected date window. It is not a historical status-at-submission reconstruction. Every percentage must show or describe its denominator.

### Product-interest breakdown

Product analytics use normalized **submission-level source interests**, not contact-level manual CRM overrides. A multi-select submission contributes once to every selected product category, so product-category totals can exceed total submission count.

## M5.1 — Core analytics slice

- date presets: 7 / 30 / 90 days / all + custom local calendar dates;
- explicit submission vs unique-contact KPIs;
- repeat submission count/rate;
- submissions-per-contact ratio;
- current WON count/rate with explicit unique-contact denominator;
- daily submission + unique-contact trend;
- current-status funnel/distribution;
- country breakdown;
- platform breakdown;
- multi-select product-interest breakdown;
- visible metric-definition panel;
- Light/Dark theme via existing application tokens.

## Remaining M5 scope

1. Campaign breakdown.
2. Form breakdown.
3. Ad set breakdown.
4. Ad breakdown.
5. Ranking/search behavior for high-cardinality dimensions.
6. Small analytics summary on Dashboard while keeping Dashboard attention-first.
7. 10k/25k analytics performance validation.
8. Final Windows Rust + frontend + NSIS gate.

## Non-goals

- no spend, CPL, ROAS or cost-per-qualified metrics because paid-media spend is not available in V1;
- no historical lifecycle reconstruction unless a later milestone explicitly models status intervals;
- no cloud analytics service;
- no mutation of immutable submission/source records.

## Acceptance criteria

- [ ] Unique-contact and submission metrics are visibly distinct.
- [ ] Date filtering changes acquisition metrics using submission/source date.
- [ ] Repeat submission metric excludes exact duplicates by construction.
- [ ] Multi-select product submissions contribute to every selected category.
- [ ] Current-status funnel denominator is explicit.
- [ ] Country/platform/product breakdowns match SQLite test fixtures.
- [ ] Campaign/form/ad set/ad breakdowns are implemented.
- [ ] Dashboard receives a compact analytics summary without displacing attention queues.
- [ ] Common analytics queries remain responsive at 10k contacts / 25k submissions.
- [ ] Light and Dark themes remain readable.
- [ ] Final frontend, Windows Rust and NSIS package gates pass.
