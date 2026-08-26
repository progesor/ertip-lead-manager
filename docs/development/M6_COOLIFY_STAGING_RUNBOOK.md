# M6 — Coolify Staging Deployment Runbook

## Purpose

Deploy the M6 centralized backend to a real Coolify staging environment before M7 switches any production Tauri client to API mode.

This environment is for server/PostgreSQL/auth/API validation only. The frozen `v0.1.0-local` Windows application remains independent.

## Source

- Repository: `https://github.com/progesor/ertip-lead-manager`
- Branch: `feat/m6-central-backend-foundation`
- Build pack: Dockerfile
- Base directory / build context: repository root `/`
- Dockerfile location: `server/Dockerfile`
- Internal application port: `8080`
- Readiness endpoint: `/health/ready`
- Liveness endpoint: `/health/live`

Do not deploy `main` for M6 staging while PR #15 remains draft.

## 1. Create PostgreSQL resource

In the same Coolify project/environment/destination that will host the API:

1. Create a PostgreSQL database resource.
2. Select PostgreSQL **17**.
3. Use a dedicated database/user for Ertip Lead Manager if Coolify asks for them.
4. Start the database and wait for healthy status.
5. Keep public database exposure disabled.
6. Copy the Coolify **Internal URL** after the database is running.

The application `DATABASE_URL` must use this private/internal URL. Do not expose PostgreSQL port 5432 to the internet for normal application use.

## 2. Create API application

Create a new application from the GitHub repository.

Configuration:

```text
Repository: https://github.com/progesor/ertip-lead-manager
Branch: feat/m6-central-backend-foundation
Build Pack: Dockerfile
Base Directory: /
Dockerfile Location: server/Dockerfile
Ports Exposes: 8080
```

The process listens on `0.0.0.0:8080` inside the container.

The Docker image defines a dependency-aware health check against:

```text
http://127.0.0.1:8080/health/ready
```

Therefore the application will be unhealthy if PostgreSQL is unavailable even when the process itself is still running.

## 3. Runtime environment variables

Set these as runtime environment variables in Coolify. Secrets should not be enabled as build variables.

Required/current staging values:

```text
DATABASE_URL=<Coolify PostgreSQL Internal URL>
ELM_BIND_ADDR=0.0.0.0:8080
ELM_DB_MAX_CONNECTIONS=10
ELM_SESSION_TTL_HOURS=12
RUST_LOG=info,ertip_lead_manager_server=debug,tower_http=info
```

For the **first empty-database deployment only**, also configure:

```text
ELM_BOOTSTRAP_ADMIN_NAME=<admin display name>
ELM_BOOTSTRAP_ADMIN_EMAIL=<staging admin email>
ELM_BOOTSTRAP_ADMIN_PASSWORD=<long random 12-128 character password>
```

Do not commit these real values to Git.

After the initial ADMIN has been successfully created and login verified:

- remove `ELM_BOOTSTRAP_ADMIN_PASSWORD`;
- remove `ELM_BOOTSTRAP_ADMIN_EMAIL` as well so the bootstrap pair is not left incomplete;
- `ELM_BOOTSTRAP_ADMIN_NAME` may also be removed.

The bootstrap logic never resets an existing user's password, but leaving the secret configured unnecessarily increases operational risk.

## 4. Public staging URL / HTTPS

Attach a staging domain to the API application and use HTTPS.

A temporary generated Coolify URL is acceptable for first health checks, but authentication validation should use HTTPS before the staging gate is considered complete because Web authentication uses a `Secure` session cookie.

Example shape only:

```text
https://lead-api-staging.example.com
```

Do not use this example hostname literally unless it is actually configured in DNS/Coolify.

## 5. First deployment behavior

On startup the server:

1. validates environment configuration;
2. creates the PostgreSQL connection pool;
3. applies embedded SQLx migrations;
4. creates the initial ADMIN only if `app_users` is empty and complete bootstrap configuration is present;
5. starts the Axum listener.

No separate migration command is required for the current M6 server deployment.

## 6. Initial validation

After Coolify reports the container healthy, verify:

```text
GET /health/live   -> HTTP 200
GET /health/ready  -> HTTP 200
GET /api/v1        -> HTTP 200
```

Then validate authentication:

```text
POST /api/v1/auth/login/tauri
GET  /api/v1/me
POST /api/v1/auth/logout
```

The Tauri login response returns an opaque bearer token. Only its SHA-256 hash is stored in PostgreSQL.

Then validate Web authentication over HTTPS:

```text
POST /api/v1/auth/login/web
GET  /api/v1/me
POST /api/v1/auth/logout
```

The Web token must remain absent from JSON and be transported through the Secure + HttpOnly + SameSite=Lax cookie.

## 7. CRM staging smoke test

With the bootstrap ADMIN session, validate at minimum:

```text
GET  /api/v1/personnel
GET  /api/v1/leads
```

After representative staging records exist, additionally validate:

- ADMIN personnel creation/update/deactivation rules;
- MANAGER personnel-read but personnel-mutation denial;
- lead assignment/unassignment;
- lead status mutation;
- note create/update/delete;
- product-interest override;
- stale `expectedRevision` returning HTTP 409 `STALE_REVISION`;
- SALES only seeing/mutating currently assigned own leads;
- audit activities using the authenticated server-side actor.

Follow-up endpoints are not a staging acceptance item until their current M6 slice is wired into HTTP and passes CI.

## 8. What must stay disabled / separate

During M6 staging:

- do not expose PostgreSQL publicly;
- do not point the installed production/local Tauri application at this API yet;
- do not migrate the real schema-v4 SQLite production dataset until the M6 migration/reconciliation tooling is complete;
- do not merge PR #15 solely because staging starts successfully;
- do not treat staging backups as a replacement for the PostgreSQL backup/restore runbook acceptance gate.

## 9. Staging acceptance evidence to record

Before M6 closes, record:

- deployed commit SHA;
- PostgreSQL major version;
- `/health/live` result;
- `/health/ready` result;
- Tauri bearer login/me/logout result;
- Web Secure-cookie login/me/logout result;
- RBAC smoke-test result;
- stale-revision conflict result;
- Coolify container health result;
- database backup/restore evidence when that runbook is implemented.
