# Pricing and Payment — NEXUS FABRIC

**Document:** `docs/business/PRICING_AND_PAYMENT.md` · **Status:** working business draft, not a decision record
**Seller:** VSNexus Technologies (founder resident in India) · **First market:** Japanese 製缶 / 板金 / ダクト / 配管 factories
**Possible channel:** Chuo-Sogo (Japanese procurement company; the founder is also its CBDO — see §7)

Everything below is either (a) a fact taken from the project files and the market research in `nf-logs/biz/competitors_japan.md` (read 2026-11-22), (b) a **proposal** by the author, clearly labelled, or (c) marked **to verify** with what to check and where. No fee percentage, tax rate, legal requirement or provider name that is not in those files is stated as fact.

---

## 0. What may be sold today — the constraint that shapes every price

| Fact (source) | Consequence for pricing |
|---|---|
| Development build; "Not validated. No output may be used for manufacturing." (`README.md`, `docs/RELEASE_READINESS.md`) | We may sell **evaluation and pilot participation only**. No SKU may be described as production, certified, validated or manufacturing-ready. |
| `APPROVED` is unreachable by design; every DXF is marked NOT RELEASED (gate G16, `docs/project/READINESS.md`) | A price cannot be justified by "released parts". It is justified by engineering time saved in review, refusal instead of guessing, and the audit record. |
| Fabrication review does not hold: 0 of 56 engineering statements reviewed; minimum K-factor and minimum seam clearance have no rule (`READINESS.md`) | Thickness / neutral-line / allowance features must not appear in a price list as included. |
| Windows `.exe` and installer never run on real Windows; only wine (`docs/PACKAGING.md`) | A paid Windows pilot needs a real-PC verification step first, or the pilot must run on the customer's Linux/browser or on our hosted evaluation server. |
| No LICENSE file; packages say "All rights reserved. Evaluation use only — terms to be provided by the owner"; terms/privacy/responsibility notices exist only as DRAFT, not reviewed by a lawyer (`docs/legal/LEGAL_READINESS.md`) | **No money may be taken before the founder supplies terms and a lawyer admitted in Japan reviews them.** |
| Database not encrypted at rest; open high findings V-21, V-22, V-23 (`READINESS.md`) | Sales wording must carry the approved plain statement; confidential customer drawings must not be requested for a pilot until the owner accepts the risk in writing. |
| Desktop mode makes no outgoing network calls, no telemetry, no updater (`docs/PACKAGING.md`) | Licence enforcement must work fully offline (§4). Any online check would break a tested product property. |
| Trademark search for "NEXUS FABRIC" not done (`LEGAL_READINESS.md` item 7) | Brand on an invoice or 見積書 is a founder risk until searched. |

**FOUNDER DECISION 0 — whether to charge at all before the release gates hold.**
Recommendation: charge only for a **paid pilot** (evaluation licence + engineering collaboration), keep a free evaluation tier, and take no production licence revenue until G-R12 (fabrication-engineer review) holds. Trade-off: early revenue and committed reference factories versus the reputational risk of a factory believing it bought a production tool.

---

## 1. Price anchors from the Japanese market (facts, as printed on the cited pages, 2026-11-22)

| Product | Model | Price as published | Notes |
|---|---|---|---|
| CADTOOL 板金展開9 | Perpetual, node-locked | **¥101,200 tax incl.** | Floating: 1 ¥185,900 / 3 ¥363,000 / 5 ¥533,500; upgrade ¥39,600; **no maintenance or subscription listed**; media only, no download sale |
| 展開WIN (大新技研) | Perpetual, quotation | **to verify** — no price published; extra shapes quoted per pattern | Check: request a quote via the vendor's inquiry form; 3-month full trial exists |
| Cutwise Unfold | One-time, Microsoft Store | **¥2,350** | 30-day trial; 34 shapes; mid-plane conversion, sheet-fit check, tiled print |
| 板金展開 7.5 (安田BTP) | Registration fee unlocks output | **¥4,510 (Vector) vs ¥4,000 (author)** — pages disagree, **to verify** | Check: Vector SR024000 page and the author's registration page |
| 展開図ラボ | Freemium web | Free; Pro **¥500/month** or **¥5,000/year**; drafting service **¥8,800 per sheet** | DXF save is free |
| 板金展開図メーカー / 山本建設 tool | Free web | ¥0 | SVG/CSV only, no DXF |
| Godo Nazca SQ ダクト展開 | Perpetual module + annual support | **¥110,000 tax incl.** + **¥88,000/year incl.** support | Requires Nazca SQ Laser CAD/CAM and first-year maintenance |
| SheetPartner (テイクソフト) | Suite, quotation | Band **¥1M–¥5M** (ipros) | IT導入補助金 2024 eligible, ½ up to ¥1.5M (as listed) |
| AMADA SheetWorks / VPSS, iCAD SML, Mitsubishi SMOPlus, Sigmatech, Kanpre | Quotation | **to verify** — not published | Check: vendor inquiry forms; AMADA states Japanese domestic market only |
| Autodesk Fusion | Subscription | ~**¥14,300/month**, **¥116,600/year**, **¥350,900/3 years** (secondary source) | **to verify** on Autodesk JP price page |
| Plate'n'Sheet V4 (AU) | Perpetual | **US$671** (reseller) | No Japanese UI; demo cannot print/export |
| Sheet Lightning v6 (UK) | Perpetual | **US$99 / US$150** single; **US$250 / US$375** site | Demo output disabled |
| Design2Fab (US) | Subscription | **US$79.99/user/month** | English only, online licence check |

**Reading of the anchors.** The 製缶 unfolder market splits into a ¥2,350–¥4,510 hobby/one-time band, a ~¥100,000 perpetual professional band (CADTOOL, Godo module before its ¥88,000/year support), and a ¥1M+ machine-tied suite band. There is a visible empty space between ¥5,000/year web tools and ¥101,200 perpetual desktop tools. The gap analysis in the research file already recommends: publish yen prices, monthly tier **below CADTOOL ¥101,200 and above Cutwise ¥2,350**, no dongle.

**Honesty constraint on positioning:** the same file lists six areas where competitors are ahead of us today (dimension basis / mid-plane, seam and lap allowance, sheet-fit and cost figures, shape count, tiled 1:1 printing, JWW). A price above Cutwise's must be justified by verification, refusal and the audit record — not by feature count.

---

## 2. Pricing model options

| # | Model | How it works | Anchor | Advantages for a Japanese factory | Disadvantages / risk |
|---|---|---|---|---|---|
| A | **Perpetual + annual maintenance** | One licence fee, optional yearly support/updates | CADTOOL ¥101,200 (no maintenance listed); Godo ¥110,000 + ¥88,000/yr | Matches 設備投資 habit; one 稟議 instead of yearly renewal; works offline forever | Cash arrives once; maintenance attach rate **to verify**; we must support old builds forever; refund exposure high |
| B | **Annual subscription per seat** | Yearly fee per named user, licence file expires (§4) | Fusion ¥116,600/yr; Design2Fab US$79.99/user/mo; 展開図ラボ ¥5,000/yr | Low entry cost; predictable renewal; matches our continuous verification work | Some small shops dislike 月額/年額; renewal 稟議 every year; expiry must never destroy access (§4) |
| C | **Per-site licence** | One fee for one factory, N seats | CADTOOL floating 5 ¥533,500; Sheet Lightning site US$250/375 | Simple for a 多品種少量 shop with several drafters; no seat counting arguments | Seat enforcement is harder offline; price must be justified per site |
| D | **Paid pilot (time-boxed)** | Fixed fee for 3 months, 1–2 seats, onboarding + reference-case work | No competitor pilot fee is published — **to verify** (none of the cited pages lists a paid pilot) | Factory can buy through a small 決裁 without a capital request; gives us RC-001 material and factory answers | Must be sold as evaluation, not production; delivery effort is real |
| E | **Free evaluation** | 30 days, full function, output marked NOT RELEASED | CADTOOL 15 days full; 展開WIN 3 months full; Cutwise 30 days | Removes the trial barrier; matches market expectation | 3 months (展開WIN) is the generous benchmark; a 15-day limit looks stingy here |
| F | **Free / open distribution** | No charge until gates hold | Mahiro, Yamamoto, Jw_cad 外変 are free | Zero payment/legal setup; fastest adoption | No revenue, no funded engineer review, support burden |

---

## 3. Recommendation — FOUNDER DECISION 1 (prices)

**Recommendation: model B (annual subscription per seat) as the standard, with model C as a site tier, entered through model D (paid pilot) and model E (free evaluation).** Model A (perpetual + maintenance) is held back until the release gates hold, because a perpetual buyer of an unvalidated tool is the worst mismatch we can create.

All figures below are **proposals by the author, not market facts**. They are placed deliberately between the ¥5,000/year web band and the ¥101,200 perpetual band. Tax status: **all figures proposed as tax-exclusive; whether Japanese consumption tax applies to a cross-border sale, and who must charge it, is to verify (§5).**

| SKU (proposed) | Scope | Proposed price (JPY, excl. tax) | Anchor reasoning |
|---|---|---|---|
| **Evaluation** | 1 seat, 30 days, all shipped families, every DXF NOT RELEASED | **¥0** | Cutwise 30 days; CADTOOL 15 days |
| **Pilot 3 months** | 1–2 seats, onboarding, one reference case worked through with the factory, written findings | **¥30,000** | Below CADTOOL's ¥101,200 by a wide margin; ~3× a 展開図ラボ drafting sheet (¥8,800) ×3 sessions; credited against the first annual fee |
| **Standard (per seat / year)** | 1 named user, offline licence file, updates during the term | **¥48,000** (≈¥4,000/month) | Under half of CADTOOL perpetual; well under Fusion ¥116,600/yr; far above Cutwise ¥2,350 one-time, justified by verification + audit |
| **Site (up to 5 seats / year)** | One factory, one 請求書 | **¥180,000** | vs CADTOOL floating 5 at ¥533,500 perpetual and Godo ¥110,000 + ¥88,000/yr |
| **Additional seat** | Added to a site licence mid-term, pro-rated | **¥36,000/yr** | 20% below the standalone seat price (proposal) |
| **Launch discount** | First 10 factories, first year only | **30% off Standard/Site** → ¥33,600 / ¥126,000; pilot fee credited in full | Compensates for the unvalidated status and the missing features listed in §1 |
| **Perpetual (later, not at launch)** | 1 seat + optional maintenance | **to verify / not offered yet** | Only after G-R12; anchor CADTOOL ¥101,200 and Godo's ¥88,000/yr support ratio |

Trade-offs the founder must weigh:

| Choice | Recommendation | Trade-off |
|---|---|---|
| Subscription vs perpetual | Subscription first | Recurring revenue funds the engineer review; but a factory that refuses 年額 loses to CADTOOL's one-off model |
| Price level | ¥48,000/seat/yr | Low enough for a 課長-level 決裁 in many shops (**to verify** with Chuo-Sogo purchasing contacts); high enough to fund review work. Lower price = faster adoption, weaker ability to pay for the fabrication engineer we must hire |
| Pilot fee | Charge ¥30,000, credit it | Filters out non-serious factories; but a fee before validation may be read as over-claiming — the 見積書 must say 評価版 |
| Launch discount | 30%, first 10, first year | Creates reference customers; sets a low anchor that is hard to raise later |
| Publishing the price list | Publish yen prices on the site | Opaque quoting is a documented market pain (D7); publishing removes negotiation room and exposes us to Cutwise's ¥2,350 comparison |
| IT導入補助金 | Do not claim eligibility | SheetPartner's 2024 eligibility is in the research file; **our eligibility is to verify** (check the subsidy secretariat's IT vendor registration requirements and current年度 rules). Never print a subsidy claim before registration is confirmed |

---

## 4. Offline licence enforcement (design proposal — not implemented today)

Nothing in `docs/PACKAGING.md` or the capability matrix describes licence enforcement; the packages currently carry no licence mechanism at all. This section is a design to be built and tested.

### 4.1 Licence file

| Item | Proposal |
|---|---|
| File | `licence.nflic`, placed in the data folder or uploaded through the interface |
| Format | Canonical JSON payload + detached **Ed25519** signature; public key compiled into `nf-server`; private signing key never shipped |
| Payload fields | licensee legal name (Japanese and romanised), licence id, edition (evaluation / pilot / standard / site), seats, issued-at, expires-at, grace days, update-entitlement-until, contract reference, signing key id |
| Verification | At every start, in both server and desktop mode, before the API accepts work; signature, edition, expiry, seat count |
| Network | **None.** Must keep the no-outbound property tested by `tests/offline_check.sh`; no activation server, no phone-home, no dongle (gap D7 recommendation) |
| Binding | Customer name + seats + expiry, **not** a hardware fingerprint at first. A device identifier would become personal data and change the privacy statement — **to verify** with the lawyer (`LEGAL_READINESS.md` item 8) |
| Seats | Server mode: count of active user accounts. Note the product **deactivates users, never deletes them** (`storage.rs` facts in `LEGAL_READINESS.md` §4) — seat counting must count active accounts only. Desktop mode: one seat, one data folder (the existing `desktop.lock` already refuses a second copy) |
| Renewal | Manual: customer receives a new `.nflic` by e-mail or through Chuo-Sogo and uploads it. There is no updater by design, so nothing can renew itself |
| Key custody | Who holds the signing key, and where the offline backup lives, is a **FOUNDER DECISION** (see §6): the signing entity must be the entity named in the contract |
| New dependency | An Ed25519 crate changes the dependency list: regenerate `packaging/THIRD_PARTY_NOTICES.txt` with `packaging/generate_notices.sh` and re-run `tests/notices_check.sh`, or the quality gate fails |

### 4.2 Behaviour on failure and expiry

| State | Behaviour (proposal) |
|---|---|
| No licence file | Evaluation edition; the existing development-build notices stay on every screen and DXF |
| Bad signature / edited file | Paid features off, evaluation mode on, clear Japanese and English message, event written to the audit chain. Never a silent downgrade |
| Valid, expiring in ≤30 days | Banner in the interface with the date and the renewal contact |
| Expired, within grace (**proposed 14 days**) | Full function continues; banner states the grace end date |
| Expired, grace over | **Read-only.** New calculations and new DXF exports stop. Existing records, audit chain, stored documents and previously generated DXF files remain openable and downloadable. **Nothing is deleted, ever.** No lockout of the account, no data-folder change |
| Clock moved backwards | Grace is not extended; the last-seen timestamp is kept in the append-only audit store. The program still opens; it does not accuse the user |
| Seats exceeded | Existing users keep working; creating or reactivating an additional active user is refused with a plain message and the sales contact |
| Licence removed while running | Re-checked at next start; the running session finishes and can save its work |

**Why read-only and not lockout:** the audit trail and the job records are the customer's engineering evidence. Blocking access to them would contradict the product's own retention design (audit append-only, users deactivated not deleted) and would create a legal argument we do not need.

**Tests to add before any paid licence ships:** signature valid/invalid/expired/grace, seat counting with deactivated users, clock rollback, read-only mode still opens old results, offline start with the licence present and absent, and a wording test in the existing style that fails if the interface or packages contain untrue marketing words (`LEGAL_READINESS.md` §2 item 2 lists "certified", "approved", "validated", "secure", "hack-proof").

**Not claimed:** licence enforcement is not cryptography for the customer's data. The database remains unencrypted (V-21); the licence file must never be described as protecting drawings.

---

## 5. Payment routes — India-based seller, Japanese company buyers

**Every provider name, fee, tax rate, registration threshold and filing requirement below is to verify.** None of the context files contains payment-provider terms, Japanese tax rules or Indian foreign-exchange rules. The table states the *structure* of each route and exactly what must be checked before it is used.

| Route | Seller of record to the factory | Japanese consumption tax + インボイス制度 | Indian export / FX paperwork | Payout to us | Refunds |
|---|---|---|---|---|---|
| **R1. Merchant of record (MoR) service** | The MoR | MoR's obligation and its Japanese registration status — **to verify**: does the provider register as a qualified invoice issuer in Japan, and does it issue 適格請求書 to Japanese companies? | We invoice the MoR; classification (software licence vs service vs royalty), GST treatment and any export declaration — **to verify** with an Indian chartered accountant | MoR pays our Indian bank on its own cycle; minimum payout, currency, FX spread — **to verify** on the provider's terms page | Handled by the MoR under its own policy; our contract with the MoR governs clawback — **to verify** |
| **R2. Card processor, we are the seller** | VSNexus (or the entity chosen in §6) | Our obligation if we sell into Japan; whether a foreign seller must register and can issue a qualified invoice — **to verify** with a Japanese tax accountant (税理士) | Same export classification question; inward card settlement via a payment aggregator and its FX reporting — **to verify** with the bank | Processor settlement to an Indian account; supported countries and currency — **to verify** | We refund through the processor; chargeback exposure sits with us |
| **R3. International bank transfer against a Japanese-style 請求書** | VSNexus (or chosen entity) | Our invoice must satisfy the buyer's input-tax-credit needs; whether we can issue a qualified invoice as a foreign seller, and what the buyer does without one — **to verify** with a 税理士 and with the factory's 経理 | Inward remittance documentation, purpose classification, any software-export declaration, GST export treatment — **to verify** with the CA and the bank's export desk | Direct wire, JPY or USD; correspondent-bank fees and who bears them — **to verify** with the bank | We wire back; bank fees and FX loss on refunds — **to verify** |
| **R4. Distributor / reseller — Chuo-Sogo** | **Chuo-Sogo** | Chuo-Sogo issues the Japanese invoice; its qualified-invoice registration number and status — **to verify** with Chuo-Sogo's 経理 | We invoice Chuo-Sogo (one cross-border B2B customer, not many); same export/FX questions as R3, but one recurring counterparty | Chuo-Sogo collects yen from factories and pays us wholesale; margin, currency, payment site — **to verify / negotiate** | Chuo-Sogo handles the factory refund; our reseller agreement defines who bears it |
| **R5. Agent / referral — Chuo-Sogo introduces, we invoice** | VSNexus (or chosen entity) | Same as R3 | Same as R3 | We receive the full amount and pay a referral fee to Chuo-Sogo; fee level and its tax treatment — **to verify** | We refund; agent fee clawback — **to verify** |
| **R6. No payment at all (free evaluation)** | — | None | None | None | None |

**Recommendation — FOUNDER DECISION 2 (route).**
Start with **R4 (Chuo-Sogo as reseller of record)** for the first factories, with **R3 (direct invoice + bank transfer)** as the fallback for a factory that will not buy through a distributor. Add **R1 (MoR)** only later, for self-service evaluation-to-paid conversion.

Trade-off: R4 removes almost all Japanese invoicing, collection and language friction and gives the factory a familiar 請求書 from a Japanese company, at the cost of a distributor margin and of dependence on a related party (§7). R3 keeps the full margin but pushes Japanese tax and invoice questions onto us and onto a minor's entity (§6). R1 is the least paperwork per sale but takes a percentage and may not support Japanese purchase-order terms at all — **to verify**.

### 5.1 Japanese B2B order-to-cash flow to confirm with Chuo-Sogo

The steps below are the flow to **confirm**, not a stated fact about any specific factory.

| Step | Document | What to confirm (**to verify**, with Chuo-Sogo purchasing and the factory's 経理) |
|---|---|---|
| 1 | 見積書 (quotation) | Whether an evaluation licence line item is acceptable; validity period; tax status wording |
| 2 | 発注書 / 注文書 (PO) | Whether a PO is mandatory before delivery; supplier registration (取引先登録) forms, bank details, anti-social-forces clauses |
| 3 | 納品 (delivery) | What counts as delivery for a licence file: e-mail of `.nflic`, or a 納品書 PDF |
| 4 | 検収 (acceptance) | Acceptance criteria and period — critical: our product is an evaluation build, so acceptance must be defined against evaluation deliverables, not manufacturing output |
| 5 | 請求書 (invoice) | Required fields, whether a qualified invoice registration number is demanded, 締め日 and 支払サイト (month-end close, payment terms), 振込手数料 bearer |
| 6 | 入金 (payment) | Currency, bank, correspondent fees, remittance advice reference |
| 7 | 更新 (renewal) | Whether renewal needs a fresh PO each year — this decides how early the renewal `.nflic` must be sent |

Also **to verify** with a Japanese lawyer/税理士: stamp duty (印紙) on a licence or distributor agreement, withholding on cross-border licence payments, and whether our terms are 定型約款 (already an open lawyer question in `LEGAL_READINESS.md` §4).

---

## 6. Who signs and who receives money — the founder is a minor

**FOUNDER DECISION 3 (entity and signatories).** No option below can be chosen by the development lead. What each option needs is listed; every legal requirement is **to verify** with an Indian corporate lawyer / CA and, for the Japanese side, a lawyer admitted in Japan (already required by `LEGAL_READINESS.md` §3).

| Option | Who signs the customer contract | Who receives the money | What it needs | Main risk |
|---|---|---|---|---|
| **O1. Guardian-signed sole proprietorship** | A parent/guardian as proprietor, contracting for the minor's business | Guardian's business bank account | Whether a minor may own a business and a guardian may contract on their behalf; capacity and ratification rules; bank account opening rules; PAN/GST registration in the proprietor's name — **to verify** (Indian contract and company law counsel; the bank) | Personal liability sits with the guardian; the arrangement may be voidable — **to verify** |
| **O2. Indian private limited company** | Adult directors authorised by the board; the company is the seller | Company current account | Incorporation with at least the required number of adult directors; whether a minor may hold shares and through whom; registered office, statutory filings, audit; export/GST registrations — **to verify** (corporate lawyer + CA) | Setup cost and compliance load before any revenue; the minor cannot direct the company — **to verify** |
| **O3. Japanese entity (new or existing)** | Its representative director | Japanese bank account | Incorporation or use of an existing company, a resident representative, visa/status questions for the founder, corporate registration and tax filings — **to verify** (Japanese lawyer, 司法書士, 税理士) | Highest setup burden; if an existing company is used, related-party terms must be documented |
| **O4. Chuo-Sogo as seller of record (R4)** | Chuo-Sogo's authorised signatory signs the factory contract; a guardian-signed or company-signed VSNexus entity signs the upstream licence to Chuo-Sogo | Chuo-Sogo collects from factories; pays us under the distribution agreement | A written distribution agreement with arm's-length terms; board approval at Chuo-Sogo; disclosure of the founder's dual role; still needs O1 or O2 for our side | Conflict of interest (§7); dependence on one channel |
| **O5. Defer all revenue** | Nobody; free evaluation only | Nothing | Nothing beyond the current DRAFT terms and the honest notices | No funding for the fabrication-engineer review that gates the whole product |

**Recommendation:** **O4 on top of O2** — incorporate an Indian company with adult directors as the licensor, and let Chuo-Sogo be the seller of record to Japanese factories. If incorporation is too slow, use **O1 + O4** as a bridge with a guardian-signed distribution agreement, and **O5** until either is in place.
Trade-off: O2 + O4 gives a clean counterparty, limited liability and a Japanese invoice the factory trusts, but costs setup time and a distributor margin. O1 is faster but puts a parent's personal liability behind an unvalidated engineering product. O5 is safest and slowest.

**Signing key custody follows the entity:** the Ed25519 licence-signing key (§4) must be held by the entity named in the contract, controlled by an adult signatory, with an offline backup. A key held only by the minor is a continuity risk — **to verify** how the chosen entity documents key custody.

**Also founder-only:** the EULA and liability cap, governing law and court, the trademark search, code signing for Windows, whether to publish releases at all, and the price list (`LEGAL_READINESS.md` §3, `docs/PACKAGING.md` open items).

---

## 7. Chuo-Sogo channel — conflict of interest must be handled first

| Fact | Issue | Action |
|---|---|---|
| The founder is CBDO of Chuo-Sogo and owner of the licensor | Related-party transaction on both sides of the same contract | Written disclosure of the dual role to Chuo-Sogo's board; approval recorded by a person who is not the founder; the founder does not sign both sides |
| Chuo-Sogo is a procurement company, not a software reseller | Whether software resale fits its business, contracts and invoicing | **to verify** with Chuo-Sogo management: business scope, existing supplier agreements, whether it can register as a software reseller |
| Distributor margin | No number exists in any context file | **to verify / negotiate**; do not print a margin percentage anywhere until agreed in writing |
| Support and liability split | Our terms say the customer must have a qualified engineer review every result; a reseller must pass that through unchanged | Reseller agreement must obligate Chuo-Sogo to deliver the DRAFT-reviewed terms, the NOT RELEASED notice and the security wording unaltered |
| Exclusivity | Whether Chuo-Sogo asks for exclusive rights in Japan | **FOUNDER DECISION**; recommendation: no exclusivity, or exclusivity limited to a named account list for 12 months. Trade-off: channel commitment versus being locked to one related party |

---

## 8. Setup checklist — in order

| # | Step | Owner | Done when |
|---|---|---|---|
| 1 | Confirm what may be sold: evaluation and pilot only, no manufacturing claim | Founder + lead | Written scope note referencing `RELEASE_READINESS.md` and gate G-R12 |
| 2 | Decide the entity and the adult signatories (§6) | **FOUNDER DECISION** | Option chosen in writing; lawyer/CA engaged |
| 3 | Incorporate or register the chosen entity; open the business bank account | Founder + guardian/directors | Registration documents and account confirmed by the bank |
| 4 | Engage a lawyer admitted in Japan; hand over the three DRAFT documents | Founder | Engagement letter; review started on terms, privacy, engineering-responsibility notice |
| 5 | Ask the lawyer the open questions already listed (liability cap, 定型約款, no-erasure of audit data, unencrypted desktop DB, export of cryptography, AI-assisted code ownership → rephrase as "code ownership and disclosure") | Founder + lawyer | Written answers; placeholders in the drafts filled |
| 6 | Order the trademark search for the product name in Japan | **FOUNDER DECISION** | Search report; registration decision |
| 7 | Decide the price list and the launch discount (§3) | **FOUNDER DECISION** | Signed internal price sheet, tax status stated |
| 8 | Decide the payment route (§5) and, if R4, disclose the conflict and get Chuo-Sogo board approval | **FOUNDER DECISION** | Route chosen; disclosure minuted |
| 9 | Verify the Japanese tax and invoice position with a 税理士 for the chosen route | Founder + 税理士 | Written answer on consumption tax, qualified invoice, withholding, stamp duty |
| 10 | Verify the Indian export/FX position with a CA and the bank's export desk | Founder + CA | Written answer on classification, declarations, GST export treatment, inward-remittance documents |
| 11 | Confirm the order-to-cash flow with Chuo-Sogo and one target factory (§5.1) | Founder / Chuo-Sogo | 見積書→発注書→納品→検収→請求書→入金 agreed, with 締め日 and 支払サイト |
| 12 | Sign the distribution or agency agreement (arm's-length, non-founder signatory on the Chuo-Sogo side) | Founder + Chuo-Sogo | Executed agreement; margin and refund responsibility written |
| 13 | Finalise and publish the customer terms; add the acceptance step to the installer / first-run screen | Lead + lawyer | No "terms to be provided by the owner" text remains in any package |
| 14 | Verify the Windows build on a real Windows PC (installer, SmartScreen, first run, uninstall) | Lead | Real-PC results recorded; `docs/PACKAGING.md` "not tested" list updated |
| 15 | Decide code signing and obtain a certificate, or publish the SHA256 verification instruction prominently | **FOUNDER DECISION** | Signed binaries or an explicit, tested user instruction |
| 16 | Build the offline licence mechanism (§4): Ed25519 file, start-up check, grace, read-only expiry | Lead | Unit + integration tests; offline check still passes; notices regenerated and `tests/notices_check.sh` green |
| 17 | Set up signing-key custody under the chosen entity, with an offline backup | Founder + adult signatory | Custody note; a test licence issued and verified on a clean machine |
| 18 | Add the wording test that fails on untrue marketing words in interface, packages and price list | Lead | Test in the quality gate |
| 19 | Prepare the pilot package: 見積書 template, pilot scope, deliverables, acceptance criteria, security wording | Lead + Chuo-Sogo | One factory signs a pilot PO |
| 20 | Run the first paid pilot; collect factory answers (CAM/DXF requirements, dimensioning convention, process parameters) and RC-001 material | Lead + factory | Pilot report; inputs for the fabrication-engineer review |
| 21 | Only after G-R12 and the security decisions: consider a production SKU and a perpetual model | **FOUNDER DECISION** | New price sheet with evidence behind every claim |

---

## 9. "To verify" register (nothing here may be printed as fact until checked)

| Item | Where to check | Owner |
|---|---|---|
| 展開WIN price; AMADA / iCAD / Mitsubishi / Sigmatech / Kanpre prices | Vendor inquiry forms (URLs in `competitors_japan.md` §6) | Lead |
| 安田BTP registration fee (¥4,000 vs ¥4,510) | Author's page and Vector SR024000 | Lead |
| Fusion JP prices | Autodesk JP official price page (research used a secondary source) | Lead |
| MoR / card-processor fees, supported countries, payout terms, Japanese invoice capability | Each provider's own published terms and supported-countries page | Founder |
| Japanese consumption tax on a cross-border software licence; qualified invoice registration for a foreign seller; withholding; stamp duty | 税理士 and the National Tax Agency's published guidance | Founder + 税理士 |
| Indian export classification, FX/remittance documentation, GST export treatment, any software-export declaration | Indian CA and the bank's export desk | Founder + CA |
| Minor's capacity to contract / hold shares; guardian signing; director requirements | Indian corporate lawyer | Founder |
| Chuo-Sogo: reseller capability, invoice registration status, margin, payment site | Chuo-Sogo management and 経理 | Founder |
| Factory purchasing flow: PO necessity, 締め日, 支払サイト, 振込手数料, supplier registration | Target factory's 購買 / 経理, via Chuo-Sogo | Founder |
| IT導入補助金 eligibility for our product in the current年度 | The subsidy secretariat's IT vendor registration requirements | Founder |
| Whether a device fingerprint in the licence file becomes personal data | Lawyer, against the privacy statement draft | Founder + lawyer |
| Refund and cancellation policy for an evaluation licence | Lawyer; MoR/reseller terms | Founder |

---

## 10. Statements that must never appear in a price list, quotation or invoice

- "Production-ready", "validated", "certified", "approved", "manufacturing-ready" — no result can reach APPROVED; every DXF is NOT RELEASED.
- Any thickness, neutral-line, K-factor, kerf or allowance capability — 0 of 56 engineering statements are reviewed; two rules do not exist yet.
- "Works with AMADA / Mitsubishi / Nazca" or any CAM acceptance — DXF acceptance by CAM is untested.
- "Encrypted", "secure", "hack-proof" without the approved qualification — the database is not encrypted at rest.
- "Verified on Windows" — the installer has run only under wine.
- Any subsidy eligibility, tax rate, provider fee or legal requirement from §9 before it is verified.
- Any claim that a shape family we do not ship is included — the shipped families are cylinder shell, cone frustum, eccentric reducer, branch stub, header with one branch opening.

**One-line positioning that is true today and worth ¥48,000/year to the right factory:** it refuses instead of guessing, it shows what it could not verify, and it keeps a record of every decision — and it still requires a qualified engineer before anything is cut.
