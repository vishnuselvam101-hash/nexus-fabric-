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
| 0-4 | The tee fills the frame, slowly orbiting. "One wrong curve." The branch sits wrong: its cut edge lights red on beat 2 (1.0 s) and light glows along the gap. | Header: job 5 plate (`pat.header5`). Wrong branch: job 6's branch plate (`pat.branch6`, saddle computed for a 158 mm header) set on job 5's 150 mm header and slid down its axis to first contact (shift formula in `film/shots.js`). Illustration of a real mismatch; the gap size is not stated. |
| 4-8 | The wrong branch lifts away; the true intersection curve draws on the steel in blue, lifts off and holds; a marker runs along it (6.25-7.75 s). "Before you cut — how do you know?" | Job 5's opening, 720 product points mapped to 3D on the header. |
| 8-12 | The correct branch lands; "NEXUS FABRIC", then "Dimensions in." with five callouts on the geometry; the branch lifts, the header unrolls flat under a camera that ends straight above it; match dissolve at 10.0 s into the real application showing the same plate; "Flat pattern out." | Callouts: job 5 request (header radius 150, length 1,200, branch position 600, branch radius 100, angle 60°) with the catalogue's parameter labels (`base.params`). Real frame: capture take `result` frame 33 (job #5), cropped along panel borders (properties + preview; the gates panel is left out). |
| 12-20 | Five families, cut on the music's accents (12.0/13.5/15.0/16.5/18.0 s): cylindrical shell unrolls, concentric reducer rolls up, eccentric reducer unrolls, branch stub rolls up, header with opening unrolls; the computed outline locks in blue; the opening's 720 points appear point by point. | Jobs 1-5 patterns (`pat.shell`, `cone`, `ecc`, `branch4`, `header5`); point count = length of job 5's opening polyline. Each family is scaled for the screen (no size is stated). |
| 20-26 | Both plates of the joint, flat, roll up and close onto each other on the music's fold/close (20.6-21.9 s): the opening edge (blue) and the saddle edge (orange) meet; one green curve. "Both edges. One curve." Counter and rows tick on the music's 28 ticks: "n of 28 checks run", PASS n. | Job 5 header and branch plates; checks: job 6 `checkList` (28 names, verdicts) and `checkCounts`; JA names from the catalogue where it has one (18), identifiers otherwise (10), as the application shows them. |
| 26-30 | The header plate unrolls under a top camera, turns into a white sheet and its DXF entities draw (ink), the read-back draws over them (blue), the tick locks at 27.6 s; DXF rows stream on the right; NOT RELEASED stamps on at 29.0 s with the real file name. | `dxf` parsed from the product's file `dxf-6-header.dxf` (4 LINE, 1 POLYLINE of 720 vertices, rows as written); plate: job 6 header pattern (the file is job 6's). `tb.nr` from the catalogue. |
| 30-42 | AMOEBA: the job's parameter records appear as tags (identifier and value) tethered to the steel, the derived records join, and the 29 links draw into a graph across the frame. "Change one number.": header_radius 150 → 158 mm, the 3D header grows and the branch takes job 6's saddle; a pulse runs along the real edges and the five dependents turn amber one per beat (36.0-38.0 s), the intersection curve amber on the steel; "11 raised · 8 discharged · 3 open"; the real dependency-graph screen (38.4-40.4 s); replay: a ring scans the header while the count runs to 43; "Replay: match." | `deps` (18 nodes, 29 edges, levels computed from the edges), `job6.params`, `job6.corr` (150 → 158), `simple.flagOrder` and names, `obligations` (11) and `oblCounts` (8 / 3), `job6.replay` (43 checked, 0 failing). Real frame: capture take `am_post` frame 250 (job 6 graph), crop of the graph panel. |
| 42-46 | The tee in the background; "What it can't confirm, it shows."; the 13 gate verdicts settle into their groups with their counts: PASS 5, INCONCLUSIVE 7, NOT IMPLEMENTED 1 "Human approval (not implemented)"; REVIEW REQUIRED once (44.5 s). | `gates6`, `gateCounts6` (never merged), `ui` result labels; PASS shown with the film's res_pass wording (JA 内部チェック通過). |
| 46-50 | "English · 日本語": four of the application's own parameter labels, tethered to the tee, flip from English to Japanese on 46.9 s; "Runs fully offline." | `base.params` en/ja (catalogue). |
| 50-54 | End card: the tee small above NEXUS FABRIC, "Engineering, Reimagined.", "Request a demonstration", both addresses, "Engineering platform powered by AMOEBA." (52.6 s), disclaimer (labels=1), JA voice credit. | `script/onscreen4.js`. |

## What is real software, product data, illustration
- Real software (captured frames, pixels unaltered, cropped and moved only): 10.0-12.0 s (job #5 result screen) and 38.4-40.4 s (job 6 dependency graph). Labelled "Actual software · development build". The crops leave out the gates panel, whose G14 text names the algorithm's evidence level in words this film does not show.
- Product data: every outline, opening, saddle, DXF entity, number, check, gate, graph node and edge, and every catalogue label.
- Illustration (labelled "Visualisation made for this film · data from the product"): the 3D studio rendering, the rolling between the flat and formed states (exact for cylinders and cones, approximate in between for the eccentric reducer), the wrong-curve hook, the light, the pulse, the replay ring, the per-family screen scaling.

## Labels option
`labels=1` (default; the makers recommend this version): source labels, "Zero-thickness reference surfaces. Thickness, bend allowance, weld gap and kerf are not modelled." (scene 4), "Internal consistency checks — not an approval." (scene 5), "Development build. Not validated for manufacturing." (scene 8) and the end-card disclaimer. `labels=0` leaves those out; REVIEW REQUIRED, NOT RELEASED, the real file name, "Human approval (not implemented)", the gate counts, the contacts and the JA voice credit always stay. The clean files are not committed (`.gitignore`).

## Known weaknesses
- At 390 px wide (phone) the headlines read; 40 px labels, check names, DXF rows and tag identifiers are about 8 px and do not read.
- The two real application frames are small on screen (application text about 13-15 px at 1080p); they prove the software exists rather than inform. The first shows job #5 (superseded) as captured.
- The wrong-curve hook is an illustration (job 6's branch on job 5's header); the gap is about a centimetre and reads mainly through the red line and the light.
- Scene 4 scales each family to the screen; the five parts are not shown at a common scale.
- Steel seen straight from above (end of scene 3, start of scene 6) is almost white in this bright set.
- In scene 7 the tags' identifiers are shown as the application stores them (header_radius ...), in both languages; derived records are unlabeled dots except the one just flagged.
- The header grows by scaling its radius between the job 5 and job 6 states (the opening of job 6 replaces job 5's at the half-way point, a shift of a few millimetres); the branch blends between the two computed saddles.
- Japanese lines marked NEW in NEW_STRINGS.md (points_line, one_curve, dxf_layer_note) are proposals for native review; dxf_layer_note is not used on screen.
- Rendering uses SwiftShader (CPU WebGL): about 1.7 s per 1080p frame for four variants, about 4 s per 4K frame.

## Ideas cut
- A camera dolly along a line of the five families (scene 4): the moves landed between parts on the accents and left empty frames; replaced by cuts on the music's accents.
- Real-time shadow maps: soft shadows were slow and hard-edged on SwiftShader; a blurred contact shadow (one 512 px pass) is used instead.
- The real "lang" take for the language flip: a third application screen would bring back the "unrelated screens" problem; the flip uses the application's own catalogue labels on the hero instead.
- A label on every graph node at 40 px: 18 labels do not fit without collisions; only the flagged record is named, one at a time.
- Unrolling job 6's header in the replay: its developed radius (150) differs from its contact radius (158), so a flat state would misstate the plate; a scan ring is used instead.
