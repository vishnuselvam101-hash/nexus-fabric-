# EffectCraft build notes (verified on this machine 2026-11-24)

Each scene is ONE ExtendScript file `scenes/Sxx.jsx` (After Effects scripting object model). The build step runs, from the ad60 folder:
`cat data/geometry.js scenes/lib.jsx scenes/Sxx.jsx > _build_Sxx.jsx && effectcraft-cli script _build_Sxx.jsx --save-as Sxx.ecproj` then
`effectcraft-cli render Sxx.ecproj --comp Sxx --out renders/Sxx-<lang>.mov --format prores --resolution full --quality best`.
So: `GEOM` (real curves, mm; see data/geometry.js) and helpers from scenes/lib.jsx are already defined; file paths are relative to the ad60 folder (e.g. "screens/dark/13-checks.png"); the script must NOT read or write files. Language: the global `LANG` is set to "en" or "ja" by a first line `var LANG = "en";` the build prepends; every on-screen string comes from `T(en, ja)` (defined in lib.jsx).

Verified API (use exactly these forms):
- comp: `var comp = app.project.items.addComp("S04", 3840, 2160, 1, DURATION_SECONDS, 60);` (name MUST be the scene id)
- solid: `comp.layers.addSolid([0,0,0], "BG", 3840, 2160, 1);`
- image: `var f = app.project.importFile(new ImportOptions(new File("screens/dark/08-job.png"))); var L = comp.layers.add(f);`
- 3D: `L.threeDLayer = true;` transform via `L.property("ADBE Transform Group").property("ADBE Rotate Y" | "ADBE Rotate X" | "ADBE Position" | "ADBE Scale" | "ADBE Opacity" | "ADBE Anchor Point")`; camera `comp.layers.addCamera("Cam", [1920,1080]);` (default camera shows a 3D layer at z=0 at its 2D size).
- keyframes: `prop.setValueAtTime(t, v)`; easing `var e = new KeyframeEase(0, 75); prop.setTemporalEaseAtKey(k, [e], [e]);` (position/scale: one ease per dimension → [e,e] or [e,e,e]).
- text: `var t = comp.layers.addText("..."); var td = t.property("ADBE Text Properties").property("ADBE Text Document"); var d = td.value; d.font = "Inter-SemiBold"; d.fontSize = 152; d.fillColor = [0.949,0.957,0.961]; d.justification = ParagraphJustification.LEFT_JUSTIFY; d.tracking = -20; td.setValue(d);`  Fonts: "Inter-Regular", "Inter-Medium", "Inter-SemiBold", "Inter-Bold"; Japanese: "NotoSansCJKjp-Regular", "NotoSansCJKjp-Medium", "NotoSansCJKjp-Bold". Text position = baseline-left (LEFT) / baseline-centre (CENTER).
- shapes: `var sh = comp.layers.addShape(); var g = sh.property("ADBE Root Vectors Group"); var pth = g.addProperty("ADBE Vector Shape - Group").property("ADBE Vector Shape"); var s = new Shape(); s.vertices = [[x,y],...]; s.closed = false; pth.setValue(s);` — keyframe the path with `pth.setValueAtTime(t, shapeAtT)` (vertex count must stay constant across keys). Stroke: `g.addProperty("ADBE Vector Graphic - Stroke")` → "ADBE Vector Stroke Color" / "ADBE Vector Stroke Width"; fill "ADBE Vector Graphic - Fill" → "ADBE Vector Fill Color"; trim "ADBE Vector Filter - Trim" → "ADBE Vector Trim End" (0–100); rect "ADBE Vector Shape - Rect" → "ADBE Vector Rect Size".
- effects: `L.property("ADBE Effect Parade").addProperty("ADBE Gaussian Blur 2")` (property(1) = blurriness); masks via `L.property("ADBE Mask Parade").addProperty("ADBE Mask Atom")` with "ADBE Mask Shape" (Shape) and "ADBE Mask Feather".
- layer timing: `L.inPoint`, `L.outPoint`, `L.startTime`.
Pitfalls: layer added last is on TOP; a 3D wireframe must be projected to 2D by the script (compute perspective in JS and key shape paths, e.g. one key every 2 frames for a moving camera); keep vertex counts constant; no `$.sleep`, no file I/O, no `alert`.
