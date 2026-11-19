# Cook ledger: NEXUS FABRIC flagship film

Started: 2026-11-19
Time: cap 1.5 h (owner asked for low usage) — **cap reached after round 4**
Bar: composition 9 | light 9 | color 9 | materials 9 | detail 9 | motion 9 (out of 10)
Calibration offset: none yet (no owner estimate)
Usage: one zero-context critic per scored round
Gate: stills approved **no** | final render **not started**
Best so far: round 4, `cook/stills/r04/`

## Decisions

Made without asking, one line each.
- No visual references were supplied; the approved storyboard is the target and the first owner-approved stills become the reference.
- Text on screen kept (captions, real/concept labels, disclaimer) against cook's default, because the campaign's accuracy rules require it.
- Hard cap instead of cook's 8 h floor, and one critic instead of several, because the owner asked to limit usage.
- Geometry is computed in the film with the same equations as the product (Spec 02 eq. 4.3; isometric bend), not exported from the engine.
- Scenes 2, 6, 7 use the real application screenshots already captured for the draft animatic.
- State kept under `marketing/nexus-fabric/film/cook/`, not the repository root.

## User feedback inbox

- (none yet)

---

## Round 1
Focus: harness and first build. three.js in headless Chromium (SwiftShader), `window.seek(t)`, still renderer. Header shell with opening, branch stub, unrolling.
Measured: 0.5 to 1.8 s per 1080p frame. Scores: not scored (self-review only). Faults seen: paper-thin walls, jagged opening edge, label boxes, weak flat layout.

## Round 2
Focus: wall thickness (inner surface + rims), per-pixel opening, labels, layout.
Measured: 2.1 to 3.3 s per frame.
Critic (1, zero context), eyeballed scores, median per dimension: composition 5 | light 4 | colour 5 | materials 5 | detail 4.
Critic's main points: no world (objects float in a void); joint physically fake (no weld); weak first and last frames; heavy caption boxes.

## Round 3
Focus: the world. Fabrication bay in code (concrete floor, clad wall, columns, overhead strip lights, stands), environment map generated from that bay, weld bead on the exact intersection curve, fat-line drawing with anchored dimensions, lighter captions, new end card.
Not scored (self-review): steel read as woven/carbon and too dark; drawing cut by the floor; odd stands.

## Round 4
Focus: steel texture and brightness, drawing position, stands.
Measured: 1.2 to 4.5 s per frame.
Critic (1, zero context), eyeballed scores, median per dimension: composition 5 | light 5 | colour 5 | materials 5 | detail 4.
Change against round 2: light +1; others unchanged.
Critic's main points: steel still reads as a procedural shader (uniform streaks, clipped highlight, no scale/heat tint/bevels; weld too regular); setting not believable (dark void, floor debris texture, untextured stands; flat plates read as cards); captions collide with picture in places; end-card caption duplicates the tagline.

## Stop check

- Every dimension meets the bar: **no** (5/5/5/5/4 against 9).
- Owner approved the stills: **no**.
- Final deliverable rendered: **no** (gated).
Stopped on the hard cap. Reported plainly as below the bar.

## Next round, if continued

1. Steel: physically based maps with real variation (mill scale patches, heat tint band beside the weld, edge chamfers), irregular weld bead, no clipped highlights.
2. Setting: visible lit bay (brighter walls, gantry, racks, plate stock), textured stands, cutting table under the flat patterns with contact shadows.
3. Layout: caption-safe framing; no caption on the end card.
4. Motion has not been reviewed at all: only stills exist.

## Round 5 (production strategy changed at the owner's instruction)

References: R01 geometry (branch on pipe), R02 fabrication hall, R03 TIG weld macro (owner-supplied; kept local in `cook/refs/`, not committed).
Bar: 9/10 in each of seven categories from two independent zero-context critics. Fixed budget: one build pass, two correction passes, one scored review. Stills only; no video rendered.

Changes: physical steel material (anisotropic, roughness and albedo variation), AgX tone mapping, ribbed weld bead with heat tint on the exact intersection curve, fabrication hall (skylights, windows, gantry, benches, welders, pipe rack, plate stack, cutting table), plan board for the drawing, new camera sequence (board, approach, joint, weld macro, unroll to table, hero), depth of field, unboxed labels inside title-safe margins, separate JA caption layout, no caption on the end card.

### Measured acceptance criteria (stills in `cook/stills/r05/`)

| Criterion | Limit | Result |
|---|---|---|
| Clipped highlights (luma >= 250) per frame | < 0.5 % | pass, max 0.24 % (t18.5) |
| Crushed shadows (luma <= 8) per frame | < 2 % | fail on end card t0053 (2.82 %); others <= 0.14 % |
| Caption and labels inside 5 % title-safe margins | all frames | pass |
| Labels clear of scene objects | all frames | fail (t0036 label overlapped by plan board; amber note over windows) |
| No console errors | 0 | pass (one NaN in heat-tint strip found and fixed) |

### Critic scores (unaltered)

| Category | Critic A | Critic B | Bar |
|---|---|---|---|
| Composition | 5 | 6 | 9 |
| Lighting | 4 | 5 | 9 |
| Materials | 4 | 5 | 9 |
| Geometry | 6 | 6 | 9 |
| Realism | 3 | 4 | 9 |
| Typography | 7 | 6 | 9 |
| Storytelling | 7 | 6 | 9 |

Result: bar not met in any category. Compared with round 4 the critics were stricter (two reviewers, seven categories, direct comparison with R02/R03); this is not a like-for-like drop, and no score was adjusted.

### Defects recorded (both critics agree unless marked)

1. Weld bead reads as a twisted pearlescent ribbon lying on the surface: no fillet section, no fusion at the toes, ripples are folded flaps not overlapping crescents; bead colour pink rather than steel grey.
2. Heat tint is a wide airbrushed pale halo, lighter than the parent metal; hard overlay edge on the branch; should be narrow dark oxide bands at the toes.
3. Hall is a block-out: blank white window rectangles, untextured walls, box props, no haze, no floor wear or reflections; far from R02.
4. Steel reads as brushed aluminium with oversized smeared streaks; tonal/UV seam across the header at the branch; branch looks matte.
5. Supports: single small roller visible per stand, pipe appears to float; unexplained warm glow under the right stand.
6. Flat-pattern shot: blown blotch on the plate (0 % clipped by measure, but reads as an artefact), no visible plate thickness, no part marks; (B) the opening appears rotated off the shell axis and must be checked against the equations before any further use; unroll frame t0032 poorly framed and parts hover.
7. Labels collide with scene objects; amber note unreadable over windows; (B) JA label and disclaimer too small, tagline punctuation spacing.
8. Drawing: title-block text clipped; 1200 and ⌀200 lack dimension lines; a stray line crosses the header.
9. End card: hero dim and cropped, blank wall in the right third.
10. (B) Lower branch wall appears translucent in the macro.

Budget for this round is spent. Stop for an owner decision before further rounds.
