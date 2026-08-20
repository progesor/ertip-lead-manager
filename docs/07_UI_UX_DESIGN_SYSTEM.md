# 07 — UI/UX Design System

## 1. Product feel

A professional internal sales tool: clean, fast, information-dense, modern, and calmer than a generic CRM.

Avoid:

- oversized marketing-style cards;
- excessive gradients/glass effects;
- spreadsheet-level visual noise;
- hidden actions that require too many clicks;
- modal dialogs for routine lead navigation.

## 2. Primary navigation

Recommended left sidebar:

1. Dashboard
2. Leads
3. Pipeline
4. Analytics
5. Imports
6. Settings

Optional compact footer items: app version, data location, backup status.

## 3. Dashboard layout

### Top KPI row

- Unique Leads
- New
- Qualified
- Quote Sent
- Won
- Conversion

### Action row

- New uncontacted leads
- Follow-ups due today
- Overdue follow-ups
- Data-quality issues

### Charts

Prioritize a maximum of 3–4 visible charts before scrolling:

- lead/submission trend
- platform/source breakdown
- country breakdown
- product interest or funnel

Additional analytics belong on the Analytics page.

## 4. Leads page

Desktop split layout is preferred:

```text
┌────────────────────────────── Leads ─────────────────────────────┐
│ Search  [filters...]                      Import / view controls │
├─────────────────────────────────────┬────────────────────────────┤
│ virtualized / paginated lead table  │ detail drawer (optional)  │
│                                     │                            │
│ Name | Country | Products | Source  │ Contact                    │
│ Date | Status | Follow-up | Warnings│ Submissions                │
│                                     │ Timeline                   │
└─────────────────────────────────────┴────────────────────────────┘
```

The detail drawer can become a full-page route if the window is narrow.

## 5. Lead table columns

Default suggested columns:

- Name
- Country
- Products (compact multi-value chips, with overflow count)
- Source platform
- Latest campaign/ad (compact)
- Latest submission date
- Submission count / repeat marker
- Status
- Follow-up
- Warning indicator

Do not show every raw Excel column by default. Raw marketing dimensions remain available through detail view and column customization later.

## 6. Search

One search field should match:

- display/raw names
- e-mail
- normalized/raw phone
- external lead ID

Debounce lightly or execute server-side SQLite FTS/LIKE queries. Search must not require clicking “Search”.

## 7. Filters

Common quick filters visible:

- Status
- Date
- Country
- Product interests (multi-select filter; default match mode = contains any selected interest)
- Platform
- Follow-up state

Advanced filter panel:

- Campaign
- Ad set
- Ad
- Form
- Repeat only
- Data-quality issue
- Organic

Active filters display as removable chips and can be cleared in one action.

## 8. Lead detail

Header:

- Name
- status control
- repeat badge
- data-quality badge
- contact actions (copy phone/e-mail; external launch actions can remain simple `mailto:`/URL schemes if desired later)

Overview should show product interests as multiple compact chips and distinguish source-derived interests from manual corrections where relevant.

Sections:

1. Overview
2. Follow-up
3. Notes / Activity timeline
4. Submissions
5. Source data

### Source data

Clearly label source values as read-only.

## 9. Pipeline

Kanban columns follow lifecycle statuses. Cards display only essential info:

- name
- country
- product interests (up to 2 chips + overflow count)
- latest source/date
- follow-up/warning markers

Drag/drop status changes must:

- be accessible with an alternate non-drag control;
- create the same activity as any status change;
- handle failures by returning the card to original column and showing a clear error.

For performance, optionally exclude WON/LOST/INVALID from default board or place them behind toggles.

## 10. Import UX

Import flow:

1. Select file.
2. Parse/validate.
3. Preview summary.
4. Show row groups: New, Repeat, Duplicate, Warning, Error.
5. User commits.
6. Show result with link to new leads/import history.

Example preview KPI strip:

```text
Rows 387 | New 41 | Repeat 5 | Existing 339 | Warnings 2 | Errors 0
```

Exact duplicate rows should not be alarming; they are expected in cumulative exports.

## 11. Visual status semantics

Use restrained semantic colors and never rely on color alone.

- New: neutral/blue
- Contacted/Replied: informative
- Qualified: positive accent
- Quote Sent: highlighted
- Won: positive
- Lost/Invalid: muted/negative
- Overdue: warning/negative

Final palette should be implemented through design tokens, not hard-coded per component.

## 12. Typography and density

- Body text around standard desktop UI size (14–16 px depending on font).
- Tables may use compact density but must keep readable row height.
- Monospace only for IDs/debug/source fields.
- Long e-mails/campaign names should truncate with tooltip/copy access.

## 13. Keyboard support

Desired shortcuts after core UI works:

- `/` focus search
- `Ctrl+I` import
- `Ctrl+K` optional command/search palette later
- Arrow/Enter navigate lead rows
- `Esc` close detail drawer/modal

Do not prioritize shortcut work before correctness.

## 14. Window behavior

Design primarily for 1366×768 and above, with comfortable behavior at 1920×1080. Support resizing down to a practical minimum width. No mobile responsive requirement for V1.
