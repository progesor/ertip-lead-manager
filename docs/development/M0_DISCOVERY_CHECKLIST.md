# M0 — Discovery / Repository Checklist

## Goal

Lock the repository foundation before product code begins.

## Checklist

- [ ] Create GitHub repository (suggested name: `ertip-lead-manager`).
- [ ] Add this documentation package to `main`.
- [ ] Confirm `docs/00_PROJECT_CANON.md` as source of truth.
- [ ] Confirm Windows x64 is the first supported platform.
- [ ] Confirm V1 remains manual Excel import and local-only.
- [ ] Confirm working application name or record a rename.
- [x] Confirm canonical multi-select product-interest taxonomy: six customer-facing options + internal `UNKNOWN`.
- [ ] Decide whether UI labels are English, Turkish, or bilingual; keep domain enum keys language-neutral.
- [ ] Confirm acceptable installer behavior for internal PCs.
- [ ] Confirm local data/backup retention expectations.
- [ ] Add sanitized `.xlsx` fixture generated from the legacy CSV fixture for parser tests.
- [ ] After the new Meta form produces its first Excel export, inspect and document the exact product-question header and multi-select serialization; then add a sanitized multi-select fixture. This does **not** block M1, but must be resolved before M2 multi-select parsing is finalized.
- [ ] Create initial issue/milestone labels if GitHub Issues will be used.
- [ ] Record any changes as ADR/canon updates before M1.

## Recommended Git policy

For a small project:

- `main` remains buildable.
- short-lived feature branches.
- one milestone/feature per focused PR where practical.
- documentation changes accompany behavioral changes.

## Exit criteria

M0 passes when there are no unresolved product-boundary questions blocking M1 and the documentation is committed to the repository.
