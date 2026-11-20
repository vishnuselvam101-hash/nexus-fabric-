# NEXUS FABRIC — legal readiness (working list, not legal advice)

Date: 20 Nov 2026. Prepared by the development lead as a list of facts, gaps and drafts for a qualified lawyer.
**No software can be made "legally proof", and nothing in this file is a legal opinion.** What can be done is: remove
avoidable risks, tell users the truth, respect other people's licences, and have a lawyer admitted in each market
(first market: Japan) review the terms before the software is sold. Until that review, the product stays "evaluation
use only".

## 1. Status

| # | Area | Status today | What is needed | Who |
|---|---|---|---|---|
| 1 | Truthful statements (no misleading representation) | Largely in place: every screen, DXF, installer and film says development build, not validated, not for manufacturing; no result can be approved; planned items are labelled or absent | Keep it enforced by tests. **Owner decision 20 Nov: the product contains no AI — the words "AI-native" and any AI feature announcement are removed from the film and must be removed from every other document and page.** | Lead (in progress) |
| 2 | Licence to the customer (EULA / terms of use) | **Missing.** The packages say "All rights reserved. Evaluation use only — terms to be provided by the owner." | A written licence: what the customer may do, evaluation limits, no warranty of fitness for manufacturing, limitation of liability, governing law and court, termination, confidentiality. A draft can be prepared here; a lawyer must finalise it, in Japanese for Japan | Owner + lawyer |
| 3 | Limitation of liability and engineering responsibility | Statements exist in the product (review required, not released, engineer responsible), but they are not contract terms | Contract clauses stating that results must be reviewed by a qualified engineer before use and that the vendor does not approve parts; a lawyer must say how far such clauses hold under Japanese law (consumer and product-liability rules cannot simply be excluded) | Lawyer |
| 4 | Third-party software licences | **Checked 20 Nov:** 102 third-party libraries are compiled into the program; all are under permissive licences (MIT, Apache-2.0, BSD-3-Clause, Zlib, Unicode-3.0, Unlicense, BSL-1.0); one offers an LGPL option but also MIT/Apache, which is the one used; no GPL or other copyleft-only component. SQLite (public domain) is bundled | **Gap: the packages do not yet ship the required licence texts and copyright notices.** MIT and Apache-2.0 require them. A THIRD_PARTY_NOTICES file generated from the build must be added to every installer and package | Lead (queued) |
| 5 | Fonts, voices and music in the films | Fonts: Inter, Noto Sans JP, JetBrains Mono (open font licences; licence files to be confirmed in the film folders). Voices: Kokoro (model stated as Apache-2.0 by its package; model card not checked) and VOICEVOX Nemo (credit「VOICEVOX Nemo」required and shown). Music: synthesised in code, original | Confirm the font licence files are shipped with the film sources; open and read the VOICEVOX Nemo terms page and the Kokoro model card before publishing (not done: no downloads were allowed) | Owner / lead |
| 6 | Film style | Inspired by product-launch films in general; no third-party logo, typeface, device image, footage or wording is used | Keep it that way; do not name or imitate a specific company's advertisement | Lead |
| 7 | Names and trademarks | Not checked. "NEXUS FABRIC", "AMOEBA" and the mark have not been searched in any trademark register | Trademark search and, if clear, registration in Japan (and other markets) before public launch | Owner + trademark attorney |
| 8 | Personal data | The software stores user names, password hashes (Argon2) and an audit log on the customer's own machine or server. No telemetry; nothing is sent anywhere (tested: runs with the network removed) | A short privacy statement saying exactly that; for the hosted server, the operator (the owner) is the one holding the data and needs a privacy policy under Japan's personal-information law | Owner + lawyer |
| 9 | Security statements | The database is not encrypted; desktop key handling is weak (see docs/security). Owner approved plain wording | Put the approved wording in the install guide and first-run screen (queued). Never use "secure", "encrypted" or "hack-proof" without the qualification | Lead (queued) |
| 10 | Source of the code | The software was written with AI assistance and says so (evidence level EV-1: "AI-generated, not independently reviewed") | Ask the lawyer about ownership and registration of AI-assisted code in the target markets, and keep the disclosure | Lawyer |
| 11 | Export and sanctions | The program contains standard encryption (AES-GCM, Argon2, SHA-2) | Ask whether any export notification applies when distributing outside Japan | Lawyer |
| 12 | Open pull request and repository | Repository is private; one pull request is open | Decide the licence of the repository before giving anyone access | Owner |

## 2. What the lead will do without waiting (no legal judgement needed)

1. Generate THIRD_PARTY_NOTICES from the exact build and ship it in every package (item 4).
2. Remove every AI claim from the film, the interface texts and the documents (item 1), and add a test that fails if
   the interface or packages contain marketing words that are not true ("AI-native", "certified", "approved",
   "validated", "secure", "hack-proof").
3. Put the approved desktop-security wording into the install guide and first-run screen (item 9).
4. Prepare DRAFT terms of use, privacy statement and engineering-responsibility notice in English and Japanese, each
   marked "DRAFT — not reviewed by a lawyer", for the owner's lawyer to correct.

## 3. What only the owner can do

- Engage a lawyer admitted in Japan (and in any other country of sale) to review items 2, 3, 8, 10, 11.
- Order the trademark search (item 7).
- Read the VOICEVOX Nemo terms and the Kokoro model card, or choose other voices (item 5).
- Decide the commercial model (evaluation, paid licence, subscription), because the terms depend on it.
