// scenes/S11.jsx — S11 "Offline. In your shop."
// Film 57.0–62.4 s → local 0.0–5.4 s (324 f @ 60 fps). L1 composite, three planes.
// All three devices stay inside title-safe (x 280–3560, y 280–1880) at every frame,
// including the 3% drift; headline column x 280–1620 is left clear.

var DUR = 5.4;
var SETTLE = 0.6;    // each plane comes to rest over 0.6 s
var STAGGER = 0.3;   // entries 0.0 / 0.3 / 0.6 → all at rest by 1.2 (58.2)
var DRIFT_T0 = 1.2;  // 58.2 — parallax drift starts once every plane has settled
var LINK_IN = 1.8;   // 58.8
var LINK_DUR = 0.6;  // → 59.4

var comp = app.project.items.addComp("S11", 3840, 2160, 1, DUR, 60);
comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);

// --------------------------------------------------------------- motion ---

// settle (tIn→tS), hold to DRIFT_T0, then drift to tEnd.
function settleThenDrift(prop, tIn, vIn, tS, vRest, tEnd, vEnd) {
  prop.setValueAtTime(tIn, vIn);
  prop.setValueAtTime(tS, vRest);
  if (DRIFT_T0 > tS + 1e-6) prop.setValueAtTime(DRIFT_T0, vRest);
  prop.setValueAtTime(tEnd, vEnd);
  ease(prop);
  return prop;
}

// ---------------------------------------------------------------- planes ---

// One tilted plane: capture cropped (desktop) or whole (phones), never upscaled.
// Reflection is added only where it lands inside the frame.
function addPlane(comp, box, tIn) {
  var f = app.project.importFile(new ImportOptions(new File(box.file)));
  var c = box.crop || { x0: 0, y0: 0, x1: box.srcW, y1: box.srcH };
  var cw = c.x1 - c.x0, ch = c.y1 - c.y0;
  var cx = (c.x0 + c.x1) / 2, cy = (c.y0 + c.y1) / 2;
  var s0 = Math.min(box.w / cw, box.h / ch) * 100; // <= 100% of capture pixels
  var s1 = s0 * 1.03;                              // 3% parallax push
  var h0 = ch * s0 / 100, h1 = ch * s1 / 100;
  var tS = tIn + SETTLE;

  function face(mirror) {
    var L = comp.layers.add(f);
    L.threeDLayer = true;
    L.startTime = 0;
    L.inPoint = tIn;
    L.outPoint = DUR;
    if (box.crop) maskRect(L, c.x0, c.y0, c.x1, c.y1, 0);

    var tr = L.property("ADBE Transform Group");
    tr.property("ADBE Anchor Point").setValue([cx, cy, 0]);
    tr.property("ADBE Rotate Y").setValue(box.rotY);

    var sy = mirror ? -1 : 1;
    var yA = mirror ? box.y + h0 + 24 : box.y;
    var yB = mirror ? box.y + h1 + 24 : box.y;
    var yIn = mirror ? yA - 32 : yA + 32;

    settleThenDrift(tr.property("ADBE Position"),
      tIn, [box.x, yIn, 120], tS, [box.x, yA, 0], DUR, [box.x + box.dx, yB + box.dy, 0]);
    settleThenDrift(tr.property("ADBE Scale"),
      tIn, [s0, sy * s0, 100], tS, [s0, sy * s0, 100], DUR, [s1, sy * s1, 100]);

    var op = tr.property("ADBE Opacity");
    op.setValueAtTime(tIn, 0);
    op.setValueAtTime(tS, mirror ? 18 : 100);
    ease(op);

    if (mirror) {
      var mh = Math.min(420 / (s0 / 100), ch); // 420 screen px of fade, in source px
      maskRect(L, c.x0, c.y1 - mh, c.x1, c.y1, mh * 0.5);
      L.property("ADBE Effect Parade").addProperty("ADBE Gaussian Blur 2").property(1).setValue(48);
    }
    return L;
  }

  var refl = null;
  if (box.y + h0 / 2 + 24 < CAP_H) refl = face(true); // reflection first: plane sits on top
  var card = face(false);
  return { card: card, refl: refl };
}

// One cyan hairline linking the three planes (the shot's only accent).
function addLink(comp, pts, tIn, dur) {
  var sh = comp.layers.addShape();
  sh.inPoint = tIn;
  sh.outPoint = DUR;
  var tr = sh.property("ADBE Transform Group");
  tr.property("ADBE Anchor Point").setValue([0, 0]);
  tr.property("ADBE Position").setValue([0, 0]);

  var g = sh.property("ADBE Root Vectors Group");
  g.addProperty("ADBE Vector Shape - Group").property("ADBE Vector Shape").setValue(polyShape(pts, false));
  var st = g.addProperty("ADBE Vector Graphic - Stroke");
  st.property("ADBE Vector Stroke Color").setValue(COL.accent);
  st.property("ADBE Vector Stroke Width").setValue(2);
  move(g.addProperty("ADBE Vector Filter - Trim").property("ADBE Vector Trim End"), tIn, 0, tIn + dur, 100);
  return sh;
}

// ----------------------------------------------------------------- cast ---

// Desktop 1500×844 at (2500, 900): x 1716–3261, y 478–1327 after drift.
var desktop = {
  file: "screens/dark/02-convert.png",
  srcW: 3840, srcH: 2160,
  crop: { x0: 640, y0: 360, x1: 3200, y1: 1800 }, // content box C
  x: 2500, y: 900, w: 1500, h: 844, rotY: -8, dx: -12, dy: -8
};

// JA cut: 06-ja leads at (2780,1180); 05-mobile takes the second slot.
// Lead 620×1341: x 2461–3109, y 510–1877 after drift (bottom edge ≤ 1880).
var lead = {
  file: T("screens/dark/05-mobile.png", "screens/dark/06-ja.png"),
  srcW: 780, srcH: 1688,
  x: 2780, y: 1180, w: 620, h: 1341, rotY: -12, dx: 10, dy: 6
};

// Second 560×1211: x 2972–3556, y 635–1872 after drift.
var second = {
  file: T("screens/dark/06-ja.png", "screens/dark/05-mobile.png"),
  srcW: 780, srcH: 1688,
  x: 3260, y: 1240, w: 560, h: 1211, rotY: -16, dx: 8, dy: 8
};

// Link line behind the planes; drawn 58.8–59.4.
addLink(comp, [[desktop.x, desktop.y], [lead.x, lead.y], [second.x, second.y]], LINK_IN, LINK_DUR);

addPlane(comp, desktop, 0.0);            // 57.0
addPlane(comp, second, 2 * STAGGER);     // 57.6 — behind the leading phone
addPlane(comp, lead, STAGGER);           // 57.3 — on top

comp.layers.addCamera("Cam", [1920, 1080]);

// ----------------------------------------------------------------- text ---

addHeadline(comp, T("Offline.\nIn your shop.", "オフライン。\n自社の環境で。"),
  280, 940, 0.6, DUR, { justify: "left" });
addSub(comp, T("Desktop · shop server · phone browser", "PC・社内サーバー・スマホのブラウザ"),
  280, 1228, 0.9, DUR, { justify: "left" });
addCaption(comp, T("English / 日本語", "English / 日本語"),
  280, 1312, 1.2, DUR, { justify: "left" });
addStatus(comp, 0.6, DUR, "left");

comp.numLayers;
