# CRITIC A — Design Review, Round 1
Film: NEXUS FABRIC launch 30s (EN + JA), 1920x1080 @30fps
Reviewer stance: independent creative director, design only (engineering credibility and audio not judged).

## Method
- Frames extracted at 2 fps from both files across the full 30 s (60 frames each), contact sheets of both, and full-resolution reads of at least two frames per scene per language (EN 3.0, 5.5, 7.0, 9.0, 10.0, 11.5, 15.0, 19.5, 23.5, 28.5; JA 3.0, 7.0, 15.5, 23.5, 28.5).
- 10 fps bursts as contact sheets over 4.5–5.9, 10.5–12.0, 17.5–19.0, 24.5–26.5 s (EN and JA).
- Files: /tmp/build/criticA/

## Structure observed
| Time | Scene | Content |
|---|---|---|
| 0.0–4.7 | 01/05 | Line drawing of a 60 deg pipe branch builds on, inside a title-block frame. "Hours of engineering work" / 膨大な設計工数 |
| 4.7–10.6 | 02/05 | Real UI screenshot (workspace, gates panel), crop-zooms to the canvas, crossfade to developed-pattern view, then crossfade to a "Records by family and kind" table. "Meet NEXUS FABRIC / engineering platform with automated drawing reading (not implemented), powered by AMOEBA" |
| 10.6–17.7 | 03/05 | Dip to black, drawing redraws, extrudes into a grey 3D tee; formula left, parameter table right. "Understand. Reconstruct. Calculate." |
| 17.7–24.7 | 04/05 | Tee rotates, separates, both parts unroll to flat patterns, then become 2D dimensioned DXF-style outlines plus an "IMPLEMENTED" bullet list. "Traceable CAD deliverables" |
| 24.7–30.0 | end | Line sweeps in, wordmark NEXUS FABRIC, "Engineering, Reimagined.", "Drawings to manufacturing intelligence", "UNDERSTAND. RECONSTRUCT. VERIFY. MANUFACTURE.", disclaimer |

## Scores (1–10)
| Criterion | Score | Justification |
|---|---|---|
| Visual restraint | 6 | Palette discipline is good (graphite ground, white line, one blue, sparse orange), but every scene is loaded with chrome (wordmark, 0X/05 counter, one or two caps disclaimers, side tables, bullets) and scene 2 dumps a dense UI full of orange INCONCLUSIVE badges. |
| Originality | 4 | No Apple imitation, but the grammar is the default "dark deck": centered sans headline top, content box below, footer caption, page counter, dip-to-black between slides; nothing here belongs uniquely to NEXUS FABRIC. |
| Typography | 4 | EN headlines are set in what reads as Arial/Liberation Sans at a single regular weight with tight default tracking, so the type has no character and no hierarchy; the end card is the same face blown up with very wide tracking. |
| Composition | 5 | Grids and margins are consistent (96 px gutters, footer baseline 1015), but the headline-to-content gap is cramped in scene 2 (subtitle about 25 px above the screenshot), the 3D tee floats with dead space above it, and scene 4 is a slide layout (diagram left, bullets right). |
| Motion quality | 5 | The drawing build-on and the tee unrolling are the right idea and ease smoothly, but all four scene changes are full dips to black lasting about 0.6–0.9 s, scene 2 uses muddy crossfades between screenshots, and the 3D look is flat and untextured. |
| Brand identity | 4 | The small wave-V mark appears only in the header and disappears from the end card; the wordmark is generic wide-tracked caps; the end-line verbs (four) contradict the scene-3 headline (three). |
| Looks like a premium engineering product | 5 | The technical drawing and the dimensioned flat pattern feel credible and precise, but the grey plastic tee render, the raw records table with "No snapshots yet", and the warning-heavy UI read as an internal demo, not a flagship. |
| Japanese layout quality | 6 | Noto Sans JP is better than the EN face and copy fits well, but the middle-dot spacing in 理解·再構築·計算 is uneven, tracked 。 in the end-card verb line creates holes, the wordmark switches font between scene 2 (Noto Latin) and the end card (Arial), and the UI stays English. |

**Overall: 5 / 10.** This is competent, honest template work with two strong ideas: the drawing-to-solid-to-pattern thread and the dimensioned flat pattern. It does not yet have the typographic voice, the transitions or the render quality of a flagship film.

## Defects, worst first

1. **All transitions are dip-to-black slide changes** (both languages; 4.4–5.0, 10.5–11.3, 17.5–18.3, 24.3–25.3). Each scene fades fully out, then the next fades in with its headline. Around 10.8–11.2 and 24.6–25.0 the frame is almost empty. This is the main reason it reads as slides. *Severity: critical.* Fix: carry one object across every cut. The drawing at 4.5 should scale into the UI canvas; the canvas outline at 10.5 should become the drawing that extrudes; the tee is already continuous across 17.7, so keep the headline swap only; and the flat pattern at 24.5 should collapse into the blue rule of the end card. Swap headlines with a short (≤8 frame) offset cross-dissolve or mask wipe, not a fade through black.

2. **Generic typeface** (EN, every headline and the end wordmark). The headlines use a single regular weight of Arial/Liberation-class sans with default tracking. "Meet NEXUS FABRIC" at 5.5–10.5 and "Traceable CAD deliverables" look like PowerPoint. *Severity: critical.* Fix: license or choose one disciplined grotesk with real optical sizes, such as Inter Display, Söhne, Neue Haas or IBM Plex Sans as a cheaper fallback. Set headlines at medium/regular with −1.5 to −2% tracking, and use one mono (already present) only for data. Use the same family for the Latin in the JA version.

3. **Scene 2 crossfade ghosting and weak final screen** (both languages; 8.8–9.4 and 9.4–10.5). The developed-pattern screen dissolves through the records table, and both are legible at once (frame 9.0 shows table rows over the curve). The scene then rests for about 1 s on a raw "Records by family and kind" table whose right card says "No snapshots yet" and is clipped ("A snapshot is recorded when a proje"). *Severity: high.* Fix: cut the records table, or replace it with a tight, framed crop of a meaningful record. Use hard cuts or masked pushes between UI states, never 50% dissolves of two dense screens.

4. **Clipped UI crops** (both languages; 6.5–9.0). After the zoom, the left parameter column is cut mid-glyph ("100 mm" shows as "00 mm"; "outside", "_OUTER" and "opening" are truncated), and the gates panel is cut at the bottom ("G18 obligation closure"). *Severity: high.* Fix: choose crop rectangles that end on panel gutters, or mask the edges with a soft vignette. Never cut through text.

5. **Orange warning noise in the hero product shot** (both languages; 5.5–10.5). Nine orange INCONCLUSIVE / NOT IMPLEMENTED badges, plus an orange bar and an orange status line, make the brand's first product moment amber. *Severity: high (design).* Fix: frame the canvas and one gate row. If the honesty must stay, show it once, small, and keep orange to a single element on screen.

6. **3D render quality** (both languages; 12.5–21.5). The tee is matte grey with no environment reflection, ground shadow or edge AO. The white outline rims look like toon shading, and during the unroll (19.5–20.5) the header shows a hairline seam edge and a doubled end-ring. *Severity: high.* Fix: brushed-steel or titanium material with an HDRI studio environment, soft contact shadow, thin wall thickness visible at the cut ends, and no outline pass. Hide the seam edge until the unroll begins.

7. **Messaging inconsistency in the identity system** (both languages; 11.5–17.5 vs 26.5–30). The headline says "Understand. Reconstruct. Calculate." and the end card says "UNDERSTAND. RECONSTRUCT. VERIFY. MANUFACTURE." Two triad/tetrad lines compete, and "Engineering, Reimagined." is a stock tagline. *Severity: medium-high.* Fix: one verb sequence, used in both places. Drop "Reimagined" for a line specific to fabrication.

8. **End card lacks the brand mark and is over-stacked** (both languages; 26–30). Five text levels (wordmark, rule, tagline, descriptor, verb line) plus a disclaimer. The wave-V symbol from the header is absent, and the wordmark tracking (about 0.5 em) feels stretched rather than engineered. *Severity: medium.* Fix: symbol plus wordmark lock-up at about 0.12–0.18 em tracking, one line beneath, disclaimer. Resolve with the blue line drawing the symbol's wave.

9. **Chrome clutter on every scene** (both languages). Header wordmark, 0X/05 counter, a blue footer caption and often a second orange footer caption. The 0X/05 counter is pure deck furniture. *Severity: medium.* Fix: remove the page counter; merge the provenance labels into one small footer line per scene, set at lower contrast.

10. **Scene 2 headline crowding** (EN 5.5–10.5; JA same). Subtitle baseline about 200 px, screenshot top 230 px, while scene 1 leaves about 80 px between headline and frame. *Severity: medium.* Fix: hold one headline-to-content distance (at least 64 px) across scenes, or shrink the screenshot to 1180 px wide.

11. **Scene 3 dead space and side-panel imbalance** (both languages; 13–17.5). About 230 px of empty band between the headline and the tee. The formula block (left) and the parameter table (right) sit at different vertical centres (formula about 520, table about 550) and at different widths. *Severity: medium.* Fix: lower the headline zone or raise the tee by about 60 px. Centre both side panels on the tee's centroid line.

12. **Scene 4 ends as a bullet slide** (both languages; 22.5–24.5). An "IMPLEMENTED" list of four bullets next to a diagram. *Severity: medium.* Fix: replace the bullets with annotations attached to the geometry (leader lines to the cut contour, the DXF layer names, the re-read check), drawn on in sequence.

13. **Scene 1 opens on a near-empty frame** (both; 0.0–0.5). Black with a faint stroke. It is acceptable, but the title-block frame draws before the subject. *Severity: low.* Fix: start the pipe centreline first, then the frame last.

14. **JA: middle-dot spacing** (13–17.5). 理解·再構築·計算 uses a half-width "·" with asymmetric side bearings, so the dot sits closer to the following word. The 2D-scene footer has the same issue (実演·製品の…). *Severity: medium.* Fix: use the full-width 「・」 with palt off, or set explicit equal spacing.

15. **JA: tracked full stops in the end-card verb line** (26.5–30). 理解する。 再構築する。… is letterspaced with 。, which leaves large holes and an uneven rhythm. *Severity: medium.* Fix: remove tracking on kana and kanji and separate the items with fixed 1em spaces, or drop the 。.

16. **JA: wordmark font switches** (6–10.5 vs 25.5–30). In scene 2 NEXUS FABRIC is set in the Noto Sans JP Latin glyphs (light, narrower). On the end card it is Arial with wide tracking, so the brand name has two shapes in one film. *Severity: medium.* Fix: render the wordmark as a fixed vector asset in both languages.

17. **JA: formula caption in proportional font** (13–17.5). "R = 150 r = 100 β = 60°" is set in Noto with an italic β, while EN uses mono. *Severity: low.* Fix: keep data captions in the mono face in both languages.

18. **Scene 1 headline weight vs. the drawing** (EN 0.5–4.5). The large white headline is visually heavier than the 1 px drawing it introduces, so the eye goes to the words, not the product. *Severity: low.* Fix: reduce the headline to about 48 px, or bring it in after the drawing completes.

## What works (keep)
- The narrative thread: drawing, then solid, then unrolled pattern, then DXF. This is genuinely product-first and could become the film's signature move.
- The scene-1 technical drawing and the scene-4 dimensioned flat pattern: precise, quiet and ownable.
- The palette discipline: graphite, white and one engineering blue.
- The live φ / s(φ) readout tied to the blue intersection curve in scene 3.
