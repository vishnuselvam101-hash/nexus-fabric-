# CRITIC A — Design Review, Round 2
Film: NEXUS FABRIC launch 30s, revised (EN + JA), 1920x1080 @30fps.
Design only. Audio not judged.

## Method
I extracted new frames at 2 fps from both files across the full 30 s, contact-sheeted both, and read full-resolution frames from every scene:
- EN: 3.5, 5.5, 7.0, 8.0, 9.5, 15.0, 19.5, 23.5, 28.5 s, plus transition stills at 5.07, 10.97, 11.1, 17.9, 18.1, 24.6 and 25.05 s.
- JA: 6.5, 23.5 and 28.5 s, plus transition stills at 5.07 and 11.1 s.

I also made 10 fps burst contact sheets of both languages at 4.3–6.3, 10.0–12.0, 17.2–19.2 and 24.0–26.0 s.
Frames are in /tmp/build/criticA/r2/

## Deliberate decisions (judged on execution only)
- **Amber "planned" marks beside headlines.** These are executed cleanly as one coloured line with a square bullet. However, they use the same size and weight as the subtitle, so in scenes 3 and 4 the amber line reads as the scene's subhead. See defect N4.
- **Inconclusive gate badges shown in one beat** (7.4–8.7). The crop is clean, sharp and legible, and the summary chips (5 PASS / 7 INCONCLUSIVE / 1 NOT IMPLEMENTED) are a good anchor. The problem is the wall of body text, about 17 px paragraphs, which cannot be read in 1.3 s and looks like a document dump. See defect N6.

## Scores (1–10)
| Criterion | R1 | R2 | Justification |
|---|---|---|---|
| Visual restraint | 6 | 7 | The page counter and double footers are gone and there is one caption per scene. Each frame now holds one subject. |
| Originality | 4 | 6 | The saddle curve becoming the logo wave (24.5–25.5) and the UI canvas collapsing into the drawing (10.3–11.2) are ownable moves. The left-aligned headline over a centred subject is still a familiar keynote layout. |
| Typography | 4 | 7 | A real grotesk display face with tight tracking, a clear headline/subhead/caption scale, the same family across both languages, and a proper symbol + wordmark lock-up. It is held back by the colliding headline crossfades and the inverted end-card hierarchy. |
| Composition | 5 | 6 | Margins are consistent (96 px). However, the left-aligned headline column and the centred content blocks never share an axis. There is also an orphan leader line in scene 4. |
| Motion quality | 5 | 6 | The dips to black are gone and objects now carry across cuts. Every headline swap, though, double-exposes two headlines in the same spot for about 0.2–0.3 s, which is the most visible flaw in the film. |
| Brand identity | 4 | 6 | The wave mark now ends the film and is derived from the product geometry. The tagline is still stock and the verb lines still disagree. |
| Looks like a premium engineering product | 5 | 6 | Better steel shading, a soft contact shadow, grain and vignette, and no toon outlines. The tube walls have no thickness, there is a step at the header end during the unroll, and the gates beat looks like a document. |
| Japanese layout quality | 6 | 7 | Full-width ・, untracked verb line, Latin set consistently, and a disclosed English UI. The tagline is over-tracked, and the JA headline swaps collide the same way as EN. |

**Overall: 6 / 10** (R1: 5). This is a real step up from template work. The film now has an identity idea and the basic transitions work. It is still not flagship quality because of the headline collisions, loose grid discipline and a stock end line.

## Round-1 defects 1–8: status
| # | R1 defect | Status | Evidence |
|---|---|---|---|
| 1 | Every transition was a dip to black | **Fixed, but a new defect came in with it** | 4.7–5.6: the drawing pushes off as the UI pushes on. 10.3–11.2: the UI canvas reduces to the drawing outline. 17.7: the tee stays on screen. 24.5–25.5: the saddle curve becomes the mark. There are no empty frames. The headline swaps at those cuts now collide (N1). |
| 2 | Generic Arial-like typeface | **Fixed** | All headlines (for example 3.5, 15.0, 23.5) use a display grotesk at about 64 px, semibold, with negative tracking. |
| 3 | Scene 2 muddy dissolves and records table | **Fixed** | The records table is gone. The sequence is UI (5.5–7.2), then the gates panel enlarged (7.4–8.7), then the canvas alone (9.0–10.3). No two dense screens are blended. |
| 4 | Clipped UI crops and orange noise | **Partly** | Crops now end on panel edges (6.5–7.2, left panel intact). Amber is still heavy in 5.5–7.2: a warning banner, a REVIEW REQUIRED bar, the status line, the "planned" mark and a yellow cut contour. The gates beat is deliberate. |
| 5 | Flat 3D render, seam | **Partly** | 12.5–21: graded steel shading, a contact shadow, no outline pass. Still missing: wall thickness at the open ends. A step/notch shows at the header's right end at 19.5–20.0. Faint shading moiré shows around the joint at 13–17. |
| 6 | Verb lines contradict; stock tagline | **Not fixed** | Scene 3 says "Understand. Reconstruct. Calculate." while the end card (26–30) says "UNDERSTAND. RECONSTRUCT. VERIFY. MANUFACTURE." The EN end card still reads "Engineering, Reimagined."; JA reads 設計を、再定義。 |
| 7 | JA middle dot, tracked 。, wordmark font switch | **Fixed** | 理解・再構築・計算 uses a full-width ・ (11–17.5). The end-card verb line is untracked (28.5). The wordmark is the same face in both languages and in every scene. |
| 8 | Deck furniture (counter, double footers, bullet slide) | **Fixed** | The 0X/05 counter is removed, each scene has one footer caption, and the scene-4 bullets became leader annotations (22.5–24.5). |

## Remaining and new defects, worst first

**N1. Headline crossfades double-expose. Critical. EN and JA.**
- At 5.0–5.2, "Hours of engineering work" and "Meet NEXUS FABRIC" sit on the same baseline at about 50% each, which makes an illegible pile-up. JA shows the same with 膨大な設計工数 and NEXUS FABRIC.
- The same happens at about 10.95–11.2 (Meet NEXUS FABRIC / Understand…; JA: NEXUS FABRIC / 理解・再構築・計算) and at about 17.9–18.15 (Understand… / Plate developments…).
- The subheads/amber lines double-expose at the same moments.

Fix: never cross-dissolve two text blocks in the same place. Fade the outgoing headline completely (about 6 frames, ease-in), hold 2–3 frames, then bring the new one in (about 8–10 frames, ease-out). Alternatively, use a masked vertical slide: the old line rises out of a clip box and the new line rises in behind it.

**N2. The headline column and content never share an axis. High. EN and JA, every scene.**
- Headlines and captions hang on x = 96.
- The content blocks start elsewhere: the drawing frame at 340, the UI at 320 and later 494, the gates panel at 470, the plate at 294, and the tee at about 690.
- So the frame has two unrelated alignment systems.

Fix: put the content on the same 12-column grid. Either start each content block at the headline's x (96, or an agreed inner column) or centre the headline as well. The DXF plate and the drawing frame should share a left edge across scenes 1 and 4.

**N3. Scene 4 annotation errors. Medium-high. EN and JA, 22.5–24.5.**
- A vertical leader rises from the saddle top (about x 1242, y 683–742) and ends in empty space. It points at nothing.
- The "Interface check between parts" leader runs from the hole through the plate border into its label. Its label sits on a dark plate that clips the leader.
- The upper leader starts at a dot on the plate edge, which reads as arbitrary.

Fix: one leader per callout, each ending on the label's text baseline, using elbow (orthogonal) leaders. Anchor the interface check to the saddle contour and the hole contour together, for example as a bracket. Remove the orphan line.

**N4. The amber "planned" line competes with the headline system. Medium. EN and JA, 5.5–24.5.**
- In scene 2 it sits on the subtitle line at the same size, so the eye reads "…powered by AMOEBA ■ Automated drawing reading: planned" as one sentence.
- In scenes 3 and 4 it is the only line under the headline, so it becomes the subhead.

Fix: set it as a caption (about 15–16 px, medium weight, amber about 80%) on its own line about 14 px below the subhead, or in the headline's top-right as a tag. Keep it visible but secondary.

**N5. End-card build and hierarchy. Medium. EN and JA, 25.0–30.**
- At 25.0–25.4 the wordmark fades in on top of the still-visible plate, callouts and dimension lines, and the travelling wave crosses the hole contour. This makes a muddy half-second.
- Once settled, the disclaimer (about 20 px, near-white) is brighter and larger than the verb line (about 14 px, grey, tracked caps) and visually the second-loudest element.
- The tagline is stock (see R1 #6).

Fix:
- Clear the plate before the wordmark arrives; keep only the saddle curve.
- Set the disclaimer at 14 px and 55% white, and give the verb line more contrast.
- Replace "Engineering, Reimagined." with one line specific to fabrication, and make the verbs match scene 3.

**N6. Gates beat reads as a document. Medium. EN and JA, 7.4–8.7.**
The enlarged panel shows seven paragraphs of 17 px body text for about 1.3 s. Nobody can read it, so it registers as clutter rather than honesty.

Fix: keep the scale, but dim the paragraphs to about 35% and hold focus on the summary chips row and the badge column. Optionally sweep a highlight down the badges.

**N7. 3D finish. Medium. EN and JA.**
- No visible wall thickness at the open ends (13–21).
- During the unroll (19.5–20.0) the header's right end shows a stepped notch.
- Faint shading moiré/banding around the joint (13–17).
- The branch's inner face is flat dark grey with no falloff.

Fix: model the wall thickness (even 6 mm reads at this scale), hide the seam until it opens, raise the anti-aliasing and normal resolution near the intersection, and add an inner-face gradient.

**N8. Scene 1 to scene 2 push mixes scales. Low-medium. EN and JA, 4.8–5.4.**
The drawing scales up about 1.4× while sliding left. At 5.07 the enlarged title block sits next to a UI at a different scale, divided by a hard vertical seam.

Fix: push at constant scale, or scale the drawing so its pipe outline lands on the UI canvas outline (match-cut by geometry).

**N9. Amber density in the first product shot. Low-medium. EN and JA, 5.5–7.2.**
The orange warning banner, REVIEW REQUIRED bar, status line and amber "planned" mark appear together, so amber again dominates the first look at the product. This is the remaining part of R1 #4.

Fix: keep the app's real UI, but frame the crop to exclude the top banner and the bottom status bar for this beat. The gates beat carries the honesty.

**N10. JA tagline tracking. Low. JA, 26–30.**
設計を、再定義。 is tracked at about +0.1 em, so the 、 leaves a visible hole and the line looks wider than the wordmark rhythm.

Fix: no tracking on kanji/kana, with palt (proportional alternates) on for the punctuation.

**N11. Scene 3 data block is undersized. Low. EN and JA, 13–17.5.**
The parameter line and the φ / s(φ) table are about 13 px mono at low contrast under a large formula, so the live readout, the interesting part, is the hardest thing to read.

Fix: enlarge the φ and s(φ) values to about 22 px and drop the static parameter line to one row.

## What improved most
- Continuity: the canvas becomes the drawing, and the saddle becomes the mark.
- The typeface and the lock-up.
- Removal of the records table, the counter and the bullet slide.
- Japanese typography.
