# 09 — Security, Backup and Operations

## 1. Security state

The project has two security contexts:

1. **Frozen local fallback (`v0.1.0-local`)** — single Windows-user trust boundary with local SQLite.
2. **Centralized production target (M6+)** — authenticated multi-user API with private PostgreSQL.

The centralized model introduces a real network/authentication/authorization boundary and must not inherit local single-user assumptions.

## 2. Centralized threat model

Primary risks include:

- stolen/guessed credentials;
- session theft/replay;
- privilege escalation between SALES / MANAGER / ADMIN;
- trusting client-supplied actor/user IDs;
- internet exposure of PostgreSQL;
- lost updates from concurrent users;
- PII leakage through logs/backups/errors;
- injection or malformed API input;
- insecure secret storage/deployment configuration;
- accidental migration/data loss during SQLite → PostgreSQL transition.

M6 is an internal business system, not a public multi-tenant SaaS, but authentication and server-side authorization are still mandatory.

## 3. Network topology

Target Coolify layout:

```text
Internet
   │ HTTPS
   ▼
API/Auth container
   │ private/internal network
   ▼
PostgreSQL
```

Rules:

- PostgreSQL is not exposed with a public internet port.
- Clients never receive `DATABASE_URL` or DB credentials.
- TLS terminates at the Coolify/reverse-proxy edge.
- API secrets are injected as runtime secrets/environment variables.
- Secrets are never committed to Git.

## 4. Authentication

M6 uses server-side sessions with opaque session material.

- Passwords, if used, are hashed with Argon2id.
- Plaintext passwords are never stored.
- Raw session tokens are not logged.
- Session records support expiry and revocation.
- Web uses Secure + HttpOnly cookies.
- Tauri uses the same server-side session via opaque bearer token; secure OS-backed token storage is an M7 production gate.
- Login failures must not reveal whether a specific account exists more than operationally necessary.
- Baseline rate limiting / abuse protection is required for login endpoints before production.

## 5. Authorization and audit identity

Stable roles:

- `ADMIN`
- `MANAGER`
- `SALES`

Authorization is enforced server-side in application/API policy.

Critical rule:

> `actor_user_id` is derived from the authenticated server-side session. A client-supplied actor ID is never trusted as audit identity.

UI hiding may improve UX but is not authorization.

Personnel records are deactivated rather than hard-deleted so historical audit/assignment references remain valid.

## 6. Session and credential logging

Never log:

- plaintext passwords;
- password hashes unless unavoidable for a secure migration diagnostic (normally never);
- raw session tokens/cookies;
- database connection passwords;
- complete authorization headers.

Prefer request ID + application UUID + operation code.

## 7. PII-minimized logs

Good:

```text
request_id=... actor_user_id=... action=lead_status_change contact_id=... result=ok
```

Avoid:

```text
Changed john@example.com +90... to WON
```

Import diagnostics should use row numbers, batch IDs and issue types instead of full contact data where possible.

## 8. PostgreSQL data safety

- Versioned migrations.
- Foreign keys/check constraints enabled by schema.
- Canonical writes use transactions.
- Source submissions/raw payloads remain immutable after insertion.
- Mutable CRM writes use explicit concurrency/lost-update protection.
- Migration tooling preserves stable IDs.
- Reconciliation is mandatory before switching production authority from SQLite to PostgreSQL.

## 9. Centralized backup

Before M7 production rollout, define and test:

- automated PostgreSQL backup schedule;
- backup retention;
- storage location independent of the running DB volume where practical;
- restore procedure into a separate validation database;
- recovery-point expectations;
- documented ownership/responsibility.

A backup is not considered valid until restore has been tested.

## 10. SQLite fallback safety

The frozen local application continues to use its Tauri app-data SQLite file.

Its release record/checksum is stored in `docs/development/LOCAL_RELEASE_V0_1_0.md`.

Do not reuse the frozen release branch for M6 work.

SQLite backups used for PostgreSQL migration testing must be handled as PII-bearing production data and must not enter Git/fixtures.

## 11. SQLite → PostgreSQL migration security

- Migration tooling runs in a controlled environment.
- No production lead data is committed to source control.
- Export/intermediate files are temporary and protected.
- Migration reports should prefer counts/IDs over raw PII.
- Stable IDs and immutable source/audit history are preserved.
- Reconciliation must detect missing/extra key records before cutover.

## 12. API validation/error handling

Backend validates all external input even if the client already validated it.

Errors should return a stable code + user-readable message + request ID without exposing:

- SQL details;
- stack traces;
- secrets;
- internal paths;
- unnecessary PII.

Unexpected technical detail belongs in protected server logs.

## 13. CORS/CSRF direction

- Web cookie sessions require explicit allowed origins and CSRF-safe request design.
- Never use wildcard credentialed CORS in production.
- Tauri bearer-token requests are authorization-header based and do not remove the need for server authorization.
- Exact production domains/origins are configured through deployment environment, not hard-coded assumptions.

## 14. Operations/health

Server exposes:

- `/health/live` for process liveness;
- `/health/ready` for PostgreSQL readiness.

Health endpoints must not expose secrets or detailed DB connection data.

Structured logs should include request IDs and useful failure categories.

## 15. Repository/privacy controls

`.gitignore` is not a security control by itself.

- Sanitized fixtures only.
- No real spreadsheet exports, SQLite databases, PostgreSQL dumps, backups or secret `.env` files in Git.
- No Coolify runtime secrets in issues/PRs/chat transcripts.

## 16. Release direction

- Frozen local fallback may remain unsigned for internal recovery use.
- Online production requires HTTPS, tested backup/restore, authenticated sessions and authorization tests.
- Windows code signing remains desirable before broad external distribution.
- Automatic updater is deferred until centralized production rollout requirements are clear.
