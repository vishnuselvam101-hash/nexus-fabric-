# Critic B, round 4: NEXUS FABRIC film 3 (54 s, JA and EN)

**Method.** Contact sheets at 2 fps (20 tiles per sheet, both films) were watched first. Frames 50.0–51.5 were missing from my first sheet 6 and were re-extracted. I viewed 12 full-size stills and 3 stills at 390 px. Text heights and contrast were measured from frame pixels (ink height at 1920 px; multiply by 0.203 for 390 px). Part 1 was written to `/tmp/claude-0/-home-user-nexus-fabric-/7d5fb6f5-1c9e-5151-ae27-9132ba7d2536/scratchpad/criticB4/buyer_scores_written_before_docs.txt` before any document was opened. Documents were read after that: READINESS.md, handoff §1, SCRIPT.md, flagship3 README.md and NEW_STRINGS.md, narration.json, both subtitle files, job data (gates, checks, obligations, capture, lessons), tests/offline_check.sh, audio/VOICES.md, CREDIT_NEEDED.txt, and a name search in crates/ for read-back. No other report was read. No project file was edited. Limits: I could not hear the sound; spoken lines come from narration.json and subtitles. Japanese is judged as a reader, not as a native fabrication engineer.

## Part 1: the buyer (scored before reading any document)

Scores (0–10): relevance to my work **6**; believability **5**; desire to see a demonstration **4**.

- **What it does, as I read it:** it turns the dimensions of a header, branch, reducer or shell into flat patterns and DXF files, runs its own checks, keeps a record of each value and flags items for review. It says it is a development build, not validated for manufacturing, and that approval is not implemented.
- **Problem it claims to solve:** wrong curves and wrong developed sizes found after cutting, with no trail of what changed.
- **Do I believe it:** partly. "NOT RELEASED", "not an approval" and "what it cannot confirm, it shows" are the most believable lines. "Redefine design", "checks its own work" and "one wrong curve" I discount. The patterns carry no thickness, bend allowance, weld gap or kerf, and those decide whether a plate part fits. As shown, I could not cut from it.
- **What I would want to know:** when thickness and allowances arrive; which joints beyond these five; what the DXF opens in, and whether a fabricator has run a real part through it; who makes it and how to reach them (the end card has no address); what AMOEBA is, in one sentence.
- **Where I lost interest:** about 26–29 s (empty dashed boxes, no legend for "output, re-read, compare") and again at 40–41 s (a counter climbing to 43 records).
- **Would I press "Request a demonstration":** not yet. Single change: put the arithmetic on screen for one case. For example, Ø1,000 mm gives 3,142 mm = π × 1,000, so I can check a number in three seconds. Also say which joints the demo covers.

Japanese as a native reader would see it (proposals need review by a native fabrication engineer):

| Quoted (time) | Reader's reaction | Proposal |
|---|---|---|
| 自ら、検算。 (20.0) | Who checks? 検算 means an arithmetic re-check, not the geometric checks shown | 結果を、自ら点検。 |
| 出力・再読込・照合 (26.0) | 再読込 is software jargon; unclear what is compared | DXF出力・読み戻し・照合 |
| 未考慮：板厚、各種の代、溶接ギャップ、カーフ (12.5) | 各種の代 is unnatural; bend allowance (曲げ代) is missing, and it is the key one | 未考慮：板厚、曲げ代などの各種の代、溶接ギャップ、カーフ幅 |
| 板厚ゼロの基準面です。 (12.5) | 基準面 needs a second read | 板厚はゼロとみなした基準面での展開です。 |
| 合格 (42.5) | Reads as "passed inspection / approved" | 内部チェック通過 |
| 未実装 技術者承認 (42.5) | 技術者 narrows "human" to engineers | 人による承認（未実装） |
| 設計を、再定義。 (50.5) | Hype; what is being redefined? | 寸法から板展開まで、すべての値を記録。 |
| 切る前に、どう確かめる？ (4.0) | ？ is too casual; the voice says どう確かめますか | 切る前に、どう確かめますか。 |
| 完全オフラインで動作 (48.0) | 完全 claims more than the tests cover | 接続なしで動作（試験済み） |
| 数値を、ひとつ変える。 (33.0) | The product records a correction, not a change | 数値を、ひとつ訂正する。 |

## Part 2: credibility

### 2.1 Phone legibility (measured)

| Element (time) | Ink at 1920 px | At 390 px | On screen | Contrast | Phone verdict |
|---|---|---|---|---|---|
| Source labels, every scene (font 26 px) | 24 px | 4.9 px | 6–10 s per scene | 5.4:1 | Unreadable |
| Limits caption JA (12.5–19.5) | 36–37 px | 7.3–7.5 px | 7.5 s | 2.44:1 | Only with effort |
| Limits caption EN (12.5–19.5) | 30–39 px | 6.1–7.9 px | 7.5 s | not measured | Only with effort |
| Real-software banner (10.0–11.5) | 24 px | 4.9 px | 1.5 s | not measured | Unreadable |
| "Not an approval" footnote (20.0–25.5) | about 37 px (estimate) | about 7.5 px | about 5.5 s | not measured | Marginal |
| Flag-rule footnote (35.0–36.5) | 37 px | 7.5 px | about 2 s | not measured | Marginal |
| Development-build line, gate scene (44.0–45.5) | 38 px | 7.7 px | 1.5 s | not measured | Too brief |
| End-card development line and JA credit (50.5–54.0) | 38 px | 7.7 px | 3.5 s | 17:1 / 7.6:1 | Just readable |
| DXF file name (28.5–29.5) | 36 px | 7.3 px | 1 s | not measured | Marginal |
| Headlines and gate numbers (42.5–45.5) | 89–125 px | 18–25 px | 3+ s | high | Readable |
| Real status bar, EN (37.0) | 8 px | 1.6 px | 0.5 s | — | Unreadable |
| End-card AMOEBA line (52.5) | faint | — | fades in 52.5–53.0 | 1.19:1 at 52.5 | Invisible at first |

Three 390 px stills: at JA 12.5 the limits caption and footer are unreadable. At JA 43.5 the numbers, tag and headline read; the footer does not. At JA 52.5 the wordmark, tagline, button and development line read; the AMOEBA line is gone and the credit is small. The makers' own README already says the limit lines "do not read comfortably on a phone". The measurements confirm it.

### 2.2 Statement table (JA film unless marked EN; VO = spoken line)

Codes: T true; TQ true with qualification; NS not supported; MI misleading by implication; LG legibility or labelling failure.

| t (s) | Statement or picture | Code | Evidence | Fix |
|---|---|---|---|---|
| 0.0–1.0 | Picture with no source label (label starts at 1.0 s) | LG | README label timing | Label from 0.0 s |
| 0.5–3.5 | "One wrong curve." / VO "…and the steel doesn't fit" | T | General trade claim; no product claim | Keep |
| 1.0–3.5 | Label "not a product view" | LG | 24 px ink = 4.9 px at 390 | 40 px or more |
| 4.0–7.5 | "Before you cut — how do you know?" / 切る前に、どう確かめる？ | T | Question; screen register differs from VO どう確かめますか | Use the VO form |
| 4.5–7.5 | Label "data from the product" over a curve drawn for the film | MI | README: question curve drawn for this film | "Visualisation made for this film" only |
| 8.0–11.5 | "Dimensions in. Flat pattern out." / 寸法から、板展開へ。 | T | Handoff §1: develops the surface from entered dimensions | Keep |
| 10.0–11.5 | Real screen, Job #5 REVIEW REQUIRED; banner "cannot be released for manufacturing" | T / LG | Job 5 state per README; banner 24 px | Keep; banner 40 px or more |
| 10.0–12.0 | Real crop without the product's development-build banner | MI | README says the source frame has it; SCRIPT: notice never hidden where it would be in frame | Keep the banner in frame |
| 10.0 | Entry time on screen: EN 02:40, JA 02:54 | MI | Two separate captures; data: EN 02:43:22 UTC, JA 02:57:21 UTC, matching neither | One capture; drop the time |
| 12.0–13.5 | Cylindrical shell, 展開幅 3,142 mm | T / MI | π × 1,000 = 3,141.6 (job-1 data); outline has bowed top and bottom edges, but a plain cylinder develops to a rectangle | Draw a rectangle, or mark it schematic |
| 12.5–19.5 | 板厚ゼロの基準面です。 / Zero-thickness reference surfaces. | T / LG | Handoff §1 | 40 px or more; darker |
| 12.5–19.5 | 未考慮：板厚、各種の代、溶接ギャップ、カーフ / Thickness, bend allowance, weld gap and kerf are not modelled. | T / LG | Handoff §1; ink 36–39 px; contrast 2.44:1; JA drops 曲げ代 | 48 px or more, 4.5:1 or more, 6 s or more, name 曲げ代 |
| 13.5–14.5 | 同心レジューサ · 母線長 316 mm | T | 316 present in product output (job-2 slant_length) | Keep |
| 15.0–16.5 | 偏心レジューサ · 頂点からの母線長（0°）600 mm (EN "Slant radius at 0° 600 mm") | NS / TQ | 600 not found as a named value in data/en job-3; EN "radius" vs JA "length" | Verify the value; one term |
| 17.0–18.5 | 枝管スタブ · 展開幅 628 mm | TQ | 2π × 100; branch_dev_radius 100 is OPEN in job 6 (obligation 9) | Show it as pending, or cite the job 6 status |
| 18.5–19.5 | 開口付き母管胴 · 展開幅 942 mm | TQ | 2π × 150; header_dev_radius 150 is OPEN in job 6 (obligation 11: engineer to re-confirm) | As above |
| 12.0–19.5 | Label "visual for this film; data is product output" | MI | README: header and eccentric rolls are approximations | "Outlines schematic; figures from the product" |
| 20.0–20.5 | "It checks its own work." / 自ら、検算。 | TQ / MI | Internal checks exist (handoff §1); READINESS: geometry verification not claimed; 検算 = arithmetic re-check | "It runs its own checks."; 点検 |
| 20.0–25.5 | "Internal consistency checks — not an approval." / 内部の整合チェックです。承認ではありません。 | T / LG | READINESS wording; about 37 px; about 5.5 s | Keep; enlarge |
| 21.0–25.5 | "28 of 28 checks run" / 28 / 28 件の検算 | MI | checks-5.json: 28 PASS belongs to job 5, which is superseded; job 5 has G00 FAIL "must not be used"; job 6 has 56 PASS verdicts over 26 checks | Use job 6 figures, or label job 5 superseded and show G00 |
| 20.5–23.5 | Counter 5, 9, 13, 17, 21, 25, 28 | TQ | Counter animation; intermediate values are not product states | Show the final figure only |
| 20.5–25.5 | Green ticks on every row | MI | All PASS in job 5; ticks read as validation beside "checks its own work" | Keep, with the not-an-approval line at readable size |
| 21.0–23.0 | JA rows show raw identifiers (opening_developed_length_equals_3d_arc_length and others) between Japanese labels | T / LG | Product shows machine names for 10 of 26 checks (NEW_STRINGS) | Show the 16 translated rows, or translate the rest |
| 20.0–25.5 | Footer "Interface simplified for this film · data from the product" | T / LG | Labelled for the whole scene | 40 px or more |
| 26.0–29.5 | "Written. Read back. Compared." / 出力・再読込・照合 | TQ / LG | Code: nf-core job.rs re-read quality gate; nf-dxf re-import check (real). Screen boxes unlabelled | Label source and read-back; 読み戻し |
| 26.0–27.5 | Empty dashed box; the blank solid box appears at 27.5 | LG | README: source and read-back outlines not labelled on screen | Label both |
| 28.0–29.5 | Stamp 未リリース / NOT RELEASED | T | dxf-6-header.dxf annotation; READINESS: every DXF is NOT RELEASED | Keep |
| 28.5–29.5 | File NOT-RELEASED_REVIEW_REQUIRED_calc6_header_1b9517ae.dxf | T / LG | manifest.json; 36 px ink | Keep; larger |
| 30.0–32.5 | "AMOEBA — Every value and every step, recorded and linked." / すべての値と手順を記録し、つなぎます。 | T | Handoff §1 | Keep |
| 30.5–32.5 | Header contact radius 150 mm; shell length 1,200 mm; branch position 600 mm | TQ | Job 6: length 1200, station 600; 150 is the pre-correction value (lessons: wrong value) | Mark as before the correction |
| 33.0–33.5 | "Change one number." / 数値を、ひとつ変える。 | MI | The product records a correction (capture: correction; lessons: corrected_value 150 → 158) | "Correct one number." |
| 33.5–36.5 | Simplified graph draws the corrected radius as an entered input | MI | Real job-6 graph (EN 37.0): header_radius is "Derived · level 1" | Draw it as derived; mark the correction |
| 34.0–36.5 | 150 → 158 mm | T | Job 6 header_radius 158; lessons 150 → 158 | Add the reason: drawing dimension is the outside diameter of the thicker pipe |
| 34.5 | 5 件の精査項目が発生 / 5 obligations raised | TQ | Five of the 11 are downstream obligations (obligations.json) | Say which |
| 35.0–36.5 | 11 件の精査項目が発生 / 11 obligations raised | TQ | 11 is correct: 8 discharged, 3 open (not shown; the real screen shows 3 open) | "11 raised, 8 discharged, 3 open" |
| 35.0–36.5 | 要精査の印：暫定規則、未レビュー / Flag rule: provisional, not yet reviewed. | T | Data: provisional-0.1 (EV-1, unreviewed) | Keep; it is the honest line |
| 35.0–36.5 | Three or four nodes with dots, against "11" | TQ | Obligations sit on parameters, parts and interfaces, not on nodes | Explain, or count per node |
| 33.5 VO | "everything that depended on it is flagged for review" | TQ | Three of the 11 are common-cause flags (convention_of), not dependencies | Say "and values sharing its convention" |
| 37.0–39.5 | Real graph, Job #6, "Actual software · development build" | T / LG | Capture; label 24 px | 40 px or more |
| 37.0 | Top banner "Development build — not validated — outputs must not be used for manufacturing" (EN) | T / MI | Visible at 37.0 only; the push-in removes it | Keep in frame, or repeat the notice |
| 37.0–39.5 | Real status "3 open scrutiny obligations"; tab "要精査項目 3" | MI | The film shows 11 raised with no reconciliation; status text 8 px | Show "11 raised, 3 open" |
| 38.0–39.5 | Rule-table line "provisional-0.1 (EV-1, unreviewed)" cropped out | LG | README: the crop stops above that line | Keep the "unreviewed" disclosure visible |
| 40.0–41.5 | "10 / 30 / 43 records checked, 0 failing" | TQ | 43 and 0 failing match the replay text; 10 and 30 are counter steps | Show the final figure only |
| 41.0–41.5 | "Replay: match." / 再実行：一致 (JA adds 不一致0件) | T / TQ | Capture: "Replay match". It re-checks stored records. EN "0 failing" vs JA "不一致0件" | Say "stored records" |
| 42.0–45.5 | "What it can't confirm, it shows." / 確認できないことは、隠さない。 | T | Honest and specific | Keep |
| 42.5–43.5 | 5 / 7 / 1 (合格 / 未確定 / 未実装) and REVIEW REQUIRED tag | T | gates-6: PASS 5, INCONCLUSIVE 7, NOT_IMPLEMENTED 1 of 13; job 6 state REVIEW_REQUIRED | Keep |
| 42.5–45.5 | 合格 (PASS) | MI | Reads as passed or approved in a Japanese trade | 内部チェック通過 (review) |
| 42.5–45.5 | 未実装 技術者承認 / NOT IMPLEMENTED human_approval | T / TQ | G16 is not implemented; JA narrows "human" to engineers | 人による承認（未実装） |
| 44.0–45.5 | "Development build. Not validated for manufacturing." / 開発版です。製造用途での検証は未実施です。 | T / LG | First visible at 44.0 (absent at 43.5); 1.5 s | 3 s or more; larger |
| 46.0–49.5 | English · 日本語 switch | T | Handoff §1; i18n strings in data | Keep |
| 48.0–49.5 | "Runs fully offline." / 完全オフラインで動作 | TQ | offline_check.sh: the dynamic part needs root and unshare; READINESS: suites pass offline in the lead's runs | "Runs without a network connection (tested)" |
| 50.0–54.0 | JA credit 音声：VOICEVOX Nemo | T | Nemo terms require the credit; given; 7.6:1 | Keep |
| 50.0–54.0 | EN end card: no voice credit | T / TQ | VOICES.md: Kokoro needs none; no licence file found for the model weights | Confirm the licence before publishing EN |
| 50.5–53.5 | "NEXUS FABRIC · 設計を、再定義。" / "Engineering, Reimagined." | MI | Hype; 設計 is not "engineering" | Factual line (review) |
| 52.0–53.5 | "デモのご依頼" / "Request a demonstration" (button appears from 52.0) | TQ | No address or contact on the card (README open item) | Add a contact |
| 50.5–53.5 | "Development build. Not validated for manufacturing." | T / deviation | SCRIPT.md locked text: "Product concept. Capabilities in development. Not validated for manufacturing." | Reconcile with the locked text |
| 52.5–53.5 | "AMOEBAを核とする設計基盤" / "Engineering platform powered by AMOEBA." | MI | Meanings differ; fades in (1.19:1 at 52.5) | Same meaning in both; fix the fade |
| 20–46 VO JA | Narration cut (L5–L8): そして / その / 結果は / は removed. 再実行すれば、一致しなければなりません has no subject; DXF出力し lacks a particle | TQ | narration.json use=cut; owner approved the audio | Restore the subject |
| all | "AI" | T | None in the 12 full stills, the contact sheets, narration, subtitles or on-screen strings. The product's own gate G14 says algorithms are "AI-generated" | Never show it; never claim "no AI" |

### 2.3 The specific checks

- **Implied approval or readiness, and simplified pieces.** Yes: the 28/28 green ticks are from superseded job 5, 合格 reads as approval, and the solid blue DXF box beside a grey dashed one reads as "compared OK". The simplified pieces invent no function (the exceptions are the counters and the graph's input styling). Each is labelled for its whole time, at 24 px.
- **Real-software moments, limits and internal checks.** Both real moments are labelled (10.0–11.5, 37.0–39.5), at 24 px. The limits fail on phone size and time (7.3–7.9 px, 7.5 s, 2.44:1). The "not an approval" line is present, at about 7.5 px for about 5.5 s.
- **Development build.** In frame at 44.0–45.5 and 50.5–54.0. The product's own top banner is cropped out after 37.0.
- **Numbers.** Gates (5/7/1 of 13), obligations (11; 3 open), replay (43), the correction (150 → 158) and the developed widths match product output. The 28 checks belong to job 5 (job 6 has 56 verdicts). The 600 mm value is not located. Counters show intermediate values.
- **REVIEW REQUIRED and NOT RELEASED.** Visible at 10.0–11.5 (banner), 28.0–29.5 (stamp and file name) and 42.5–49.5 (tag); the banner and file name are too small on a phone. No "AI" in any sampled frame, narration, subtitles or on-screen strings.
- **Voice credit.** Present on the JA end card from 50.0 to 54.0 s; the EN end card has none.

### 2.4 JA and EN: differences in meaning

| Item | EN | JA | Difference |
|---|---|---|---|
| Brand line | Engineering, Reimagined. | 設計を、再定義。 | Different claim (design redefined) |
| Positioning | Engineering platform powered by AMOEBA. | AMOEBAを核とする設計基盤 | "powered by" vs "core of a design foundation" |
| Checks | It checks its own work. | 自ら、検算。 | Self-check vs arithmetic re-check |
| Second part | Concentric reducer; VO "Cones" | 同心レジューサ; VO 円錐 | VO names a different shape than the label |
| Eccentric, 0° | Slant radius at 0° | 頂点からの母線長（0°） | Radius vs length |
| Limits | bend allowance | 各種の代 | JA drops bend allowance |
| Gate verdicts | PASS; human_approval | 合格; 技術者承認 | 合格 suggests approval; "human" becomes "engineer" |
| Replay | 0 failing | 不一致0件 | Failing checks vs mismatches |
| Job 5 entry time | 02:40 | 02:54 | Two captures with different data |
| Register | "shows"; plain forms | どう確かめる？ / 隠さない (VO: どう確かめますか / 隠しません) | Register only |

## Part 2 scores and verdict

**Truthfulness 6/10.** Most statements are product output, and the refusals are on screen. Deducted for the superseded-job check scene, the "change one number" framing, open values behind the 628 and 942 mm figures, the unlocated 600 mm value, and the deviation from the locked disclaimer.

**Reputational safety 5/10.** A fabricator who checks job history will find that 28/28 came from a superseded job with a failing gate. "Redefine design" and unreadable limits add to that risk.

**Verdict: publishable after listed fixes.** Fixes 1–4 are needed first. The EN release also needs fix 10.

## Must-fix list, in order

1. **Check scene (20.0–25.5, both languages).** Fix the job identity. Show job 6's counts (56 verdicts over 26 checks), or label job 5 superseded and show its G00 FAIL. Superseded green ticks must not read as the current result.
2. **Limits line (12.5–19.5).** 48 px or more, 4.5:1 contrast or more, 6 s or more. JA should name 曲げ代.
3. **Source labels and real-software banners (24 px, 4.9 px at 390).** 40 px or more, or move the statement into the caption. Keep the product's development-build banner in frame during 10.0–12.0 and 37.0–39.5.
4. **Gate-scene development line (44.0–45.5).** Hold it for 3 s or more and enlarge it. Reconcile the end-card wording with the locked disclaimer in SCRIPT.md.
5. **"Change one number" becomes "Correct one number".** State the reason from the record (outside-diameter convention, 150 → 158 mm). Draw header_radius as derived. State 8 discharged and 3 open. Correct "everything that depended on it".
6. **628 mm and 942 mm.** Job 6 lists the development radii behind them (100 and 150) as open for engineer re-confirmation. Show that status, or cite the open item.
7. **Wording that reads as approval or hype.** 合格 → 内部チェック通過; 技術者承認 → 人による承認（未実装）; 設計を、再定義。 → a factual line; 自ら、検算。 → 自ら点検。 (native review required).
8. **Animated counters** (10 and 30 records; 3 and 4 gates; the 5 to 28 check count; 5 to 11 obligations). Show final values only, or label them as animation.
9. **Unlabelled diagrams.** Label the DXF source and read-back boxes. Change "data from the product" to "figures from the product; outlines schematic". Verify the 600 mm eccentric value or remove it.
10. **Before publishing.** Confirm the EN voice model licence (no licence file for the Kokoro weights; VOICES.md and README disagree). Add a contact for the call to action (owner item). Get native fabrication-engineer review of all JA text (the film's own notes say it is unreviewed).

## Three things to keep

1. **The refusals on screen:** "Internal consistency checks — not an approval", "What it can't confirm, it shows", the 未実装 approval line, and the REVIEW REQUIRED and NOT RELEASED marks.
2. **Labelling discipline:** every real-software moment says so, every simplified panel says so, and the hook says it is not a product view.
3. **Checkable product figures with their qualifiers:** 13 gates (5/7/1), 11 obligations raised, "provisional, not yet reviewed", and the correction 150 → 158 mm. Keep them, and add the reason.
