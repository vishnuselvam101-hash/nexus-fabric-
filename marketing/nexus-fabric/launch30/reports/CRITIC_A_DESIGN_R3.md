# CRITIC A — Design Review, Round 3
Film: NEXUS FABRIC launch 30s, picture-only revision (EN + JA), 1920x1080 @30fps. I judged design only and did not judge audio or narration.

## Method
- Extracted fresh 2 fps frames from both files across the whole 30 s and built contact sheets for both languages.
- Read these frames at full resolution:
  - EN: 5.0, 6.0, 8.0, 15.0, 19.5, 23.5 and 28.5 s.
  - JA: 15.0 and 28.5 s.
- Made 30 fps strips of the headline zone (crop 1100x260) across each headline change: 4.95–5.35, 10.85–11.3 and 17.8–18.25, for both EN and JA.
- Made a 15 fps strip of the end transition at 24.3–26.1 s for both languages.

Frames are in /tmp/build/criticA/r3/

## Scores (1–10)
| Criterion | R1 | R2 | R3 | Justification |
|---|---|---|---|---|
| Visual restraint | 6 | 7 | 7 | Each scene now holds one subject and one caption. The boxed amber caps tag under every headline is the loudest graphic on most frames. |
| Originality | 4 | 6 | 6 | The saddle curve becoming the logo wave, now executed cleanly, is ownable. The overall grammar is still a centred headline over a centred object on near-black. |
| Typography | 4 | 7 | 7 | The display grotesk, the scale and the lock-up are solid, and the scene-3 readout is now properly sized. The amber tag in tracked caps inside a 1 px box works against the headline's calm. |
| Composition | 5 | 6 | 7 | Headlines and content now share the centre axis, scene-4 callouts are correct, and the gates beat is well framed. The scene-2 product crop cuts through text again. |
| Motion quality | 5 | 6 | 7 | Every headline change is now clean: the old line fades out, there are about 3 frames of empty headline zone, then the new line fades in. The end transition clears the plate before the mark resolves. The scene 1→2 push still mixes scales. |
| Brand identity | 4 | 6 | 6 | The mark-from-geometry ending is strong. The end line "Engineering, Reimagined." and the four-verb line that contradicts scene 3 are still on screen. |
| Looks like a premium engineering product | 5 | 6 | 7 | The pipe now shows wall thickness, the unroll is clean, and the gates beat is legible and confident. The render is still matte CG with no environment reflection, and the product shot is clipped. |
| Japanese layout quality | 6 | 7 | 8 | Full-width ・, untracked tagline and verb line, a consistent Latin face and correct handoffs. Only the amber tag box is slightly cramped around the kana. |

**Overall: 7 / 10** (R1 5, R2 6). This is a disciplined, well-made film with one ownable idea. It is not yet a flagship film.

## Status of earlier defects
| Item | Status | Evidence |
|---|---|---|
| N1 headline double-exposure | **Fixed** | 30 fps strips, EN and JA: at 5.0–5.3, 10.88–11.3 and 17.9–18.2 the outgoing headline reaches zero before the incoming one appears. No overlap appears in any frame. |
| N2 headline and content axis mismatch | **Fixed** | Headlines, subheads, tags and footers are centred, so content and type share the same axis (every scene). |
| N3 scene-4 orphan leader and bad leaders | **Fixed** | 22.5–24.5: the "DXF written…" callout is anchored to the plate corner. The interface-check callout carries a leader down to the saddle contour, with a data line (CUT_INNER 701.663 ↔ saddle_cut 701.664). There is no stray line. |
| N4 amber "planned" line competes | **Not fixed (changed form)** | 5.5–24.5: the line is now its own line, in tracked caps, about 20 px, bright amber, inside a 1 px amber box. It no longer merges with the subtitle, but it is now a warning sticker under every headline and louder than the subtitle. |
| N5 end-card build and hierarchy | **Partly** | 24.3–26: the plate and callouts fade first, the saddle curve travels alone and becomes the mark, and the wordmark resolves on clean black. The disclaimer is now grey and secondary. This is excellent. The stock tagline remains (see R1-6). |
| N6 gates beat reads as a document | **Fixed** | 7.4–8.7: three rows at large size, summary chips prominent, every word readable. Honest and confident. |
| R1-4 clipped crops and amber density | **Worse (clipping) / partly (amber)** | 5.5–7.2: the top banner and status bar are now cropped out, which reduces amber. However, the crop edges cut through text: the left nav reads "rojects & Documents", "ngineering Workspace" and "dministration", and the right gates column is cut mid-word ("confirm", "declarat") with badges halved at the edge. |
| R1-5 3D render | **Partly** | 12.5–21: wall thickness is now visible at all open ends, the unroll at 19.5–20 shows no notch, and the shadow is soft. Still matte: no environment reflection or specular highlight, and the inner surfaces are flat grey. |
| R1-6 verb lines and stock tagline | **Not fixed** | EN 26–30: "Engineering, Reimagined." / "UNDERSTAND. RECONSTRUCT. VERIFY. MANUFACTURE." still contradicts scene 3 ("Understand. Reconstruct. Calculate."). JA: 設計を、再定義。 |

## Remaining and new defects, worst first

**D1. Scene-2 product crop cuts through text. High. EN and JA, 5.5–7.2.**
This is a regression of R1-4. It is the first look at the real product and it reads as sloppy.

Fix: crop exactly on panel gutters. Include the full left nav or exclude it at the gutter, and end the right edge on the gates panel's outer border. Alternatively, apply a 60–80 px soft edge mask so no glyph is visibly cut.

**D2. Amber tag is too loud. Medium-high. EN and JA, 5.5–24.5.**
The boxed, tracked-caps amber label is the second-strongest element in every scene and makes the film read cautionary. The honesty decision stands; the execution is too heavy.

Fix: remove the box and use sentence case at about 15 px, 70–80% amber, with a small amber square. Keep it on its own line 12–16 px under the subhead.

**D3. End line and verb line. Medium-high. EN and JA, 26–30.**
"Engineering, Reimagined." is generic. The verb line also contradicts the scene-3 headline. This is on-screen text, so it is a picture decision even with the narration locked.

Fix: either make the end-card verbs match scene 3, or drop the verb line entirely and let the mark carry it. Replace the tagline with a line specific to fabrication.

**D4. Scene 1→2 push mixes scales. Medium. EN and JA, 4.8–5.4.**
The drawing enlarges while it slides off, so at about 5.0 the enlarged title-block fragment sits beside the UI with a hard seam.

Fix: push at constant scale, or match-cut the drawing's pipe outline onto the UI canvas outline.

**D5. Matte 3D. Medium. EN and JA, 12.5–21.**
There is no reflected environment, so the steel reads as grey plastic under studio light. The inner bores are uniformly dark.

Fix: add a soft HDRI or two long strip-light reflections along the pipe axis and a subtle anisotropic brushed highlight. Grade the bore interior to falloff.

**D6. Uneven vertical rhythm between headline block and content. Low-medium.**
The gap is about 60 px in scene 2 (tag to UI), about 130 px in the gates beat, about 200 px in scene 3 (tag to tee top) and about 80 px in scene 4 (tag to dimension).

Fix: one fixed headline block height and one content safe area, with content centred in that area.

**D7. Header wordmark at top left is the only left-aligned element. Low.**
Since R3 centred everything else, the small "NEXUS FABRIC" lock-up at x=96 is alone on the left.

Fix: keep it, but also anchor the footer caption, or remove the header and let the end card carry the brand.

## What it would take to reach 9
1. **Remove every craft blemish.** The product crop must not cut text, all scenes must share one vertical rhythm, and the amber tag must become a quiet caption. A 9 has no frame a designer would pause on and wince at.
2. **Final brand language.** A single owned end line, specific to fabrication, matching the verb sequence used in the film. A wordmark set with custom spacing rather than default tracking.
3. **Material quality of a hardware film.** Real steel (reflections, anisotropy, bevelled cut edges, a visible weld-prep chamfer on the saddle) and one hero lighting move, such as a slow light sweep across the tee during scene 3.
4. **One signature camera idea that runs through the whole film.** A single continuous geometric thread (drawing line → canvas → solid → pattern → mark) with no pushes or fades, so the film reads as one shot rather than five scenes.
5. **More silence.** Fewer simultaneous text elements per frame (at most a headline plus one caption) and at least one held beat of pure product with no type.
