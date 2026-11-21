# Launch film, Round 6: production report

Status on 2026-11-19: **not final**. Three revisions rendered and independently reviewed; best scores 7 (design) and 8 (credibility) against a bar of 9. No person has listened to the audio.

## What changed from Round 5
Round 5 was a photoreal three.js film (critics: realism 3–4 of 10). Round 6 replaced it, with the owner's approval, by a 30-second motion-graphics film built on the real application, real computed geometry and the product's own equation, in graphite, titanium and engineering blue, with voice, original music and sound design.

## Agents and what each delivered
Lead: this session's model (a requested Opus lead was not possible; Opus was used for audit, localisation and critic roles).

| Role(s) | Delivered |
|---|---|
| Motion graphics, cinematography, visual direction | `film.html`, `render.js`, `cues.json`, `FONTS.md`; three revisions |
| Japanese localisation | `script/narration.json`, `script/onscreen-ja-review.json`; two passes |
| Audio (voice, music, sound, mix, subtitles) | `audio/` sources and scripts, `subtitles/launch30-{en,ja}.{srt,vtt}` |
| Engineering and mathematical verification | `reports/ENGINEERING_REVIEW.md`; corrected three numeric labels |
| Product demonstration | real captures in `assets/` from an R150/r100/60° job, 2x resolution |
| Critic A, design | `reports/CRITIC_A_DESIGN_R1..R3.md` |
| Critic B, credibility | `reports/CRITIC_B_CREDIBILITY_R1..R3.md` |

## Critic scores (unaltered)

| | R1 | R2 | R3 |
|---|---|---|---|
| Design, overall | 5 | 6 | 7 |
| Credibility, overall | 5 | 7 | 8 |

## What the critics found, and what was corrected
- **Product defect (Critic B, R1):** the application's header pattern had its opening in the wrong frame. Confirmed and fixed in the product (`docs/LEARNING_AND_STRESS_TESTING.md`, Defect 002); footage recaptured.
- Corrected: fades through black; generic typeface; slide furniture; dissolving screens; small or late "planned" marks; "computed by the product's engine" wording; mirrored branch pattern; unnatural Japanese terms; overlapping headlines; split layout grid; stray leader lines; unreadable gates beat; no wall thickness.
- Open after R3, design: scene-2 crop cuts through text at 5.5–7.2 s; amber status tag judged too loud; end line and verb lines; matte steel; scale change in the scene 1→2 push; uneven vertical rhythm.
- Open after R3, credibility: the voice states planned capabilities in the present tense in scenes 3 and 4 (the critic states the film cannot reach 9 with that narration); "AI-native" and "traceable" unqualified in the voice; wall thickness shown in scene 4 under "same equations as the product" although the product models none; "MANUFACTURE." slogan; "complex… hours" over a simple part.
- The two critics conflict on the status tags (A: quieter; B: must stay prominent) and on wall thickness (A: wanted; B: overstates in scene 4). Lead's ruling: tags stay prominent; thickness stays in scene 3 only (labelled as a film visualisation) and scene 4 should unroll a thin shell. The scene-4 change is not yet made.

## Implemented versus planned, as shown
| Scene | Shown | Status on screen |
|---|---|---|
| 1 | Fictional drawing made for the film | Concept |
| 2 | Real application: workspace, pattern canvas, gates | Actual software, development build; "AI drawing interpretation: planned" |
| 3 | 3D form, governing equation with live values | Film visualisation, not a product view; "Drawing reading and 3D reconstruction: planned"; pattern mathematics implemented |
| 4 | Unroll to flat patterns, cut lengths, DXF | Same equations as the product, redrawn for the film; "Manufacturing-constraint evaluation: planned" |
| 5 | Brand, tagline, four verbs | Product concept, not validated for manufacturing |

## Technical validation of the revision-3 files (`final/`)
Both languages: 1920×1080, 30 fps, 30.000 s, 900 frames, 0 decode errors, 0 black passages, audio and video start together, −16.0 LUFS, peaks −1.4 dB (EN) and −4.2 dB (JA), subtitle cues in range without overlap. The only held picture is the end card. Speech-recognition round trips recovered every line.

## Remaining limitations
- **Not watched or heard by a person.** Review was by extracted frames (critics: every 0.5 s plus 30 fps strips at transitions) and automated checks. The lead inspected five full-size stills per revision, and a one-frame-per-second sheet for revisions 1 and 2 only.
- **Voice:** locally run Kokoro model (Apache-2.0); the Japanese voice was judged below native quality by the audio specialist and needs a native listener or a better voice.
- **Japanese film shows the English interface** (labelled); the application has no Japanese interface.
- Fonts are installed with `npm install` in `launch30/` before rendering.
- 4K was not rendered.
- The bar of 9 was not met. Further picture rounds are not recommended until the narration for scenes 3 and 4 is decided.
