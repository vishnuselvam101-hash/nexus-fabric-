// scenes/S01.jsx — S01 · film 0.0–6.0 s · local 0 = film 0.
// L1 stage: the real header + branch, projected to 2D from the exact placement
// helpers in data/geometry.js (header3D / branch3D). No hand-drawn curves.
// HEADER (white 3 px, trim 0.6–2.4): two view-tangent silhouette generators +
// both end circles. BRANCH (cyan, trim 2.4–3.6, the only accent): the cut edge,
// the far-end circle and two view-tangent generators running from the cut edge
// to that circle. Camera yaw 14°→0°, z −2600→−2480, baked every 2 frames.
// Framing: whole part 62% of frame width, optical centre y 980, headline below.

var DUR = 6.0;
var FPS = 60;
var FRAMES = 360;

var comp = app.project.items.addComp("S01", 3840, 2160, 1, DUR, FPS);
comp.layers.addSolid(COL.black, "BG", 3840, 2160, 1);

// ------------------------------------------------------------------ source ---

var PRM = GEOM.params;
var R = PRM.header_radius;      // 150
var HL = PRM.header_length;     // 1200
var RB = PRM.branch_radius;     // 100
var XST = PRM.branch_station;   // 600
var SEND = PRM.s_end;           // 400
var BETA = PRM.beta_deg * Math.PI / 180;
var CB = Math.cos(BETA), SB = Math.sin(BETA);
var HC = 2 * Math.PI * R;       // 942.4778 mm of header arc
var BC = 2 * Math.PI * RB;      // 628.3185 mm of branch arc

function curvePts(part, name) {
  var cs = GEOM.parts[part].curves;
  for (var i = 0; i < cs.length; i++) if (cs[i].name === name) return cs[i].points;
  return [];
}

// Constant vertex count across every key; keeps the first and the closing point.
function resample(pts, n) {
  var m = pts.length, out = [], i;
  if (!m) { for (i = 0; i < n; i++) out.push([0, 0]); return out; }
  if (m < 2) { for (i = 0; i < n; i++) out.push(pts[0]); return out; }
  for (i = 0; i < n; i++) out.push(pts[Math.round(i * (m - 1) / (n - 1))]);
  return out;
}

var SADDLE = curvePts("branch", "saddle_cut");        // 721 pts, s 0..628.3185
var OPENING = curvePts("header", "branch_opening");   // 720 pts, same curve in 3D

// Real cut-edge station along the branch axis at arc length s (linear in the data).
function saddleY(s) {
  var n = SADDLE.length;
  if (!n) return SEND;
  var u = clamp(s, 0, SADDLE[n - 1][0]);
  var lo = 0, hi = n - 1, mid;
  while (hi - lo > 1) {
    mid = Math.floor((lo + hi) / 2);
    if (SADDLE[mid][0] <= u) lo = mid; else hi = mid;
  }
  var a = SADDLE[lo], b = SADDLE[hi];
  var f = (b[0] > a[0]) ? (u - a[0]) / (b[0] - a[0]) : 0;
  return a[1] + f * (b[1] - a[1]);
}

// ------------------------------------------------------- model orientation ---

// Model frame: header axis +x, branch leaves (600,0,0) at 60° in the x-y plane
// toward +y. Present with a fixed 22° tilt about the header axis so the junction
// is seen from above, +y up on screen. SCALE and OFF are solved for the framing.
var PITCH = -22 * Math.PI / 180;
var CP = Math.cos(PITCH), SP = Math.sin(PITCH);
var CENTRE = [XST, 120, 0];
var SCALE = 1;
var OFF = [0, 0];

function rot(v) { return [v[0], -(v[1] * CP - v[2] * SP), v[1] * SP + v[2] * CP]; }

function place(p) {
  var q = rot([p[0] - CENTRE[0], p[1] - CENTRE[1], p[2] - CENTRE[2]]);
  return [q[0] * SCALE, q[1] * SCALE, q[2] * SCALE];
}

function sub(a, b) { return [a[0] - b[0], a[1] - b[1], a[2] - b[2]]; }
function dot(a, b) { return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]; }
function wrap(v, m) { v = v % m; return v < 0 ? v + m : v; }

// Presented radial bases (unit, perpendicular to their cylinder axis).
var HE1 = rot([0, 1, 0]), HE2 = rot([0, 0, 1]);
var BE1 = rot([-SB, CB, 0]), BE2 = rot([0, 0, 1]);

// ------------------------------------------------------------------ camera ---

function bezAt(p1, p2, t) {
  var m = 1 - t;
  return 3 * m * m * t * p1 + 3 * m * t * t * p2 + t * t * t;
}

// cubic-bezier(0.22, 0.61, 0.36, 1), solved in JS so the baked keys match lib easing.
function easeU(u) {
  u = clamp(u, 0, 1);
  if (u <= 0) return 0;
  if (u >= 1) return 1;
  var lo = 0, hi = 1, t = 0.5;
  for (var i = 0; i < 20; i++) {
    t = (lo + hi) / 2;
    if (bezAt(0.22, 0.36, t) < u) lo = t; else hi = t;
  }
  return bezAt(0.61, 1, t);
}

function camAt(t) {
  var u = easeU(t / DUR);
  return { yawDeg: 14 - 14 * u, z: -2600 + 120 * u };
}

// Eye of project3D's camera in presented space (yaw rotates x/z, camera y = 0).
function eyeAt(cam) {
  var psi = cam.yawDeg * Math.PI / 180;
  return [-cam.z * Math.sin(psi), 0, cam.z * Math.cos(psi)];
}

function screen(p, cam) {
  var q = project3D(place(p), cam);
  return [q[0] + OFF[0], q[1] + OFF[1]];
}

// ------------------------------------------------------------- silhouettes ---

// The two generators whose surface normal is perpendicular to the view ray:
// n(th) = cos th·e1 + sin th·e2, n·(A − E) + r = 0. n ⟂ axis, so the condition
// is exact at every station along the cylinder (true perspective tangency).
function silAngles(A, E, e1, e2, r) {
  var w = sub(A, E);
  var a = dot(w, e1), b = dot(w, e2);
  var m = Math.sqrt(a * a + b * b);
  if (m < 1e-6) m = 1e-6;
  var d = Math.atan2(b, a);
  var q = Math.acos(clamp(-r / m, -1, 1));
  return [d + q, d - q];
}

// ----------------------------------------------------------------- framing ---

var TARGET_W = 0.62 * 3840;
var CENTRE_Y = 980;

var FIT = [];
(function () {
  var i, j;
  var xs = [0, HL / 4, HL / 2, 3 * HL / 4, HL];
  var ys = [240, 320, SEND];
  for (i = 0; i < xs.length; i++)
    for (j = 0; j < 36; j++) FIT.push(header3D(xs[i], HC * j / 36));
  for (i = 0; i < ys.length; i++)
    for (j = 0; j < 36; j++) FIT.push(branch3D(BC * j / 36, ys[i]));
  var sd = resample(SADDLE, 60);
  for (i = 0; i < sd.length; i++) FIT.push(branch3D(sd[i][0], sd[i][1]));
})();

function fitBox() {
  var x0 = 1e9, x1 = -1e9, y0 = 1e9, y1 = -1e9;
  for (var k = 0; k <= 8; k++) {
    var cam = camAt(DUR * k / 8);
    for (var i = 0; i < FIT.length; i++) {
      var q = screen(FIT[i], cam);
      if (q[0] < x0) x0 = q[0];
      if (q[0] > x1) x1 = q[0];
      if (q[1] < y0) y0 = q[1];
      if (q[1] > y1) y1 = q[1];
    }
  }
  return { x0: x0, x1: x1, y0: y0, y1: y1 };
}

for (var it = 0; it < 5; it++) {
  var fb = fitBox();
  SCALE *= TARGET_W / (fb.x1 - fb.x0);
}
var box = fitBox();
OFF = [1920 - (box.x0 + box.x1) / 2, CENTRE_Y - (box.y0 + box.y1) / 2];

var A_H = place([XST, 0, 0]);
var A_B = place([XST + 250 * CB, 250 * SB, 0]);

// ---------------------------------------------------------------- geometry ---

var HN = 72, BN = 56, JN = 180;

function headerRing(x) {
  var out = [];
  for (var i = 0; i < HN; i++) out.push(header3D(x, HC * i / HN));
  return out;
}

function branchRing(y) {
  var out = [];
  for (var i = 0; i < BN; i++) out.push(branch3D(BC * i / BN, y));
  return out;
}

function headerGen(cam, idx) {
  var ang = silAngles(A_H, eyeAt(cam), HE1, HE2, R * SCALE);
  var s = wrap(R * (ang[idx] - Math.PI), HC);
  return [header3D(0, s), header3D(HL, s)];
}

function branchGen(cam, idx) {
  var ang = silAngles(A_B, eyeAt(cam), BE1, BE2, RB * SCALE);
  var s = wrap(RB * (ang[idx] + Math.PI / 2), BC);
  return [branch3D(s, saddleY(s)), branch3D(s, SEND)];
}

// The junction is drawn ONCE: header "branch_opening" (header3D) and branch
// "saddle_cut" (branch3D) are the same curve in 3D. It rides with the branch,
// cyan, per the storyboard; saddle_cut anchors the branch generators above.
var junction = [];
(function () {
  var op = resample(OPENING, JN);
  for (var i = 0; i < op.length; i++) junction.push(header3D(op[i][0], op[i][1]));
})();

var headerDefs = [
  { pts: headerRing(0), closed: true },
  { pts: headerRing(HL), closed: true },
  { gen: function (cam) { return headerGen(cam, 0); }, closed: false },
  { gen: function (cam) { return headerGen(cam, 1); }, closed: false }
];

var branchDefs = [
  { pts: junction, closed: true },
  { pts: branchRing(SEND), closed: true },
  { gen: function (cam) { return branchGen(cam, 0); }, closed: false },
  { gen: function (cam) { return branchGen(cam, 1); }, closed: false }
];

// ------------------------------------------------------------- wire layers ---

function wire(name, defs, color, t0, t1) {
  var sh = comp.layers.addShape();
  sh.name = name;
  sh.inPoint = 0;
  sh.outPoint = DUR;

  var tr = sh.property("ADBE Transform Group");
  tr.property("ADBE Anchor Point").setValue([0, 0]);
  tr.property("ADBE Position").setValue([0, 0]);

  var g = sh.property("ADBE Root Vectors Group");
  var paths = [], i;
  for (i = 0; i < defs.length; i++) {
    paths.push(g.addProperty("ADBE Vector Shape - Group").property("ADBE Vector Shape"));
  }

  for (var f = 0; f <= FRAMES; f += 2) {
    var t = f / FPS;
    var cam = camAt(t);
    for (var k = 0; k < defs.length; k++) {
      var mp = defs[k].gen ? defs[k].gen(cam) : defs[k].pts;
      var scr = [];
      for (var j = 0; j < mp.length; j++) scr.push(screen(mp[j], cam));
      paths[k].setValueAtTime(t, polyShape(scr, defs[k].closed));
    }
  }

  var st = g.addProperty("ADBE Vector Graphic - Stroke");
  st.property("ADBE Vector Stroke Color").setValue(color);
  st.property("ADBE Vector Stroke Width").setValue(3);
  try { st.property("ADBE Vector Stroke Line Cap").setValue(2); } catch (e1) {}
  try { st.property("ADBE Vector Stroke Line Join").setValue(2); } catch (e2) {}

  var end = g.addProperty("ADBE Vector Filter - Trim").property("ADBE Vector Trim End");
  move(end, t0, 0, t1, 100);
  return sh;
}

wire("HEADER", headerDefs, COL.white, 0.6, 2.4);
wire("BRANCH", branchDefs, COL.accent, 2.4, 3.6);

// -------------------------------------------------------------------- text ---

addHeadline(comp, T("Round to flat.", "曲面を、平面に。"),
            1920, 1720, 1.2, DUR, { justify: "center" });
addCaption(comp, T("header r150 · branch r100 · 60°", "母管 r150 · 枝管 r100 · 60°"),
           1920, 1810, 1.5, DUR, { justify: "center" });

comp.numLayers;
