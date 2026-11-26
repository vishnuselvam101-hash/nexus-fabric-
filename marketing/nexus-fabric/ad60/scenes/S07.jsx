// scenes/S07.jsx — S07 "942.478 mm. Exactly." · 432 f · local 0 = film 34.2 s.
// GEOM-driven unroll (header + branch) into the flat plate, 0.6 s cross-dissolve
// to the real pattern capture, cyan opening redraw, true-circumference dimension.

var S7_DUR = 7.2;
var S7_FPS = 60;
var S7_UNROLL = 2.4;      // 34.2–36.6
var S7_DISSOLVE = 3.0;    // 36.6–37.2 (0.6 s)
var S7_OPEN_A = 3.0;      // 37.2
var S7_OPEN_B = 4.2;      // 38.4
var S7_DIM_A = 4.2;       // 38.4
var S7_DIM_B = 4.8;       // 39.0
var S7_BRANCH_OUT = 1.8;  // branch tube leaves before the plate is flat
var S7_K = 0.92;          // flat plate: GEOM mm -> screen px, fits the L3 card area
var S7_CAMZ = -2600;
var S7_FOCAL = 2600 * S7_K;

// ------------------------------------------------------------------ helpers ---

function s7ease(p) { // cubic-bezier(0.22, 0.61, 0.36, 1)
  var t = clamp(p, 0, 1), i;
  for (i = 0; i < 6; i++) {
    var mt = 1 - t;
    var x = 3 * mt * mt * t * 0.22 + 3 * mt * t * t * 0.36 + t * t * t;
    var dx = 3 * mt * mt * 0.22 + 6 * mt * t * 0.14 + 3 * t * t * 0.64;
    if (dx < 1e-6) break;
    t = clamp(t - (x - p) / dx, 0, 1);
  }
  var m2 = 1 - t;
  return 3 * m2 * m2 * t * 0.61 + 3 * m2 * t * t + t * t * t;
}

function s7pt(p) { return (p instanceof Array) ? [p[0], p[1]] : [p.x, p.y]; }

function s7raw(curve) {
  if (!curve) return [];
  var a = (curve instanceof Array) ? curve : (curve.points || curve.pts || curve.vertices || []);
  var out = [];
  for (var i = 0; i < a.length; i++) out.push(s7pt(a[i]));
  return out;
}

function s7find(src, name) {
  if (!src) return null;
  if (src[name]) return src[name];
  if (src instanceof Array) {
    for (var i = 0; i < src.length; i++) if (src[i] && src[i].name === name) return src[i];
  }
  return null;
}

// Uniform arc-length resample; vertex count stays constant across every key.
function s7resample(pts, n) {
  var m = pts.length, i;
  if (m === 0) return [];
  if (m === 1 || n < 2) return [[pts[0][0], pts[0][1]]];
  var acc = [0], total = 0;
  for (i = 1; i < m; i++) {
    var dx = pts[i][0] - pts[i - 1][0], dy = pts[i][1] - pts[i - 1][1];
    total += Math.sqrt(dx * dx + dy * dy);
    acc.push(total);
  }
  var out = [];
  if (total < 1e-9) {
    for (i = 0; i < n; i++) out.push([pts[0][0], pts[0][1]]);
    return out;
  }
  var j = 1;
  for (i = 0; i < n; i++) {
    var d = total * i / (n - 1);
    while (j < m - 1 && acc[j] < d) j++;
    var span = acc[j] - acc[j - 1];
    var f = span > 1e-9 ? (d - acc[j - 1]) / span : 0;
    out.push([pts[j - 1][0] + (pts[j][0] - pts[j - 1][0]) * f,
              pts[j - 1][1] + (pts[j][1] - pts[j - 1][1]) * f]);
  }
  return out;
}

function s7cross(a, b) {
  return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
}

function s7stroke(comp, pts, closed, color, width, tIn, tOut) {
  var sh = comp.layers.addShape();
  sh.inPoint = tIn;
  sh.outPoint = tOut;
  var tr = sh.property("ADBE Transform Group");
  tr.property("ADBE Anchor Point").setValue([0, 0]);
  tr.property("ADBE Position").setValue([0, 0]);
  var g = sh.property("ADBE Root Vectors Group");
  var pth = g.addProperty("ADBE Vector Shape - Group").property("ADBE Vector Shape");
  if (pts) pth.setValue(polyShape(pts, closed));
  var st = g.addProperty("ADBE Vector Graphic - Stroke");
  st.property("ADBE Vector Stroke Color").setValue(color);
  st.property("ADBE Vector Stroke Width").setValue(width);
  return { layer: sh, group: g, path: pth };
}

function s7trim(o, t0, t1) {
  var e = o.group.addProperty("ADBE Vector Filter - Trim").property("ADBE Vector Trim End");
  move(e, t0, 0, t1, 100);
  return e;
}

// ------------------------------------------------------------------- comp -----

var comp = app.project.items.addComp("S07", 3840, 2160, 1, S7_DUR, S7_FPS);
comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);

// ------------------------------------------- real pattern capture (L3 card) ---

var s7cropA = { x0: 640, y0: 360, x1: 3200, y1: 1800 };   // C, 1.50x
var s7cropB = { x0: 942, y0: 536, x1: 3018, y1: 1704 };   // 1.85x, centre 1980,1120
var s7card = addCard(comp, "screens/dark/09-pattern.png", s7cropA, "L3", S7_UNROLL, S7_DUR, {
  cropB: s7cropB, rotYFrom: -4, rotYTo: -2, tEnd: S7_DUR
});
move(s7card.card.property("ADBE Transform Group").property("ADBE Opacity"), S7_UNROLL, 0, S7_DISSOLVE, 100);
move(s7card.refl.property("ADBE Transform Group").property("ADBE Opacity"), S7_UNROLL, 0, S7_DISSOLVE, 18);

// ------------------------------------------------------- GEOM curve set-up ----

var S7_P = GEOM.params;
var S7_R = S7_P.header_radius;
var S7_BR = S7_P.branch_radius;
var S7_STA = S7_P.branch_station;
var S7_BETA = S7_P.beta_deg * Math.PI / 180;
var S7_FULL_H = 2 * Math.PI * S7_R;
var S7_FULL_B = 2 * Math.PI * S7_BR;

var s7curves = [];

function s7addCurve(src, name, isBranch, forceClosed) {
  var raw = s7raw(s7find(src, name));
  if (raw.length < 2) return null;
  var a = raw[0], b = raw[raw.length - 1];
  var wrap = Math.abs(b[1] - a[1]) < 1e-6 &&
             Math.abs(Math.abs(b[0] - a[0]) - (isBranch ? S7_FULL_B : S7_FULL_H)) < 1.0;
  var n = raw.length > 8 ? 180 : (wrap && isBranch ? 96 : 2);
  var c = {
    name: name,
    branch: isBranch,
    closed: !!(forceClosed || wrap),
    pts: s7resample(raw, n),
    path: null, layer: null
  };
  s7curves.push(c);
  return c;
}

var s7hc = GEOM.parts.header.curves;
var s7bc = GEOM.parts.branch.curves;
s7addCurve(s7hc, "seam_a", false, false);
s7addCurve(s7hc, "top", false, false);
s7addCurve(s7hc, "seam_b", false, false);
s7addCurve(s7hc, "bottom", false, false);
var s7opening = s7addCurve(s7hc, "branch_opening", false, true);
s7addCurve(s7bc, "saddle_cut", true, false);
s7addCurve(s7bc, "seam_b", true, false);
s7addCurve(s7bc, "square_end", true, false);
s7addCurve(s7bc, "seam_a", true, false);

// Flat-plate bounds in GEOM mm -> exact screen mapping of the last unroll key.
var s7fx0 = 1e9, s7fx1 = -1e9, s7fy0 = 1e9, s7fy1 = -1e9, s7i, s7j;
for (s7i = 0; s7i < s7curves.length; s7i++) {
  if (s7curves[s7i].branch) continue;
  var sp = s7curves[s7i].pts;
  for (s7j = 0; s7j < sp.length; s7j++) {
    if (sp[s7j][0] < s7fx0) s7fx0 = sp[s7j][0];
    if (sp[s7j][0] > s7fx1) s7fx1 = sp[s7j][0];
    if (sp[s7j][1] < s7fy0) s7fy0 = sp[s7j][1];
    if (sp[s7j][1] > s7fy1) s7fy1 = sp[s7j][1];
  }
}
var s7fcx = (s7fx0 + s7fx1) / 2, s7fcy = (s7fy0 + s7fy1) / 2;

function s7flat(p) { return [1920 + (p[0] - s7fcx) * S7_K, 1290 + (p[1] - s7fcy) * S7_K]; }

// --------------------------------------------------------- unroll mapping -----

function s7headerPt(p, u) { return unrollPoint(p[1], p[0], u, S7_R); }

function s7branchFrame(u) {
  var s = Math.PI * S7_R;                       // station opposite the seam
  var base = unrollPoint(s, S7_STA, u, S7_R);
  var phi = u >= 0.999999 ? 0 : s * (1 - u) / S7_R;
  var nrm = [0, Math.sin(phi), -Math.cos(phi)];
  var w = [0, Math.cos(phi), Math.sin(phi)];
  var cb = Math.cos(S7_BETA), sb = Math.sin(S7_BETA);
  var d = [cb, nrm[1] * sb, nrm[2] * sb];
  return { base: base, d: d, w: w, v: s7cross(d, w) };
}

function s7branchPt(p, f) {
  var a = p[0] / S7_BR, ca = Math.cos(a), sa = Math.sin(a), y = p[1];
  return [f.base[0] + f.d[0] * y + S7_BR * (ca * f.w[0] + sa * f.v[0]),
          f.base[1] + f.d[1] * y + S7_BR * (ca * f.w[1] + sa * f.v[1]),
          f.base[2] + f.d[2] * y + S7_BR * (ca * f.w[2] + sa * f.v[2])];
}

// ------------------------------------------------------- wireframe layers -----

for (s7i = 0; s7i < s7curves.length; s7i++) {
  var s7c = s7curves[s7i];
  var s7o = s7stroke(comp, null, s7c.closed, COL.white, 2, 0,
                     s7c.branch ? S7_BRANCH_OUT + 0.02 : S7_DISSOLVE);
  s7c.layer = s7o.layer;
  s7c.path = s7o.path;
  var s7op = s7c.layer.property("ADBE Transform Group").property("ADBE Opacity");
  if (s7c.branch) {
    s7op.setValueAtTime(0, 100);
    s7op.setValueAtTime(0.9, 100);
    s7op.setValueAtTime(S7_BRANCH_OUT, 0);
  } else {
    s7op.setValueAtTime(0, 100);
    s7op.setValueAtTime(S7_UNROLL, 100);
    s7op.setValueAtTime(S7_DISSOLVE, 0);
  }
  ease(s7op);
}

// One key every 2 frames; projected to 2D in script, plate centred in the card.
var s7step = 2 / S7_FPS;
var s7keys = Math.round(S7_UNROLL / s7step);
for (var s7k = 0; s7k <= s7keys; s7k++) {
  var s7t = s7k * s7step;
  var s7u = s7ease(s7t / S7_UNROLL);
  var s7cam = { yawDeg: 12 * (1 - s7u), z: S7_CAMZ, focal: S7_FOCAL };
  var s7bf = s7branchFrame(s7u);
  var s7proj = [];
  var s7bx0 = 1e9, s7bx1 = -1e9, s7by0 = 1e9, s7by1 = -1e9;
  for (s7i = 0; s7i < s7curves.length; s7i++) {
    var s7cc = s7curves[s7i], s7arr = [];
    for (s7j = 0; s7j < s7cc.pts.length; s7j++) {
      var s7m = s7cc.branch ? s7branchPt(s7cc.pts[s7j], s7bf) : s7headerPt(s7cc.pts[s7j], s7u);
      var s7q = project3D(s7m, s7cam);
      s7arr.push(s7q);
      if (!s7cc.branch) {
        if (s7q[0] < s7bx0) s7bx0 = s7q[0];
        if (s7q[0] > s7bx1) s7bx1 = s7q[0];
        if (s7q[1] < s7by0) s7by0 = s7q[1];
        if (s7q[1] > s7by1) s7by1 = s7q[1];
      }
    }
    s7proj.push(s7arr);
  }
  var s7dx = 1920 - (s7bx0 + s7bx1) / 2;
  var s7dy = (1080 + 210 * s7u) - (s7by0 + s7by1) / 2;
  for (s7i = 0; s7i < s7curves.length; s7i++) {
    var s7cd = s7curves[s7i];
    if (s7cd.branch && s7t > S7_BRANCH_OUT + 1e-6) continue;
    var s7flatPts = [];
    for (s7j = 0; s7j < s7proj[s7i].length; s7j++) {
      s7flatPts.push([s7proj[s7i][s7j][0] + s7dx, s7proj[s7i][s7j][1] + s7dy]);
    }
    s7cd.path.setValueAtTime(s7t, polyShape(s7flatPts, s7cd.closed));
  }
}

// ------------------------------------------- cyan opening redraw (one accent) --

if (s7opening) {
  var s7openPts = [];
  for (s7j = 0; s7j < s7opening.pts.length; s7j++) s7openPts.push(s7flat(s7opening.pts[s7j]));
  var s7cyan = s7stroke(comp, s7openPts, true, COL.accent, 2, S7_OPEN_A, S7_DUR);
  s7trim(s7cyan, S7_OPEN_A, S7_OPEN_B);
}

// ------------------------------------- 942.478 mm dimension (2 pi x 150) ------

var s7left = s7flat([s7fx0, s7fy1])[0];
var s7right = s7flat([s7fx1, s7fy1])[0];
var s7bottom = s7flat([s7fx0, s7fy1])[1];
var s7dimY = s7bottom + 84;

addHairline(comp, s7left, s7dimY, s7right, s7dimY, S7_DIM_A, S7_DIM_B - S7_DIM_A, COL.sub);

var s7ahL = s7stroke(comp, [[s7left + 20, s7dimY - 10], [s7left, s7dimY], [s7left + 20, s7dimY + 10]],
                     false, COL.sub, 2, S7_DIM_A, S7_DUR);
s7trim(s7ahL, S7_DIM_B - 0.18, S7_DIM_B);
var s7ahR = s7stroke(comp, [[s7right - 20, s7dimY - 10], [s7right, s7dimY], [s7right - 20, s7dimY + 10]],
                     false, COL.sub, 2, S7_DIM_A, S7_DUR);
s7trim(s7ahR, S7_DIM_B - 0.18, S7_DIM_B);

// Value appears whole (opacity only, never a count-up).
textLayer(comp, T("942.478 mm", "942.478 mm"), "sub", 1920, s7dimY - 18, S7_DIM_B - 0.30, S7_DUR,
          { justify: "center", rise: 0, color: COL.text });

// ------------------------------------------------------------------- text -----

addHeadline(comp, T("942.478 mm. Exactly.", "942.478 mm。そのまま。"), 1920, 480, 0.6, S7_DUR,
            { justify: "center" });
addSub(comp, T("The plate is the circumference: 2π × 150.", "板の幅は円周そのもの：2π × 150。"),
       1920, 596, 0.9, S7_DUR, { justify: "center" });
addCaption(comp, T("942.478 mm × 1200 mm · 720-point opening", "942.478 mm × 1200 mm · 開口 720点"),
           1920, 680, 1.2, S7_DUR, { justify: "center" });
addStatus(comp, 0.6, S7_DUR, "center");

comp.numLayers;
