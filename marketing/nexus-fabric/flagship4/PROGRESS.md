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

## State (round 4, 22 Nov 2026)
- Round 4 applied: hook starts on the problem (frame 0) with a "gap" callout; large real flat pattern at 10-12 s with
  its numbers; families re-framed (plain steel on sectors, crisper shadow); two-row check list with popping ticks;
  DXF match cut (plate slides onto the sheet) and 4-row listing top right; 30 s match cut (joint curve into its
  record); replay shows 43 directly; headline hand-overs without blank frames; scope line in the gates scene (also in
  clean); real app EN->JA wipe at 46-48 s; offline symbol beside the words; end card with the email action text.
- Finals rendered (1,896 s, 0 page errors) and reviewed from the MP4s (stills listed in README, both languages, full
  size and 400 px). Nothing committed; 4K is the coordinator's.

## Done docs
- README.md (concept, rebuild, scene table, real/data/illustration, labels, weaknesses, cut ideas), NEW_STRINGS.md, .gitignore.

## Commands
    node tools/build_film4_data.mjs
    NODE_PATH=/opt/node-tools/node_modules node tools/frames.js en <dir> 0-53:1
    python3 tools/sheet.py <dir> en 0-19 <out.jpg> 5 384
