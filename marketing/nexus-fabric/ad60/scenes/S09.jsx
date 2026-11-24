// scenes/S09.jsx — S09 · 324f · film 46.8–52.2 → local 0.0–5.4
// L2. screens/dark/03-result: amber pill + "Download DXF · not released", captured verbatim.
// Push 0.0–3.6 (film 46.8–50.4), then hold 1.8 s. No accent in this shot (budget §3).
// Both crops start above the pill (capture y 100–150) and keep the buttons row
// (y 160–200) in frame for the whole shot: the pill is never clipped.

var DUR = 5.4;
var PUSH_END = 3.6;
var T_IN = 0.6;

var comp = app.project.items.addComp("S09", 3840, 2160, 1, DUR, 60);
comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);

// --------------------------------------------------------------- grade ------

// 12% desaturation of the card so the captured amber is the only warm pixel.
// The amber itself is never recoloured: the Yellows channel cancels the cut.
function satProp(fx) {
  var i, p, n;
  for (i = 1; i <= fx.numProperties; i++) {
    try {
      p = fx.property(i);
      n = String(p.name);
      if (n.indexOf("Saturation") >= 0 && n.indexOf("Colorize") < 0) return p;
    } catch (e) {}
  }
  return null;
}

function desaturateSurround(layer, amount) {
  var fx = null, range = null, master = null, warm = null, masterName = "";
  try { fx = layer.property("ADBE Effect Parade").addProperty("ADBE HUE SATURATION"); } catch (e0) { return; }
  if (!fx) return;
  try { range = fx.property(1); } catch (e1) { range = null; }
  try {
    if (range) range.setValue(1); // Master
    master = satProp(fx);
    if (master) {
      masterName = String(master.matchName);
      master.setValue(-amount);
    }
    if (range) {
      range.setValue(3); // Yellows — the amber pill band
      warm = satProp(fx);
      if (warm && String(warm.matchName) !== masterName) warm.setValue(amount);
      range.setValue(1);
    }
  } catch (e2) {}
}

// ---------------------------------------------------------------- card ------

// 60 px margin above the pill top (y 100); buttons row (y 160–200) well inside.
var cropA = { x0: 882,  y0: 40, x1: 2958, y1: 1208 }; // 2076x1168, 1.85x
var cropB = { x0: 1027, y0: 40, x1: 2773, y1: 1022 }; // 1746x982,  2.20x, centre 1900,531

var shot = addCard(comp, "screens/dark/03-result.png", cropA, "L2", 0, DUR, {
  cropB: cropB,
  tEnd: PUSH_END
});

desaturateSurround(shot.inner.layers[1], 12);

comp.layers.addCamera("Cam", [1920, 1080]);

// ---------------------------------------------------------------- text ------

var HL_Y = 1032; // two-line block, optical centre y 1080, column x 280–1620

addHeadline(comp, T("Converted.\nNot verified.", "変換のみ。\n未検証。"),
            280, HL_Y, T_IN, DUR, { justify: "left" });

addSub(comp, T("Not released.", "未リリース。"),
       280, HL_Y + 176 + 96, T_IN + 0.3, DUR, { justify: "left" });

addStatus(comp, T_IN, DUR, "left");

comp.numLayers;
