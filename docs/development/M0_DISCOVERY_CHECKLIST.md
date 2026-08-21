# M0 — Discovery / Repository Checklist

## Goal

Lock the repository foundation before product code begins.

## Checklist

- [x] Create GitHub repository: `progesor/ertip-lead-manager`.
- [x] Add the documentation package to `main`.
- [x] Confirm `docs/00_PROJECT_CANON.md` as source of truth.
- [x] Confirm Windows 10/11 x64 as the first supported platform.
- [x] Confirm V1 remains manual Excel import and local-only.
- [x] Confirm working application name: **Ertip Lead Manager**.
- [x] Confirm canonical multi-select product-interest taxonomy: six customer-facing options + internal `UNKNOWN`.
- [x] Confirm V1 UI labels are primarily Turkish while domain enum keys remain language-neutral.
- [x] Confirm internal installer behavior: NSIS current-user; unsigned internal builds are acceptable during development.
- [x] Confirm local retention: no automatic data deletion; backups remain local and user-controlled.
- [x] Add a sanitized `.xlsx` fixture generated from the legacy CSV fixture for parser tests.
- [ ] After the new Meta form produces its first Excel export, inspect and document the exact product-question header and multi-select serialization; then add a sanitized multi-select fixture. This does **not** block M1, but must be resolved before M2 multi-select parsing is finalized.
- [x] GitHub Issues/PRs may be used for milestone tracking; custom labels are optional and can be added when the workflow needs them.
- [x] Record M1 operational defaults in `docs/adr/0005-m1-foundation-decisions.md` before implementation.

## Git policy

- `main` remains buildable.
- short-lived feature branches.
- one milestone/feature per focused PR where practical.
- documentation changes accompany behavioral changes.
- squash merge is preferred for foundation/feature PRs unless history benefits from preserving commits.

## Exit criteria

M0 passes when there are no unresolved product-boundary questions blocking M1 and the documentation is committed to the repository.

**M0 status:** PASS. The pending real Meta multi-select export is explicitly an M2 input and does not block M1.
