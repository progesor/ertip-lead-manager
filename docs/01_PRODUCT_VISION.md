# 01 — Product Vision

## Vision

Replace ad-hoc spreadsheet review with a focused local workspace where an Ertip sales/marketing operator can import new lead exports, immediately understand what is new, find repeated prospects, prioritize follow-ups, and measure where useful leads are coming from.

## Primary user

A Windows desktop user responsible for reviewing and following up Meta leads. V1 assumes one local operator at a time and therefore avoids account/permission complexity.

## Jobs to be done

### Daily / frequent

- Import the newest spreadsheet export.
- See exactly how many records are new, duplicates, repeats, or problematic.
- Find leads quickly by name, e-mail, phone, country, product, campaign, or status.
- Mark progress through the sales pipeline.
- Record concise notes.
- Set a follow-up date.
- Identify leads requiring attention today.

### Weekly / monthly

- Compare lead volume by platform, campaign, ad, country, form, and product interest.
- Compare raw submissions with unique contacts.
- Measure conversion through statuses.
- Find repeat submissions and high-interest contacts.
- Check data-quality patterns that should be fixed in lead forms.

## Product principles

1. **Import safely.** Never mutate the database before a preview and deterministic validation step.
2. **Preserve evidence.** Raw imported values remain available.
3. **Do not over-merge.** Incorrectly combining two customers is worse than showing two separate contacts.
4. **Action before decoration.** Dashboard cards should answer “what needs attention?” before “what chart looks nice?”.
5. **Offline is a feature.** Core functionality should not depend on external services.
6. **Future-ready, not future-heavy.** Leave clean integration seams for Google/Meta/Odoo later without implementing them now.
7. **Fast desktop workflow.** Dense tables, keyboard navigation, quick filters, and a detail drawer are preferred over deep navigation.

## Success criteria for V1

V1 is successful when a user can complete the following without opening Excel after import:

1. Import an updated lead file.
2. See how many submissions are new versus already known.
3. Open a lead and see all submissions from that contact.
4. Change status and add a note.
5. schedule and later find a follow-up.
6. Filter leads by common marketing dimensions.
7. View reliable counts by unique contacts and by submissions.
8. Back up the local database safely.
