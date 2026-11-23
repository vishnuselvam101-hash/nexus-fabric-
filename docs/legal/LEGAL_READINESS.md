# NEXUS FABRIC — legal readiness (working list, not legal advice)

Date: 20 Nov 2026. Prepared by the development lead as a list of facts, gaps and drafts for a qualified lawyer.
**No software can be made "legally proof", and nothing in this file is a legal opinion.** What can be done is: remove
avoidable risks, tell users the truth, respect other people's licences, and have a lawyer admitted in each market
(first market: Japan) review the terms before the software is sold. Until that review, the product stays "evaluation
use only".

## 1. Status

| # | Area | Status today | What is needed | Who |
|---|---|---|---|---|
| 1 | Truthful statements (no misleading representation) | Largely in place: every screen, DXF, installer and film says development build, not validated, not for manufacturing; no result can be approved; planned items are labelled or absent | Keep it enforced by tests. **Owner decision 20 Nov: the product contains no automated drawing reading — claims of automated intelligence and any automated-reading feature announcement are removed from the film and must be removed from every other document and page.** | Lead (in progress) |
| 2 | Licence to the customer (EULA / terms of use) | **Missing.** The packages say "All rights reserved. Evaluation use only — terms to be provided by the owner." | A written licence: what the customer may do, evaluation limits, no warranty of fitness for manufacturing, limitation of liability, governing law and court, termination, confidentiality. A draft can be prepared here; a lawyer must finalise it, in Japanese for Japan | Owner + lawyer |
| 3 | Limitation of liability and engineering responsibility | Statements exist in the product (review required, not released, engineer responsible), but they are not contract terms | Contract clauses stating that results must be reviewed by a qualified engineer before use and that the vendor does not approve parts; a lawyer must say how far such clauses hold under Japanese law (consumer and product-liability rules cannot simply be excluded) | Lawyer |
| 4 | Third-party software licences | **Checked 20 Nov:** 102 third-party libraries are compiled into the program; all are under permissive licences (MIT, Apache-2.0, BSD-3-Clause, Zlib, Unicode-3.0, Unlicense, BSL-1.0); one offers an LGPL option but also MIT/Apache, which is the one used; no GPL or other copyleft-only component. SQLite (public domain) is bundled | **Gap: the packages do not yet ship the required licence texts and copyright notices.** MIT and Apache-2.0 require them. A THIRD_PARTY_NOTICES file generated from the build must be added to every installer and package | Lead (queued) |
| 5 | Fonts, voices and music in the films | Fonts: Inter, Noto Sans JP, JetBrains Mono (open font licences; licence files to be confirmed in the film folders). Voices: Kokoro (model stated as Apache-2.0 by its package; model card not checked) and VOICEVOX Nemo (credit「VOICEVOX Nemo」required and shown). Music: synthesised in code, original | Confirm the font licence files are shipped with the film sources; open and read the VOICEVOX Nemo terms page and the Kokoro model card before publishing (not done: no downloads were allowed) | Owner / lead |
| 6 | Film style | Inspired by product-launch films in general; no third-party logo, typeface, device image, footage or wording is used | Keep it that way; do not name or imitate a specific company's advertisement | Lead |
| 7 | Names and trademarks | Not checked. "NEXUS FABRIC", "AMOEBA" and the mark have not been searched in any trademark register | Trademark search and, if clear, registration in Japan (and other markets) before public launch | Owner + trademark attorney |
| 8 | Personal data | The software stores user names, password hashes (Argon2) and an audit log on the customer's own machine or server. No telemetry; nothing is sent anywhere (tested: runs with the network removed) | A short privacy statement saying exactly that; for the hosted server, the operator (the owner) is the one holding the data and needs a privacy policy under Japan's personal-information law | Owner + lawyer |
| 9 | Security statements | The database is not encrypted; desktop key handling is weak (see docs/security). Owner approved plain wording | Put the approved wording in the install guide and first-run screen (queued). Never use "secure", "encrypted" or "hack-proof" without the qualification | Lead (queued) |
| 10 | Source of the code | The software is unreviewed and says so (evidence level EV-1: "not independently reviewed") | Ask the lawyer about ownership and registration of the code in the target markets, and keep the disclosure | Lawyer |
| 11 | Export and sanctions | The program contains standard encryption (AES-GCM, Argon2, SHA-2) | Ask whether any export notification applies when distributing outside Japan | Lawyer |
| 12 | Open pull request and repository | Repository is private; one pull request is open | Decide the licence of the repository before giving anyone access | Owner |

## 2. What the lead will do without waiting (no legal judgement needed)

1. Generate THIRD_PARTY_NOTICES from the exact build and ship it in every package (item 4).
2. Remove every claim of automated intelligence from the film, the interface texts and the documents (item 1), and add a test that fails if
   the interface or packages contain marketing words that are not true ("automated intelligence", "certified", "approved",
   "validated", "secure", "hack-proof").
3. Put the approved desktop-security wording into the install guide and first-run screen (item 9).
4. Prepare DRAFT terms of use, privacy statement and engineering-responsibility notice in English and Japanese, each
   marked "DRAFT — not reviewed by a lawyer", for the owner's lawyer to correct. **Drafted 21 Nov 2026 (task 4.5); see section 4.**

## 3. What only the owner can do

- Engage a lawyer admitted in Japan (and in any other country of sale) to review items 2, 3, 8, 10, 11.
- Order the trademark search (item 7).
- Read the VOICEVOX Nemo terms and the Kokoro model card, or choose other voices (item 5).
- Decide the commercial model (evaluation, paid licence, subscription), because the terms depend on it.

## 4. Drafts for the lawyer (task 4.5, 21 Nov 2026)

Each file holds the English text (Part A), a Japanese translation (Part B) and, at the end, "Questions for the
lawyer". Each is headed "DRAFT — not reviewed by a lawyer" / 「下書き — 弁護士による確認を受けていません。」. None is
in force. **The Japanese text of all three needs review by a native-speaking lawyer admitted in Japan.** Square
brackets mark what only the owner or the lawyer can supply; nothing in them was guessed.

| File | Covers (section 1 item) | Placeholders left open |
|---|---|---|
| `TERMS_OF_USE.md` | 2, 3, 11 | company legal name, address, contact, effective date, version, hosted-server address, evaluation period / licence model and fees, liability cap, confidentiality terms for the hosted server, delivery channel, support, term, notice period, export/deletion of data on termination, change procedure, governing law, court, prevailing language |
| `PRIVACY_STATEMENT.md` | 8 | company legal name, address, contact, effective date, hosted-server address, hosting provider and location, legal exceptions to sharing, proxy-log retention, OS-log retention, whether the hosted server uses `deploy/nf-backup.sh`, procedure for deletion requests, change procedure |
| `ENGINEERING_RESPONSIBILITY_NOTICE.md` | 3 | company legal name, address, contact, effective date, how the vendor responds to reported errors |

Facts in the privacy statement were checked against the code on base `2bf369a` (re-check on the release commit):
tables `crates/nf-server/src/storage.rs:18-30`; sessions 12 h (`main.rs:854`, cookie `main.rs:859-860`, HttpOnly,
SameSite=Strict); failed sign-ins (user name, time, client address) removed after 15 min (`main.rs:648`, `791`,
`3775-3785`, hourly at `3952`); client address recorded in the audit detail of sign-in events (`main.rs:803`, `836`,
`857`), taken from the peer or a trusted proxy (`main.rs:3252-3261`); audit append-only with no deletion path outside
tests; users deactivated, never deleted (no `DELETE FROM users`); file contents AES-256-GCM (`crypto.rs:34`), file
names in clear in the database (`storage.rs:21`); desktop data folder `desktop.rs:74-108`, listens on 127.0.0.1 only
(`desktop.rs:5`); browser storage holds display preferences and form drafts without passwords (`web/app.js:36-40`,
`403-421`, `488-541`); no outbound-capable library (`tests/offline_check.sh:15`); hosted proxy log
(`deploy/Caddyfile:6-8`); backups keep 14 (`deploy/nf-backup.sh:13`).

### Open questions for the lawyer (summary; details at the end of each draft)

1. Commercial model, governing law, court, prevailing language, and whether the terms are 定型約款 (owner decides
   the model first).
2. How far the warranty disclaimer and limitation of liability hold under Japanese law (Consumer Contract Act,
   Product Liability Act), given that a DXF is still produced for review-required cases, marked NOT RELEASED;
   amount and form of any liability cap.
3. Whether "review by a qualified engineer before any use" can bind the customer, and how "qualified engineer"
   should be defined.
4. **No erasure:** the product cannot delete a user account or audit entries, and the audit log keeps user names and
   client network addresses indefinitely by design. Compatibility with deletion/suspension requests under the Act on
   the Protection of Personal Information; required retention period or pseudonymisation.
5. Disclosure of the unencrypted desktop database (copied data folder in the threat model) and its effect on the
   operator's safety-management duty.
6. Obligations of the owner as operator of the hosted evaluation server (purpose of use, contact point, proxy logs,
   backups, cross-border transfer if hosted abroad, entrustment agreement for customers' drawings).
7. Disclosure, ownership and registrability of code and engineering rules at evidence level EV-1, not independently reviewed (item 10).
8. Export notification for the cryptography outside Japan (item 11).
9. Where the notices must be shown to take effect (installer, first-run screen, each DXF, contract); the product
   has no acceptance screen today.

### Inconsistency found while drafting (not fixed here; task 4.5 is docs/legal only)

- `README.md` line 5 still claims automated drawing reading, which contradicts the owner decision of 20 Nov 2026
  (item 1: the product contains no automated drawing reading). It must be corrected before any draft is shown to a customer.
- `docs/security/DESKTOP_KEY_ASSESSMENT.md` section 6 says "secure backups"; the drafts say "protected backups"
  to keep rule A.3.
