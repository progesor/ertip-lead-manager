# M6 — Coolify Staging Validation Record

## Status

**PASS:** foundation/auth/bootstrap, follow-ups, pipeline/dashboard/analytics, manual imports, additional-user credential lifecycle.

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

## Credential lifecycle staging

A staging-only synthetic SALES identity was used; no real employee credential was required.

Validated:

1. ADMIN created the personnel identity with `revision = 0` and `authEnabled = false`;
2. ADMIN issued a one-time `PROVISION` token;
3. activation enabled authentication and advanced personnel revision to 1;
4. the SALES user could open multiple bearer sessions;
5. self password change retained the current session and revoked the other session (`401`);
6. the old password returned `401` while the replacement password logged in successfully;
7. ADMIN issued a `RESET` token and revision advanced to 2;
8. all target sessions returned `401` after reset initiation;
9. the previously correct password returned `401` while reset was pending;
10. reset-token activation installed a replacement password and advanced revision to 3;
11. final login succeeded as `SALES`;
12. the synthetic personnel identity was deactivated after the test.

No raw invitation/reset tokens or passwords are recorded. This closes the additional-user credential lifecycle across PostgreSQL 17 integration tests and real Coolify staging.

## Credential security properties

The validated implementation uses:

- ADMIN-only 24-hour `PROVISION` / `RESET` tokens;
- SHA-256 token hashes only;
- user-selected Argon2id passwords;
- single-use tokens;
- immediate session revocation on ADMIN reset;
- reset-pending old-password login denial;
- self password change with other-session revocation;
- security-event audit;
- atomic reset/login session issuance under PostgreSQL credential-row locking.

PostgreSQL 17 server suite: **28/28 PASS** for the credential checkpoint.

## Remaining M6

1. PostgreSQL backup/restore recoverability evidence;
2. SQLite-v4 → PostgreSQL migration/reconciliation;
3. secure Tauri token storage before M7 production rollout.

Coolify auto-deploy stays OFF and PR #15 stays draft/open until all gates pass.
