# Film 4 — progress log (resume from here)

## Done
- Data: `tools/build_film4_data.mjs` -> `data/film4-data.js` (window.NF4). Plate wrap fits checked against the data
  (residuals ~1e-13 mm); cone and eccentric developments checked; DXF parsed (4 LINE, 1 POLYLINE 720 vertices).
- Footage stills: `tools/extract_footage.py` -> `footage/<lang>/result-33.webp`, `am_post-250.webp` (lossless, from flagship3 atlases).
- three.js 0.170.0 copied to `lib/` (module + RoomEnvironment + licence). Fonts in `fonts/` (Inter, Noto Sans JP subsets, JetBrains Mono).
- Page: `film.html`, `film/main.js` (seek, frameVariants), `film/stage.js` (studio, steel, contact shadow, plate parts),
  `film/geo.js` (column meshes from product outlines, exact cylinder wrap, cone sector morph, eccentric table),
  `film/ov.js` (type 132/48/40, spring), `film/shots.js` (all 10 scenes), `script/onscreen4.js` (strings).
- WebGL in headless Chromium works with `--use-angle=swiftshader --enable-unsafe-swiftshader` (canvas not black, checked).
  About 0.75 s per 1080p frame for the 3D; all four 1080p variants can come from one pass (3D drawn once, overlays per variant).
- Review round 0 and round 1 (EN contact sheets at 1 fps) done; fixes applied: entry alpha 60 % on the bar, larger hero,
  scene 4 as hard cuts per family, app window beside a poster headline, check panel wrap, labels placement.

## In progress
- 1080p finals done (final/NEXUS-FABRIC-film4-54s-{en,ja}[-clean].mp4, second render after round-3 fix), reviewed from the MP4s:
  1 fps sheets both languages, clean variants (labels gone, REVIEW REQUIRED / NOT RELEASED / approval line / contacts / JA credit kept),
  stills 1, 10, 16, 23, 35, 44, 52 s, 390 px strips, 0-2.4 s strip, boundary strips (page).
- 4K60 render of all four variants running (`--scale 2 --fps 60 --suffix -4k60`, about 4 s per frame, ~3.6 h;
  log in the scratchpad `flagship4/render4k.log`). If interrupted: re-run
  `NODE_PATH=/opt/node-tools/node_modules node tools/render_master.js --variants en:1,ja:1,en:0,ja:0 --scale 2 --fps 60 --suffix -4k60`.
- After 4K: look at frames from the 4K files, ffprobe all files, fill README finals section, final report.

## Done docs
- README.md (concept, rebuild, scene table, real/data/illustration, labels, weaknesses, cut ideas), NEW_STRINGS.md, .gitignore.

## Commands
    node tools/build_film4_data.mjs
    NODE_PATH=/opt/node-tools/node_modules node tools/frames.js en <dir> 0-53:1
    python3 tools/sheet.py <dir> en 0-19 <out.jpg> 5 384
