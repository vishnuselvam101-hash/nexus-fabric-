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

## State (fix round finished, 22 Nov 2026)
- Fix round done: must-fix list applied, three review rounds (page frames, then two rounds from the rendered MP4s).
- final/ holds the four 1080p30 files of the last render (sizes and ffprobe figures in README "Finals").
- 4K: the coordinator renders it (chunks with --from/--to). This session does not.
- Nothing committed (the coordinator commits).
- Open: native review of the NEW Japanese lines (NEW_STRINGS.md); phone legibility of 40 px captions.

## Done docs
- README.md (concept, rebuild, scene table, real/data/illustration, labels, weaknesses, cut ideas), NEW_STRINGS.md, .gitignore.

## Commands
    node tools/build_film4_data.mjs
    NODE_PATH=/opt/node-tools/node_modules node tools/frames.js en <dir> 0-53:1
    python3 tools/sheet.py <dir> en 0-19 <out.jpg> 5 384
