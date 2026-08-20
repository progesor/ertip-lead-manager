# ADR 0003 — Submission identity and conservative contact grouping

**Status:** Accepted for V1 bootstrap

## Context

The reference spreadsheet demonstrated two distinct realities:

1. the same external lead ID can appear more than once;
2. the same person can submit again and receive a different external lead ID.

A flat table cannot represent both safely.

## Decision

- `external_lead_id` uniquely identifies a submission.
- Contacts and submissions are separate entities.
- Duplicate external ID => no new submission.
- New external ID with consistent exact normalized e-mail/phone match => repeat submission linked to existing contact.
- Name alone never merges.
- Conflicting identity signals require review.

## Consequences

- Analytics can correctly distinguish unique contacts vs submissions.
- Repeat interest remains visible.
- Import logic is more complex but significantly safer.
