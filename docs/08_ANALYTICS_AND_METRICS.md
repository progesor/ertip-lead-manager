# 08 — Analytics and Metrics

## 1. Core rule: contacts and submissions are different

Every analytics screen must distinguish:

- **Unique leads/contacts** — application-level contact count.
- **Submissions** — unique external Meta lead-form submissions.

A repeat lead can increase submissions without increasing unique contacts.

## 2. Date basis

Default marketing analytics date basis: `source_created_at_utc` of submissions.

Lifecycle/follow-up analytics may use application activity timestamps.

UI should make date basis clear when ambiguity exists.

## 3. KPI definitions

### Total Leads

Count of unique lead contacts matching the selected scope.

### Total Submissions

Count of unique `external_lead_id` submissions matching the selected date/scope.

### New

Unique contacts whose current status is `NEW` within selected query context. When used as “newly acquired in period,” use first submission date and label explicitly as `New Leads in Period`.

### Qualified

Contacts with current status `QUALIFIED`, `QUOTE_SENT`, or `WON` may optionally be considered “ever qualified” only if using activity history. To avoid ambiguity, first V1 KPI `Qualified` should mean **current status = QUALIFIED** unless another metric is explicitly named.

### Quote Sent

Current status `QUOTE_SENT` unless historical funnel analytics are implemented.

### Won

Current status `WON`.

### Lead conversion rate

Default:

```text
Won contacts / unique contacts × 100
```

Display denominator in tooltip/help.

### Qualification rate

If implemented from current statuses:

```text
(QUALIFIED + QUOTE_SENT + WON) / unique contacts × 100
```

Historical “ever reached stage” funnels should use activity events and be labeled separately.

### Repeat submission rate

```text
contacts with >1 submission / unique contacts × 100
```

### Duplicate import rows

Exact external-ID duplicates are import hygiene, not a marketing KPI. Keep them primarily in Imports/Data Quality analytics.

## 4. Breakdown dimensions

Submission-based dimensions:

- Platform
- Country
- Campaign
- Ad set
- Advertisement
- Form
- Organic flag
- Product raw answer / normalized product-interest membership

When counting unique contacts by a submission dimension, define attribution:

- **Latest-touch** (latest submission in range)
- **First-touch** (first submission ever/in range)
- **Submission count**

V1 should prefer **submission counts** for source breakdowns because they are unambiguous. Unique-contact attribution can be added with clearly labeled first/latest-touch semantics.

Product interest is multi-valued. A submission that selected Micromotor and FUE Punches contributes **one count to each of those product categories**. Therefore category totals may exceed total submissions; the UI must explain this and must not show product-category percentages as if the categories were mutually exclusive unless using a multi-response denominator label.

## 5. Recommended V1 charts

### Dashboard

- Submissions by day
- Leads by status
- Submissions by platform
- Top countries

### Analytics

- Trend by day/week
- Campaign breakdown table
- Product-interest breakdown
- Form version breakdown
- Country breakdown
- Repeat lead table/rate
- Funnel based on current status (clearly labeled)

## 6. Filters

Global analytics filters:

- date range
- platform
- country
- campaign
- ad set
- ad
- form
- product interests (multi-select; contains-any by default)

Status filtering is useful but should not accidentally alter submission-source metrics without a visible indicator.

## 7. Empty / unknown values

Do not silently drop blanks. Group as:

- `Unknown`
- `Missing`

depending on semantic meaning.

## 8. Product combination analysis

Useful multi-select analytics that may be included in V1 if inexpensive, otherwise V1.1:

- count of leads/submissions by each product interest;
- most common two-product combinations;
- leads interested in both A and B;
- product-interest count per submission (1, 2, 3+).

Combination metrics must be computed from canonical product-interest memberships, never by splitting raw text ad hoc.

## 9. Future paid-media metrics

When Meta Ads spend is integrated later, possible metrics:

- Spend
- Impressions
- Clicks
- Raw leads
- Unique contacts
- Qualified leads
- Cost per lead
- Cost per qualified lead
- Won sales
- Revenue
- ROAS

These are explicitly future scope and must not be simulated in V1.
