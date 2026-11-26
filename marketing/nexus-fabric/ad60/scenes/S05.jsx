// scenes/S05.jsx — "13 gates." L2 gate list. Local time 0 = film 23.4 s.
// GEOM + scenes/lib.jsx are prepended by the build; LANG is set by the build.

var DUR = 5.4; // 324 f @ 60 fps

var comp = app.project.items.addComp("S05", 3840, 2160, 1, DUR, 60);
comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);

// ------------------------------------------------------------------- card ---

var FILE = T("screens/dark/08-job.png", "screens/dark/11-job-ja.png");
var CROP_A = { x0: 640, y0: 360, x1: 3200, y1: 1800 };   // C, 1.50x
var CROP_B = { x0: 735, y0: 453, x1: 3105, y1: 1787 };   // 1.62x, centre 1920,1120

var card = addCard(comp, FILE, CROP_A, "L2", 0, DUR, {
  scaleFrom: 100,
  scaleTo: 103,
  cropB: CROP_B,
  tEnd: DUR
});

// -------------------------------------------------------------- gate rows ---

// The 13 captured gate rows are revealed by one soft dark scrim inside the
// card that lifts top-to-bottom, film 24.6-26.55 -> local 1.20-3.15.
// No drawn lines, no boxes, no recolouring, no invented rows. The scrim lives
// in capture px inside the card comp so it tracks the content push exactly.

var lay = LAYOUT.L2;
var A = normCrop(CROP_A, lay.w, lay.h);
var B = normCrop(CROP_B, lay.w, lay.h);
var f0 = Math.min(100, fitScale(A, lay));
var f1 = Math.min(100, fitScale(B, lay));

var GATE_X0 = 820, GATE_X1 = 3040;
var GATE_Y0 = 560, GATE_Y1 = 1720;
var SCRIM_BOTTOM = 2160;
var T_LIFT0 = 1.20, T_LIFT1 = 3.15;

var scrim = card.inner.layers.addShape();
scrim.inPoint = 0;
scrim.outPoint = card.inner.duration;

var st = scrim.property("ADBE Transform Group");
st.property("ADBE Position").setValue([lay.w / 2, lay.h / 2]);
move(st.property("ADBE Anchor Point"), 0, [A.cx, A.cy], DUR, [B.cx, B.cy]);
move(st.property("ADBE Scale"), 0, [f0, f0], DUR, [f1, f1]);
move(st.property("ADBE Opacity"), T_LIFT1, 55, T_LIFT1 + 0.30, 0);

var g = scrim.property("ADBE Root Vectors Group");
g.addProperty("ADBE Vector Shape - Group").property("ADBE Vector Shape")
  .setValue(rectShape(0, 0, CAP_W, CAP_H));
var fill = g.addProperty("ADBE Vector Graphic - Fill");
fill.property("ADBE Vector Fill Color").setValue(COL.stage);

var m = scrim.property("ADBE Mask Parade").addProperty("ADBE Mask Atom");
move(m.property("ADBE Mask Shape"),
     T_LIFT0, rectShape(GATE_X0, GATE_Y0, GATE_X1, SCRIM_BOTTOM),
     T_LIFT1, rectShape(GATE_X0, GATE_Y1, GATE_X1, SCRIM_BOTTOM));
m.property("ADBE Mask Feather").setValue([0, 80]);

// ------------------------------------------------------------------- text ---

var HEAD_Y = 1132;          // optical centre y 1080, single line
var SUB_Y = HEAD_Y + 96;    // 40 px gap + 56 px sub size

addHeadline(comp, T("13 gates.", "13の関門。"), 280, HEAD_Y, 0.6, DUR, { justify: "left" });
addSub(comp, T("Nothing passes silently.", "黙って通さない。"), 280, SUB_Y, 0.9, DUR, { justify: "left" });

addStatus(comp, 0.6, DUR, "left");

comp.numLayers;
