# 12 — Future Integrations

This document records future seams so V1 can remain deliberately offline/manual.

## 1. Google Sheets

Future behavior:

- connect to a selected sheet;
- periodically or manually fetch unseen rows;
- convert rows into the same `RawSubmissionRow` format used by XLSX import;
- pass them through the same validation, normalization, identity, and persistence pipeline.

Do **not** create a separate direct-to-DB Sheets path.

## 2. Meta Lead Ads API

Potential source adapter should produce the same canonical submission DTO and preserve external IDs/raw payload.

Benefits:

- remove export/download step;
- near-real-time leads;
- richer source metadata.

## 3. Meta Ads performance

Separate paid-media facts from lead facts.

Likely entities:

- ad performance fact by date/ad/adset/campaign
- spend/clicks/impressions

Join by stable external ad/adset/campaign IDs.

## 4. Odoo

Future sales outcome adapter could connect lead contact/submission to quotation/order facts.

Do not couple core contact identity directly to Odoo partner IDs; use an integration mapping table.

## 5. WhatsApp / e-mail

Potential integrations should log communication metadata only with clear privacy controls. V1 can use copy/open conveniences without API automation.

## 6. Multi-user/cloud

Moving from local SQLite to shared multi-user architecture is a major product change involving:

- authentication;
- authorization;
- concurrency;
- server API;
- central database;
- sync/conflict semantics;
- audit identities.

Do not pretend this is a small configuration switch.
