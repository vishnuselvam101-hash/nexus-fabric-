# Critic B — engineering and product credibility, round 2

Same role and strictness as round 1. Date: 2026-11-19. Films: `final/NEXUS-FABRIC-launch-30s-{en,ja}.mp4` (revision 2, commit 45ec342).

## Method

- Fresh frames every 0.5 s from both films (60 each) at 1920x1080. Read 14 EN and 4 JA frames at full size, plus a strip of the bottom status band for all 60 frames in each language.
- Recomputed the numbers shown. Read `crates/uem/src/develop.rs` and `verify.rs` after commit dac9018 and ran `cargo test -p uem` (18 pass, including `header_outline_and_opening_share_one_frame`).
- Read `docs/LEARNING_AND_STRESS_TESTING.md` (Defect 002) and `docs/backend/DEFECT_REGISTER.md` (BE-001 to BE-034).
- Audio not heard; narration judged from `narration.json` and the SRT timings.
- Not read at full size: the transition frames between those examined, and the branch-pattern capture in scene 2 if it is still used (I did not land on it).

## Verification of the product fix (round-1 defect 1)

- `header_with_opening` now rebuilds the outline as x ∈ [0, length], y ∈ [0, 2πR] (`develop.rs:304-305`), the frame the opening is computed in.
- `verify.rs:236-237` adds two checks comparing the outline extents with the input length and 2πr; a unit test confirms the old construction is rejected.
- New capture, measured on the frame at 7.0 s against the app's ruler: plate 1,200.000 × 942.478; opening spans about 572–803 along the length and 360–583 around. Computed: 571.13–802.07 and 361.78–580.70. Correct.
- The app's curve table now reads seam_a 1,200.000, top 942.478, branch_opening 701.663, "Plate extent x × y 1,200.000 × 942.478", area 1,130,973.355. All correct.

## Numbers recomputed (new in this revision)

| On screen | Recomputed | Verdict |
|---|---|---|
| φ = 215.2° → 178.53 mm (EN 16.0 s) | 178.53 | correct |
| φ = 153.1° → 113.15 mm (JA 15.5 s) | 113.15 | correct |
| Drawing: 600 station, 400 along branch axis, ⌀200, ⌀300, 1200, 60° | to scale on the frame (400 dimension measures 404 at 0.5 px/mm) | correct |
| 2πR = 942.478, 2πr = 628.319, CUT_INNER 701.663, saddle_cut 701.664 | unchanged from round 1 | correct |
| Redrawn branch pattern | short side now at three quarters of the width, as in the product | correct |

## Scores

| Criterion | R1 | R2 | Justification |
|---|---|---|---|
| Technical correctness | 7 | 9 | Everything drawn or captured now recomputes, and the film's pattern and the product's pattern agree. |
| Honesty, implemented versus planned | 5 | 7 | Planned marks are now about 20 px, amber, directly under each headline from the first frame of the scene; the voice still states planned capabilities in the present tense. |
| Authenticity of application footage | 7 | 9 | 2x captures of the corrected build, with a legible gates panel and a "screen is in English" note in the JA film. |
| Engineering geometry | 6 | 9 | Correct plate, correct opening, dimensions that define the part; still reference-surface only, which the gates panel itself says. |
| Mathematical accuracy | 9 | 9 | All read-outs correct; "δ = 0" replaces the ambiguous "zero offset". |
| Storytelling | 5 | 6 | Clearer "drawing → part → patterns → checks" line, but the spoken story is still the planned product. |
| Communicates a real differentiated product | 4 | 6 | The gates panel (5 pass, 7 inconclusive, 1 not implemented) and the interface-check call-out now read on screen; the narration never mentions them. |
| Japanese | 6 | 8 | Tagline consistent between voice and screen, 取合い確認 and トレーサブル are shop terms, English-interface note added. |

**Overall: 7 / 10** (round 1: 5). A sceptical engineer who watches with the sound off would believe it. With the sound on, the voice still sells what the amber marks say is planned.

## Round-1 defects

| # | Defect | Status | Evidence |
|---|---|---|---|
| 1 | Header opening transposed against the plate | **Fixed** | Code, tests and the 5.5–10.5 s captures; see above. |
| 2 | The automated-intelligence claim presented as implemented | **Partly** | 5.5–10.5 s: amber "Automated drawing reading: planned" / 「図面の自動読み取り：計画中」 beside the line at the same size; tag now reads "Actual software · development build" with no "Implemented". Judgement below. |
| 3 | Narration states planned capabilities; marks small, late, with gaps | **Partly** | Marks present at 11.0 s and 18.0 s (first frame of each scene), about 20 px, under the headline. No gaps. Narration text unchanged. |
| 4 | "Computed by the product's mathematics engine" | **Fixed** | 11–18 s "Visualisation made for this film · not a product view"; 18–24.5 s "Same equations as the product · redrawn for this film". |
| 5 | Branch pattern mirrored | **Fixed** | 23 s onward. |
| 6 | "Traceable CAD deliverables" | **Partly** | Headline is now "Plate developments. DXF output." / "板展開。DXF出力。". EN voice still says "generate traceable CAD deliverables" (21.9–24.5 s); JA voice "トレーサブルなCADデータの生成". |
| 7 | "Independent verification checks" | **Fixed** | Call-outs read "Verification checks · Interface check between parts" and "DXF written, re-read and compared". Gate G10 text, legible at 8.5 s, states the limit itself. |
| 8 | "MANUFACTURE." slogan; late, small disclaimer | **Partly** | Disclaimer about 20 px, full strength 25.0–30.0 s. Slogan unchanged at 27.5–30 s, now small and grey. |
| 9 | "Complex… hours" over a simple lateral | **Partly** | 600 and 400 dimensions added, so the drawing now defines the part. The claim is unchanged. |

Round-1 items 10–12: "zero offset" fixed (δ = 0); gates text fixed (large and legible at 8.5 s); Japanese mostly fixed.

## Is the automated-intelligence positioning line honest enough?

On screen: acceptable, narrowly. The amber mark sits beside the claim at equal size for the whole scene, the gates panel under it says "drawing intelligence is not implemented", and the request panel says "ENTERED MANUALLY". A reader is not misled.

Two reservations:
- The mark says automated *drawing reading* is planned. The repository says no automated-reading component of any kind exists. The mark is true but narrower than the fact. "Automated-reading components: planned" would be exact.
- A listener who is not reading hears the positioning line calling it an engineering platform with automated drawing reading, with no qualifier. For a film that will be played in rooms, that is the remaining honesty gap.

I would not block release on it. I would not call it a 9.

## Does anything shown depend on a defect in `DEFECT_REGISTER.md`?

Nothing shown is made wrong by a listed defect. Three touch points:
- **BE-011 (engine version never changes):** the capture's status bar reads "Engine nf-core 0.1.0 / uem 0.1.0", the same string as builds with the transposed opening. It is small, but it is on screen at 5.5–7 s. Bump the version and recapture when BE-011 is fixed.
- **BE-001 (contact radius, reference surface and development radius not cross-checked):** the filmed job uses 150/150 and 100/100 with "outside", so it does not exercise the defect. Gate G02 is shown INCONCLUSIVE, which is the honest state.
- **BE-002 (release state fails open on damaged stored gates):** the film shows "REVIEW REQUIRED — this result cannot be released", which is true for this job. The film does not claim approval is impossible, so it does not depend on BE-002. Do not add such a claim until BE-002 is closed.

BE-003 to BE-010 and BE-012 onward concern refusals, corrections, storage, concurrency and operations; none appears in the film.

## Remaining and new defects, worst first

1. **Voice sells the planned product (11.2–15.5 s, 18.2–24.5 s, EN and JA). High.** "Understand complex drawings. Reconstruct 3D geometry." and "evaluate manufacturing constraints" are present tense with no spoken qualifier. Fix: one spoken word per planned item ("Planned: …" / "計画中：…"), or reorder so the voice describes what is on screen: calculation, checks, DXF.
2. **"Traceable CAD deliverables" in the voice (EN 21.9–24.5 s, JA 22.1–24.7 s). Medium.** Per-entity traceability is not started; the DXF button reads "not released". Fix: "DXF output with a recorded calculation trail".
3. **Differentiator is silent (8.0–9.5 s, 22–25 s). Medium.** The gates panel and the interface check are the reason to ask for a demonstration and get about 1.5 s and no narration. Fix: hold the gates panel 3 s and give it the scene-2 line.
4. **Closing slogan "UNDERSTAND. RECONSTRUCT. VERIFY. MANUFACTURE." / "…製造する。" (27.5–30 s). Medium-low.** Three of four verbs are planned. The disclaimer now balances it better. Fix: "Calculate. Verify. Record." or mark it as the roadmap.
5. **Problem statement (0–5 s). Medium-low.** "Complex… hours" over one lateral. Fix as in round 1: ask whether the pattern is right and the parts fit.
6. **Amber mark narrower than the fact (5.5–10.5 s). Low.** See above.
7. **Engine version string on screen (5.5–7 s). Low.** BE-011.
8. **Transition artefact (24.5 s, both). Low.** A second copy of the saddle curve crosses the header plate before becoming the logo mark; for half a second it reads as a stray cut line on the pattern. Start the morph after the plate has faded.
9. **Japanese (low).** "AIネイティブの設計基盤" still narrows "engineering" to design; drawing still uses ⌀ where Japanese drawings write φ.
