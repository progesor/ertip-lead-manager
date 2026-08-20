# ADR 0004 — Model Product Interest as Multi-Select

**Status:** Accepted  
**Date:** 2026-08-20

## Context

The original Meta lead export used a free-text product question. Answers were inconsistent and difficult to analyze. The lead form is being updated to offer six structured product groups, and prospects may reasonably be interested in several groups at once.

## Decision

- Product interest is many-valued.
- Use the six stable canonical codes defined in `docs/15_META_FORM_PRODUCT_INTEREST_SPEC.md`.
- Keep `UNKNOWN` as an internal legacy/ambiguity state only.
- Preserve raw legacy answers.
- Support legacy free-text and new multi-select imports in the same database.
- Store normalized submission product interests relationally rather than in one `normalized_product` column.
- Do not guess the future Excel serialization; verify it from the first real post-change export.

## Consequences

Positive:

- more faithful customer intent;
- cleaner marketing analytics;
- product combinations can be analyzed;
- historical data remains usable.

Costs:

- data model and filters require many-to-many semantics;
- analytics category totals are not mutually exclusive;
- importer needs schema/version-aware product parsing.
