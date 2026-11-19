# Critic B — engineering and product credibility, round 1

Reviewer stance: senior fabrication engineer who buys engineering software. Date: 2026-11-19.
Films: `final/NEXUS-FABRIC-launch-30s-en.mp4`, `final/NEXUS-FABRIC-launch-30s-ja.mp4` (1920x1080, 30 fps, 30.0 s).

## Method

- Extracted 60 frames per language (every 0.5 s) at full resolution. Read 19 EN and 5 JA frames at full size, plus full-width strips of the status-label band for every frame from 10.0 s to 30.0 s in both languages (label timing).
- Recomputed every number shown in Python (numpy, 2,000,000-point quadrature).
- Read `crates/uem/src/develop.rs`, `verify.rs`, `docs/IMPLEMENTATION_STATUS.md`, `docs/RELEASE_READINESS.md`, `film.html`, `script/narration.json`, both SRT files.
- Audio was not heard. Narration is judged from text and subtitle timings.
- Not viewed at full size: the remaining 0.5 s frames (transitions between the ones read). On-screen text in those scenes was read in neighbouring frames.

## Numbers recomputed

| On screen | Recomputed | Verdict |
|---|---|---|
| s(φ) formula, R=150, r=100, β=60° | matches `Branch::s` and Spec 02 eq. 4.3 with δ=0; every point lies on the header to 6e-14 | correct |
| φ=0.0° → 129.10 mm | 129.0994 | correct |
| φ=226.2° → 195.33 mm (EN 16.5 s) | 195.3345 | correct |
| φ=87.7° → 115.45 mm (JA 15.5 s) | 115.4546 | correct |
| 2πR = 942.478 | 942.47780 | correct |
| 2πr = 628.319 | 628.31853 | correct |
| CUT_INNER 701.663 / saddle_cut 701.664 | exact curve length 701.66509 (3D, branch development and header development all equal); 720-chord values 701.6628 / 701.6642 | correct |
| App: 288.196 (branch height) | 400 − s_min = 288.1966 | correct |
| App: area 1130973.355 | 1200 × 2πR = 1130973.355 | correct |
| Scene 1 and 3 drawing proportions | header 1200×300, branch ⌀200 at 60°, axis meets header axis at station 600, saddle dip 38.2 mm, end-view ellipse — all correct to the pixel | correct |
| Scene 4 opening position | spans x = 571…802 (computed 571.13…802.07), centred on the half circumference | correct |

The film's own mathematics is right. The defects are elsewhere.

## Scores

| Criterion | Score | Justification |
|---|---|---|
| Technical correctness | 7 | Every number the film draws recomputes, but the real-software shot shows a header plate whose opening is transposed against its outline, and the redrawn branch pattern is mirrored against the product's. |
| Honesty, implemented versus planned | 5 | Planned marks exist and are mostly on time, but "AI-native" sits under an IMPLEMENTED tag while the repository states "no AI component exists", and the marks are about 13 px against 50 px headlines. |
| Authenticity of application footage | 7 | Real captures that honestly show REVIEW REQUIRED, INCONCLUSIVE and NOT IMPLEMENTED, but the text is about 11 px for 2 s per screen and the JA film shows an English interface without saying so. |
| Engineering geometry | 6 | The film's drawing, 3D model and patterns are dimensionally exact, but the product output on screen at 6–8 s is geometrically wrong and everything is zero-thickness reference surface. |
| Mathematical accuracy | 9 | Formula and all read-outs are correct to the printed digit; "zero offset" is ambiguous. |
| Storytelling | 5 | A stranger learns "drawing in, flat pattern out", which is the part that does not exist; the part that exists (verified patterns with a record trail) gets a 3-second bullet list and no narration. |
| Communicates a real differentiated product | 4 | The narration is generic AI-platform copy; the actual differentiator (independent checks, interface check, DXF round trip, gates that refuse release) is never spoken. |
| Japanese correctness and naturalness | 6 | Grammatical and readable, but several terms are translated rather than shop-floor Japanese, and voice and screen disagree on the tagline. |

**Overall: 5 / 10.** Plausible but unconvincing. I would believe the mathematics and not the product description. The one thing that would make me want a demonstration (results that check themselves and refuse release) is on screen but illegible.

## Defects, worst first

### 1. Product output on screen is geometrically wrong: header opening transposed against the plate outline
- **Time / language:** 6.0–8.0 s, EN and JA (scene 2, header pattern capture `assets/app-pattern-header.png`).
- **What is wrong:** The plate is drawn 942.478 wide (circumference) × 1200 tall (length). The opening on it spans about 569–801 horizontally and 360–582 vertically (measured on the frame against the app's own ruler). Those are the station range (571–802) and the circumferential range (362–581). So the opening is plotted as (station, circumference) on an outline that is (circumference, length). With station 600 on a 1200 plate the opening should be centred at 471 across and 687 up; it is at 687 across and 471 up, and rotated 90°.
- **Code:** `cylinder_shell` builds the outline as x ∈ [0, 2πR_dev], y ∈ [0, length] (`develop.rs:18-19`). `header_with_opening` builds the hole as x = station + …, y = r_dev·(ω+π) (`develop.rs:305`) and tests x against `length` (`develop.rs:319`). The two disagree on which axis is which.
- **Why checks pass:** `verify_interface` folds the hole back with the hole's own convention, so it cannot see the mismatch. The DXF re-import compares the DXF to the same wrong geometry.
- **Consequence:** The DXF this film calls "written, re-read and compared" would cut the opening in the wrong place and orientation. The film's redrawn pattern at 22–25 s is correct, so the film shows a result the product does not produce.
- **Severity:** Critical (product), high (film).
- **Fix:** Fix the product first (make outline and hole share one axis convention; add a check that the opening's axial extent equals `opening_x_min/max` on the length axis). Recapture scene 2. Until then, do not show the header capture and do not claim the DXF check for this part.
- **Confidence:** High on the code reading; the frame measurement agrees with it.

### 2. "AI-native engineering platform" shown as implemented; no AI exists
- **Time / language:** 5.0–11.0 s, EN ("AI-native engineering platform powered by AMOEBA") and JA ("AMOEBAを核とする、AIネイティブの設計基盤"). Spoken and on screen, with the tag "IMPLEMENTED · ACTUAL SOFTWARE · DEVELOPMENT BUILD" / "実装済み".
- **What is wrong:** `docs/RELEASE_READINESS.md`: "no AI component exists". AMOEBA in the build is a record store with dependency edges; scheduler, probes, coverage ledger and lanes are NOT STARTED. The only AI on screen is gate G14: "Algorithms are evidence level EV-1 (AI-generated, not independently reviewed)" — which a careful viewer will read as the opposite of a selling point.
- **Severity:** High.
- **Fix:** Drop "AI-native" from scene 2, or move it to a line marked planned. Suggested: "Meet NEXUS FABRIC — verified plate development with a full record trail" / "検証付きの板展開と、すべての計算記録".

### 3. Narration states planned capabilities as present tense; the qualifier is small and silent
- **Time / language:** 11.2–15.3 s EN ("Understand complex drawings. Reconstruct 3D geometry."), 11.2–15.5 s JA; 20–22 s both ("evaluate manufacturing constraints" / "製造制約の評価"); 24.6–27 s ("From drawings to manufacturing intelligence").
- **What is on screen:** amber "PLANNED CAPABILITY · CONCEPT" bottom left, full strength from 12.0 s; "DRAWING READING AND 3D RECONSTRUCTION: PLANNED" bottom right, full strength only from 13.0 s; "MANUFACTURING-CONSTRAINT EVALUATION: PLANNED" full from 19.5 s. Cap height about 12–13 px, letter-spaced, in the corners. The headline "Understand. Reconstruct. Calculate." is about 50 px, white, unqualified. A listener who is not reading corners hears a product that reads drawings.
- **Gaps with no status mark at all:** 11.0–11.5 s and 18.0–19.0 s (narration for scene 4 starts at 18.2 s).
- **Severity:** High.
- **Fix:** Put the word in the voice and headline: "Planned: drawing understanding and 3D reconstruction. Working today: pattern calculation." Raise the marks to at least 22 px and attach them to the headline. Have each mark on screen from the first frame of its scene.

### 4. "Computed by the product's mathematics engine" is not literally true
- **Time / language:** 19.0–25.5 s, both.
- **What is wrong:** `film.html` recomputes the geometry in JavaScript (`sOf`, `polyLen(…, 720)`). The values match the product, but the product's engine did not produce these pictures, and for the header the product's actual output differs (defect 1). The 3D model and the unrolling animation also do not exist in the product ("3D visualisation: NOT STARTED").
- **Severity:** Medium-high.
- **Fix:** "Values match the product's output · animation made for this film", and state "3D view: film only".

### 5. Redrawn branch pattern is the mirror image of the product's
- **Time / language:** 21.5–25.5 s versus 8.0–9.5 s, both.
- **What is wrong:** The app shows the short side of the stub (φ=270°) at three-quarters of the width; the film shows it at one quarter. A mirrored pattern is a different part once you mark and roll it.
- **Severity:** Medium.
- **Fix:** Draw the film pattern in the product's u direction, or state the viewing face in both.

### 6. "Traceable CAD deliverables" overstates
- **Time / language:** 18–25 s headline and narration, both ("追跡可能なCADデータ").
- **What is wrong:** Entity-to-source traceability is IN DEVELOPMENT (per-entity map NOT STARTED). The DXF is R12 LINE/ARC/POLYLINE, "no CAM system has been tested", and the app's own button reads "DXF · not released". "Deliverable" implies something you can send to the shop.
- **Severity:** Medium.
- **Fix:** "DXF with a recorded calculation trail — not released for manufacture".

### 7. "Independent verification checks" overstates
- **Time / language:** 22.5–25.5 s list, both ("独立した検証チェック").
- **What is wrong:** Status: "different formulation, same crate"; release readiness: "Partial … no separate implementation, no kernel cross-check". Defect 1 shows what that independence misses.
- **Severity:** Medium.
- **Fix:** "Second-formulation checks" / "別定式による照合".

### 8. Closing slogan promises manufacture; disclaimer is late and small
- **Time / language:** 26.5–30 s. "UNDERSTAND. RECONSTRUCT. VERIFY. MANUFACTURE." / "理解する。再構築する。検証する。製造する。"; disclaimer full strength only from 27.5 s, about 13 px.
- **What is wrong:** Three of the four verbs are not implemented and APPROVED is unreachable by design. 2.5 s of small text does not balance it.
- **Severity:** Medium.
- **Fix:** Show the disclaimer from 25.0 s at 20 px or more, or change the slogan to what exists ("Calculate. Verify. Record.").

### 9. The problem statement is not credible to the target viewer
- **Time / language:** 0–5 s, both. "Complex engineering drawings. Difficult geometry. Hours of engineering work." / "膨大な工数".
- **What is wrong:** The drawing is a single 60° lateral on a straight header. A plate-development engineer does this in minutes with existing tools. "Complex" and "hours" over this picture reads as not knowing the trade. The drawing also lacks the station dimension and the branch length that the calculation needs.
- **Severity:** Medium.
- **Fix:** Keep the lateral but change the claim to the real pain: "Is the pattern right? Do the two parts fit? Can you prove it later?" Add the 600 and 400 dimensions.

### 10. "reference surface · zero offset" is ambiguous
- **Time / language:** 14.5–18 s, EN and JA ("基準面・オフセット 0").
- **What is wrong:** It means axis offset δ = 0. Next to "reference surface" an engineer reads it as "no thickness offset", which is also true but is a limitation (no neutral-axis, weld-gap or kerf allowance), not a parameter.
- **Severity:** Low-medium.
- **Fix:** "δ = 0 (axes intersect) · plate thickness not modelled" / "δ = 0(軸交差)・板厚未考慮".

### 11. Application text is not legible at viewing speed
- **Time / language:** 6–10.5 s, both.
- **What is wrong:** Gate text is about 11 px and each screen is held about 2 s. The most persuasive content in the film (a tool that says INCONCLUSIVE about itself) cannot be read without pausing.
- **Severity:** Low-medium.
- **Fix:** Hold one screen and zoom on three gate rows with a spoken line.

### 12. Japanese wording and consistency
- **Language:** JA. Severity low to medium each.
- 0.2 s–: nothing wrong with "膨大な設計工数"; narration says "膨大な工数" — acceptable.
- 26.6 s: voice and subtitle say "設計、再定義。"; screen says "エンジニアリングを、再定義する。". Pick one.
- "製造の知恵へ": "知恵" is folksy for "intelligence". Use "図面から、製造へ" or "製造インテリジェンス".
- "追跡可能なCADデータ": understood, but the trade says "トレーサビリティ". Use "トレーサブルなCADデータ" or "計算履歴付きDXF".
- "部品間の接合照合": unnatural. Use "部品間の取合い照合".
- "独立した検証チェック": redundant. Use "独立検証" (and see defect 7).
- "実演・製品の数理エンジンによる計算結果": "実演" means a live performance. Use "計算例" or "実証".
- "設計基盤" for "engineering platform": "設計" narrows it to design; for plate development "エンジニアリング基盤" or "展開設計基盤" fits better.
- Drawing uses ⌀300; Japanese shop drawings write φ300. Minor.
- The interface shown is English; status lists Japanese localisation as NOT STARTED. Add "画面は英語版(開発版)".

## What works and should be kept

- The lateral drawn to scale, with "s(φ) = ?" on the saddle, and the same curve answered in scene 3.
- Printing 701.663 and 701.664 rather than a rounded common value.
- Showing REVIEW REQUIRED, INCONCLUSIVE and NOT IMPLEMENTED in the real interface.
- The amber planned marks as a system; they need size and timing, not removal.
