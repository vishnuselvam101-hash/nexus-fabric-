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
