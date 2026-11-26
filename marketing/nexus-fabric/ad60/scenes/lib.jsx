// scenes/lib.jsx — shared helpers for S01–S12.
// Concatenated after data/geometry.js (GEOM) and after `var LANG = "en"|"ja";`.
// ExtendScript only: var/function, no file I/O, no alerts, verified AE API forms only.

// ---------------------------------------------------------------- language ---

function isJA() { return (typeof LANG !== "undefined") && LANG === "ja"; }
function T(en, ja) { return isJA() ? ja : en; }

// ------------------------------------------------------------------ colour ---

function hex(h) {
  var s = h.replace("#", "");
  return [parseInt(s.substr(0, 2), 16) / 255,
          parseInt(s.substr(2, 2), 16) / 255,
          parseInt(s.substr(4, 2), 16) / 255];
}

var COL = {
  stage:    hex("#000000"), // stage black
  surround: hex("#0B0D0E"), // card surround
  text:     hex("#F2F4F5"), // primary
  sub:      hex("#B7BEC1"), // secondary
  caption:  hex("#8A9296"), // technical / status caption
  status:   hex("#6E7679"), // end-card status only
  accent:   hex("#35D6E8"), // the only accent; budget per storyboard §3
  white:    [1, 1, 1],
  black:    [0, 0, 0]
};

// -------------------------------------------------------------- typography ---

// Sizes/leading @4K, tracking in AE units (1/1000 em: -2% = -20, +1% = +10).
var TYPE = {
  headline:  { size: 152, leading: 176, weight: "SemiBold", trackEn: -20, trackJa: 0,  color: COL.text,    rise: 28 },
  sub:       { size: 56,  leading: 76,  weight: "Regular",  trackEn: 0,   trackJa: 20, color: COL.sub,     rise: 16 },
  caption:   { size: 44,  leading: 60,  weight: "Regular",  trackEn: 10,  trackJa: 10, color: COL.caption, rise: 0 },
  status:    { size: 44,  leading: 60,  weight: "Regular",  trackEn: 10,  trackJa: 10, color: COL.caption, rise: 0 },
  wordmark:  { size: 96,  leading: 112, weight: "Bold",     trackEn: 80,  trackJa: 80, color: COL.text,    rise: 0, latin: true },
  cta:       { size: 64,  leading: 80,  weight: "Medium",   trackEn: 20,  trackJa: 20, color: COL.accent,  rise: 0 },
  contacts:  { size: 44,  leading: 60,  weight: "Regular",  trackEn: 10,  trackJa: 10, color: COL.caption, rise: 0, latin: true },
  endstatus: { size: 40,  leading: 56,  weight: "Regular",  trackEn: 10,  trackJa: 10, color: COL.status,  rise: 0 }
};

var FONTS = {
  en: { Regular: "Inter-Regular", Medium: "Inter-Medium", SemiBold: "Inter-SemiBold", Bold: "Inter-Bold" },
  // Noto Sans CJK JP ships Regular/Medium/Bold only; SemiBold resolves to Bold.
  ja: { Regular: "NotoSansCJKjp-Regular", Medium: "NotoSansCJKjp-Medium", SemiBold: "NotoSansCJKjp-Bold", Bold: "NotoSansCJKjp-Bold" }
};

function roleKey(role) {
  var k = String(role || "caption").toLowerCase().replace(/[^a-z]/g, "");
  return TYPE[k] ? k : "caption";
}

function fontFor(role, weight) {
  var spec = TYPE[roleKey(role)];
  var set = (isJA() && !spec.latin) ? FONTS.ja : FONTS.en; // wordmark + contacts stay Inter in both cuts
  return set[weight || spec.weight] || set.Regular;
}

function FONT(role) { var k = roleKey(role); return fontFor(k, TYPE[k].weight); }

// ------------------------------------------------------------------ easing ---

// cubic-bezier(0.22, 0.61, 0.36, 1): short out-handle (22%), long in-handle (64%).
var EASE_OUT_INFL = 22;
var EASE_IN_INFL = 64;

function ease(prop) {
  var n = 0;
  try { n = prop.numKeys; } catch (e0) { return prop; }
  if (!n) return prop;
  for (var k = 1; k <= n; k++) {
    var v = null, dims = 1;
    try { v = prop.keyValue(k); } catch (e1) { v = null; }
    if (v instanceof Array) dims = v.length;
    var ins = [], outs = [];
    for (var d = 0; d < dims; d++) {
      ins.push(new KeyframeEase(0, EASE_IN_INFL));
      outs.push(new KeyframeEase(0, EASE_OUT_INFL));
    }
    try { prop.setTemporalEaseAtKey(k, ins, outs); } catch (e2) {}
  }
  return prop;
}

function clamp(v, lo, hi) { return v < lo ? lo : (v > hi ? hi : v); }

// Set a static value, or a two-key move when t1 > t0; always eased.
function move(prop, t0, v0, t1, v1) {
  if (t1 > t0 + 1e-6) {
    prop.setValueAtTime(t0, v0);
    prop.setValueAtTime(t1, v1);
    ease(prop);
  } else {
    prop.setValue(v0);
  }
  return prop;
}

function justifyOf(j) {
  if (j === "center" || j === "CENTER" || j === "centre" || j === "L3" ||
      j === ParagraphJustification.CENTER_JUSTIFY) return ParagraphJustification.CENTER_JUSTIFY;
  return ParagraphJustification.LEFT_JUSTIFY;
}

// -------------------------------------------------------------------- text ---

// Shared text builder. (x, y) is baseline-left (LEFT) or baseline-centre (CENTER).
// Entrance: opacity 0->100 and y +rise->0 over 0.30 s at tIn; out over last 0.30 s.
function textLayer(comp, str, role, x, y, tIn, tOut, opts) {
  opts = opts || {};
  var spec = TYPE[roleKey(role)];
  var just = justifyOf(opts.justify);
  var rise = (opts.rise === undefined) ? spec.rise : opts.rise;
  var fade = (opts.fade === undefined) ? 0.30 : opts.fade;

  var L = comp.layers.addText(String(str).replace(/\r\n/g, "\n").replace(/\n/g, "\r"));
  L.inPoint = tIn;
  L.outPoint = tOut;

  var td = L.property("ADBE Text Properties").property("ADBE Text Document");
  var d = td.value;
  d.font = fontFor(role, opts.weight);
  d.fontSize = (opts.size === undefined) ? spec.size : opts.size;
  d.autoLeading = false;
  d.leading = (opts.leading === undefined) ? spec.leading : opts.leading;
  d.fillColor = opts.color || spec.color;
  d.justification = just;
  d.tracking = (opts.tracking === undefined) ? (isJA() ? spec.trackJa : spec.trackEn) : opts.tracking;
  td.setValue(d);

  var tr = L.property("ADBE Transform Group");
  var op = tr.property("ADBE Opacity");
  var pos = tr.property("ADBE Position");

  var tB = Math.min(tIn + fade, (tIn + tOut) / 2);
  var tC = Math.max(tOut - fade, tB);
  op.setValueAtTime(tIn, 0);
  op.setValueAtTime(tB, 100);
  if (tC > tB + 1e-6) op.setValueAtTime(tC, 100);
  op.setValueAtTime(tOut, 0);
  ease(op);

  if (rise) move(pos, tIn, [x, y + rise], tB, [x, y]);
  else pos.setValue([x, y]);

  return L;
}

// Headline: 152/176, SemiBold, <= 2 lines ('\n' splits; leading per spec).
function addHeadline(comp, text, x, y, tIn, tOut, opts) {
  return textLayer(comp, text, "headline", x, y, tIn, tOut, opts);
}

// Sub-line: 56/76 Regular, #B7BEC1.
function addSub(comp, text, x, y, tIn, tOut, opts) {
  return textLayer(comp, text, "sub", x, y, tIn, tOut, opts);
}

// Technical caption: 44 Regular, #8A9296, +1%.
function addCaption(comp, text, x, y, tIn, tOut, opts) {
  return textLayer(comp, text, "caption", x, y, tIn, tOut, opts);
}

// Required status caption on every software shot (S03–S11), baseline y 2058.
function addStatus(comp, tIn, tOut, justify) {
  var centre = justifyOf(justify) === ParagraphJustification.CENTER_JUSTIFY;
  return textLayer(comp,
    T("Actual software · development build", "実際の画面・開発版"),
    "status", centre ? 1920 : 280, 2058, tIn, tOut, { justify: centre ? "center" : "left" });
}

// ------------------------------------------------------------------- cards ---

var LAYOUT = {
  L2: { w: 1728, h: 972,  x: 2660, y: 1080, rotYFrom: -7, rotYTo: -4, rotX: 2 },
  L3: { w: 2048, h: 1152, x: 1920, y: 1290, rotYFrom: -4, rotYTo: -2, rotX: 2 }
};

var CAP_W = 3840, CAP_H = 2160, MAX_CROP_ZOOM = 2.20;

// Normalise a crop: keep centre/aspect, enforce capture zoom <= 2.20x and
// crop >= card size (cards are never upscaled above 1.0x of cropped pixels).
function normCrop(c, cardW, cardH) {
  var x0 = Math.min(c.x0, c.x1), x1 = Math.max(c.x0, c.x1);
  var y0 = Math.min(c.y0, c.y1), y1 = Math.max(c.y0, c.y1);
  var w = Math.max(1, x1 - x0), h = Math.max(1, y1 - y0);
  var cx = (x0 + x1) / 2, cy = (y0 + y1) / 2;
  var needW = Math.max(CAP_W / MAX_CROP_ZOOM, cardW);
  var needH = Math.max(cardH, needW * (h / w));
  var k = Math.max(1, needW / w, needH / h);
  w = Math.min(CAP_W, w * k);
  h = Math.min(CAP_H, h * k);
  x0 = clamp(cx - w / 2, 0, CAP_W - w);
  y0 = clamp(cy - h / 2, 0, CAP_H - h);
  return { x0: x0, y0: y0, x1: x0 + w, y1: y0 + h, w: w, h: h, cx: x0 + w / 2, cy: y0 + h / 2 };
}

function fitScale(c, lay) { return Math.min(lay.w / c.w, lay.h / c.h) * 100; }

function rectShape(x0, y0, x1, y1) { return polyShape([[x0, y0], [x1, y0], [x1, y1], [x0, y1]], true); }

function maskRect(layer, x0, y0, x1, y1, featherY) {
  var m = layer.property("ADBE Mask Parade").addProperty("ADBE Mask Atom");
  m.property("ADBE Mask Shape").setValue(rectShape(x0, y0, x1, y1));
  m.property("ADBE Mask Feather").setValue([0, featherY]);
  return m;
}

// addCard: fixed-size floating card on a 3D plane + mirrored reflection.
// The capture lives in an inner comp sized to the layout box, so the card's
// on-screen size/place never changes: only the content zooms from crop A to
// crop B inside the fixed frame (comp bounds clip; no mask on the card).
// crop = box A; push.cropB (optional) = box B the content settles on.
// push = {scaleFrom, scaleTo (PERCENT of the layout card: 100 = layout size,
//         104 = 4% drift), rotYFrom, rotYTo, xFrom, xTo, cropB, tEnd}.
function addCard(comp, file, crop, layout, tStart, tEnd, push) {
  var lay = LAYOUT[layout] || LAYOUT.L2;
  push = push || {};
  var A = normCrop(crop, lay.w, lay.h);
  var B = push.cropB ? normCrop(push.cropB, lay.w, lay.h) : A;

  var s0 = (push.scaleFrom === undefined ? 100 : push.scaleFrom); // % of layout box
  var s1 = (push.scaleTo === undefined ? 100 : push.scaleTo);
  var x0 = (push.xFrom === undefined ? lay.x : push.xFrom);
  var x1 = (push.xTo === undefined ? x0 : push.xTo);
  var ry0 = (push.rotYFrom === undefined ? lay.rotYFrom : push.rotYFrom);
  var ry1 = (push.rotYTo === undefined ? lay.rotYTo : push.rotYTo);
  var tP = (push.tEnd === undefined ? tEnd : push.tEnd);

  var f0 = Math.min(100, fitScale(A, lay)); // <= 100% : never upscale capture pixels
  var f1 = Math.min(100, fitScale(B, lay));

  // --- inner comp: the card face, exactly the layout box; content push inside ---
  var inner = app.project.items.addComp(comp.name + "_card", lay.w, lay.h, 1,
                                        Math.max(tEnd, 1 / 60), 60);
  var f = app.project.importFile(new ImportOptions(new File(file)));
  var src = inner.layers.add(f);
  src.startTime = 0;
  src.inPoint = 0;
  src.outPoint = inner.duration;
  var st = src.property("ADBE Transform Group");
  st.property("ADBE Position").setValue([lay.w / 2, lay.h / 2]);
  move(st.property("ADBE Anchor Point"), tStart, [A.cx, A.cy], tP, [B.cx, B.cy]);
  move(st.property("ADBE Scale"), tStart, [f0, f0], tP, [f1, f1]);
  st.property("ADBE Opacity").setValue(100);

  var h0 = lay.h * s0 / 100, h1 = lay.h * s1 / 100; // on-screen card height
  var ry = lay.y; // card centre; reflection mirrors about the card's bottom edge +24 px
  var anchor = [lay.w / 2, lay.h / 2, 0];

  var refl = comp.layers.add(inner); // added first so the card sits on top
  var card = comp.layers.add(inner);
  refl.threeDLayer = true;
  card.threeDLayer = true;
  refl.startTime = 0; card.startTime = 0;
  refl.inPoint = tStart; refl.outPoint = tEnd;
  card.inPoint = tStart; card.outPoint = tEnd;

  // --- card: fixed box, only the small layout drift moves it ---
  var ct = card.property("ADBE Transform Group");
  ct.property("ADBE Anchor Point").setValue(anchor);
  move(ct.property("ADBE Position"), tStart, [x0, ry, 0], tP, [x1, ry, 0]);
  move(ct.property("ADBE Scale"), tStart, [s0, s0, 100], tP, [s1, s1, 100]);
  move(ct.property("ADBE Rotate Y"), tStart, ry0, tP, ry1);
  ct.property("ADBE Rotate X").setValue(lay.rotX);
  ct.property("ADBE Opacity").setValue(100);

  // --- reflection: flipped Y, 18%, blur 48, 420 px fade in inner-comp coords ---
  var mh = Math.min(420, lay.h);
  maskRect(refl, 0, lay.h - mh, lay.w, lay.h, mh * 0.5); // top of inner = nearest the card
  var rt = refl.property("ADBE Transform Group");
  rt.property("ADBE Anchor Point").setValue(anchor);
  move(rt.property("ADBE Position"), tStart, [x0, ry + h0 + 24, 0], tP, [x1, ry + h1 + 24, 0]);
  move(rt.property("ADBE Scale"), tStart, [s0, -s0, 100], tP, [s1, -s1, 100]);
  move(rt.property("ADBE Rotate Y"), tStart, ry0, tP, ry1);
  rt.property("ADBE Rotate X").setValue(lay.rotX);
  rt.property("ADBE Opacity").setValue(18);
  refl.property("ADBE Effect Parade").addProperty("ADBE Gaussian Blur 2").property(1).setValue(48);

  return { card: card, refl: refl, inner: inner };
}

// ------------------------------------------------------------------ shapes ---

function polyShape(points, closed) {
  var s = new Shape();
  var v = [], ti = [], to = [];
  for (var i = 0; i < points.length; i++) {
    v.push([points[i][0], points[i][1]]);
    ti.push([0, 0]);
    to.push([0, 0]);
  }
  s.vertices = v;
  s.inTangents = ti;
  s.outTangents = to;
  s.closed = !!closed;
  return s;
}

// Cyan hairline (2 px) drawn by trim-path 0->100% over dur.
function addHairline(comp, x0, y0, x1, y1, tIn, dur, color) {
  var sh = comp.layers.addShape();
  sh.inPoint = tIn;
  var tr = sh.property("ADBE Transform Group");
  tr.property("ADBE Anchor Point").setValue([0, 0]);
  tr.property("ADBE Position").setValue([0, 0]);

  var g = sh.property("ADBE Root Vectors Group");
  var pth = g.addProperty("ADBE Vector Shape - Group").property("ADBE Vector Shape");
  pth.setValue(polyShape([[x0, y0], [x1, y1]], false));

  var st = g.addProperty("ADBE Vector Graphic - Stroke");
  st.property("ADBE Vector Stroke Color").setValue(color || COL.accent);
  st.property("ADBE Vector Stroke Width").setValue(2);

  var end = g.addProperty("ADBE Vector Filter - Trim").property("ADBE Vector Trim End");
  move(end, tIn, 0, tIn + dur, 100);
  return sh;
}

// --------------------------------------------------------------- geometry ----

// Perspective projection to 2D screen px, centred at (1920, 1080).
// cam.z is negative (camera sits at -z looking toward +z); focal defaults to
// the AE default-camera zoom at 4K (3555.56 px).
function project3D(p, cam) {
  cam = cam || {};
  var yaw = (cam.yawDeg === undefined ? 0 : cam.yawDeg) * Math.PI / 180;
  var cz = (cam.z === undefined ? -2600 : cam.z);
  var focal = (cam.focal === undefined ? 3555.56 : cam.focal);
  var c = Math.cos(yaw), s = Math.sin(yaw);
  var xr = p[0] * c + p[2] * s;
  var zr = -p[0] * s + p[2] * c;
  var d = zr - cz;
  if (d < 1) d = 1;
  var k = focal / d;
  return [1920 + xr * k, 1080 + p[1] * k];
}

// Cylinder about the X axis, seam at theta = 0 on the plane z = 0.
// Consistent with unrollPoint(s = r*theta, x, 0, r).
function cylPoint(rAxisMm, theta, xMm) {
  return [xMm, rAxisMm * Math.sin(theta), rAxisMm * (1 - Math.cos(theta))];
}

// Unroll: bend radius R/(1-t) about the fixed seam line (s = 0, z = 0).
// Arc length is preserved (rho * phi = s); t = 1 is the flat limit [x, s, 0].
function unrollPoint(sMm, xMm, t, R) {
  var u = clamp(t, 0, 1);
  if (u >= 0.999999) return [xMm, sMm, 0];
  var rho = R / (1 - u);
  var phi = sMm / rho;
  return [xMm, rho * Math.sin(phi), rho * (1 - Math.cos(phi))];
}
