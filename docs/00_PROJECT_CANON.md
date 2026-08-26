# 00 — Project Canon

**Project:** Ertip Lead Manager  
**Working product type:** Lead-management and analytics platform with Windows Tauri client and planned Web client  
**Initial business owner:** Ertip Medical  
**Frozen fallback release:** `v0.1.0-local` — local-first, single-user, offline-capable  
**Target production model:** centralized authenticated multi-user API + PostgreSQL  
**Canonical status:** This document defines the non-negotiable domain/data rules and current product direction.

## 1. Problem

Meta lead forms currently arrive in tabular exports. The source export is useful as raw evidence, but it is inefficient for daily sales work because it does not provide reliable deduplication, repeat-lead recognition, lifecycle tracking, notes, follow-ups, assignment, data-quality warnings, audit attribution, or business-oriented analytics.

The product must turn supported lead sources into a durable CRM record while preserving immutable source evidence and conservative identity rules.

## 2. Product evolution

The first complete internal product was frozen as `v0.1.0-local`: a Windows Tauri application using local SQLite. That release remains a recoverable fallback and proves the core CRM/domain behavior.

Operational demand after M5/M5.5 requires personnel ownership, authenticated users and shared state across multiple people/devices. The target production architecture is therefore centralized.

> Ertip Lead Manager will use an authenticated HTTPS backend API as the production authority, backed by private PostgreSQL. The Windows Tauri client and future Web client must use the same API/domain rules. The frozen local SQLite release remains a fallback/development/migration reference, not the authoritative multi-user production database.

## 3. Core product capabilities

The canonical product includes:

- Manual `.xlsx` and `.csv` Meta lead import.
- Import preview before database mutation.
- Import history.
- Exact duplicate prevention using external Meta lead ID.
- Repeat-submission recognition using normalized contact identifiers.
- Conservative contact grouping with conflict detection.
- Lead/contact list with search, sorting, filters and assignee visibility.
- Lead detail workspace.
- Status pipeline.
- Notes and activity timeline.
- Follow-up date/time tracking.
- Personnel and lead assignment.
- Multi-select product-interest normalization with legacy free-text compatibility.
- Basic data-quality warnings.
- Dashboard and analytics.
- Immutable activity audit.
- Authenticated actor attribution in centralized mode.
- Windows x64 client.
- Future browser-hosted Web client using the same backend API.

## 4. Current scope boundaries

Not part of the M6 centralized-backend milestone:

- Meta Lead Ads API.
- Meta Ads spend API.
- WhatsApp API integration.
- E-mail sending integration.
- Odoo integration.
- Mobile application.
- AI-dependent lead scoring.
- Automated messaging.
- Web UI implementation itself (planned for M8).

These may be added later only through the canonical backend/domain model.

## 5. Canonical data philosophy

The system separates **source submissions** from **application-managed CRM data**.

### 5.1 Source submission data

Imported from a supported source and treated as immutable evidence of what arrived from Meta/form export.

Examples:

- external lead ID
- source timestamp
- ad/campaign/form identifiers and names
- platform
- raw form answers
- raw name/e-mail/phone/country/status fields

Unknown additional source columns may be preserved in raw payload metadata but must not silently become CRM fields.

The observed agency-maintained columns `Status` and `İletişime Geçme Tarihi` are **not** application lifecycle inputs and are ignored by default. The lower-case machine field `lead_status` is a separate source metadata field and is preserved raw only.

### 5.2 Application data

Created or modified inside Ertip Lead Manager.

Examples:

- current lead status
- normalized/effective product interests
- current assignee
- personnel records and roles
- notes
- follow-ups
- tags / qualification state when introduced
- quote/sale metadata when introduced
- data-quality resolution
- activity/audit metadata

The original source values must remain recoverable even if application-managed values change.

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
- Stable application IDs must be preserved during SQLite → PostgreSQL migration.

## 7. Canonical lead statuses

Stable statuses:

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

The canonical customer-facing taxonomy for the Meta multi-select question is:

1. `FUE_MICROMOTOR_SYSTEMS` — FUE Micromotor Systems
2. `LONG_HAIR_FUE_SOLUTIONS` — Long Hair FUE Solutions
3. `FUE_PUNCHES` — FUE Punches
4. `IMPLANTERS_FORCEPS_SURGICAL_INSTRUMENTS` — Implanters, Forceps & Surgical Instruments
5. `MEDICAL_CHAIRS_CLINIC_FURNITURE` — Medical Chairs & Clinic Furniture
6. `OTHER_GENERAL_INFORMATION` — Other Products / General Information

`UNKNOWN` is an internal classification state and is **not** shown as a form option.

Raw form answers are always preserved. The post-change export observed on **2026-08-21** keeps the existing product-question header and serializes selected machine values using `|` as the delimiter. Single selections contain one machine value; multiple selections contain several values separated by `|`. Product selections must never be comma-split because one canonical machine value itself contains commas.

Legacy free-text answers remain supported through deterministic normalization rules. Unknown or ambiguous legacy text remains `UNKNOWN` until manually classified.

The application must never force a lead into exactly one product category. Filters and analytics use set-membership semantics.

## 9. Time handling

- Source timestamps may contain different UTC offsets.
- Parse valid ISO-8601 timestamps to UTC for comparisons.
- Preserve the exact source timestamp string.
- Persist application timestamps canonically in UTC.
- Display dates in the user's local timezone by default.
- Timestamp formatting differences must not create duplicate submissions when external lead ID is identical.

## 10. Personnel, authentication and audit

Canonical personnel roles:

- `ADMIN`
- `MANAGER`
- `SALES`

Rules:

- Personnel/application user IDs are stable CRM identities.
- Personnel are deactivated rather than hard-deleted in normal flows.
- Historical assignments/audit remain readable after deactivation/name changes.
- Centralized authentication binds an authenticated identity to the stable CRM user.
- The backend derives current user and `actor_user_id` from the authenticated server-side session.
- A client-supplied actor/user ID must never be trusted as audit identity.
- Authorization is enforced server-side; hiding UI controls is not sufficient authorization.

## 11. UX principles

- Desktop-first density without becoming spreadsheet-like clutter.
- Dashboard and Kanban remain primary daily-use surfaces.
- Fast keyboard/mouse workflows.
- Search and filters should be persistent and predictable.
- Important warnings are visible but not blocking unless data integrity is at risk.
- Import is preview-first; users see the effect before commit.
- Destructive actions require confirmation.
- The dashboard prioritizes actionable work, not decorative charts.
- Network/auth/concurrency errors in online mode must be explicit rather than silently losing work.

## 12. Technology direction

### Frozen local fallback

- Tauri 2 desktop shell
- React + TypeScript frontend
- Rust commands/services
- SQLite / SQLx
- Calamine for `.xlsx`
- Rust `csv` crate for `.csv`

### Centralized production target

- Same React/Tauri Windows client initially
- Rust + Axum HTTP backend
- PostgreSQL + SQLx
- server-side authenticated sessions
- HTTPS API namespace starting at `/api/v1`
- Coolify deployment with PostgreSQL on private/internal network
- future Web client using the same API/auth contract

Windows/Web clients must not receive PostgreSQL credentials and must not connect directly to PostgreSQL.

Business rules should move toward reusable Rust domain/application code rather than being independently reimplemented in the HTTP layer.

## 13. Performance and concurrency targets

The proven local baseline remains at least:

- 10,000 contacts
- 25,000 submissions
- 100 import batches

Centralized mode adds concurrency expectations:

- common API read operations should remain interactive at the existing data scale;
- mutable CRM writes must protect against lost updates;
- stale concurrent writes must produce an explicit conflict rather than silently overwriting newer data;
- database indexes/queries must be validated on PostgreSQL rather than assumed from SQLite behavior.

These are engineering targets, not contractual SLAs.

## 14. Release direction

- `v0.1.0-local` is the frozen local fallback release.
- M6 builds the centralized backend/auth/data migration capability.
- M7 switches the Windows production client to the centralized API.
- M8 adds the Web client against the same backend.
- Code signing is desirable before broad Windows distribution.
- Centralized PostgreSQL backup/recovery and migration reconciliation are release gates before multi-user production use.
