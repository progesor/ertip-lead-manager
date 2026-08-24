# M5 — Dashboard and Analytics

## Goal

Turn the local lead/submission history into practical marketing and sales insight without inventing data that V1 does not store.

**Branch:** `feat/m5-analytics-reporting`  
**Issue:** #9  
**Status:** COMPLETE / PASS

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

## Delivered scope

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
- campaign / form / ad set / ad breakdowns with ID + name identity;
- searchable high-cardinality marketing-dimension panel;
- compact 30-day Dashboard analytics summary while keeping attention queues primary;
- visible metric-definition panel;
- Light/Dark theme parity;
- 10k-contact / 25k-submission full analytics smoke coverage.

## Non-goals

- no spend, CPL, ROAS or cost-per-qualified metrics because paid-media spend is not available in V1;
- no historical lifecycle reconstruction unless a later milestone explicitly models status intervals;
- no cloud analytics service in this milestone;
- no mutation of immutable submission/source records.

## Acceptance criteria

- [x] Unique-contact and submission metrics are visibly distinct.
- [x] Date filtering changes acquisition metrics using submission/source date.
- [x] Repeat submission metric excludes exact duplicates by construction.
- [x] Multi-select product submissions contribute to every selected category.
- [x] Current-status funnel denominator is explicit.
- [x] Country/platform/product breakdowns match SQLite test fixtures.
- [x] Campaign/form/ad set/ad breakdowns are implemented.
- [x] Dashboard receives a compact analytics summary without displacing attention queues.
- [x] Common analytics queries remain responsive at 10k contacts / 25k submissions.
- [x] Light and Dark themes remain readable.
- [x] Final frontend, Windows Rust and NSIS package gates pass.

## Final validation

GitHub Actions CI run `32707368482` / run #474 passed all three gates:

- Frontend checks: PASS
- Rust tests (Windows): PASS
- Tauri debug NSIS package (Windows): PASS
