# CRITIC B: NEXUS FABRIC film 3, revision 3c (round 3)

Reviewer B: buyer and credibility critic. Date 20 Nov 2026. Films: `final/NEXUS-FABRIC-film3-54s-en.mp4` and `-ja.mp4` (54.0 s, 1920x1080, 30 fps). Paths in the evidence column are relative to `flagship3/`.
Method: 12 contact sheets of both films (2 fps, 640 px tiles, 4x5 per sheet), watched before any document. 14 full-size frames extracted, 12 viewed; 3 views at 390 px wide. Then the documents named in the evidence column. The audio was not heard; spoken lines come from `script/narration.json` and `subtitles/`. The MP4 has no subtitle track.

**Scores.** Buyer: relevance 7/10, believability 4/10, wish to see a demonstration 4/10. Credibility: truthfulness 6/10, reputational safety 4/10.
**Verdict: publishable after the listed fixes. Not publishable as it stands.**

## Part 1: the buyer

Engineering manager, steel fabrication shop (pipe headers, reducers, branches). One viewing, on a phone.

- **What it does.** You enter the dimensions of a header, branch or reducer. It develops the plate into a flat pattern, writes a DXF, runs 28 self-checks, and records every value, so changing one dimension flags what depends on it. The product marks its outputs development build, review required and not released. It runs offline.
- **Problem it claims.** Wrong curves when cutting branch joints; checking before cutting; a traceable record when a dimension changes.
- **Do I believe it?** Half. The five part types are my work, and the honesty marks ("not released", "approval not implemented", "what it cannot confirm, it shows") make me trust the vendor more. But "Dimensions in. Flat pattern out." says the output is ready to cut, and nothing says the pattern has no plate thickness, no bend allowance, no weld gap and no kerf. I would not cut from it.
- **What I want to know, and the film does not say:**
  1. How thickness, allowance and kerf are handled, and which surface the pattern is developed on.
  2. Which face to mark. The product calls this a provisional convention, not reviewed.
  3. Whether the DXF opens in our nesting or plasma software. It has not been tested against any CAM system.
  4. Accuracy and tolerance on a 1,200 mm header. The film shows figures to 0.001 mm.
  5. Whether a fabrication engineer has reviewed the outputs. The readiness file says 0 of 56 engineering statements.
  6. Who can release a result, and when. Approval is not built.
  7. Where to request the demonstration (the end card has no address), and whether we can run our own drawing instead of job 5.
  8. What happens to our data, and whether it is encrypted. The film is silent; the product does not claim it.
- **Lost interest** at about 20 s, on the checks list ("difference 2.54e-13", English names). I came back at 40 s for the real AMOEBA screen and at 42 s for "What it cannot confirm, it shows", which I liked. The DXF text dump at 26 to 29 s reads as code.
- **Would I press "Request a demonstration"?** Not on this viewing. I would book a call if we could run one of our header-branch drawings with thickness and kerf declared.
- **One change that would most raise my interest.** At 8 to 12 s, one line on screen: "Not modelled yet: plate thickness, bend allowance, weld gap, kerf. Nominal patterns, for checking, not for cutting." It tells me they know the gap, and it stops me reading the film as a cutting tool.

### Japanese wording, as a Japanese reader

Every proposal below needs a native fabrication engineer's review.

| Time (s) | Wording | Problem | Proposal |
|---|---|---|---|
| 0.5-3.5 | 曲線を、ひとつ誤れば。 | Trails off; the consequence is absent on screen | 曲線をひとつ誤れば、鋼板は合いません。 |
| 8.0 | 寸法から、板展開へ。 | Right trade term; vague about the drawing | 寸法から板展開図へ。 |
| 8-12 (spoken) | 寸法を入れれば、板展開が出ます。 | Colloquial; "展開が出る" implies cut-ready | 寸法を入力すると、板展開図が出力されます。 |
| 20-26 | 自ら、検算。 | "自ら" is literal; unclear who checks | 自己検算。 |
| 22-25 | 検証項目 | "検証" claims a verification the product has not reached | 照合項目 |
| 22-25, 42-45 | 合格 | Means passed inspection; wrong for a self-check or a gate | 一致 for numeric checks; avoid 合格 |
| 42-45 | 未確定 | The usual QA term is 判定保留 | 判定保留 |
| 22-25 | 差 2.54e-13 | Exponent form reads as an accuracy claim | 差 0 (丸め誤差の範囲) |
| 26-30 | 出力・再読込・照合 | 再読込 is software jargon | 出力 → 読み直し → 照合 |
| 36 | 件の精査項目を発生 | Odd word order | 精査項目が11件発生 |
| 40 | 保存済みの記録31件を確認、不合格0件。 | 不合格 means a rejected part; replay failures are 不一致 | 保存済みの記録43件を照合、不一致0件。 |
| 30-36 | 土台が、AMOEBA。 | "が" sounds forced | 土台は AMOEBA です。 |
| 39-42 (spoken, cut) | 再実行すれば、一致しなければなりません。 | The cut drops the subject; what must match is unclear | 再実行の結果が一致しなければなりません。 |
| 42-46 (spoken, cut) | 結果「要レビュー」のまま。 | The cut drops は | 結果は「要レビュー」のままです。 |
| 40.5 (real screen) | 設計オーガニズムの状態 | Unnatural product copy, and the film shows it | 設計データの状態 (product copy fix) |
| 52.6-54 | 製品コンセプトです。 | "Concept" contradicts the working build shown | 開発版です。 |
| 50-54 | 設計を、再定義。 | Generic slogan; English "Engineering" becomes 設計 | 板展開の根拠を、残す。 (candidate) |
| 20-29, 42-49 | 本映像用に画面を簡略化しています・データは製品の出力です | Long and bureaucratic; unreadable at phone size | 画面は簡略化しています。数値は製品の出力です。 |
| 22-25, 42-45 | English identifiers: provenance_complete, opening_matches_3d_intersection | Untranslated in a Japanese film: about 10 of 26 check names and all 13 gate names | Use the product's Japanese names (the real Japanese screen shows 来歴の完全性 for G01) |

Keep as they are: 実際のソフトウェア・開発版; 確認できないことは、隠しません。; デモのご依頼.

## Part 2: credibility

Verdict key: True / True with qualification / Not supported / Misleading by implication. "data" means `flagship3/data/en/*.json`, the product output at build time. "real" means the product's own screen in the footage.

| Time (s) | Statement, as shown or spoken | Verdict | Evidence | Fix |
|---|---|---|---|---|
| 0.0-4.0 | L1 "One wrong curve — and the steel doesn't fit." / 曲線をひとつ誤れば、鋼板は合いません。 | True (general fact; the curve is a drawing) | README scene 1 | none |
| 0.5-3.5 | Caption "One wrong curve." / 曲線を、ひとつ誤れば。 | True with qualification (JA trails) | onscreen.js | full sentence |
| 0.0-4.0 | Top-left "not a product view" / 製品画面ではありません | True; about 5 px at phone size | 390 px view | enlarge |
| 4.0-8.0 | "Before you cut — how do you know?" / 切る前に、どう確かめる？ | True (a question, no claim) | onscreen.js | none |
| 6.5-8.0 | Blue opening curve from job 5 product data | Misleading by implication: no NOT RELEASED or development mark; the real drawing carries "Not released · review required" | README scene 2; real 10.5 | add the mark |
| 8.0-10.0 | L3 "Dimensions in. Flat pattern out." / 寸法を入れれば、板展開が出ます。 | Misleading by implication: zero-thickness reference surface; thickness, bend allowance, weld gap, kerf not modelled | handoff §1; READINESS | add limits line |
| 8.5-10.0 | Fields: header contact radius 150 mm; header shell length 1200 mm; branch position 600 mm | True (job 5 request) | real 10.5 shows the same | none |
| 9.5-10.0 | Card counts up from "Length 1.000 mm / Developed width 1.000 mm" | Misleading by implication: a 1 mm pattern reads as a result | README (count-up from 1.000) | start blank |
| 9.5-11.5 | "Development build — not validated" and REVIEW REQUIRED on the card | True (about 6 px at phone size) | frames; 390 px view | enlarge |
| 10.5-12.0 | Real window, job 5; "Actual software · development build" | True; label about 22 px; the real banner says outputs must not be used for manufacturing | real 10.5 | crop the banner larger |
| 10.5-12.0 | "Correct" beside each input value on the real screen | Misleading by implication: reads as "verified" | real 10.5 | confirm meaning; explain |
| 12.0-19.5 | Part types: cylindrical shell, concentric and eccentric reducer, branch stub, header with opening | True | handoff §1 | none |
| 12.0-19.5 | "Every cut computed, point by point." | True with qualification: vertex lists are product output; the 3D eccentric roll is an approximation | README weakness 3 | label as illustration |
| 12.5-19.5 | Figures 3,141.593; 316.228; 298,037.648; 628.319; 942.478 mm | True in value: they agree with each other (pi x 1000; pi x 300 header; pi x 200 branch of radius 100 mm; area = pi x 300 x 316.228). Precision is misleading by implication | data; real 10.5 | round to 1 mm |
| 12.0-19.5 | Five patterns with no NOT RELEASED or development mark | Misleading by implication: looks cut-ready; the marks should never be hidden | real 10.5; SCRIPT rules | add marks |
| 20.0-26.0 | L5 "Then it checks its own work." / 自ら、検算。 | True with qualification: 28 product checks, internal consistency only | checks-5.json; READINESS | say what they test |
| 20.0-26.0 | "Folded back in 3D, both edges must be one curve." / 立体に戻し…かを確かめます | Misleading by implication: the fold and seam are drawn (3D checks do exist) | README weakness 4 | label as drawn |
| 20.0-26.0 | Joint labelled "Visualisation made for this film · data from the product" | Misleading by implication: the fold is not product output | README | relabel |
| 20.0-26.0 | Card header 検証項目 (English "Checks") | Misleading by implication: 検証 claims verification | NEW_STRINGS | 照合項目 |
| 22.0-25.5 | Checks card: 28 rows, PASS, 合格, counter to 28 | True with qualification: all 28 PASS in data; PASS and 合格 read as approval | checks-5.json; READINESS | rename; say what is tested |
| 22.0-25.5 | Difference column (2.54e-13 etc.) | True with qualification: real values, but an accuracy-type figure the owner rule excludes | SCRIPT rules | drop or round |
| 22.0-25.5 | JA rows show English identifiers | True with qualification: product output, untranslated in the Japanese film | README | translate or drop |
| 26.0-28.5 | L6 "The DXF is written, read back, and compared." | True with qualification: each DXF is re-read in a quality gate; not tested against a CAM system | DEFECT_REGISTER | say CAM not tested |
| 26.0-29.5 | DXF text rows (ENTITIES, LINE, CUT_OUTER…) | True with qualification: first 14 non-empty lines only | README weakness 5 | label as excerpt |
| 26.0-30.0 | DXF is job 6; checks, gates and pattern are job 5 | Misleading by implication: two jobs, no caption | README; file name calc6 | caption both |
| 26.0-30.0 | Marking side not mentioned | Misleading by implication (omission): provisional, not reviewed; an open point on the real screen | DEFECT_REGISTER BE-035; real 10.5 | state it |
| 29.0-29.5 | Stamp 未リリース / NOT RELEASED and file NOT-RELEASED_REVIEW_REQUIRED_calc6_… | True; file name about 5 px | READINESS | enlarge |
| 30.0-31.5 | L7 "Underneath runs AMOEBA." / 土台が、AMOEBA。 | True | handoff §1 | keep |
| 30.5-36.0 | "Every value and every step is recorded and linked." | True with qualification: append-only records; the chain on screen is append order, not dependency | handoff §1; README weakness 6 | say "in order" |
| 30.5-33.5 | Hash chips; 18-node graph with 29 edges | True (product data; labelled simplified) | deps-6.json | keep |
| 34.0-35.5 | "Change one number." 150 to 158 mm | True (job 5 to job 6 request) | job data | caption the jobs |
| 36.0-38.0 | 8 to 11 obligations; 1 to 3 open | True: 8 discharged + 3 open = 11; real screen shows 要精査項目 3 | obligations.json; real 40.5 | keep |
| 34.0-39.5 | "everything that depended on it is flagged for review" | True with qualification: only under provisional-0.1 (EV-1, unreviewed), which the film does not say | deps-6.json; real 40.5 | say so |
| 38.5-39.5 | "Replay: match." with ticked hashes | True with qualification: an identity check, 0 failing | job-6.json | say "identity check" |
| 39.5-40.0 | "1 stored records checked" (EN, first 0.4 s) | True with qualification: grammar error | README weakness 6 | fix |
| 40.0 | "31 stored records checked, 0 failing" / 保存済みの記録31件 | Not supported: a mid-count value; the product says 43 checked; the next real screen shows 156 records (dashboard 157) | job-6.json; dashboard.json; real 40.5 | show "43 of 157" |
| 40.5-42.0 | Real AMOEBA screen: 記録 156; 要精査項目 3; provisional-0.1 (EV-1, unreviewed) | True; labelled; EV-1 is unexplained | real 40.5 | explain or crop |
| 42.0-45.5 | L8 "What it cannot confirm, it shows. The result stays: review required." / 確認できないことは、隠しません。 | True | gates data | keep |
| 42.6-43.0 | PASS 5 · INCONCLUSIVE 7 · NOT IMPLEMENTED 1 | Misleading by implication: data agree (5/7/1), but the job-5 Gates tab at 10.5 s shows 5 pass, 6 inconclusive, 1 not implemented, 1 NOT RUN; NOT RUN is merged | real 10.5; gates-5.json | show NOT RUN; caption the correction |
| 42-46 | G00 FAIL on superseded job 5 omitted ("no FAIL is shown") | Misleading by implication: the product output has a FAIL; the film's own rule says it shows what the product shows | gates-5.json; README | show it |
| 42-45 | JA gate names as English IDs (G01 provenance_complete ...) | True with qualification: the IDs are product output; the product's Japanese screen shows 来歴の完全性 | real 10.5 JA | use product names |
| 42-45 | G16 human_approval: 未実装 / NOT IMPLEMENTED | True; the most important honest fact | READINESS (approval) | keep; enlarge |
| 44.6-45.5 | REVIEW REQUIRED stamp | True | README | keep |
| 46.0-47.5 | Vocabulary tiles (PASS, 合格, REVIEW REQUIRED, NOT RELEASED ...) | True with qualification: product words; 合格 carries an approval meaning | NEW_STRINGS | see wording table |
| 46.0-47.5 | "In English and Japanese." / 日英対応。 | True (switch shown on real screens) | real 10.5, 40.5 | keep |
| 48.0-49.5 | "Runs fully offline." / 完全オフラインで動作 | True with qualification: offline_check.sh and the suites pass with no network; not shown on film | tests/offline_check.sh; READINESS | say "tested with network off" |
| 48.3-49.6 | Checks counter 1 to 28 on the offline card | True with qualification: a film device; reads as a run result | README | label as device |
| 48.5-49.5 | "Visualisation made for this film · not a product view" | True; about 5 px | 390 px view | enlarge |
| 50.0-54.0 | NEXUS FABRIC, "Engineering, Reimagined." / 設計を、再定義。 | True with qualification: marketing line; EN and JA differ (list below) | onscreen.js | fix JA |
| 51.5-54.0 | "Request a demonstration" / デモのご依頼 | True with qualification: no address or URL | frames | add URL or email |
| 52.6-54.0 | "Engineering platform powered by AMOEBA." / AMOEBAを核とする設計基盤 | True | handoff §1 | keep |
| 52.6-54.0 | "Product concept. Capabilities in development. Not validated for manufacturing." / 製品コンセプトです。…未実施です。 | True; on screen 1.4 s at about 5 px | README; 390 px view | 5 s or more, 48 px or more |
| 50-54 (JA) | 音声：VOICEVOX Nemo | True: present, bottom right, about 5 px | CREDIT_NEEDED.txt | enlarge |
| 50-54 (EN) | No voice credit | True per audio/VOICES.md; the Kokoro model card was not read at source (handoff §13 asks for it) | audio/VOICES.md | check before publishing |

### Checks the brief asked for

- **Authorship disclosure:** none in legible text, narration, subtitles, onscreen.js, NEW_STRINGS or film-data.js (grep count 0). The real-window crops end at the frame edge, so footer text below them is not visible. The product itself discloses that its algorithms are evidence level EV-1, not independently reviewed (handoff §1; "EV-1" is on screen at 40.5 s). The owner should decide the answer if a buyer asks.
- **Approved, validated or ready to cut, implied?** Yes: 22-25 s (合格 and PASS, 28 of 28), 38.5-40.5 s (ticks and "match"), 12-19.5 s (patterns with no marks). No frame claims approval; "validated" appears only negated ("not validated", 未検証).
- **Simplified pieces:** the only invented UI is counters and a wifi glyph, both labelled. The simplified label is missing only for the first half-second of scene 3 (8.0-8.5 s).
- **Real moments:** two (10.5-12.0 s and 40.5-42.0 s). Both are labelled for their whole time.
- **Disclaimer readable at phone size:** 0 seconds. Labels are 22 to 30 px in the 1920 frame, which is 4 to 6 px at 390 px width. The end-card line is on screen 1.4 s (52.6-54.0).
- **Numbers agree with each other:** geometry does; obligations do (8 + 3 = 11); gates do not (7 inconclusive against 6 plus 1 not run on the job-5 screen); the replay count (31 to 43, against 156 or 157 stored) is unexplained.
- **REVIEW REQUIRED and NOT RELEASED:** present on the checks card, gates, pattern card, DXF stamp and file name; missing on scenes 2 and 4, where the product shows them on the same drawings.
- **EN and JA say the same thing?** No (list below). JA narration L5-L8 is an unaccepted cut of the locked script (`narration.json`, "use": "cut").
- **JA voice credit on the end card:** present at 52.5 and 53.0 s (frames), and the README says it runs for the whole card. The EN voice (Kokoro, af_heart) has no credit, and none is needed under the terms read locally; that reading is not verified at source.

### Differences in meaning, EN to JA

1. L5: "both edges must be one curve" (a requirement) against 確かめます (checks whether).
2. L7: "flagged for review" against 要精査の印 (scrutiny). The product keeps 要レビュー and 精査 apart.
3. L7 cut: 結果は is removed, so the subject of 一致 is lost.
4. Checks header: "Checks" against 検証項目 ("verification items").
5. PASS against 合格 (passed inspection); "failing" against 不合格 (rejected).
6. "obligations raised" against 件の精査項目 (review items).
7. "Slant radius at 0°" against 頂点からの母線長（0°）: a radius against a length.
8. "It checks its own work" against 自ら検算 (calculation check; narrower).
9. "Engineering platform powered by AMOEBA" against AMOEBAを核とする設計基盤; "Engineering, Reimagined" against 設計を、再定義.
Same in both: the simplified, not-a-product-view, development-build, actual-software and not-validated labels; Request a demonstration; Runs fully offline.

## Verdict and must-fix list, in order

**Verdict: publishable after the listed fixes.** Not publishable as it stands: items 1 to 4 change what the product shows.

1. Say what the pattern is not, wherever a pattern shows (8-12 s, 12-19.5 s, 26-30 s): zero-thickness reference surface; thickness, bend allowance, weld gap and kerf not modelled; marking side provisional and not reviewed.
2. Show the product's gate counts as they are for job 5 before correction: 5 PASS, 6 INCONCLUSIVE, 1 NOT RUN, 1 NOT IMPLEMENTED. Never merge NOT RUN into INCONCLUSIVE.
3. Stop omitting the FAIL. Show that job 5 is superseded by job 6 (G00, current revision: FAIL), and caption job 5 (before correction) and job 6 (after) wherever they appear.
4. Replace 合格 and PASS-as-approval wording, and 検証項目, with 照合 wording. At 20-25 s and 42-45 s say: internal consistency checks; approval not implemented; geometry validation not yet claimed.
5. Add the development-build and NOT RELEASED marks to the scene-2 outline and the scene-4 patterns, as the real drawing carries them.
6. Disclaimers: "Not validated for manufacturing" on screen for at least 5 s at 48 px or more (now 1.4 s at about 5 px); every label at 40 px or more; crop the real banner larger.
7. Japanese: no English identifiers. Use the product's Japanese gate and check names, and translate the rest with native review.
8. Replay counter: no count-up through 31 to 43 on screen. Show "43 of 157 records checked, 0 failing", and fix "1 stored records" in the English.
9. Relabel drawn pieces: the scene-5 fold and seam ("drawn for this film"), the 3D eccentric roll, and the DXF text ("excerpt, first 14 lines").
10. Figures: round to 1 mm; drop or round the difference column (owner rule: no accuracy figures). This is an owner decision.
11. The 1.000 mm count-up starts blank. Every figure names its job.
12. End card: add an address or URL for the demonstration; change 製品コンセプト to 開発版; keep the "not validated for manufacturing" wording.
13. Before publishing: the lead accepts or rejects the JA cut lines (`use: cut` is still a proposal); a native engineer reviews the Japanese; the Kokoro model terms are read at source (handoff §13).

**Keep (the three things that build trust):**
1. The two real-software moments, unretouched and labelled, with the product's own banners ("development build · outputs must not be used for manufacturing"; "REVIEW REQUIRED · cannot be released for manufacturing").
2. The approval gap on screen: G16 NOT IMPLEMENTED, the NOT RELEASED stamp on the DXF, and REVIEW REQUIRED kept on the result. This is the film's best line: "What it cannot confirm, it shows."
3. Traceable numbers: 28 checks, 8 discharged and 3 open (11), the job 5 to job 6 radius change, and the replay identity check. Each matches the product's data and carries a "data from the product" label.
