// scenes/S02.jsx — "Five shape families." Film 6.0–11.4 s → local 0.0–5.4 s.
// L1 stage: black + five wireframe icons, static camera, plane scale 100→103%.
// No accent in this scene (accent budget §3); all strokes white 2 px.

var DUR = 5.4;
var comp = app.project.items.addComp("S02", 3840, 2160, 1, DUR, 60);
comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);

// ------------------------------------------------------------- constants ---

var SLOT_X = [620, 1240, 1860, 2480, 3100];
var SLOT_Y = 1000, SLOT_W = 480, SLOT_MAX_H = 440;
var HEAD_IN = 0.60;      // film 6.6
var STAGGER = 0.30;      // film 6.6 / 6.9 / 7.2 / 7.5 / 7.8
var DRAW = 0.60;         // trim-path draw per icon
var LABEL_LAG = 0.30;    // label fades 0.3 s after its draw
var CAM = { yawDeg: -22, z: -2600 };

var PRM = GEOM.params;
var R = PRM.header_radius;          // 150
var RB = PRM.branch_radius;         // 100
var HL = PRM.header_length;         // 1200
var ST = PRM.branch_station;        // 600
var SEND = PRM.s_end;               // 400
var BETA = PRM.beta_deg * Math.PI / 180;

// --------------------------------------------------------------- helpers ---

// GEOM.parts.<part>.curves is an ARRAY of {name, kind, points}; look up by name.
function curveByName(part, name) {
  var cs = (part && part.curves) ? part.curves : [];
  for (var i = 0; i < cs.length; i++) {
    if (cs[i] && cs[i].name === name) return cs[i];
  }
  return null;
}

function curvePts(c) {
  if (!c) return [];
  var a = c.points ? c.points : c;
  if (!a || !a.length) return [];
  var out = [], i, p;
  for (i = 0; i < a.length; i++) {
    p = a[i];
    out.push(p instanceof Array ? [p[0], p[1]] : [p.x, p.y]);
  }
  return out;
}

// Constant vertex count for every key; closed loops drop a duplicated last point.
function resample(pts, n, closed) {
  var m = pts.length, out = [], i, idx;
  if (m < 2) return pts;
  if (closed) {
    if (Math.abs(pts[m - 1][0] - pts[0][0]) < 1e-6 &&
        Math.abs(pts[m - 1][1] - pts[0][1]) < 1e-6) m = m - 1;
    for (i = 0; i < n; i++) {
      idx = Math.floor(i * m / n) % m;
      out.push([pts[idx][0], pts[idx][1]]);
    }
  } else {
    for (i = 0; i < n; i++) {
      idx = Math.round(i * (m - 1) / (n - 1));
      out.push([pts[idx][0], pts[idx][1]]);
    }
  }
  return out;
}

function yAtS(pts, s) {
  var i, a, b;
  for (i = 0; i < pts.length - 1; i++) {
    a = pts[i]; b = pts[i + 1];
    if (s >= a[0] && s <= b[0] && b[0] > a[0]) {
      return a[1] + (b[1] - a[1]) * (s - a[0]) / (b[0] - a[0]);
    }
  }
  return pts[0][1];
}

// Ring about the X axis, axis at (y = cy, z = cz).
function ring(rad, x, cy, cz, n) {
  var p = [], i, a;
  for (i = 0; i < n; i++) {
    a = 2 * Math.PI * i / n;
    p.push([x, cy + rad * Math.sin(a), cz + rad * Math.cos(a)]);
  }
  return p;
}

// Header rings share the lib seam convention: cylPoint(r, theta, x).
function ringCyl(rad, x, n) {
  var p = [], i;
  for (i = 0; i < n; i++) p.push(cylPoint(rad, 2 * Math.PI * i / n, x));
  return p;
}

// Branch frame: axis leaves the header axis at station ST, angle BETA.
var BA = [Math.cos(BETA), Math.sin(BETA), 0];
var BU = [-Math.sin(BETA), Math.cos(BETA), 0];
var BO = [ST, 0, R];

function branchPt(t, phi) {
  var c = RB * Math.cos(phi), s = RB * Math.sin(phi);
  return [BO[0] + BA[0] * t + BU[0] * c,
          BO[1] + BA[1] * t + BU[1] * c,
          BO[2] + s];
}

function branchRing(t, n) {
  var p = [], i;
  for (i = 0; i < n; i++) p.push(branchPt(t, 2 * Math.PI * i / n));
  return p;
}

// Branch pattern (s, y) → 3D on a stub whose axis is +X.
function stubPt(s, y) {
  var a = s / RB;
  return [y, RB * Math.cos(a), RB * Math.sin(a)];
}

// One polyline = one shape layer: path + white 2 px stroke + trim draw.
// Anchor/position at the plane centre so all icons scale together 100→103%.
function wireLayer(pts2, closed, tIn, drawDur) {
  var sh = comp.layers.addShape();
  sh.inPoint = tIn;
  sh.outPoint = DUR;

  var tr = sh.property("ADBE Transform Group");
  tr.property("ADBE Anchor Point").setValue([1920, SLOT_Y]);
  tr.property("ADBE Position").setValue([1920, SLOT_Y]);
  move(tr.property("ADBE Scale"), 0, [100, 100], DUR, [103, 103]);
  tr.property("ADBE Opacity").setValue(100);

  var g = sh.property("ADBE Root Vectors Group");
  g.addProperty("ADBE Vector Shape - Group").property("ADBE Vector Shape")
    .setValue(polyShape(pts2, closed));

  var st = g.addProperty("ADBE Vector Graphic - Stroke");
  st.property("ADBE Vector Stroke Color").setValue(COL.white);
  st.property("ADBE Vector Stroke Width").setValue(2);

  var end = g.addProperty("ADBE Vector Filter - Trim").property("ADBE Vector Trim End");
  move(end, tIn, 0, tIn + drawDur, 100);
  return sh;
}

// Project the static 3D icon, fit it to the 480 px slot, draw every polyline.
function drawIcon(polys, slotX, tIn) {
  var proj = [], i, j, q, pp;
  var minX = 1e9, maxX = -1e9, minY = 1e9, maxY = -1e9;
  for (i = 0; i < polys.length; i++) {
    pp = [];
    for (j = 0; j < polys[i].p.length; j++) {
      q = project3D(polys[i].p[j], CAM);
      pp.push(q);
      if (q[0] < minX) minX = q[0];
      if (q[0] > maxX) maxX = q[0];
      if (q[1] < minY) minY = q[1];
      if (q[1] > maxY) maxY = q[1];
    }
    proj.push({ p: pp, c: polys[i].c });
  }
  var k = Math.min(SLOT_W / Math.max(1, maxX - minX),
                   SLOT_MAX_H / Math.max(1, maxY - minY));
  var cx = (minX + maxX) / 2, cy = (minY + maxY) / 2;
  for (i = 0; i < proj.length; i++) {
    pp = [];
    for (j = 0; j < proj[i].p.length; j++) {
      pp.push([slotX + (proj[i].p[j][0] - cx) * k,
               SLOT_Y - (proj[i].p[j][1] - cy) * k]);
    }
    wireLayer(pp, proj[i].c, tIn, DRAW);
  }
}

// ------------------------------------------------------------ the icons ----

var ICON_L = 520, R_SMALL = 85;
var HALF = Math.PI / 2, THREE = 3 * Math.PI / 2;

// 1 · Cylinder shell
var ic1 = [
  { p: ringCyl(R, 0, 72), c: true },
  { p: ringCyl(R, ICON_L, 72), c: true },
  { p: [cylPoint(R, HALF, 0), cylPoint(R, HALF, ICON_L)], c: false },
  { p: [cylPoint(R, THREE, 0), cylPoint(R, THREE, ICON_L)], c: false }
];

// 2 · Cone frustum
var ic2 = [
  { p: ring(R, 0, 0, 0, 72), c: true },
  { p: ring(R_SMALL, ICON_L, 0, 0, 72), c: true },
  { p: [[0, R, 0], [ICON_L, R_SMALL, 0]], c: false },
  { p: [[0, -R, 0], [ICON_L, -R_SMALL, 0]], c: false }
];

// 3 · Eccentric reducer (small end offset, one side flat)
var ECC = -(R - R_SMALL);
var ic3 = [
  { p: ring(R, 0, 0, 0, 72), c: true },
  { p: ring(R_SMALL, ICON_L, ECC, 0, 72), c: true },
  { p: [[0, R, 0], [ICON_L, ECC + R_SMALL, 0]], c: false },
  { p: [[0, -R, 0], [ICON_L, ECC - R_SMALL, 0]], c: false }
];

// 4 · Branch stub — real saddle cut + square end from GEOM
var saddleFull = curvePts(curveByName(GEOM.parts.branch, "saddle_cut"));
var saddle = resample(saddleFull, 180, true);
var saddle3 = [], si;
for (si = 0; si < saddle.length; si++) saddle3.push(stubPt(saddle[si][0], saddle[si][1]));
var yA = yAtS(saddleFull, 0);
var yB = yAtS(saddleFull, Math.PI * RB);
var ic4 = [
  { p: saddle3, c: true },
  { p: ring(RB, SEND, 0, 0, 96), c: true },
  { p: [[yA, RB, 0], [SEND, RB, 0]], c: false },
  { p: [[yB, -RB, 0], [SEND, -RB, 0]], c: false }
];

// 5 · Header shell with branch — real opening curve from GEOM
var openFull = curvePts(curveByName(GEOM.parts.header, "branch_opening"));
var openPts = resample(openFull, 180, true);
var open3 = [], oi;
for (oi = 0; oi < openPts.length; oi++) {
  open3.push(cylPoint(R, openPts[oi][1] / R, openPts[oi][0]));
}
var t0 = Math.sqrt(R * R - RB * RB) / Math.sin(BETA);
var ic5 = [
  { p: ringCyl(R, 0, 96), c: true },
  { p: ringCyl(R, HL, 96), c: true },
  { p: [cylPoint(R, HALF, 0), cylPoint(R, HALF, HL)], c: false },
  { p: [cylPoint(R, THREE, 0), cylPoint(R, THREE, HL)], c: false },
  { p: open3, c: true },
  { p: branchRing(SEND, 96), c: true },
  { p: [branchPt(t0, HALF), branchPt(SEND, HALF)], c: false },
  { p: [branchPt(t0, THREE), branchPt(SEND, THREE)], c: false }
];

// ------------------------------------------------------------- assembly ----

addHeadline(comp, T("Five shape families.", "5つの形状。"),
            1920, 640, HEAD_IN, DUR, { justify: "center" });

var ICONS = [ic1, ic2, ic3, ic4, ic5];
var EN = ["Cylinder shell", "Cone frustum", "Eccentric reducer", "Branch stub", "Header shell with branch"];
var JA = ["円筒", "円錐台", "偏心異径管", "枝管", "母管と枝管"];

for (var i = 0; i < ICONS.length; i++) {
  var tIn = HEAD_IN + STAGGER * i;
  drawIcon(ICONS[i], SLOT_X[i], tIn);
  addCaption(comp, T(EN[i], JA[i]), SLOT_X[i], 1330,
             tIn + DRAW + LABEL_LAG, DUR, { justify: "center" });
}

comp.numLayers;
