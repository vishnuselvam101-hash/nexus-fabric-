# NEXUS FABRIC film 4 (flagship4)

54.0 s, English and Japanese, identical picture timing on the locked bar grid (0, 4, 8, 12, 20, 26, 30, 42, 46, 50, 54 s),
muxed with the owner-approved mixes `../flagship3/audio/mix-en.wav` and `mix-ja.wav` (used as they are; not copied).

## Concept
One hero object, and every claim is an action on it. The hero is the real job's steel header-and-branch joint
(job 5: header radius 150 mm, length 1,200 mm, branch at 600 mm, branch radius 100 mm, 60°), rendered in true 3D
with three.js 0.170.0 (copied to `lib/`, no network): brushed light steel, a soft studio key and a rim light, a soft
contact shadow, a calm bright set. The signature move is the development the software computes, made visible: the
product's real flat patterns roll into the real parts and unroll back. The plate meshes are built from the product's
own outlines and holes (`data/film4-data.js`, from the product's job files), so the opening in the header plate and
the saddle edge of the branch plate are the computed curves, point for point.

How the wrap is exact: a plate point at along-axis distance u and arc distance v from the anchor line, bent to
curvature kappa / R, sits at `O + u a + R n + rho sin(v/rho) t - rho (1 - cos(v/rho)) n` with rho = R / kappa
(`film/geo.js`). At kappa = 1 that is the cylinder, at kappa = 0 the flat plate; lengths on the plate are kept in
between. How each plate sits on its part (angle origin and direction) is fitted from the data: the header's opening
must land on the branch cylinder and the branch's saddle on the header cylinder; the fits land to about 1e-13 mm
(`tools/build_film4_data.mjs`, printed at build time). Cone sectors roll exactly (each in-between state is a cone
developed from the same sector). The eccentric reducer's end states are exact (an oblique cone; its development was
checked against the product's large-end polyline, 0.017 mm sampling residual), its in-between states are an
approximation; it carries the visualisation label like the rest of scene 4.

## Rebuild
    node tools/build_film4_data.mjs                       # data/film4-data.js from ../flagship3/data (read only)
    python3 tools/extract_footage.py en result:33 am_post:250   # real application stills (same for ja)
    node tools/build_strings.mjs                          # NEW_STRINGS.md
    NODE_PATH=/opt/node-tools/node_modules node tools/frames.js en <dir> 0-53:1       # review frames
    python3 tools/sheet.py <dir> en 0-19 <sheet.jpg> 5 384                            # contact sheet
    NODE_PATH=/opt/node-tools/node_modules node tools/render_master.js --variants en:1,ja:1,en:0,ja:0           # 1080p30, four files
    NODE_PATH=/opt/node-tools/node_modules node tools/render_master.js --variants en:1,en:0 --scale 2 --fps 60 --suffix -4k60   # 4K60 masters
The page is `film.html?lang=en|ja&labels=1|0&scale=1|2` with `window.seek(t)` and `window.READY`; same t gives the
same frame (no clock, no random numbers, no network). Headless Chromium needs WebGL through SwiftShader:
`--use-angle=swiftshader --enable-unsafe-swiftshader`; the renderer checks that the 3D layer is drawn and not black
before it starts. The 3D layer of a frame is drawn once and the overlay of each language/label variant is drawn on
it, so one pass writes several files; frames go straight into ffmpeg and are never stored.

## Scenes (slot grid locked; all numbers read from `data/film4-data.js`, NF4)
| Slot, s | What the viewer sees | Data source of every number, name and curve |
|---|---|---|
| 0-4 | The tee fills the frame. A branch comes down and lands at 0.9 s with its cut a quarter turn out of orientation: on beat 2 (1.0 s) its edge lights red and the gap to the header shows as a red ribbon; "One wrong curve."; on 2.5 s the camera snaps in to the widest gap. | Header: job 5 plate (`pat.header5`). Branch: job 5's own branch plate (`pat.branch5`), its computed saddle turned 90° about the branch axis (an orientation/marking error) and slid down to first contact (formula in `film/shots.js`). The cut is the product's; only its placement is wrong. Illustration; the gap size is not stated. |
| 4-8 | The wrong branch lifts away with its red edge; the true intersection curve draws in blue on the header surface around the opening (pipe interiors are dark, so the opening reads as an opening); a marker runs along it (6.25-7.75 s). "Before you cut — how do you know?" | Job 5's opening, 720 product points mapped to 3D on the header. |
| 8-12 | The correct branch lands; "NEXUS FABRIC", then "Dimensions in." with five inputs on the geometry, every label in one column on the right with a leader line; the branch lifts, the header unrolls flat under a camera that ends straight above it; match dissolve at 10.0 s into the real application's preview of the same plate, which pushes in to the opening and the 942.478 dimension; "Flat pattern out." | Callouts: job 5 request (150, 1,200, 600, 100, 60°) with the catalogue's parameter labels. Real frame: capture take `result` frame 33 (job #5), cropped to the plate with its rulers and dimensions (the gates panel is never in the crop). |
| 12-20 | Five families, cut on the music's accents: cylindrical shell unrolls, concentric reducer (right cone frustum) rolls up, eccentric reducer (oblique cone frustum) unrolls, branch stub rolls up, header shell with opening unrolls. Each name stays up for its whole slot; when flat, the computed outline locks in blue and one dimension of the pattern appears with a leader (developed width 3,142 mm; sector angle 114°; developed angle 107°; longest cut distance 231 mm; opening perimeter 702 mm); the opening's 720 points appear point by point. | Jobs 1-5 patterns and quantities; names and kinds from the catalogue (`family`, `familyLong`), quantity labels from the catalogue; angles converted from radians (deg = rad × 180 / π). Each family is scaled for the screen. |
| 20-26 | Both plates of the joint, flat, roll up and close onto each other on the music's fold/close (20.6-21.9 s): the opening edge (blue) and the saddle edge (orange) meet; one green curve. "Both edges. One curve." Counter and rows tick on the music's 28 ticks: "n of 28 checks run", PASS n. | Job 5 header and branch plates; checks: job 6 `checkList` (28 names, verdicts) and `checkCounts`; JA names from the catalogue where it has one (18), identifiers otherwise (10), as the application shows them. |
| 26-30 | The header plate unrolls under a top camera while the file's rows stream; it turns into a white sheet and all DXF entities draw together (ink), the read-back draws over them (blue), the tick locks at 27.6 s with "5 of 5 entities read back · largest deviation 0 mm"; NOT RELEASED stamps on at 29.0 s below the opening, the real file name beside it. | `dxf` parsed from the product's `dxf-6-header.dxf`; job 6's re-import report for the header (`reimport`: entities_read 5, entities_written 5, max_point_deviation 0); plate: job 6 header pattern. |
| 30-42 | AMOEBA: the job's 18 records leave the steel as dots and settle into a strip by level; the 29 links draw. "Change one number.": one large tag, Header contact radius 150 → 158 mm, tethered to the header; the header grows and the branch takes job 6's saddle; a pulse runs along the real links and the five dependents turn amber one per beat (36.0-38.0 s), listed by name at 48 px, the intersection curve amber on the steel; then the obligations in plain words: 11 flagged for review, 8 cleared by recomputed checks, 3 require an engineer's decision; the real dependency-graph screen zoomed to the derived records (38.4-40.4 s); replay: a ring scans the header while the count runs to 43; "Replay: match." | `deps` (18 nodes, 29 edges), `job6.corr`, `simple.flagOrder` and names, `obligations` (11) and `oblCounts` (8 / 3) with the wording taken from the obligations' notes ("recomputed; … checks pass", "requires an engineer's decision"), `job6.replay` (43, 0). Real frame: take `am_post` frame 250. |
| 42-46 | The tee in the background; "What it can't confirm, it shows."; the 13 gate verdicts settle into their groups with their counts: PASS 5, INCONCLUSIVE 7, NOT IMPLEMENTED 1 "Human approval (not implemented)"; REVIEW REQUIRED once (44.5 s). | `gates6`, `gateCounts6` (never merged), `ui` result labels; PASS shown with the film's res_pass wording (JA 内部チェック通過). |
| 46-50 | "English · 日本語": four of the application's own parameter labels, tethered to the tee, flip from English to Japanese on 46.9 s; "Runs fully offline." | `base.params` en/ja (catalogue). |
| 50-54 | End card: the tee small above NEXUS FABRIC, "Engineering, Reimagined."; from 50.3 s the "Request a demonstration" button and both addresses at 48 px (the button pulses on the music's cta at 51.6 s); "Engineering platform powered by AMOEBA." (52.6 s); disclaimer (labels=1); JA voice credit. | `script/onscreen4.js`. |

## What is real software, product data, illustration
- Real software (captured frames, pixels unaltered, cropped and moved only): 10.0-12.0 s (job #5 result screen) and 38.4-40.4 s (job 6 dependency graph). Labelled "Actual software · development build". The crops leave out the gates panel, whose G14 text names the algorithm's evidence level in words this film does not show.
- Product data: every outline, opening, saddle, DXF entity, number, check, gate, graph node and edge, and every catalogue label.
- Illustration (labelled "Visualisation made for this film · data from the product"): the 3D studio rendering, the rolling between the flat and formed states (exact for cylinders and cones, approximate in between for the eccentric reducer), the wrong-curve hook, the light, the pulse, the replay ring, the per-family screen scaling.

## Labels option
`labels=1` (default; the makers recommend this version): source labels, "Zero-thickness reference surfaces. Thickness, bend allowance, weld gap and kerf are not modelled." (scene 4), "Internal consistency checks — not an approval." (scene 5), "Development build. Not validated for manufacturing." (scene 8) and the end-card disclaimer. `labels=0` leaves those out; REVIEW REQUIRED, NOT RELEASED, the real file name, "Human approval (not implemented)", the gate counts, the contacts and the JA voice credit always stay. The clean files are not committed (`.gitignore`).

## Known weaknesses
- At 390 px wide (phone) the headlines read; 40 px labels, check names, DXF rows and tag identifiers are about 8 px and do not read.
- The two real application frames are zoomed (about 2x of the application's own pixels at the end of each push-in), so the application's text is about 20-26 px at 1080p: readable on a monitor, not on a phone. The first shows job #5 (superseded) as captured.
- The wrong-curve hook is an illustration: the product's saddle, placed a quarter turn out of orientation. The red gap ribbon is the film's marking, not a product output.
- Scene 4 scales each family to the screen; the five parts are not shown at a common scale.
- Steel seen straight from above (end of scene 3, start of scene 6) is almost white in this bright set.
- In scene 7 the 18 records are dots without labels; only the changed value, the five flagged records and the obligation counts are written out.
- The header grows by scaling its radius between the job 5 and job 6 states (the opening of job 6 replaces job 5's at the half-way point, a shift of a few millimetres); the branch blends between the two computed saddles.
- Japanese lines marked NEW in NEW_STRINGS.md (points_line, one_curve, reimport_line, obl_flagged/cleared/open; dxf_layer_note is not used on screen) need native review. The JA tagline 設計を、再定義。 is the same wording as flagship3's end card; it was never reviewed by a native-speaking copywriter either.
- Rendering uses SwiftShader (CPU WebGL): about 1.7 s per 1080p frame for four variants, about 4 s per 4K frame.

## Ideas cut
- A camera dolly along a line of the five families (scene 4): the moves landed between parts on the accents and left empty frames; replaced by cuts on the music's accents.
- Real-time shadow maps: soft shadows were slow and hard-edged on SwiftShader; a blurred contact shadow (one 512 px pass) is used instead.
- The real "lang" take for the language flip: a third application screen would bring back the "unrelated screens" problem; the flip uses the application's own catalogue labels on the hero instead.
- A label on every graph node at 40 px: 18 labels do not fit without collisions; only the flagged record is named, one at a time.
- Unrolling job 6's header in the replay: its developed radius (150) differs from its contact radius (158), so a flat state would misstate the plate; a scan ring is used instead.

## Finals (`final/`, fix round, 22 Nov 2026)
All four from one render pass (`tools/render_master.js --variants en:1,ja:1,en:0,ja:0`, 1,965 s, page errors 0).
Every file: H.264 High, 1920x1080, 30/1 fps, 1,620 frames, 54.000 s, yuv420p, bt709 tagged, CRF 12 preset slow;
AAC LC 48 kHz stereo, encoder target 320 kbit/s, muxed with the locked mix (`../flagship3/audio/mix-<lang>.wav`).

| File | Bytes | Video kbit/s | Audio kbit/s (measured) |
|---|---|---|---|
| NEXUS-FABRIC-film4-54s-en.mp4 (labels=1, recommended) | 20,321,732 | 2,724 | 278 |
| NEXUS-FABRIC-film4-54s-ja.mp4 (labels=1, recommended) | 20,192,558 | 2,694 | 289 |
| NEXUS-FABRIC-film4-54s-en-clean.mp4 (labels=0) | 20,180,222 | 2,703 | 278 |
| NEXUS-FABRIC-film4-54s-ja-clean.mp4 (labels=0) | 19,978,426 | 2,662 | 289 |

4K60 masters: rendered by the coordinator (not by this session) with
`tools/render_master.js --variants en:1,ja:1,en:0,ja:0 --scale 2 --fps 60 --suffix -4k60` (frames can be split with
`--from/--to`); about 4-6 s per frame on SwiftShader with four cores. `final/*-4k60.mp4` and `final/*-clean*.mp4` are git-ignored.

Reviewed from the MP4s: 1 fps contact sheets of both languages, stills at 1, 10, 14, 23, 29, 36, 41, 47 and 52 s
(both languages), the same at 390 px, a 0-2.4 s strip at 30 fps, and the JA clean file (disclosures gone; REVIEW
REQUIRED, NOT RELEASED, the approval line, contacts and voice credit present).
