// scenes/S04.jsx — "Computed twice." / 「2通りで計算。」
// Film 17.4–23.4 s → local 0.0–6.0 s (360 f @ 60 fps). L2, no accent.

var S04_DUR = 6.0;

var comp = app.project.items.addComp("S04", 3840, 2160, 1, S04_DUR, 60);
comp.layers.addSolid(COL.black, "BG", 3840, 2160, 1);

// Real checks panel: primary / independent reference / difference, unretouched.
var cropA = { x0: 640, y0: 80,  x1: 3200, y1: 1520 }; // 1.50x
var cropB = { x0: 700, y0: 700, x1: 2445, y1: 1682 }; // 2.20x, the two check cards

addCard(comp,
  T("screens/dark/13-checks.png", "screens/dark/14-checks-ja.png"),
  cropA, "L2", 0, S04_DUR,
  { cropB: cropB, scaleFrom: 100, scaleTo: 100, rotYFrom: -7, rotYTo: -4, tEnd: S04_DUR });

comp.layers.addCamera("Cam", [1920, 1080]);

// Headline block: left x 280, optical centre y 1080; sub-line 40 px below.
addHeadline(comp, T("Computed twice.", "2通りで計算。"), 280, 1004, 0.6, S04_DUR,
  { justify: "left" });

// Three column labels, 0.30 s each, film 18.6 / 19.2 / 19.8 → local 1.2 / 1.8 / 2.4.
var labelsEn = ["Primary value", "independent reference", "difference shown"];
var labelsJa = ["主計算", "独立した参照計算", "差分を表示"];
for (var i = 0; i < 3; i++) {
  addSub(comp, T(labelsEn[i], labelsJa[i]), 280, 1100 + i * 76, 1.2 + i * 0.6, S04_DUR,
    { justify: "left", fade: 0.30 });
}

addStatus(comp, 0.6, S04_DUR, "left");

comp.numLayers;
