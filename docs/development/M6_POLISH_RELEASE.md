# M6 — Polish and Internal Release

## Goal

Ship a dependable internal Windows build.

## Deliverables

- backup
- restore
- improved errors and logs
- import recovery UX
- loading/empty/error states
- performance fixes/indexes
- installer configuration
- About/version diagnostics
- release notes
- documentation review

## Acceptance criteria

- [ ] Backup/restore round-trip passes.
- [ ] Release smoke checklist in `10_TEST_STRATEGY.md` passes.
- [ ] Clean install works on Windows x64.
- [ ] No production PII is present in repository/build fixtures.
- [ ] Known limitations documented.
- [ ] V1 canon updated to reflect actual shipped behavior.
