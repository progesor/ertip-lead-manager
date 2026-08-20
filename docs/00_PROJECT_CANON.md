# 00 — Project Canon

**Project:** Ertip Lead Manager  
**Working product type:** Windows desktop lead-management and analytics application  
**Initial business owner:** Ertip Medical  
**Initial release model:** Local-first, single-user, offline-capable  
**Canonical status:** This document defines the non-negotiable V1 product boundary.

## 1. Problem

Meta lead forms currently arrive in a spreadsheet. The spreadsheet is useful as a raw source, but it is inefficient for daily sales work because it does not provide reliable deduplication, repeat-lead recognition, lifecycle tracking, notes, follow-ups, data-quality warnings, or business-oriented analytics.

The application should turn periodic spreadsheet exports into a durable local lead database without altering or depending on the source spreadsheet.

## 2. V1 product statement

> Ertip Lead Manager is a Windows application that manually imports Meta lead `.xlsx` exports, preserves source submissions, groups repeat contacts conservatively, and provides lead review, pipeline tracking, follow-ups, notes, data-quality checks, and practical analytics in a local SQLite database.

## 3. V1 must include

- Manual `.xlsx` file selection and import.
- Import preview before database mutation.
- Import history.
- Exact duplicate prevention using external Meta lead ID.
- Repeat-submission recognition using normalized contact identifiers.
- Conservative contact grouping with conflict detection.
- Lead/contact list with search, sorting, and filters.
- Lead detail workspace.
- Status pipeline.
- Notes and activity timeline.
- Follow-up date/time tracking.
- Multi-select product-interest normalization with legacy free-text compatibility.
- Basic data-quality warnings.
- Dashboard and analytics.
- Local backup/restore or safe database copy workflow.
- Windows x64 packaged build.

## 4. Explicitly out of scope for V1

- Google Sheets live sync.
- Meta Lead Ads API.
- Meta Ads spend API.
- WhatsApp API integration.
- E-mail sending integration.
- Odoo integration.
- Cloud database.
- Multi-user collaboration.
- User accounts / roles / permissions.
- Mobile application.
- Browser-hosted web application.
- AI-dependent lead scoring.
- Automated messaging.

These may be added later, but V1 architecture should not block them.

## 5. Canonical data philosophy

The system separates **source submissions** from **application-managed CRM data**.

### 5.1 Source submission data

Imported from Excel and treated as immutable evidence of what arrived from Meta/form export.

Examples:

- external lead ID
- source timestamp
- ad/campaign/form identifiers and names
- platform
- raw form answers
- raw name/e-mail/phone/country/status fields

### 5.2 Application data

Created or modified inside Ertip Lead Manager.

Examples:

- current lead status
- normalized product interests (zero, one, or many)
- assignee placeholder/future owner field
- notes
- follow-up
- tags
- qualification state
- quote/sale metadata
- data-quality resolution

The original source values must remain recoverable even if normalized values change.

## 6. Canonical identity model

There are two different concepts:

1. **Submission** — a single Meta lead-form submission identified by `external_lead_id`.
2. **Lead contact** — a person/contact record that may have one or more submissions.

Rules:

- Same `external_lead_id` again => exact duplicate submission; do not insert again.
- New `external_lead_id` + matching normalized e-mail and/or phone => candidate repeat submission for an existing contact.
- Never merge solely because names match.
- If e-mail and phone point to different existing contacts, mark as identity conflict and require review.
- Contact grouping must be conservative; false non-merges are preferable to false merges.

## 7. Canonical lead statuses

V1 statuses:

1. `NEW`
2. `CONTACTED`
3. `REPLIED`
4. `QUALIFIED`
5. `QUOTE_SENT`
6. `WON`
7. `LOST`
8. `INVALID`

The UI may show friendly localized labels. Database/domain values remain stable enums.

## 8. Product-interest philosophy

Product interest is **multi-valued**, not a single category. A contact/submission may legitimately be interested in several product groups at the same time.

The canonical customer-facing taxonomy for the new Meta multi-select question is:

1. `FUE_MICROMOTOR_SYSTEMS` — FUE Micromotor Systems
2. `LONG_HAIR_FUE_SOLUTIONS` — Long Hair FUE Solutions
3. `FUE_PUNCHES` — FUE Punches
4. `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS` — Implanters, Forceps & Surgical Instruments
5. `MEDICAL_CHAIRS_CLINIC_FURNITURE` — Medical Chairs & Clinic Furniture
6. `OTHER_GENERAL_INFORMATION` — Other Products / General Information

`UNKNOWN` is an internal classification state and is **not** shown as a form option.

Raw form answers are always preserved. New multi-select answers should map to one or more canonical product-interest codes. Legacy free-text answers remain supported through deterministic normalization rules. Unknown or ambiguous legacy text remains `UNKNOWN` until manually classified.

The application must never force a lead into exactly one product category. Filters and analytics must use set-membership semantics (for example, “contains FUE Punches”).

## 9. Time handling

- Source timestamps may contain different UTC offsets.
- Parse valid ISO-8601 timestamps to UTC for comparisons.
- Preserve the exact source timestamp string.
- Display dates in the local application timezone by default.
- Timestamp formatting differences must not create duplicate submissions when external lead ID is identical.

## 10. UX principles

- Desktop-first density without becoming spreadsheet-like clutter.
- Fast keyboard/mouse workflows.
- Search and filters should be persistent and predictable.
- Important warnings are visible but not blocking unless data integrity is at risk.
- Import is preview-first; users see the effect before commit.
- Destructive actions require confirmation.
- The dashboard prioritizes actionable work, not decorative charts.

## 11. Technology direction

Canonical high-level stack:

- Tauri 2 desktop shell
- React + TypeScript frontend
- Rust backend commands/services
- SQLite local database
- Calamine for Excel parsing

Dependency-level changes are allowed through ADRs if the product constraints remain intact.

## 12. Performance targets

V1 should remain responsive with at least:

- 10,000 contacts
- 25,000 submissions
- 100 import batches

Target interactive operations:

- search/filter feedback: perceived immediate (<300 ms for common local queries)
- lead detail open: <300 ms typical
- 1,000-row import preview: a few seconds maximum on a normal office Windows PC

These are engineering targets, not contractual SLAs.

## 13. Release target

A signed installer is desirable later; the first internal release may be an unsigned Windows x64 installer if deployment policy permits. Data safety and import correctness take priority over visual polish.
