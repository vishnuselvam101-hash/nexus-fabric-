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

## State (round 3 finished, 22 Nov 2026)
- Round 3 (two reviewers: motion director 6/10, Japanese fabrication-shop owner 5/10) applied: hook with two disagreeing
  curves + dark gap + light + contact flash + held camera + honest footer; surface reference "outside" among the inputs
  and in the real-app crop (inputs panel); "Checks: n of 28 pass" vs "Gates · 13" + plain line; "R12 DXF (AC1009)" and
  "Written and re-read: 5 of 5 entities match"; one 30 px footer per scene; graph full size from 30 s; replay count to
  43 by 40.75 s with a large tick; 48 px language flip; network-off symbol; gates clear of the tube; larger families,
  720-points label by the opening; end card (facts line, larger button/addresses, platform line whole card, dev footer);
  JA 切り代, 枝管; native-review notes in NEW_STRINGS.md.
- final/ holds the four 1080p30 files of this round (README "Finals"). 4K: the coordinator's. Nothing committed.

## Done docs
- README.md (concept, rebuild, scene table, real/data/illustration, labels, weaknesses, cut ideas), NEW_STRINGS.md, .gitignore.

## Commands
    node tools/build_film4_data.mjs
    NODE_PATH=/opt/node-tools/node_modules node tools/frames.js en <dir> 0-53:1
    python3 tools/sheet.py <dir> en 0-19 <out.jpg> 5 384
