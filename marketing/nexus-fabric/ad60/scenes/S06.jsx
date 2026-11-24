// scenes/S06.jsx — "It won't guess." / 「推測はしない。」
// Film 28.8–34.2 s → local 0.0–5.4 s (324 f @ 60 fps). L2, no accent, no shake.

var DUR = 5.4;
var T_PUSH_END = 33.0 - 28.8;   // 4.2 — push ends, then hold
var T_IN = 0.6;                 // headline in at scene + 0.6 s

var comp = app.project.items.addComp("S06", 3840, 2160, 1, DUR, 60);

comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);
comp.layers.addCamera("Cam", [1920, 1080]);

// Real capture only; JA cut substitutes the real JA screen.
var src = T("screens/dark/08-job.png", "screens/dark/11-job-ja.png");

var cropA = { x0: 823, y0: 263, x1: 3017, y1: 1497 };  // 1.75x
var cropB = { x0: 984, y0: 373, x1: 2856, y1: 1427 };  // 2.05x, centre 1920,900

// Push 0.0–4.2, then hold 1.2 s. Motion stops dead; no shake, no red flash.
addCard(comp, src, cropA, "L2", 0, DUR, {
  cropB: cropB,
  tEnd: T_PUSH_END,
  scaleFrom: 100, scaleTo: 100,
  rotYFrom: LAYOUT.L2.rotYFrom, rotYTo: LAYOUT.L2.rotYTo,
  xFrom: LAYOUT.L2.x, xTo: LAYOUT.L2.x
});

addHeadline(comp, T("It won't guess.", "推測はしない。"),
            280, 1134, T_IN, DUR, { justify: "left" });

addSub(comp, T("If it cannot compute, it refuses.", "計算できなければ、拒否します。"),
       280, 1230, T_IN + 0.3, DUR, { justify: "left" });

addStatus(comp, T_IN, DUR, "left");

comp.numLayers;
