# CUSTOMER ROLLOUT — Japanese fabrication factories

Owner: VSNexus Technologies (founder resident in India). First market: Japan (製缶 / 板金 / ダクト / 配管 shops).
Status of the product today: **development build. Not validated. No output may be used for manufacturing. Not for confidential drawings.**
This document is a plan, not a claim. Every number that is not in the project records is written **to verify**. Nothing here may be shown to a customer until the wording rules in §2 are met.

Sources used: `docs/project/READINESS.md`, `docs/project/MASTER_GOAL.md`, `docs/RELEASE_READINESS.md`, `docs/PACKAGING.md`, `docs/legal/LEGAL_READINESS.md`, `README.md`, `/root/nf-logs/biz/competitors_japan.md` (2026-11-22).

---

## 1. Readiness gate — what must be true before ANY customer uses an output for manufacturing

Readiness is stated in five independent classifications (owner decision, 20 Nov 2026). Passing one never implies passing another.

| Classification | State today | What must change before manufacturing use | Evidence to produce |
|---|---|---|---|
| Software verified | Holds for commit `ed4a5fb` (263 unit/integration, 89 API, 66 security, 194 browser, 18 desktop checks; clippy/fmt clean) | Re-run on the release commit; CI workflow `quality-gate.yml` actually run on GitHub (not yet run) | CI run link + `packaging/verify_release.sh` manifest |
| Security reviewed | **Does not hold.** Deeper audit 35/40; open high: V-21 database not encrypted, V-22 key management, V-23 no multi-factor sign-in or recovery; server has no TLS (V-20) | Either close the findings or deploy only under the owner-approved **restricted deployment** class: OS disk encryption, individual OS accounts with restricted access to the data folder, documented backup and restore, no claim of application-level encryption | `docs/security/VULNERABILITY_REGISTER.md`, `DESKTOP_KEY_ASSESSMENT.md`, signed owner acceptance |
| Geometry verified | **Not claimed.** FULL corpus on 0.4.3: 1398 cases, 977 agree, 421 refused, 0 disagree, 0 wrongly accepted, 33/33 planted faults detected — but verdict FAIL on two INCONCLUSIVE product checks; `eps_num`/`eps_rep` are not proven bounds (G08 INCONCLUSIVE on every job) | Tasks 1.2–1.9: proven error bounds, conventions in the DXF (1.3), surface attribute (1.4), check from the two DXFs alone (1.5), tangent vertices (1.6), full corpus in CI (1.8), third independent review (1.9) | `validation/results/latest.json`, `COMPUTATIONAL_VALIDATION_REPORT.md` §10, review C follow-up |
| Fabrication reviewed | **Does not hold.** 0 of 56 statements reviewed; every algorithm EV-1. Two rules do not exist: minimum K-factor (neutral-axis position) and minimum seam clearance in the flat pattern | A qualified fabrication engineer classifies H1–H18 / RT1–RT10 and supplies the two missing rules; reference case RC-001 taken in (prepared, **no material received**) | `FABRICATION_ENGINEERING_REVIEW_PACKAGE.md` signed; RC-001 intake records |
| Manufacturing approved | **Does not hold and cannot.** Engineer approval not implemented; `APPROVED` unreachable; every DXF marked NOT RELEASED | Implement gate G16 approval workflow (plan 5.6) and complete it for a real result | Approval record in the job evidence |

Additional hard blockers before a customer machine is touched:

| # | Blocker | Present state | Required action |
|---|---|---|---|
| B1 | Windows never tested on real Windows | exe and installer run only under **wine 9.0**; SmartScreen, console close, `explorer.exe` browser launch, uninstaller data-deletion checkbox untested; port fallback not exercised end to end | Run `packaging/verify/windows_checks.sh` equivalents on a real Windows 10/11 PC, Japanese locale, non-admin account; record results. A wine result is never described as a Windows result |
| B2 | Files not code-signed | Windows shows "Windows protected your PC" | **FOUNDER DECISION** — buy a code-signing certificate or ship unsigned with SHA256SUMS instructions. Recommendation: sign before paid early access; trade-off is cost and lead time (**to verify**: certificate price and issuance requirements) |
| B3 | No licence terms | Packages say "All rights reserved. Evaluation use only — terms to be provided by the owner"; no LICENSE file; no acceptance screen in the product | Lawyer-admitted-in-Japan review of the three drafts (`TERMS_OF_USE.md`, `PRIVACY_STATEMENT.md`, `ENGINEERING_RESPONSIBILITY_NOTICE.md`, drafted 21 Nov 2026, all placeholders open) |
| B4 | Japanese documents | Interface EN/JA exists; install guide EN/JA exists; legal drafts' Japanese **needs review by a native-speaking lawyer**; native review of UI is Phase 6 / 5J.1, 5J.5 | Native review of quick-start, installer text, first-run screen, DXF text, legal notices |
| B5 | Drawing input claim | `MASTER_GOAL.md`: DXF/drawing import **not implemented**; documents are stored encrypted and never parsed. The competitor note lists PDF→DXF as DONE 3.1 but "converter not yet in downloadable packages" | **to verify** against the release commit which is true. Until verified, present the product as **typed-dimension input only**. No parser without an isolated, network-less, resource-limited process and fuzz tests (V-26) |
| B6 | Outdated claim in `README.md` line 5 | Contradicts the owner decision of 20 Nov 2026 about the product's components (recorded in `LEGAL_READINESS.md` §4) | Correct before any customer sees the repository or a brochure; keep the wording test that fails on untrue marketing words |
| B7 | Third-party notices | `PACKAGING.md` says `THIRD_PARTY_NOTICES.txt` ships in every package and is checked by `tests/notices_check.sh`; `LEGAL_READINESS.md` (20 Nov) still lists it as a gap | **to verify** on the release commit that the file is present in the Linux tarball, the portable zip and the installed Windows folder |

**Rule until G-R12 holds: no output may be used for manufacturing, whatever the other gates say.** Every pilot agreement must repeat this in Japanese and English.

---

## 2. Wording rules — what we may and may not say

| May say (evidence exists) | May not say |
|---|---|
| Development build; not validated; outputs marked NOT RELEASED | "production-ready", "validated", "certified", "approved", "secure", "hack-proof" |
| Independent verification engine; refuses instead of guessing (41+ refusal codes); INCONCLUSIVE is shown, never hidden | Any accuracy figure or error bound as a guarantee — bounds are not yet proven (task 1.2) |
| Families shipped: cylinder shell, cone frustum, eccentric reducer, branch stub, header with one branch opening | A shape count comparable to CADTOOL's 286 patterns; "any shape" |
| Runs offline on loopback only; no telemetry; no updater; audit chain and job evidence with hashes | "encrypted database" (V-21 open); "complete encryption" |
| English/Japanese interface; per-user Windows installer, no admin rights | "works on Windows" until B1 is closed; "works with Amada / Mitsubishi / Nazca CAM" until a person tests it (plan rule 5B.7) |
| DXF R12 subset, read back by the product's own reader | "CAM-accepted DXF" — acceptance untested; independent reader in progress (G-R4) |

Competitor comparisons stay **internal**. Trial EULAs were not printed on any vendor page (**UNVERIFIED**), and many commercial EULAs forbid publishing benchmark results without consent. Only CADTOOL 板金展開9 (15 days) and 展開WIN (3 months) export DXF during their trials, so even an internal DXF-level comparison is limited to those two.

---

## 3. Phases

### Phase 0 — Internal readiness (now)

Entry: today. Exit: B1–B7 closed or explicitly accepted in writing by the founder; §1 geometry tasks 1.2–1.6 done; engineer engaged.

### Phase 1 — Design partners (2–3 factories)

| Item | Content |
|---|---|
| Who | 2–3 Japanese 製缶/板金 shops that draft in Jw_cad or a 2D CAD and cut cylinders, cones, reducers, branches. Recruit through Chuo-Sogo relationships (§4) and one direct-outreach list |
| Commercial form | **FOUNDER DECISION** — free pilot vs paid pilot. Recommendation: **free, time-boxed, written agreement**, because nothing can be released for manufacturing and we are buying engineering review, not revenue. Trade-off: no revenue signal, and free pilots can drift; cap it at a fixed number of weeks and a fixed number of parts (numbers **to verify** with the founder) |
| Written agreement must contain | (a) development build, not validated; (b) **every output is checked by the partner's own qualified engineer before any use**, and the partner's engineer is responsible for the part; (c) no manufacturing release by VSNexus; (d) liability wording from `ENGINEERING_RESPONSIBILITY_NOTICE.md` after lawyer review; (e) confidentiality and drawing-handling terms; (f) permission to use anonymised findings internally; (g) data deletion/return terms — note the product **cannot delete user accounts or audit entries** by design |
| Data handling | Non-confidential or anonymised drawings only until the security classification changes. Prefer the partner's own PC in desktop mode (loopback only, no outbound calls) over the hosted server (no TLS, V-20) |
| What we ask for | Their dimensioning convention (内寸/外寸/中立), plate thickness practice, seam and lap practice, CAM system and DXF layer requirements, standard sheet sizes used (サブロク/シハチ/ゴトー), and classification of the 56 statements |
| What we give | Installer + SHA256SUMS, Japanese quick-start, three sample projects (§5), 30-minute demo, one training session, named support contact in JST |
| Entry criteria | B1, B3, B4 closed; engineer engaged for H1–H18/RT1–RT10; restricted-deployment wording accepted; support rota staffed |
| Success metrics (targets are proposals — **FOUNDER DECISION** to set numbers) | ≥1 partner engineer completes review of the 56 statements; ≥1 real drawing taken in as a controlled reference case (RC-001 or successor); every partner-reported defect reproduced, registered and either fixed with a regression test or explicitly accepted; zero cases where a partner used an output for cutting without their own check; partner states in writing whether they would pay |

### Phase 2 — Paid early access

| Item | Content |
|---|---|
| Entry criteria | Phase 1 metrics met; geometry-verified classification claimed with evidence (tasks 1.2–1.9); fabrication-reviewed classification holds for the shipped families; security findings closed or restricted-deployment class formally accepted per customer; Windows verified on real hardware; code signing decided; lawyer-approved Japanese terms and privacy statement; price list in yen published (**FOUNDER DECISION**) |
| Form | Named-user or per-seat licence, evaluation-then-paid. **FOUNDER DECISION**: perpetual vs monthly. Market anchors in §8. Recommendation: publish a yen price list with a monthly tier and no dongle, positioned below CADTOOL ¥101,200 and above Cutwise ¥2,350 — the exact figures are the founder's to set, not ours to invent |
| Still true in this phase | Engineer approval (G16) may still be unimplemented; if so, outputs remain NOT RELEASED and the customer's engineer approves. This must be on the price list, not only in the manual |
| Success metrics (proposals) | Renewal/continuation rate; number of parts verified per shop per week; refusal rate and how many refusals the shop resolved by correcting input vs. by asking us; support tickets per shop per month; time from install to first verified pattern; zero unresolved high defects at each release |

### Phase 3 — General availability

| Item | Content |
|---|---|
| Entry criteria | All G-R1…G-R12 hold, including G-R12 (fabrication-engineer review, authorised approval, independent mathematical review); approval workflow implemented and completed for at least one real result; security "production security target" class met (encrypted database, OS-protected keys, customer-controlled recovery key, key rotation, secure backup/restore, tests against a copied data folder); reproducible signed builds; trademark search done; export question answered by counsel |
| Success metrics (proposals) | Paying shops; churn; verified parts per shop; mean time to answer a support question in JST; defect escape rate into the field; audit-chain integrity checks passing at every release |
| Honest constraint | GA is not a marketing date. It is the date the last gate closes. Do not announce it in advance |

---

## 4. Channels

| Channel | What it is | What we need from ourselves first | What only the founder can decide |
|---|---|---|---|
| **Chuo-Sogo relationships** | The founder is CBDO of Chuo-Sogo, a Japanese procurement company; existing factory relationships are the shortest path to 2–3 design partners | Japanese one-page introduction, demo, sample DXFs, the engineering responsibility notice | **FOUNDER DECISION** — whether Chuo-Sogo acts as introducer, agent or reseller; a written related-party arrangement disclosing the founder's dual role; any commission or margin (**to verify**: no percentage exists in our records). Recommendation: start as **introduction only, no fee**, and put the dual role in writing to each factory. Trade-off: slower revenue for Chuo-Sogo, but no hidden conflict in a safety-relevant sale |
| **Direct outreach** | Small shops that live in Jw_cad; forum evidence shows unmet demand for a thickness-aware 展開 tool and shops still hand-fitting branches at the bench | Japanese brochure, 3-minute demo film, sample project pack, a reply address in Japanese | **FOUNDER DECISION** — outreach budget and whether to name competitors (recommendation: never name them in public material) |
| **Trade shows** | Sheet-metal / 製缶 / laser-cutting exhibitions in Japan | Booth or visitor plan, Japanese brochure, offline demo laptop (desktop mode works with no network), 見積書 template | **to verify**: exhibition names, dates, booth cost, application deadlines — check the organiser sites (e.g. MF-Tokyo / 日本工作機械工業会 / 日本鍛圧機械工業会 event pages) before any commitment. No name or date is asserted here |
| **Distributor / reseller** | CAD resellers already selling to these shops (the market has an established reseller layer around SOLIDWORKS, Amada, Nazca) | Reseller kit: price list, margin structure, demo licence, training material, support boundary in writing | **FOUNDER DECISION** — reseller margin (**to verify**, none in our records), exclusivity, who owns first-line support. Recommendation: no reseller before Phase 2 metrics are met; a reseller cannot answer engineering questions we cannot answer ourselves |
| **Subsidy route** | IT導入補助金 exists in this market (a competitor product is listed as 2024-eligible, ½ up to ¥1.5M) | Nothing yet | **to verify** with the 補助金事務局 whether our product and licence model can qualify; do not claim eligibility until confirmed |

### Collateral checklist (all Japanese-first)

| Item | State | Owner |
|---|---|---|
| Japanese brochure (1 page + 4 page) | Not created. Must carry the development-build wording and the family list, not a shape count | Lead + native review |
| Demo film (3 min) and 30-min live script | Launch film exists but is under revision; wording test must pass first | Lead |
| Price list in yen | **Does not exist. FOUNDER DECISION.** Tax status (税込/税抜) must be stated — **to verify** with an accountant how a foreign vendor charges Japanese consumption tax | Founder + accountant |
| 見積書 template (Japanese quotation) | Not created. Needs company legal name, address, contact — all still placeholders in the legal drafts | Founder supplies entity details |
| 請求書 / invoice and payment route | **to verify**: invoicing entity (India or Japan), currency, bank or processor fees, withholding tax on cross-border licence fees — none of these figures exist in our records | Founder + accountant |
| Install guide EN/JA | Exists (`packaging/INSTALL.md`) | Lead |
| Quick-start JA, three sample projects | Not created (§5) | Lead |
| Terms, privacy, engineering responsibility notice | Drafted 21 Nov 2026, marked "DRAFT — not reviewed by a lawyer", placeholders open | Lawyer admitted in Japan |

---

## 5. Onboarding kit

| Component | Content | Notes from the build |
|---|---|---|
| Installer + licence | Windows per-user NSIS setup (no admin rights), portable zip, Linux tar.gz, `SHA256SUMS`; licence terms supplied by the founder's lawyer | Installs to `%LOCALAPPDATA%\Programs\NexusFabric`; data in `%LOCALAPPDATA%\NexusFabric\data`; Start-menu shortcut required, desktop shortcut optional; uninstall keeps data unless the box is ticked; refuses to uninstall while running |
| First run | Browser opens on `127.0.0.1:8787`; one-time setup link carries the token in the URL fragment; second copy on the same data folder is refused | Explain the SmartScreen warning honestly if unsigned; explain that the port may change if 8787 is busy |
| Security briefing (1 page, JA) | Database not encrypted; use OS disk encryption; individual OS accounts; restrict access to the data folder; documented backup and restore; no automatic update | This is the owner-approved restricted-deployment wording; never shorten it to "secure" |
| Japanese quick-start (2 pages) | Install → first admin → create a job → enter dimensions → read the verdict (PASS / REVIEW_REQUIRED / INCONCLUSIVE / refused) → export DXF → read the NOT RELEASED banner → hand to your own engineer | Must state that the surface basis, seam, datum and offset sign are **not yet written into the DXF** (task 1.3) and that no kerf, bend line or allowance is included |
| Three sample projects (non-confidential, synthetic) | **S1** cylinder shell with an oblique cut; **S2** eccentric reducer (偏心円錐台) showing the surface question; **S3** header with one oblique branch, plus a deliberate refusal case | Use only the families we ship. Each sample ships with its input record, the verdict, the evidence JSON and the expected refusal code so the shop can reproduce it |
| Refusal demonstration | Show a case the verifier cannot confirm (e.g. `branch_angle_not_verifiable`, engine 0.4.3) and a seam gap at or below the resolution limit; show that the product refuses rather than prints a plausible pattern | This is the differentiator; it must be shown, not described |
| Training session | 90 minutes, on site or screen-share, in Japanese: 30 min demo, 30 min hands-on with S1–S3, 30 min on their own convention (内寸/外寸/中立, t, seam, sheet sizes) and what the product does **not** model | Record their answers as engineering statements for the 56-item review |
| Support | Named contact, Japanese, business hours in JST. **Proposal, FOUNDER DECISION**: 09:00–17:00 JST Mon–Fri, first response within one business day, engineering questions routed to the fabrication engineer with a stated longer target | The founder is in India; JST coverage needs a named person or an explicit narrower window. Do not promise a response time we cannot staff |
| Escalation | Any suspected wrong geometry → stop use, keep the job evidence and DXFs, open a defect, notify all pilot shops | See §6 |

### 30-minute demo script

| Min | Show | Say |
|---|---|---|
| 0–3 | Installer, first-run link, offline (network disabled) | Development build; nothing leaves this PC; no telemetry, no updater |
| 3–6 | S1 cylinder shell: enter dimensions, run | What is modelled and what is not: no thickness allowance, no kerf, no bend lines |
| 6–12 | Verifier result and evidence JSON: independent formulation, hashes, audit chain | "A drawing that looks right is not a drawing that is right." The check does not share the construction being checked |
| 12–18 | S2 eccentric reducer: show the surface question and the seam-gap quantity | The neutral-axis rule (K-factor) and the minimum seam clearance are **engineering rules we do not yet have**; we ask your engineer, we do not guess |
| 18–24 | S3 header + oblique branch, then the refusal case | Refusal codes are explicit; INCONCLUSIVE is never hidden; a DXF offered for review is still marked NOT RELEASED |
| 24–27 | DXF opened in their own CAD (Jw_cad or theirs) | R12 subset; CAM acceptance is untested; conventions are not yet in the file — this is on the roadmap (1.3, 5B.7, 5J.2) |
| 27–30 | What we want from them | Their convention, their sheet sizes, their CAM, and review of the 56 statements. Nothing is released by us; their engineer approves |

---

## 6. Feedback loop — pilot finding → product change → proof

| Step | Action | Record |
|---|---|---|
| 1 | Shop reports a finding with the job ID, inputs, DXF and evidence JSON | Intake note; if geometry is suspected wrong, tell every pilot shop to stop using that family |
| 2 | Reproduce on the reported commit; classify severity | `docs/backend/DEFECT_REGISTER.md` (or the vulnerability register for security) |
| 3 | Decide: fix, refuse-by-design, or engineering question | Engineering questions go to the 56-statement review (H1–H18 / RT1–RT10), not to a developer's judgement |
| 4 | Fix **with a regression test**; add the case to the validation corpus, including as a planted fault where applicable | `validation/` corpus; planted-fault detection count must not fall |
| 5 | Re-run the suites and the FULL geometry corpus on the release commit | Suite counts, `validation/results/latest.json` |
| 6 | Rebuild reproducibly and record what was run | `packaging/verify_release.sh` → `docs/release/MANIFEST-<commit>.md` with SHA-256, double-build identity, and the fixed "What this does NOT show" section |
| 7 | Return to the shop: the finding, the fix, the test that now fails if it returns, the new build hash | Written reply in Japanese; the shop re-runs their own case |
| 8 | Update readiness classifications **only with evidence** | `docs/project/READINESS.md` |

Rule: a fix is not proven by a screenshot. It is proven by a named test, a corpus case and a release manifest for a named commit. Windows results are only Windows results if they ran on Windows.

---

## 7. Risks and mitigations

| Risk | Why it is real here | Mitigation | Residual |
|---|---|---|---|
| **Liability for a wrong cut** | Outputs are EV-1, 0/56 statements reviewed; error bounds unproven; a wrong flat pattern scrapes plate and can delay a site | NOT RELEASED on every DXF; approval unreachable; written pilot agreement placing the check with the shop's qualified engineer; `ENGINEERING_RESPONSIBILITY_NOTICE.md` reviewed by a Japanese lawyer; refuse instead of guess | Lawyer must say how far a disclaimer holds under Japanese consumer and product-liability rules — **to verify**, cannot simply be excluded |
| **Drawing confidentiality** | Database not encrypted (V-21), one key for all blobs, no rotation (V-22), no MFA (V-23), no TLS in the server (V-20); "data folder copied off the PC" is inside the threat model | Desktop mode on the shop's own PC, loopback only; OS disk encryption; per-user OS accounts; restricted folder access; documented backup/restore; non-confidential samples in pilots; no hosted server for confidential drawings until the production security target is met | No independent penetration test; this is a self-assessment |
| **No erasure / personal data** | Users are deactivated, never deleted; audit entries keep user names and client addresses indefinitely | Disclose in the privacy statement; ask counsel about deletion/suspension requests under Japan's personal-information law | **to verify** with lawyer; may require pseudonymisation or a retention rule |
| **Support load and language** | Founder in India; customers Japanese; questions are engineering questions, not software questions | Named JST contact; scope first-line support to install/usage; route geometry to the fabrication engineer; publish the boundary in the agreement | Understaffed JST coverage is the most likely cause of pilot failure |
| **Currency and cash** | Costs likely in INR/USD, revenue in JPY; no exchange, fee or tax figure exists in our records | Price in yen; quote validity period on every 見積書; review the price list on a fixed cycle | **to verify**: FX spread, remittance/processor fees, withholding tax on cross-border licence income, Japanese consumption tax treatment, and whether an Indian entity can invoice a Japanese factory directly — accountant and counsel, not us |
| **Entity and contract** | Company legal name, address and contact are still placeholders in every legal draft; trademark never searched | Founder supplies entity details; order a trademark search for "NEXUS FABRIC" and "AMOEBA" in Japan before public launch | **FOUNDER DECISION** — contracting entity (India vs a Japanese entity), governing law and court |
| **Channel conflict** | Founder is CBDO of the introducing company | Written related-party disclosure to each factory; introduction-only at first | Reputational risk if discovered later; disclose early |
| **Overclaiming** | Competitors already do mid-plane conversion, seam/lap allowance, split development, sheet-fit check, area/weight/cost, tiled 1:1 printing; we do not | Publish the honest gap list internally and never imply parity; sell verification and refusal, not feature count | We are behind on convenience features; a shop may choose Cutwise at ¥2,350 for those |
| **Unsigned installer / SmartScreen** | Files are not code-signed | Sign before paid early access, or ship with SHA256SUMS and a clear explanation | Some IT departments will block an unsigned exe outright |
| **Comparison legal risk** | Trial EULAs unverified; many forbid publishing benchmarks | Internal comparisons only; read the installer licence screen before any sandbox test; no public competitor claims | Cannot use comparison data in marketing |

---

## 8. Price anchors (facts from the market research, 2026-11-22 — internal only)

| Product | Price as printed | Note |
|---|---|---|
| Cutwise Unfold | ¥2,350 one-time, 30-day trial | Already has mid-plane conversion, sheet-fit check, area/weight/cost, tiled print |
| 板金展開 7.5 (安田BTP) | ¥4,000 (author) vs ¥4,510 (Vector) — pages disagree, **UNVERIFIED** | Output disabled until registered |
| 展開図ラボ | Free; Pro ¥500/月 or ¥5,000/年; drafting service ¥8,800/sheet | Web, "guide only" |
| CADTOOL 板金展開9 | ¥101,200 tax incl. node-locked; floating ¥185,900 / ¥363,000 / ¥533,500; upgrade ¥39,600 | 286 patterns; JP-language OS required; media only |
| Godo Nazca SQ ダクト展開 | ¥110,000 tax incl. + ¥88,000/yr support | Requires the host CAD/CAM |
| SheetPartner | ¥1M–¥5M band (ipros) | Suite |
| 展開WIN, AMADA, SMOPlus, iCAD | Not published (quotation) | Opaque pricing is a visible pain in this market |
| Plate'n'Sheet V4 / Sheet Lightning v6 | US$671 / US$99–150 | No Japanese UI; demos cannot export |
| Fusion | ~¥14,300/月, ¥116,600/年, ¥350,900/3年 (secondary source) | General CAD, expert workarounds needed |

**FOUNDER DECISION — pricing.** Recommendation: publish a yen price list, monthly tier, no dongle, positioned between Cutwise and CADTOOL, with a free evaluation period. Trade-off: a low price anchors us as a utility while our value is verification; a high price is unsupportable before the fabrication-reviewed classification holds. Exact figures, tax status and evaluation length are the founder's to set.

---

## 9. "To verify" register

| # | Item | Where to check | Who |
|---|---|---|---|
| V1 | Trade show names, dates, booth costs, deadlines | Organiser sites for Japanese sheet-metal / 製缶 / laser exhibitions | Founder |
| V2 | Chuo-Sogo arrangement and any fee percentage | Written agreement between the two companies | Founder + lawyer |
| V3 | Reseller margin norms | Reseller contracts in the Japanese CAD market | Founder |
| V4 | FX, remittance/processor fees, withholding tax, consumption tax, invoicing entity | Accountant experienced in India–Japan cross-border licensing | Founder + accountant |
| V5 | Enforceability of the warranty disclaimer and liability cap; definition of "qualified engineer"; 定型約款 status | Lawyer admitted in Japan, using the three drafts | Lawyer |
| V6 | Audit-log retention vs deletion requests under Japanese personal-information law | Same lawyer; `LEGAL_READINESS.md` §4 question 4 | Lawyer |
| V7 | Export notification for the bundled cryptography | Same lawyer; item 11 | Lawyer |
| V8 | Trademark availability for NEXUS FABRIC / AMOEBA in Japan | J-PlatPat and a trademark attorney | Founder |
| V9 | IT導入補助金 eligibility for our product and licence model | 補助金事務局 official guidance | Founder |
| V10 | Whether PDF→DXF exists in the release build | Source at the release commit; `MASTER_GOAL.md` vs competitor note §4 | Lead |
| V11 | THIRD_PARTY_NOTICES present in all three package types | `tests/notices_check.sh`, `packaging/verify/windows_checks.sh` on the release commit | Lead |
| V12 | Real Windows behaviour: SmartScreen, console close, browser launch, uninstall data-deletion, port fallback | A physical Windows 10/11 PC, Japanese locale, non-admin account | Lead |
| V13 | CAM acceptance of our DXF (layers, units, Shift_JIS text) | The partner shop's own CAM, tested by a person | Lead + partner |
| V14 | Code-signing certificate cost and requirements | Certificate authorities' official pages | Founder |

---

## 10. Founder decisions, in the order they are needed

| Order | Decision | Recommendation | Trade-off |
|---|---|---|---|
| 1 | Engage a fabrication engineer for the 56 statements and the two missing rules (K-factor, seam clearance) | Do it now, before any pilot | Cost and calendar; without it no gate can close |
| 2 | Engage a Japanese lawyer; supply company legal name, address, contact | Do it now; the drafts are waiting on placeholders | Cost; without it we cannot sign a pilot agreement |
| 3 | Chuo-Sogo role and related-party disclosure | Introduction only, disclosed in writing | Slower channel revenue, cleaner sale |
| 4 | Pilot form: free vs paid, duration, part count | Free, time-boxed, written engineer-check clause | No revenue signal |
| 5 | Contracting entity, governing law, court, currency and invoicing route | Decide with the accountant before the first 見積書 | Delay; wrong choice is expensive to unwind |
| 6 | Code signing | Sign before paid early access | Cost and lead time |
| 7 | Price list in yen, tax status, evaluation length | Publish; monthly tier; no dongle | Anchoring risk (§8) |
| 8 | Whether to publish releases at all | Keep the workflow artefact-only until GA criteria hold | Slower distribution, no premature exposure |
| 9 | Security posture for pilots: restricted deployment vs wait for the production target | Restricted deployment on the shop's own PC, non-confidential data | Limits what a pilot can prove |
| 10 | Support staffing in JST and published response times | Name one person and a narrow, honest window | Founder time |

---

## 11. What this document does not show

- It does not make the product validated, approved, secure or Windows-verified.
- It contains no price, fee, tax rate, exchange rate, exhibition date or legal requirement that was not in the project records; all such items are marked **to verify**.
- It does not claim any competitor comparison result; no competitor software was downloaded or run.
- It does not authorise any customer to use an output for manufacturing. Until G-R12 holds, that permission does not exist.
