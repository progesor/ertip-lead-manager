# M6 — Tauri Secure Bearer Session Storage

## Goal

Provide the secure client-side storage contract required before the Windows Tauri application is switched to centralized API mode in M7.

M6 does **not** switch the installed application to the API. It only establishes and validates the native credential-vault abstraction that M7 will consume.

## Windows storage decision

The Tauri Rust layer uses `keyring` v4 through its v1 API. On Windows this resolves to the native Windows Credential Manager backend.

Logical credential identity:

```text
service = com.ertipmedical.lead-manager
account = tauri-api-session
```

The bearer token is not persisted in SQLite, `localStorage`, a JSON configuration file or another plaintext application file.

## Rust-only boundary

`src-tauri/src/services/secure_session_store.rs` owns the token storage abstraction.

There is intentionally no Tauri command that returns the raw stored bearer token to JavaScript. In M7 the Rust API client will load the token directly from the secure store and attach it to HTTPS requests as the `Authorization: Bearer ...` header.

This keeps the long-lived session secret out of the WebView storage surface.

## Secret handling

`StoredSessionToken`:

- validates non-empty/no-control-character token input;
- rejects surrounding whitespace;
- enforces a bounded token size;
- implements a redacted `Debug` representation;
- uses `zeroize`/`ZeroizeOnDrop` so the owned Rust string buffer is overwritten when dropped where supported by the type contract.

The native backend supports:

- availability check;
- store;
- load;
- idempotent clear/delete.

Credential-store unavailability or corrupt values fail closed. There is no plaintext fallback.

## M7 login / logout contract

Planned login sequence:

1. send credentials directly to the HTTPS API;
2. receive the opaque Tauri bearer token in Rust-owned response handling;
3. validate and store it in the native secure store;
4. future Rust API calls load it from the vault and attach the bearer header;
5. never mirror the raw token into SQLite/localStorage/application logs.

Planned logout sequence:

1. load the token in Rust;
2. attempt server-side `/api/v1/auth/logout` revocation;
3. clear the local secure-store credential regardless of UI state;
4. if network revocation cannot complete, the client remains locally logged out and the remote session remains bounded by server expiry/revocation policy.

Password reset/admin reset already revokes server sessions independently, so the server remains authoritative.

## Concurrency

The single logical session credential is guarded by a Rust `Mutex` so store/load/clear operations for that entry are explicitly serialized inside the process.

## Tests and build evidence

Unit tests use an in-memory `SecretBackend` so CI does not store a real bearer token in the runner credential vault. They verify:

- token validation;
- redacted debug output;
- store → load → clear behavior without plaintext file persistence.

Windows CI evidence at commit `6a40dc3dbcd92fb552cf0c2f723d31508aa5db6a`:

- Windows Rust tests: **77/77 PASS**;
- `keyring 4.1.6` and `windows-native-keyring-store` compile on the Windows Server runner;
- both secure-session unit tests PASS;
- Tauri debug NSIS `Build debug NSIS package` step PASS with the native dependency linked into the packaged application.

This closes the M6 secure Tauri bearer-token storage **contract**. Actual API login/UI integration remains M7.
