# Critic B — engineering and product credibility, round 3

Same role and strictness. Date: 2026-11-19. Films: `final/NEXUS-FABRIC-launch-30s-{en,ja}.mp4` (revision 3, commit e10ee29). Picture-only revision; narration and subtitles unchanged.

## Method

- Fresh frames every 0.5 s from both films (60 each) at 1920x1080. Read 10 EN and 2 JA frames at full size, plus a strip of the headline/tag band for all 60 EN frames.
- Recomputed the numbers that appear. Product code was verified in round 2 and not re-read.
- Audio not heard.
- Not verified: how long the gates crop is held (I read it at 8.5 s only; my attempt to tile neighbouring frames failed), and the JA tag band frame by frame (read at 8.5 s and 23.5 s).

## What I read and checked

| Time | On screen | Check |
|---|---|---|
| 3.5 s | Drawing: 1200, 600, 400, ⌀300, ⌀200, 60°, title block R150 / r100 / 60° | to scale, unchanged from round 2 |
| 6.5 s | App: plate 1,200.000 × 942.478, branch_opening 701.663, area 1,130,973.355; boxed tag "AUTOMATED DRAWING READING: PLANNED" | correct |
| 8.5 s | Gates crop: 5 PASS, 7 INCONCLUSIVE, 1 NOT IMPLEMENTED; G01 "drawing intelligence is not implemented", G02, G05 | readable at normal size |
| 15.5 s | φ = 153.1°, s(φ) = 113.15 mm; R = 150, r = 100, β = 60°, δ = 0 | 113.15, correct |
| 19.5 s | 3D parts separating, opening outlined | opening shape and position plausible |
| 23.5 s | 1200, 2πR = 942.478, 2πr = 628.319, CUT_INNER 701.663 ↔ saddle_cut 701.664, "DXF written, re-read and compared" | correct |
| 24.5 s | Saddle curve alone on black, becoming the logo mark | stray line over the plate is gone |
| 27–30 s | Logo, "Engineering, Reimagined.", slogan, disclaimer | unchanged |

Tag timing (EN strip): the boxed amber tag is on every sampled frame of scene 2 (5.0–10.5 s), scene 3 (11.0–17.5 s) and scene 4 (18.0–24.0 s). No gaps.

## Scores

| Criterion | R2 | R3 | Justification |
|---|---|---|---|
| Technical correctness | 9 | 9 | Every number recomputes; the new wall thickness is the one thing drawn that the product does not model. |
| Honesty, implemented versus planned | 7 | 8 | Boxed amber tags are now the second most prominent text in each scene and never absent; the voice is unchanged. |
| Authenticity of application footage | 9 | 9 | Real, corrected, and the gates crop is now legible. |
| Engineering geometry | 9 | 9 | Correct throughout; thick-walled render over a zero-thickness calculation costs the point it might have gained. |
| Mathematical accuracy | 9 | 9 | Correct; the 701.663 ↔ 701.664 pairing is right but unexplained. |
| Storytelling | 6 | 7 | Picture now tells the true story on its own; the voice tells a different one. |
| Communicates a real differentiated product | 6 | 7 | A stranger can now read "7 inconclusive, 1 not implemented" and the interface pairing; nobody says why that matters. |
| Japanese | 8 | 8 | No change to wording. |

**Overall: 8 / 10** (round 2: 7). The picture is at a 9. The sound track holds the film at 8.

## Round-2 remaining items

| # | Item | Status |
|---|---|---|
| 1 | Voice states planned capabilities in the present tense | **Not fixed** (owner decision pending). Mitigated: tags are larger and boxed. |
| 2 | "Traceable CAD deliverables" in the voice | **Not fixed.** |
| 3 | Differentiator silent and brief | **Partly.** Gates crop is readable and the interface pairing is on screen; still not narrated. |
| 4 | Slogan "…MANUFACTURE." | **Not fixed.** |
| 5 | "Complex… hours" over one lateral | **Not fixed.** |
| 6 | Automated-reading tag narrower than the fact | **Not fixed** in wording; **better** in prominence. |
| 7 | Engine version string on screen | **Fixed in the film**: the status bar is cropped out at 6.5 s. BE-011 itself is a product matter. |
| 8 | Stray curve at 24.5 s | **Fixed.** |
| 9 | Japanese: 設計基盤, ⌀ | **Not fixed.** Low. |

## Remaining defects, worst first

1. **Narration contradicts the tags (11.2–15.5 s, 18.2–24.5 s, both languages). High.** Voice: "Understand complex drawings. Reconstruct 3D geometry." Screen: "DRAWING READING AND 3D RECONSTRUCTION: PLANNED". A viewer hears one thing and reads the opposite.
2. **The automated-intelligence claim spoken without a qualifier (5.2–10.2 s). Medium.** Honest on screen; not in the room.
3. **"Traceable CAD deliverables" spoken (EN 21.9–24.5 s, JA 22.1–24.7 s). Medium.**
4. **New: wall thickness on the 3D model (11–22 s, both). Medium-low.** The pipes now have a visible wall. The product calculates one reference surface; gate G06 says thickness is not modelled. In scene 3 the footer says "not a product view", which covers it. In scene 4 the footer says "Same equations as the product" over a thick-walled part, which invites the question "inside, outside or neutral?" and the product has no answer. Fix: return to a thin shell in scene 4, or add "reference surface only · thickness not modelled" to the footer.
5. **Slogan (27.5–30 s). Medium-low.**
6. **Opening claim (0–5 s). Medium-low.**
7. **701.663 ↔ 701.664 unexplained (22.5–24 s). Low.** An engineer will ask why mating edges differ by 0.001. The real check is a 3D fold-back distance, not a length comparison. Fix: "same 3D curve · lengths agree to 0.001 mm (720-chord polyline)".
8. **Japanese. Low.** As round 2.

## What a 9 needs

Picture: only item 4 and item 7 above. Everything else is narration and the end slogan.

Narration I would accept (same five segments, same lengths):

**English**
1. "One branch. Two plates. Is the pattern right — and can you prove it?"
2. "Meet NEXUS FABRIC: plate development that checks its own work, built on AMOEBA."
3. "Enter the geometry. The intersection is calculated, not drawn. Reading it from the drawing is planned."
4. "Flat patterns. Mating edges checked against each other. DXF written, re-read and compared."
5. "Every result recorded. Nothing released without review. NEXUS FABRIC."

**Japanese**
1. 「枝管ひとつ、板二枚。その展開、正しいと証明できますか。」
2. 「NEXUS FABRIC。AMOEBAを核とする、自ら検証する板展開。」
3. 「寸法を入力。相貫線は作図ではなく計算で。図面からの読み取りは計画中です。」
4. 「展開図。取合いを相互に照合。DXFは出力後に再読込して照合。」
5. 「すべての計算を記録。レビューなしでは出図しない。NEXUS FABRIC。」

If the owner keeps the automated-intelligence positioning: I would accept "…an engineering platform with automated drawing reading **in development**, powered by AMOEBA" / 「開発中の、図面の自動読み取りを備えた設計基盤」 in segment 2, provided segments 3 and 4 use the lines above. With the owner's current segments 3 and 4 unchanged, the film cannot reach 9.

End slogan for a 9: "CALCULATE. VERIFY. RECORD." / 「計算する。検証する。記録する。」, or keep the four verbs with "ROADMAP" / 「ロードマップ」 in front.
