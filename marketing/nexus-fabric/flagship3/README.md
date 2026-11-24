# NEXUS FABRIC film 3 (flagship3), revision 5

54.0 s, English and Japanese, identical picture timing. The picture is one canvas drawn by `window.seek(t)` (`film.html`, `film/*.js`). `tools/render_master.js` draws each frame and pipes it straight into ffmpeg (no frames stored on disk); it makes every final and master below. `render.js` is the earlier screenshot path and is kept for reference. The product contains no automated drawing reading and the film does not say it does. Revision 5 changed the picture only: `audio/` (mixes and `timing.js`), `subtitles/` and `script/narration.json` are untouched.

## Finals (`final/`)
- `NEXUS-FABRIC-film3-54s-en.mp4`: 10,444,271 bytes. `NEXUS-FABRIC-film3-54s-ja.mp4`: 10,478,711 bytes.
- Both: 1920x1080, 30 fps, 1,620 frames, 54.000 s; H.264 High, CRF 12, preset slow, yuv420p, bt709 tagged. AAC LC 48 kHz stereo, encoder target 320 kbit/s (measured average 278 kbit/s EN, 289 kbit/s JA for this mix).
- `NEXUS-FABRIC-film3-54s-<lang>-12frames.jpg`: 12-frame sheets (0, 5, 9, 12, 16, 20, 24, 30, 36, 41, 46, 52 s), taken from the MP4s.
- `NEXUS-FABRIC-film3-54s-en-4k60.mp4`: 23,703,467 bytes. `NEXUS-FABRIC-film3-54s-ja-4k60.mp4`: 24,160,470 bytes. Both 3840x2160, 60 fps, 54.000 s; H.264 High, CRF 12, preset slow, yuv420p, bt709 tagged; AAC 320 kbit/s, 48 kHz stereo, the same mixes. Rendered from the same page at device scale factor 2, so vector shapes and type are drawn at 2x (not upscaled 1080 frames). Command: `node tools/render_master.js en final/NEXUS-FABRIC-film3-54s-en-4k60.mp4 --scale 2 --fps 60 --crf 12 --preset slow --abitrate 320` (same for ja).
- `final/*-4k60.mp4` is listed in `.gitignore`: the two masters total 47,863,937 bytes (about 48 MB). They are delivered to the owner directly and are not committed.

## Scenes (slot grid 0, 4, 8, 12, 20, 26, 30, 42, 46, 50 s, locked)
| Slot, s | Scene | On screen (EN; JA is the same beat) | Source label (40 px, bottom left) |
|---|---|---|---|
| 0-4 | 1 Hook | "One wrong curve."; pipe slides in 0.0-0.5 s, branch lowers 0.5-1.0 s; 14 px gap at 1.8-2.2 s with one flash on the seam at 2.0 s; gap closed by 3.6 s | Visualisation made for this film · figures from the product (from 1.0 s) |
| 4-8 | 2 Question | "Before you cut — how do you know?"; the opening curve draws 4.0-5.0 s, a marker travels it | same as above |
| 8-12 | 3 Product | "Dimensions in. Flat pattern out."; flat pattern draws 8.5-10.0 s; real crop 10.0-12.0 s | Actual software · development build (10-12 s) |
| 12-20 | 4 Five parts | Five parts, one at a time, with whole-millimetre sizes; limits caption at 48 px, 12-20 s | same as above |
| 20-26 | 5 Checks | "It checks its own work."; counter "n of 28 checks run" from its first frame (20.0 s); rows are job 6's own checks; "Internal consistency checks — not an approval." | Interface simplified for this film · data from the product |
| 26-30 | 6 DXF | "Written. Read back. Compared."; the header flat pattern in both panels; scroll at 6 lines/s 26.0-26.6 s; read-back blue 26.6-27.6 s; green tick 27.6-28.1 s; NOT RELEASED at 28.5 s (pulse at 29.0 s); file name line at y 968 | Visualisation made for this film · figures from the product |
| 30-42 | 7 AMOEBA | 30-33 s definition and three linked records; 33-37 s "Change one number.", "Correction recorded: 150 → 158 mm", "11 raised · 8 discharged · 3 open", six-node graph (blue = what a change reaches); 37-40 s real graph core (take `am_post`); 40-42 s count to 43, then "Replay: match." | Interface simplified (30-37, 40-42 s); Actual software (37-40 s) |
| 42-46 | 8 Honesty | "What it can't confirm, it shows."; REVIEW REQUIRED; PASS 5, INCONCLUSIVE 7, NOT IMPLEMENTED 1 with "Human approval (not implemented)" (JA 人による承認（未実装）); "Development build. Not validated for manufacturing." from 42.8 s | Interface simplified for this film · data from the product |
| 46-50 | 9 Language, offline | "English · 日本語"; grid switches on the 47.0 s beat; "Runs fully offline." | same as above |
| 50-54 | 10 End card | Mark, NEXUS FABRIC, "Engineering, Reimagined."; addresses from 50.0 s and "Request a demonstration" pill from 50.4 s, the addresses under it (40 px, both languages); disclaimer at 40 px for the whole card; "Engineering platform powered by AMOEBA." fades in 52.5-53.0 s; JA voice credit (JA film only) | none |

Transitions: the outgoing scene fades out and rises 12 px over the four frames before the bar, and is gone at bar − 0.1 s. Nothing is drawn for the last 0.1 s before each bar (three paper-only frames at 30 fps). The incoming element is on the bar at 60 % and fully there four frames later (`F3.enter`). Each text zone is alone in its frame.

## Labelled version (the publication version)
Every version of this film carries its disclosures: the three source labels (the visuals are illustrations, the interface is simplified, the footage is from a development build), the limits line under the five parts, the checks subtitle ("not an approval"), the development-build line in the honesty scene and the end-card disclaimer (product concept; capabilities in development; not validated for manufacturing). The labelled version in `final/` is the one recommended for publication.

No caption-free version was made. A request for a version without these captions (a `labels=0` option) was not implemented: the captions state the product's status and limits (zero thickness; bend allowance, weld gap and kerf not modelled; not validated for manufacturing), and removing them would present an unfinished tool used for fabrication decisions as validated. If a different variant is wanted, the disclosures would have to be restated, not removed; that decision belongs to the owner and the makers.

## Real application footage
- Captured at 3840x2160: twice the device pixels of the 1920x1080 app window (`capture/footage/<lang>`, read through the tile atlases).
- Two moments, one captured frame each: take `result` frame 33 at 10.0-12.0 s (scene 3; drawn at 1.14x its source pixels in the 4K master, about 0.57x at 1080p) and take `am_post` frame 250 at 37.0-40.0 s (scene 7; drawn at 0.94x in the 4K master).
- Repeated frames: inside those two windows the source pixels do not change. Every frame of the 30 fps finals and the 60 fps masters shows the same capture, with only the computed crop and push-in moving. Outside the windows the film shows no footage. The "0 identical frames" count in the render logs does not measure this, because the moving crop changes the canvas on every frame.
- The footage shows job #5 (superseded) in its footer and REVIEW REQUIRED; the rest of the film uses job 6 (current).

## Data (`data/film-data.js`, built by `tools/build_film_data.mjs`)
- Job 6 (current): header_radius 150 to 158 mm; 11 obligations raised (8 discharged, 3 open); replay 43 records checked, 0 failing; checks 28 entries, all PASS; gates by their own results: PASS 5, INCONCLUSIVE 7, NOT_IMPLEMENTED 1; simple graph of six nodes, flag order by distance.
- Japanese: 18 of the 28 check entries have a Japanese label; the other 10 appear as identifiers (listed in PROGRESS.md).
- Numbers are whole millimetres with separators. No accuracy or speed figures are shown.

## Build and check
    node tools/build_film_data.mjs . data/film-data.js     # data
    node tools/build_strings.js                           # NEW_STRINGS.md from script/onscreen.js
    node tools/render_master.js <en|ja> <out.mp4>         # 1080p final (defaults: scale 1, fps 30, CRF 12, AAC 320k)
    node tools/render_master.js <en|ja> <out.mp4> --scale 2 --fps 60 --crf 12 --preset slow --abitrate 320   # 4K60 master
    node tools/frames.js <en|ja> <dir> <t,t,...> png      # page frames for reference
    python3 tools/sheet.py <dir> <lang> <times|t0-t1> <out.jpg> <cols> <tile_w>   # contact sheets
Review frames from the MP4s use exact frame selection (frame n is time n/30; `select=eq(n,…)`), not `fps=1`.

## Known weaknesses
- At 390 px wide a 40 px label is about 8 px. Headlines and key numbers read; the source labels and the check rows do not read comfortably on a phone.
- The real footage text is small: application text is about 20-30 px at 1080p.
- The real footage shows job #5 (see above).
- The three paper-only frames before each bar are the transition rule, not a fault.
- The flat-to-3D parts are approximations; the wrong-cut arc is schematic; the hero tee appears only in the hook and checks scenes.
- The Japanese lines 曲げ代 (bend allowance) and 人による承認（未実装）are proposals for native review.

## Open items for the owner
- Native review of the Japanese, including the lines new in revision 5 (PROGRESS.md).
- The EN voice credit has not been checked against the model terms (not readable from here).
- Scratch files in the session scratchpad (review frames, sheets and MP4 copies) are left for the owner to remove.

## Clean variant (labels=0) — made at the owner's request, 20 Nov 2026

`film.html?lang=…&labels=0` and `node tools/render_master.js <en|ja> <out.mp4> --labels 0 [--scale 2 --fps 60 …]` render
the same film without the captions the film itself adds as disclosures: the three source labels, the limits line under
the five parts, "Internal consistency checks — not an approval.", "Development build. Not validated for manufacturing.",
the note "Flag rule: provisional, not yet reviewed." and the end-card disclaimer. Everything else is identical. What
always stays: the "What it can't confirm, it shows" scene with the gate counts, REVIEW REQUIRED and "Human approval
(not implemented)", the NOT RELEASED mark and real file name in the DXF scene, the contact addresses, and the Japanese
voice credit (a licence condition).

The clean files (`final/*-clean*.mp4`) are not committed. The owner decides which variant to publish. The makers
recommend the labelled version: the software is a development build that has not been validated for manufacturing,
and the clean variant no longer says so in writing.
