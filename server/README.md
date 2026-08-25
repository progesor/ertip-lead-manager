# Ertip Lead Manager Server

M6 centralized API/auth process.

## Local development contract

Required environment:

```text
DATABASE_URL=postgres://user:password@localhost:5432/ertip_lead_manager
```

Optional:

```text
ELM_BIND_ADDR=0.0.0.0:8080
ELM_DB_MAX_CONNECTIONS=10
RUST_LOG=info,ertip_lead_manager_server=debug,tower_http=info
```

Run:

```bash
cargo run --manifest-path server/Cargo.toml
```

Health endpoints:

```text
GET /health/live
GET /health/ready
GET /api/v1
```

`/health/live` proves the process/router is alive. `/health/ready` performs a PostgreSQL `SELECT 1` and returns a non-200 response when the database is not available.

## Docker / Coolify

Build from repository root:

```bash
docker build -f server/Dockerfile -t ertip-lead-manager-server .
```

The container listens on port `8080` by default. Supply `DATABASE_URL` through Coolify runtime secrets/environment configuration.

Recommended Coolify setup:

- Dockerfile: `server/Dockerfile`
- Build context: repository root
- Container port: `8080`
- Health/readiness path: `/health/ready`
- PostgreSQL: private/internal Coolify network only
- Public endpoint: API container via HTTPS reverse proxy

Do not expose PostgreSQL credentials to Tauri/Web clients.

## Current M6.1 boundary

This foundation does not yet implement login or CRM endpoints. It establishes the process/config/health/container/CI boundary first. Authentication, PostgreSQL migrations and CRM parity are subsequent M6 slices.
