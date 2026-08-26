# M6 — Coolify Staging Validation Record

## Status

**STAGING FOUNDATION / AUTH: PASS**

Validation date: 2026-08-26  
Environment: Coolify staging  
Public API hostname: `lead-api-staging.progesor.net`  
Database: private PostgreSQL 17  
Source branch: `feat/m6-central-backend-foundation`

This record intentionally excludes passwords, raw session tokens, database connection strings, personal e-mail addresses and other secrets/PII.

## Validated deployment path

```text
Cloudflare / HTTPS
        ↓
lead-api-staging.progesor.net
        ↓
Coolify rolling deployment
        ↓
Rust / Axum API container
        ↓
private PostgreSQL 17
```

## Deployment / health evidence

The staging application was deployed from the M6 branch through `server/Dockerfile`.

Validated:

- rolling update started a new container;
- Dockerfile custom healthcheck was detected by Coolify;
- `/health/ready` passed after the configured start period;
- readiness response identified `ertip-lead-manager-server` version `0.1.0` with status `ready`;
- new container became healthy before the previous container was removed;
- HTTPS `/health/ready` succeeds through the staging hostname;
- readiness includes a real PostgreSQL dependency check, therefore this validates API → PostgreSQL connectivity as well as process liveness.

## Bootstrap ADMIN validation

First empty-database deployment used the temporary bootstrap ADMIN environment contract.

Validated:

1. initial ADMIN identity was created in PostgreSQL;
2. Tauri login over HTTPS succeeded;
3. returned authenticated user had role `ADMIN`;
4. opaque bearer session authenticated `GET /api/v1/me`;
5. logout returned HTTP `204`;
6. the logged-out bearer token subsequently returned HTTP `401`;
7. all bootstrap ADMIN environment variables were removed from Coolify;
8. the API was redeployed without bootstrap secrets;
9. the container returned healthy after redeploy;
10. the existing ADMIN could still log in successfully after redeploy.

This proves the bootstrap variables are initial-provisioning material only. The persisted PostgreSQL user/credential remains authoritative after the bootstrap secrets are removed.

## Secret hygiene

Current staging policy:

- `DATABASE_URL` remains runtime-only;
- bootstrap ADMIN name/e-mail/password variables are no longer present after first-account validation;
- real database credentials, login passwords and bearer tokens are not committed;
- PostgreSQL remains private/internal to Coolify;
- the installed frozen local Tauri application is not pointed at staging during M6.

## Remaining staging validation

Foundation/auth staging validation is complete. M6 still requires staged validation of later API slices, including:

- personnel and role-policy smoke tests with representative users;
- assignment/status operations;
- notes and product-interest overrides;
- follow-up API after its HTTP slice passes CI and is redeployed;
- stale-revision conflict behavior;
- SALES assigned-only visibility/mutation behavior;
- pipeline/dashboard/analytics and import parity when implemented;
- PostgreSQL backup/restore evidence;
- SQLite schema-v4 → PostgreSQL migration/reconciliation evidence.

Passing this checkpoint does not close M6 or authorize switching the production Tauri client to API mode. That remains an M7 action after the complete M6 acceptance gate.
