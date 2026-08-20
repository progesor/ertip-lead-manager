# 02 — Product Requirements

## 1. Functional requirements

### FR-IMP — Import

**FR-IMP-001** The user can choose an `.xlsx` file from the Windows file picker.  
**FR-IMP-002** The system reads the first supported worksheet containing the required lead headers.  
**FR-IMP-003** The system validates required headers before showing an import preview.  
**FR-IMP-004** Preview summarizes total rows, new submissions, exact duplicates, repeat-contact candidates, warnings, and blocking errors.  
**FR-IMP-005** The user can inspect problematic rows before committing.  
**FR-IMP-006** Import commit is transactional: either the batch commits consistently or rolls back.  
**FR-IMP-007** Import history records file metadata, counts, timestamp, and outcome.  
**FR-IMP-008** Re-importing a previously imported external lead ID must not create a duplicate submission.  
**FR-IMP-009** Source fields are preserved as imported, including raw form answers.  
**FR-IMP-010** Invalid optional fields create warnings rather than blocking the entire batch where safe.

### FR-LEAD — Lead workspace

**FR-LEAD-001** A list displays unique lead contacts.  
**FR-LEAD-002** The list supports free-text search across name, e-mail, phone, and external lead IDs.  
**FR-LEAD-003** The list supports sorting by created/latest submission/follow-up/status/name.  
**FR-LEAD-004** Filters include date, country, platform, campaign, ad set, ad, form, product interest, status, repeat lead, and data-quality state.  
**FR-LEAD-005** The user can combine multiple filters.  
**FR-LEAD-006** Opening a contact shows all linked submissions in chronological order.  
**FR-LEAD-007** The user can copy phone/e-mail values quickly.  
**FR-LEAD-008** Raw source values and normalized application values are visually distinguishable.

### FR-LIFE — Lifecycle

**FR-LIFE-001** The user can change the current lead status.  
**FR-LIFE-002** Status changes create an activity entry with previous/new status and timestamp.  
**FR-LIFE-003** The user can add, edit, and delete notes.  
**FR-LIFE-004** The user can set, complete, reschedule, or clear a follow-up.  
**FR-LIFE-005** Due and overdue follow-ups are discoverable from dashboard and lead list.  
**FR-LIFE-006** Won/lost leads remain searchable and may be reopened with an audit activity.

### FR-PROD — Product interests and normalization

**FR-PROD-001** Raw product-answer text/value is retained exactly as imported.  
**FR-PROD-002** A submission/contact can have zero, one, or multiple normalized product interests.  
**FR-PROD-003** The six canonical form-backed product-interest codes are `FUE_MICROMOTOR_SYSTEMS`, `LONG_HAIR_FUE_SOLUTIONS`, `FUE_PUNCHES`, `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS`, `MEDICAL_CHAIRS_CLINIC_FURNITURE`, and `OTHER_GENERAL_INFORMATION`.  
**FR-PROD-004** `UNKNOWN` is an internal state for legacy/ambiguous answers and is not a customer-facing form option.  
**FR-PROD-005** New Meta multi-select answers map to one or more canonical product-interest codes without collapsing them to a single primary product.  
**FR-PROD-006** Legacy free-text product answers are normalized deterministically when possible and remain supported indefinitely for historical imports.  
**FR-PROD-007** The user can manually add/remove/correct normalized product interests without altering raw source values.  
**FR-PROD-008** Manual corrections take precedence over automatic legacy normalization for the affected record unless explicitly reset.  
**FR-PROD-009** Filters use set-membership semantics: selecting a product finds leads/submissions containing that interest, including records with multiple interests.  
**FR-PROD-010** Normalization rules may be managed in Settings in a later V1.x increment; seeded deterministic rules are acceptable initially.  
**FR-PROD-011** The exact new Excel header and multi-select serialization format must be verified from the first real export after the Meta form change; importer code must not guess a delimiter before that evidence exists.

### FR-DQ — Data quality

**FR-DQ-001** Detect missing/invalid e-mail formatting.  
**FR-DQ-002** Detect missing/obviously malformed phone formatting.  
**FR-DQ-003** Detect country/phone-prefix mismatch when enough information exists; classify as warning only.  
**FR-DQ-004** Detect ambiguous identity conflicts.  
**FR-DQ-005** Detect unrecognized product answers.  
**FR-DQ-006** Warnings are filterable.  
**FR-DQ-007** A user resolution/dismissal state may be stored without altering the raw source value.

### FR-ANA — Dashboard and analytics

**FR-ANA-001** Dashboard shows total unique contacts and total submissions separately.  
**FR-ANA-002** Dashboard shows New, Contacted, Qualified, Quote Sent, Won, Lost counts.  
**FR-ANA-003** Dashboard shows due/overdue follow-ups and new uncontacted leads.  
**FR-ANA-004** Analytics supports date-range filtering.  
**FR-ANA-005** Breakdown dimensions include platform, country, campaign, ad set, ad, form, product interest, and status; a multi-interest submission contributes to each selected product category.  
**FR-ANA-006** Trend charts can use submission date.  
**FR-ANA-007** Conversion metrics must define denominator explicitly.  
**FR-ANA-008** Repeat submissions are measurable independently of exact duplicates.

### FR-BKP — Backup and restore

**FR-BKP-001** The user can create a safe backup of the local SQLite database.  
**FR-BKP-002** Backups include an application/schema version marker.  
**FR-BKP-003** Restore requires confirmation and creates/retains a safety copy of the current DB first where practical.  
**FR-BKP-004** App data location is visible in Settings.  
**FR-BKP-005** No backup file is automatically uploaded anywhere in V1.

## 2. Non-functional requirements

### NFR-001 Platform

Primary supported platform: Windows 10/11 x64.

### NFR-002 Offline

All V1 core workflows function without internet access after installation.

### NFR-003 Performance

Common list queries should remain responsive with 10,000+ contacts and 25,000+ submissions.

### NFR-004 Data integrity

- Imports are transactional.
- Unique constraints enforce submission identity.
- Foreign keys are enabled.
- Database migrations are versioned.

### NFR-005 Recoverability

A corrupt import or application crash must not silently discard already committed CRM data.

### NFR-006 Privacy

Real lead exports and production DBs are never stored in the Git repository.

### NFR-007 Accessibility

Core UI should support keyboard navigation, visible focus states, semantic labels, and sufficient contrast.

### NFR-008 Maintainability

Business logic is separated from UI and persistence layers and covered with unit tests.

## 3. V1 acceptance scenario

Given an existing database with prior leads, when the user imports an updated Excel export containing old IDs, new IDs, repeat contacts, and malformed optional fields, the application must:

1. preview the impact;
2. not reinsert old external IDs;
3. link conservative repeat contacts;
4. flag identity ambiguity instead of auto-merging it;
5. retain source payloads;
6. commit new data atomically;
7. keep prior notes/status/follow-ups untouched;
8. immediately expose new records in the lead workspace and analytics.
