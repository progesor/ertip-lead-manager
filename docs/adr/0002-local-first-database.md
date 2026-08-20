# ADR 0002 — Local-first SQLite database

**Status:** Accepted for V1 bootstrap  
**Decision:** SQLite via SQLx, stored in application data directory

## Context

V1 is single-user and must work offline. A server DB would add deployment/auth/network complexity with little value at this stage.

## Decision

Persist canonical contacts, submissions, CRM state, and import history in local SQLite. Use versioned migrations and transactional imports.

## Consequences

Positive:

- zero server infrastructure;
- fast local queries;
- simple backup artifact;
- offline operation.

Costs:

- no concurrent multi-user access;
- cloud/multi-device later requires architectural work;
- backup responsibility must be explicit.
