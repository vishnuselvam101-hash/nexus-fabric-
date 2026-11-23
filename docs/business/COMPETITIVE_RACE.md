# NEXUS FABRIC — Competitive Race (Japan, first customers)

Prepared from `nf-logs/biz/competitors_japan.md` (research dated 2026-11-22), `docs/project/READINESS.md` (21 Nov 2026), `docs/project/MASTER_GOAL.md`, `docs/RELEASE_READINESS.md`, `docs/PACKAGING.md`, `docs/legal/LEGAL_READINESS.md`, `README.md`.
Vendor: VSNexus Technologies (founder in India). First market: Japanese 製缶 / 板金 / ダクト / 配管 shops.

**Standing facts that constrain every claim below**

- Development build. **Not validated for manufacturing.** No result can reach `APPROVED`; every DXF is marked NOT RELEASED (gate G16).
- Geometry verified: **not yet claimed**. Fabrication reviewed: **does not hold** (0 of 56 engineering statements reviewed; all algorithms EV-1). Software verified: holds for `ed4a5fb` only.
- Windows `.exe` and installer have **never run on real Windows** — wine 9.0 only. Not code-signed.
- No published price, no EULA in force (drafts marked "not reviewed by a lawyer"), no trademark search.
- Anything not in the context files is written **to verify** with where to check. No invented numbers.

---

## 1. Scorecard — what a Japanese factory buyer actually compares

**Scoring rubric (0–3), applied to buyer fit for our first customers, not to product prestige**

| Score | Meaning |
|---|---|
| 3 | Documented in the cited source and directly usable by a small Japanese shop |
| 2 | Present but partial, unproven, or carries a real condition |
| 1 | Weak, adjacent, or self-stated only |
| 0 | Absent, or **unknown** — an unknown scores 0 because a buyer cannot rely on it (it is not proof of absence) |

Cells show the source fact, then the score and the reason. "unknown" = the cited page does not say.

| Dimension | NEXUS FABRIC (today) | CADTOOL 板金展開9 | 展開WIN | Cutwise Unfold | 安田BTP 板金展開7.5 | 展開図ラボ (web) | Mahiro / 山本建設 (web) | Jw_cad 外部変形 | Godo Nazca ダクト展開 | AMADA SheetWorks | Plate'n'Sheet / Sheet Lightning | Fusion / SOLIDWORKS |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Shapes covered (製缶 families) | 4 families: cylinder shell, cone frustum, eccentric reducer, branch stub, header w/ 1 opening — **1** handful vs 34–286 | ~74 commands / 286 patterns — **3** widest catalogue | 35 standard patterns incl. 偏心円錐, エビ管, 三又, ホッパ — **2** closed list | 34 shapes incl. 偏心円錐台, 角丸, 三叉管, helical — **2** closed list | 59 shapes (Vector) — **3** broad for the price | ~50 tools, mixed with carpentry — **2** not all 製缶 | Few: 円錐台, 偏心円錐台, 相貫, 角丸変形 — **1** narrow | Oblique cut, Y, エビ管 only; kTen shapes UNVERIFIED — **1** | 38 patterns — **2** duct-focused | Cones/intersections not mentioned; bending-oriented — **1** wrong family | Dozens incl. oblique-cone branches, transitions — **3** | Any shape a skilled user can model — **1** not a catalogue |
| Dimension basis 内寸/外寸/中立 + thickness | Not modelled; surface attribute is PLAN 1.4; thickness/neutral = Phase 5.1, needs engineer — **0** we are behind | Vendor reply: standard method ignores thickness/stretch; v9 adds manual correction field — **1** | "Plate thickness taken into account"; how, unexplained — **2** | 内寸/外寸 + t → 中立面 automatic — **3** best in class | Thickness input, neutral-line option, some developments uncorrected — **2** | Not explained on home page — **0** unknown | Mahiro 外径/内径/中立径; Yamamoto explicitly none — **2** split | Wall thickness not described — **0** unknown | Not addressed — **0** unknown | Thickness / inner R / gap values — **3** | Not explained on pages read — **0** unknown | K-factor for bends, not rolled shells — **2** |
| Seam / lap / edge allowances, split for sheet size | `seam_gap` reported as a quantity; refuse ≤1e-9 mm; no allowance model, no plate division — **1** | unknown — **0** | Correction-value / marking mode; allowance rules unknown — **1** | Seam position, lap/edge allowance, split development — **3** | Author: joints may need extra allowance — **1** manual | unknown — **0** | Explicitly no seam, lap, kerf, weld allowance — **0** | unknown — **0** | unknown — **0** | Gap values for bends; 製缶 seam unknown — **0** | unknown — **0** | Slit made by hand in a workaround — **0** |
| Accuracy proof / error bound | Independent verifier + FULL corpus 1398 cases, 0 disagree, 33/33 planted faults; but `eps_num`/`eps_rep` **not proven upper bounds** (G08 INCONCLUSIVE) — **2** mechanism, unproven claim | No quantified claim; "live preview reduces input errors" — **0** | None stated — **0** | None; no warranty/accuracy statement — **0** | None; manual says check against a sample — **0** | "Results are a guide"; check a 100 mm line — **0** | Self-stated 0.02 % on cone, unaudited; Yamamoto triangle approximation — **1** | None; a curvature bug fixed 2021 — **0** | Not addressed — **0** | Not stated — **0** | Not stated — **0** | No independent verifier — **0** |
| Refusal of unsafe / unverifiable input | 41+ refusal codes, fail-closed, INCONCLUSIVE never hidden, `branch_angle_not_verifiable` — **3** our clearest edge | unknown — **0** | unknown — **0** | unknown; own text says no independent check — **0** | unknown; warns some developments uncorrected — **0** | No refusal logic (own limits) — **0** | Disclaimer only, "fit on site" — **0** | unknown — **0** | unknown — **0** | unknown — **0** | unknown — **0** | Loft rules fail silently (no bend lines) — **0** |
| PDF / drawing input | **Conflict in our own docs**: gap table says converter DONE 3.1 (not in packages); MASTER_GOAL says drawing import not implemented — **0** to verify | 2D DXF of lines/circles/arcs only; no PDF — **0** | No PDF input — **0** | "No drawing / part-data import" — **0** | No drawing input — **0** | No drawing input — **0** | No drawing input — **0** | No drawing input — **0** | unknown — **0** | 3D import, DXF, 3D from three-view drawings; PDF not stated — **2** | Mesh import (PnS); no PDF — **0** | Not stated in cited pages — **0** |
| DXF fit to their CAD/CAM (laser/plasma) | R12 subset, read back by **our own reader only**; CAM acceptance untested; units not machine-declared; no JWW, no Shift_JIS — **1** | DXF + direct insert into AutoCAD/図脳RAPID/CADSUPER/EASY DRAW/BELL DESIGN; no JWW — **3** | DXF only; no editing, no printing — **2** | DXF R12, named cut/bend/scribe/seam layers, true arcs; CAM untested — **2** | DXF w/ layer colour, NC data prep claimed — **2** | Free DXF; structure unknown — **1** | **No DXF** (SVG/CSV) — **0** | JWW into Jw_cad; no DXF — **0** for laser | Drawn in Nazca CAD → NC via Laser Module — **2** machine-tied | Flat pattern + bend attributes straight to Amada CAM — **3** | DXF + 3D wireframe; **demo export disabled** — **2** | DXF export after expert workaround — **2** |
| Japanese UI + Japanese documents | EN/JA shipped, i18n catalogue complete; native review Phase 6 pending — **2** | Japanese product, JP-language OS required — **3** | Japanese — **3** | JA/EN switch + PDF manual — **3** | Japanese — **3** | Japanese only — **3** | Japanese — **3** | Japanese, but garbled/legacy pages — **3** | Japanese vendor pages — **3** | Japanese; domestic market only — **3** | **No Japanese** — **0** | JA UI and JA help — **3** |
| JIS conventions (views, 定尺, notation) | Not stated in DXF (seam, datum, side, offset sign = PLAN 1.3); JIS third-angle sheet = idea only — **0** | unknown — **0** | unknown — **0** | サブロク/シハチ/ゴトー fit check, title data; JIS views unknown — **1** | unknown — **0** | unknown — **0** | unknown — **0** | unknown — **0** | unknown — **0** | unknown — **0** | unknown — **0** | unknown — **0** |
| Price model | **None published**; NOT PLANNED, owner decision — **0** | ¥101,200 incl. node-locked; floating ¥185,900 / ¥363,000 / ¥533,500; upgrade ¥39,600; media only — **2** clear but high entry | Not published (quote); extra shapes quoted — **0** | **¥2,350 one-time**, 30-day trial — **3** price anchor of the market | ¥4,000 (author) vs ¥4,510 (Vector) — **2** cheap, pages disagree (to verify) | Free; Pro ¥500/月 or ¥5,000/年; drafting ¥8,800/sheet — **3** | Free — **3** | Free — **3** | ¥110,000 incl. + ¥88,000/年 support **+ host Nazca SQ** — **1** | Not published (quote) — **0** | US$671 / US$99–150 (site US$250–375) — **2** USD, overseas | Fusion ~¥116,600/年 (secondary, to verify); SW quote — **1** |
| Support in Japan | None today. Chuo-Sogo channel is a possibility, not a contract — **0** | JP vendor, vendor-hosted reviews; no maintenance listed — **2** | JP vendor, quote per shape — **2** | Single publisher, 0 ratings, released 2026-08-31 — **1** | Single author, .NET 2.0 era — **1** | Web operator + paid drafting service — **1** | Unknown — **0** | Unknown authors, stale freeware — **0** | 新明和グループ, paid annual support — **3** | Reseller network + operator training (Ai-Link) — **3** | Overseas, no JP presence stated — **0** | JP resellers (e.g. 大塚商会) — **2** |
| Offline use | Desktop mode, loopback only, no telemetry/updater; offline proven on Linux, Windows **wine only** — **2** | Node-locked, CD-ROM, JP OS, 32-bit — **2** | Windows 11, download form PC-only — **2** | Fully offline, no network/telemetry (publisher) — **3** | Local Windows app — **2** | Browser service — **0** | Browser page, local calculation — **1** | Local, needs Jw_cad — **2** | Needs host CAD/CAM; offline unknown — **0** | unknown — **0** | PnS: no internet needed — **2** | Fusion cloud-dependent — **0** |
| Audit / evidence / job record | Audit chain + evidence JSON + NOT RELEASED banner; hashes **unsigned, not externally anchored** (V-25/V-28) — **2** | None mentioned — **0** | None mentioned — **0** | Own text: no audit trail — **0** | None — **0** | No audit trail — **0** | None — **0** | None — **0** | None stated — **0** | None stated — **0** | None stated — **0** | None stated — **0** |
| Learning curve for a shop, not a CAD operator | Parametric forms + bilingual field errors; no picture picker; no shop user evidence yet — **1** | Numeric input + live preview; version changes undocumented — **2** | Marking mode; separate CAD mandatory — **2** | Pick shape from picture list, type dimensions — **3** | Legacy UI, .NET 2.0, XP mode notes — **2** | No install, smartphone-friendly — **3** | Simple forms, but SVG only, print at 100 % — **3** | Batch files, GAWK, Jw_cad skill — **1** | Needs Nazca SQ + first-year maintenance — **2** | 3D sheet-metal CAD skill — **1** | English only for a JP shop — **1** | Multi-step expert workaround (loft→thicken→slit→unfold) — **0** |
| **Total (max 42)** | **15** | **18** | **16** | **21** | **18** | **13** | **14** | **10** | **13** | **16** | **10** | **11** |

**How to read the totals.** They are a buyer-fit snapshot, not a quality verdict. Cutwise wins on everyday shop convenience at ¥2,350; CADTOOL wins on catalogue breadth and CAD hand-off; AMADA and Godo win where the shop already owns the machine. NEXUS FABRIC scores 0 on six dimensions that are **commerce and fabrication-model gaps, not geometry gaps** — and those are the cheapest points to buy back.

---

## 2. Other products in the research file (not scored; race verdict)

| Product | Fit | Race verdict for our first customers |
|---|---|---|
| SheetPartner 板金展開 / Duct展開 (テイクソフト) | ◐ | ¥1M–¥5M band, demo only, shapes not listed. We do not compete for that budget; we compete for the shop that cannot justify it. IT導入補助金 2024 eligibility is stated for them — **to verify** whether any 2026 scheme applies to us (owner + subsidy portal). |
| 三菱電機 CamMagic SMOPlus | ○ | Unfolds **ruled surfaces**, interference check after unfolding, multi-CAD import. Price/thickness/trial not stated. Machine-tied; not our buyer. |
| iCAD SX + iCAD SML (富士通) | ○ | Bending-oriented (V-bend, hemming, bend correction). Rolled/conical 製缶 not described. |
| LITIO 2.0 / CaldereriaOnline | ○ | Sales and downloads ended 2025-01-01; support ends 2026-01-01. Not Japanese. Effectively out of the race. |
| Hypertherm Design2Fab 6 | ○ | US$79.99/user/月, English only, online licence, HVAC library. 7-day trial appears only in an undated press release — **to verify** on the current product page. |
| The展開win (Vector) | ○ | Windows 95–8 sample; author/price garbled (UNVERIFIED). Legacy. |
| 展開職人 (製缶の里) | ○ | Site says download "not yet available", last news 2018. **Not testable, not shipping.** |
| 製缶展開システム (シグマテック / カンプレ) | ○ | 43 patterns / ©2004-era page; prices, platforms, trials not published. Regional legacy. |
| kTen 3.0, 小嶋 外変, jwtool | ◐ | The JWW ecosystem is where many small shops live and **nobody serves it well** — including us (5J.6 is design-only). Note: jwtool / KITI "展開図" tools are architectural elevations, not sheet metal. |
| "Unfoldo AI", "CUTL" | — | No such sheet-metal products found in JA/EN searches. Treat as non-existent; do not repeat these names in any material. |

---

## 3. Where we win — honestly

| # | Our advantage | Evidence we can show today | The honest limit |
|---|---|---|---|
| W1 | **Refusal instead of a guess** | 41+ refusal codes; fail-closed; `branch_angle_not_verifiable` added in 0.4.3; INCONCLUSIVE never hidden | Refusal is only as good as the rule table, which is **provisional and unreviewed** (G18 never passes). |
| W2 | **An independent check that is not the construction being checked** | Independent reference in `validation/reference/`, confirmed independent by external review C; FULL corpus 1398 cases: 977 agree, 421 refused, **0 disagree, 0 wrongly accepted, 0 wrongly refused, 0 errors**, 33/33 planted faults detected | The oracle mirrors our implementation limits (RC-28); two near-tangent product checks stay INCONCLUSIVE; `eps` are not proven bounds; no third review (1.9). |
| W3 | **Job evidence and audit chain** | Evidence JSON with hashes, audit chain, NOT RELEASED on every DXF | Hashes unsigned, chain not externally anchored; a file-level insider can rewrite consistently (V-25, V-28). |
| W4 | **Bilingual EN/JA product, Japanese default** | i18n catalogue complete; bilingual field errors | Native-speaking review is Phase 6, not done. |
| W5 | **Offline desktop, no telemetry, no updater, no cloud dependency** | Loopback-only bind, second-copy lock, offline launch test in a loopback-only network namespace | Proven on **Linux**; Windows evidence is wine only. DB not encrypted at rest (V-21/V-22). |
| W6 | **Any-OS web client + desktop from one binary** | Server mode and `--desktop` mode; competitors are Windows-only, some 32-bit with JP-OS and CD media | Windows package unverified on a real PC (4.2). Do not claim parity. |

**Where the market is genuinely open:** no product in the file publishes an error bound, a per-shape method statement, or a check that works from the exported files alone. Every one of them says, in effect, "check a sample" or "fit on site" (D1, D3, D16). That is the ground we are standing on.

## 4. Where we lose — honestly

| # | Gap | Who beats us | Buyer consequence |
|---|---|---|---|
| L1 | **内寸/外寸/中立 + thickness conversion** | Cutwise (automatic 中立面), Mahiro, 安田BTP | A shop cannot enter the dimension written on its drawing and get a pattern. This is a **daily-use blocker**, not a nicety. |
| L2 | **Seam / lap / edge allowance, split development** | Cutwise | Large shells that do not fit one sheet cannot be produced at all with us. |
| L3 | **定尺 fit check (サブロク/シハチ/ゴトー), area, weight, cut length, material cost** | Cutwise at ¥2,350 | The estimator's next numbers are missing. We look like a geometry demo, not a shop tool. |
| L4 | **Shape count** | CADTOOL 286, 安田BTP 59, Godo 38, 展開WIN 35, Cutwise 34 | エビ管, 角丸, ホッパ, 三又 are ordinary work; we refuse them by absence. |
| L5 | **Tiled 1:1 printing with registration marks** | Cutwise, CADTOOL, 展開図ラボ | Hand-scribing shops cannot use our output on the bench. |
| L6 | **DXF hand-off proven on a real cutter** | CADTOOL (direct insert into 5 CADs), AMADA (straight to Amada CAM) | Our DXF is read back by our own reader only; CAM acceptance untested; no JWW, no Shift_JIS TEXT. |
| L7 | **Price, licence, support, signing** | Everyone with a published price | No price list, no EULA in force, unsigned Windows binary (SmartScreen warning), no Japan support contract. A factory purchasing process cannot start. |
| L8 | **JIS conventions in the output** | Nobody states them either — but we cannot claim a win we have not built | Seam, datum, side, offset sign, origin are not written in the DXF (1.3). |
| L9 | **Jw_cad / JWW** | Nobody serves it well; we do not serve it at all yet | The largest unmet-demand segment stays unmet. |
| L10 | **Fabrication authority** | AMADA/Godo have machine-side engineering; we have 0/56 statements reviewed | Until a qualified fabrication engineer signs, **no output may be used for manufacturing**, whatever the software gates say. |

---

## 5. Close-the-gap backlog

Priority rule: **P0 = a buyer cannot evaluate or trust us without it. P1 = a buyer cannot use us for ordinary work without it. P2 = differentiation we can sell later.** Task ids are from `CLI_WORK_PLAN.md` as cited in the research file; they were not re-checked against the code.

### P0 — before any factory demo that asks for a file

| # | Change | Why a buyer cares | How we prove it | Task / owner |
|---|---|---|---|---|
| P0-1 | Run the installer and desktop mode on a **real Windows 10/11 PC**, including SmartScreen, browser launch, console close, uninstall-with-data-delete | "Does it install on our PC?" is the first question; today the answer is "we only ran wine" | Signed-off checklist with screenshots, OS version, and the `verify_release.sh` manifest updated to say **Windows: native** | 4.2 · lead + a Windows machine (FOUNDER DECISION: buy/borrow) |
| P0-2 | Commit engine 0.4.4 and **prove `eps_num` / `eps_rep` are upper bounds**; clear the two INCONCLUSIVE product checks | Our whole pitch is "a number with a bound". An unproven bound is a claim we cannot make | FULL corpus re-run with 0 INCONCLUSIVE product checks; G08 no longer INCONCLUSIVE; written bound derivation reviewed by a second person | 1.2, 0.3, 1.6 · lead |
| P0-3 | Write **every convention into the DXF** (999 block + TEXT + result): seam, datum, side, offset sign, rolling axis, angle datum, origin, units, "no kerf" | A cutter operator must know which side of the line to cut and where the seam sits | Independent reader (not ours) parses the block; a second person reconstructs the part from the file alone | 1.3 · lead |
| P0-4 | **Surface attribute** (outside / inside / neutral / unknown) on every family; INCONCLUSIVE when the basis is unknown | Stops the silent 内寸/外寸 mistake before the thickness model exists | Refusal test per family; attribute visible in UI, DXF and evidence JSON | 1.4 · lead |
| P0-5 | **Independent DXF reader + construction-free mating check from the two exported files**; catch rotated/mirrored parts | This is the demo that no competitor can run | Take the two DXFs from a branch + header job, re-derive the joint from the files only, show agreement; plant a mirror fault and show it caught | 1.5, RC-09/RC-10 · lead |
| P0-6 | Resolve the **PDF-input contradiction** in our own documents and state one truth everywhere | We must not sell a feature the package does not contain | One corrected capability matrix; a test that fails if the UI or package claims a feature that is absent | MASTER_GOAL vs gap table · lead |
| P0-7 | **Terms of use, privacy statement, engineering-responsibility notice** finalised by a lawyer admitted in Japan; privacy wording for the unencrypted database in the install guide and first-run screen | No Japanese factory signs a purchase without terms; our drafts are marked "not reviewed by a lawyer" | Executed terms with no open placeholders; acceptance screen in the installer | 4.5 · **FOUNDER DECISION** |
| P0-8 | **Published price list in yen** and a named evaluation offer | Opaque pricing is a visible pain in this market (D7); today we are worse than the vendors we criticise | Price page live; a trial that a shop can start without a quotation call | Phase 4.5 · **FOUNDER DECISION** |
| P0-9 | Engage a **qualified fabrication engineer** for the 56 statements, the minimum K-factor rule and the minimum seam-clearance rule | Until this holds, every file we hand over is NOT RELEASED — correctly | Signed review package; EV level raised above EV-1 for reviewed statements | G-R12 / Phase 5.1 · **FOUNDER DECISION** (paid engagement) |

### P1 — before a shop can do ordinary work with us

| # | Change | Why a buyer cares | How we prove it | Task |
|---|---|---|---|---|
| P1-1 | **Thickness / neutral-line model with declared basis**, designed with the engineer; no default typed from memory | Closes L1, the daily-use blocker; Cutwise already does the conversion | Per-family proof that the developed circumference equals the circumference on the declared surface; INCONCLUSIVE with the exact missing value when the rule is absent | 1.4 + Phase 5.1 |
| P1-2 | **Seam / lap / edge allowance** as explicit, cited rules (never a silent default) | Closes L2; the shop must know what was added and why | Allowance shown as a separate quantity in the result and DXF; verifier proves the pattern with and without it | Phase 5.1 |
| P1-3 | **定尺 fit check + split development** (3×6, 4×8, 5×10, metre sizes), rotation, seam positions, each segment verified and interface-checked | Material cost dominates a small shop's quote; nobody verifies the split | Segment-by-segment verification plus a mating check between segments from the exported files | 5B.2-style + 1.5 |
| P1-4 | **Shop pattern sheet**: PDF title block, 1:1 tiled A4/A3 with registration marks and a printed 100 mm check line, scribe coordinate table | Closes L5; hand-scribing is still common | Print at 100 %, measure the check line and two registration marks with a steel rule, record the measurement | 5.4 / 5J.4 |
| P1-5 | **エビ管 (segmented elbow)** with gore numbering, then **角丸** with a bounded triangulation error | The two most requested families we lack; web tools admit approximation | Corpus cases per family with planted faults; stated bound instead of "verify by trial" | 5B.2, 5B.3 |
| P1-6 | **CAM layer profiles + Shift_JIS TEXT option**; JWW output after licence review | Closes L6/L9; direct hand-off to the shop's existing CAD | A person confirms acceptance on the named machine; until then the profile ships as data with **no "works with" claim** | 5B.7, 5J.2, 5J.6 |
| P1-7 | **DXF quality gate with an independent reader** and an INVALID state that blocks download | A file that opens is not a file that is right | Open contours, duplicates, overlaps, self-intersections, wrong layer/units detected; INVALID blocks the download button | G-R4 queued work |
| P1-8 | **Security baseline for restricted deployment**: OS disk encryption required, per-user accounts, documented backup/restore, plain wording about the unencrypted database | A factory will ask what happens if the PC is copied or lost | Restore test from a copied data folder; the approved wording present in the install guide and first-run screen | READINESS desktop classes |

### P2 — differentiation we can charge for later

| # | Change | Why a buyer cares | How we prove it | Task |
|---|---|---|---|---|
| P2-1 | **Quote pre-work**: area, weight (JIS densities from a cited table), cut length, pierce count, kg-price → material cost, inside the hashed job record | Competitors give these numbers with no provenance; we give them with a source | Every number traceable to a cited density and a verified geometry; hash in the evidence JSON | 5B.6 |
| P2-2 | **Tolerance-driven uncertainty map**: ± on diameters/angles → range on every cut length | No competitor shows how input error moves the cut | Propagation test against a second-person recomputation on RC-001 | 5B.4 |
| P2-3 | **Refold / 3D preview with joint highlight** | Replaces bench hand-fitting (D16) with pre-cut confidence | Refold check agrees with the mating check; operator sees the joint before cutting | 5B.1, 5B.8 |
| P2-4 | **ホッパ (片しぼり / 偏心), 三又 / Y-branch** families | Present in 展開WIN's 35; we lack them | Same contract: derivation + verifier + corpus + refusals | Phase 5.2 |
| P2-5 | **JIS third-angle views** on the pattern sheet; native Japanese review of every string | A drawing that does not look like a Japanese drawing is re-drawn by the shop | Native reviewer sign-off; JIS view layout checked by the fabrication engineer | 5J.1, 5J.5, Phase 6 |
| P2-6 | **Reproducible, signed builds** with a printed verification manifest | Factories distrust cloud and unsigned freeware (D13) | Two builds bit-for-bit identical; signature verified on a real Windows PC | 4.1–4.4 |
| P2-7 | **Approval workflow** so an authorised engineer can release a result | Today `APPROVED` is unreachable; the product cannot complete a real job | Gate G16 satisfied end-to-end on one reference case with a named engineer | 5.6 |

---

## 6. Founder decisions (money, entity, contracts, prices, publishing)

| # | FOUNDER DECISION | Recommendation | Trade-off |
|---|---|---|---|
| F1 | Price model and price points in yen | Publish a one-time perpetual price plus an optional annual support fee, positioned **above Cutwise ¥2,350 and well below CADTOOL ¥101,200**, with a free evaluation that exports DXF | A low price wins trials but funds no engineer review; a high price funds verification but competes with a 286-pattern catalogue we do not have. Exact numbers: **to verify** by willingness-to-pay calls, not by guesswork. |
| F2 | Evaluation limits (days, export allowed?) | Allow DXF export during evaluation, keep NOT RELEASED on every file | Only CADTOOL (15 days) and 展開WIN (3 months) export in trial; matching them removes our biggest evaluation disadvantage, but gives away the geometry for free during the trial. |
| F3 | Engage a fabrication engineer and a Japan-admitted lawyer (paid) | Do both before the first paid customer; they are gates G-R12 and legal item 2 | Cost and delay now, versus a product that can never legally or technically be used for manufacturing. |
| F4 | Legal entity / contracting party for Japanese customers (VSNexus Technologies in India vs a Japanese entity) | Take advice from the Japan-admitted lawyer before quoting any Japanese factory | An Indian contracting party may slow factory procurement and raises governing-law, export and personal-information questions; a Japanese entity costs money and time. **Not decided in any context file.** |
| F5 | Chuo-Sogo as a sales channel (founder is its CBDO) | Use it for introductions and factory interviews first; put any resale or referral agreement in writing, with the conflict of interest disclosed to both boards | Fast access to purchasing departments, but a related-party arrangement without written terms is a governance and trust risk. **Fee / margin percentage: to verify — no number exists in the context.** |
| F6 | Code-signing certificate purchase | Buy before public Windows distribution | Cost and identity verification delay, versus a SmartScreen "Windows protected your PC" warning on every install. |
| F7 | Whether to publish releases (the workflow only uploads artefacts) | Keep unpublished until P0-1, P0-7 and P0-8 close | Publishing early gets feedback; publishing an unsigned, unpriced, unlicensed build to factories damages the only asset we have — credibility. |
| F8 | Trademark search and registration for "NEXUS FABRIC" / "AMOEBA" in Japan | Order the search before any public launch material | Search cost vs rebranding after launch. Not yet searched in any register. |
| F9 | Hosted evaluation server: run it or ship desktop only | Desktop only for the first customers; hosted only after the security gaps and a privacy policy are closed | A hosted demo is easier to show, but the database is unencrypted and the operator carries personal-information duties. |
| F10 | Publishing any competitor comparison | Keep all benchmark results **internal** | Many commercial EULAs forbid publishing benchmark results without consent; the exact clauses are UNVERIFIED for every product. |

---

## 7. To verify (no number or claim may be invented)

| Item | What to check | Where |
|---|---|---|
| Chuo-Sogo channel fee / margin % | Whether any resale or referral fee exists at all | Written agreement between VSNexus and Chuo-Sogo (does not exist yet) |
| Our price points and support fee | Willingness to pay; what a shop pays today | Factory interviews; the cited competitor price pages |
| 安田BTP registration fee | ¥4,000 (author page) vs ¥4,510 (Vector) — which is current | btp.sakura.ne.jp/manual.htm and the Vector listing |
| Fusion JP price | Secondary source only (~¥14,300/月, ¥116,600/年, ¥350,900/3年) | Autodesk JP official product page |
| Cutwise trial DXF export | Whether the Store trial exports DXF | Microsoft Store listing / publisher page; needs a real Windows PC + Microsoft account |
| Design2Fab trial | 7-day trial appears only in an undated press release | Current Hypertherm product page |
| Every competitor EULA | Trial terms, benchmark-publication clause, redistribution | Installer licence screen; none of the pages read printed its EULA |
| CAM / laser / plasma DXF requirements | Layer names, units declaration, Shift_JIS TEXT, JWW acceptance | The factory's own machine manual and CAM operator; **no "works with" claim until a person tests it** |
| JWW format licence | Whether writing JWW is permitted | Jw_cad author's licence terms (5J.6 licence review) |
| IT導入補助金 | Whether any current scheme covers our product (context states 2024 eligibility for SheetPartner only) | Official subsidy portal + owner |
| JIS material densities, 定尺 sizes, K-factor, seam clearance | Cited tables and an engineer's rule, not memory | JIS standards + the fabrication engineer (Phase 5.1, 5J.7) |
| Japan support terms (hours, language, response time) | What we can actually staff | Owner decision; nothing exists today |
| Third-party notices in packages | `docs/PACKAGING.md` says notices ship and are CI-checked; `docs/legal/LEGAL_READINESS.md` still lists it as a gap | Re-check on the release commit; `tests/notices_check.sh` |

---

## 8. Positioning and the factory demo

**One-line positioning**

> NEXUS FABRIC is the flat-pattern program that **shows its proof and refuses when it cannot prove** — 内寸/外寸 declared, every quantity independently checked, every file marked NOT RELEASED until a qualified engineer signs. Development build; not yet validated for manufacturing.

Japanese shorthand for the shop floor: 「展開図に根拠を付ける。証明できなければ、出さない。」

**Three proof points we can demonstrate today, on a laptop, offline**

| # | Proof point | What we do live | What we say while doing it |
|---|---|---|---|
| 1 | **Refusal, not a guess** | Enter a branch angle the verification cannot confirm; the job ends with `branch_angle_not_verifiable` and no usable pattern. Then enter a valid case and get a result | "Every other tool in this market gives you a line and tells you to fit it at the bench. This one tells you it does not know." |
| 2 | **An independent check that catches a planted fault** | Show the FULL-corpus result (1398 cases, 977 agree, 421 refused, 0 disagree, 0 wrongly accepted, 33/33 planted faults detected), then run one planted fault live and show it detected | "The checker does not share the construction it checks. That is the whole design. The error bounds are **not yet proven upper bounds** — that work is open (task 1.2)." |
| 3 | **Evidence and offline operation** | Unplug the network, start `--desktop`, show the loopback-only address, the second-copy refusal, the evidence JSON with hashes and the NOT RELEASED banner on the DXF | "Nothing leaves this PC. Every job leaves a record. And no file is released, because engineer approval is not implemented — by design." |

**What we must not say in the demo**

- Not "validated", "certified", "approved", "manufacturing-ready", "secure", "hack-proof".
- Not "works with Amada / Mitsubishi / Nazca" — no CAM acceptance has been tested.
- Not "Windows verified" — wine is not Windows.
- Not "thickness handled" — it is not modelled; Cutwise, Mahiro and 安田BTP are ahead of us here.
- Not "PDF input" until the document contradiction (P0-6) is resolved.
- No competitor UI art, command names, manual text or code is reproduced anywhere in our material.

**Head-to-head test we are allowed to run (internal only)**

Only **CADTOOL 板金展開9 (15-day full trial)** and **展開WIN (3-month full trial)** write DXF during their trials, so a file-level comparison is possible for exactly those two, inside their trial windows, on a real Windows PC with a Japanese locale for CADTOOL. Plate'n'Sheet and Sheet Lightning disable output; 安田BTP disables output until registration; Cutwise needs a real Windows PC and a Microsoft account. Read each installer's licence screen first, keep every result internal, and publish nothing without the vendor's consent (F10).
