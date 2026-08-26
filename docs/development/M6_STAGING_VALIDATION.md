# M6 — Coolify Staging Validation Record

## Status

**PASS:** foundation/auth/bootstrap, follow-ups, pipeline/dashboard/analytics, manual imports.  
**CODE + CI PASS / STAGING PENDING:** additional-user credential lifecycle.

Environment: `lead-api-staging.progesor.net` on Coolify + private PostgreSQL 17.  
Source branch: `feat/m6-central-backend-foundation`.

This evidence excludes passwords, connection strings, raw bearer tokens, raw invitation/reset tokens and real customer PII.

## Live staging evidence

Validated:

- HTTPS API → private PostgreSQL readiness;
- rolling deploy and custom `/health/ready`;
- first ADMIN bootstrap and later bootstrap-secret removal;
- bearer login, `/me`, logout 204 and revoked-token 401;
- follow-up create/list/reschedule/stale-409/complete;
- pipeline all eight statuses with `perColumnLimit=100`;
- analytics expected zero-submission result with eight funnel buckets;
- dashboard expected synthetic-lead KPI/attention result.

## Manual import staging

A six-row synthetic CSV produced 5 importable submissions, 4 new contacts, 1 repeat and 1 exact duplicate. First commit recorded five submissions and a committed batch. Re-submitting the identical file produced 0 importable submissions and 6 exact duplicates while recording a second committed batch. Identity conflicts, row errors and warnings remained zero. Manual import idempotency/history are PASS.

## Credential lifecycle pre-staging

Implemented and PostgreSQL-CI validated:

- ADMIN-only 24-hour `PROVISION` / `RESET` tokens;
- SHA-256 token hashes only;
- user-selected Argon2id activation password;
- single-use tokens;
- self password change retaining current session and revoking all others;
- ADMIN reset blocking old-password login and revoking all target sessions;
- reset activation with replacement password;
- security-event audit;
- atomic reset/login session gate under credential-row locking.

PostgreSQL 17 server suite: **28/28 PASS**. Credential lifecycle is the next real staging gate.

## Remaining M6

1. credential lifecycle staging smoke;
2. PostgreSQL backup/restore evidence;
3. SQLite-v4 → PostgreSQL migration/reconciliation;
4. secure Tauri token storage before M7 production rollout.

Coolify auto-deploy stays OFF and PR #15 stays draft/open until all gates pass.
