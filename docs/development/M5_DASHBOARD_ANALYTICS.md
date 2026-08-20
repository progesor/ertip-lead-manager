# M5 — Dashboard and Analytics

## Goal

Provide reliable insight without confusing contacts and submissions.

## Deliverables

- dashboard KPI cards
- actionable follow-up/warning cards
- date range selector
- submissions trend
- platform breakdown
- country breakdown
- status breakdown
- product breakdown
- campaign/adset/ad/form tables
- repeat metrics
- analytics filters

## Query rules

- source breakdown charts default to submission counts;
- unique-contact metrics are labeled explicitly;
- conversion formulas follow `08_ANALYTICS_AND_METRICS.md`;
- unknown/missing values are shown, not silently dropped.

## Acceptance criteria

- [ ] Metrics match SQL fixture expectations.
- [ ] Changing date range updates all scoped widgets consistently.
- [ ] Unique contacts and submissions are never labeled interchangeably.
- [ ] Dashboard remains fast on synthetic performance dataset.
