// scenes/S03.jsx — "Enter the dimensions." / 「寸法を入力。」
// Film 11.4–17.4 s → local 0.0–6.0 s (360 f @ 60 fps). L2 software shot.

var DUR = 6.0;

var comp = app.project.items.addComp("S03", 3840, 2160, 1, DUR, 60);
comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);

// Crops in capture px: A = content box C (1.50x), B = the 9 dimension fields (1.95x).
var CROP_A = { x0: 640, y0: 360, x1: 3200, y1: 1800 };
var CROP_B = { x0: 935, y0: 626, x1: 2905, y1: 1734 };

var shot = addCard(comp, "screens/dark/07-math.png", CROP_A, "L2", 0, DUR, {
  scaleFrom: 100, scaleTo: 104,
  xFrom: 2660, xTo: 2630,
  cropB: CROP_B, tEnd: DUR
});

// --- cyan underline on one active field: film 12.6–13.2 → local 1.2–1.8, then holds ---
// Drawn inside the card comp in capture px, tracking the content push exactly.
var lay = LAYOUT.L2;
var A = normCrop(CROP_A, lay.w, lay.h);
var B = normCrop(CROP_B, lay.w, lay.h);
var f0 = Math.min(100, fitScale(A, lay));
var f1 = Math.min(100, fitScale(B, lay));

var UX0 = 1150, UX1 = 1690, UY = 1215; // active dimension field, capture px
var ul = addHairline(shot.inner, UX0, UY, UX1, UY, 1.2, 0.6, COL.accent);
ul.inPoint = 1.2;
ul.outPoint = DUR;
ul.property("ADBE Root Vectors Group")
  .property("ADBE Vector Graphic - Stroke")
  .property("ADBE Vector Stroke Width").setValue(3);

var ut = ul.property("ADBE Transform Group");
ut.property("ADBE Position").setValue([lay.w / 2, lay.h / 2]);
move(ut.property("ADBE Anchor Point"), 0, [A.cx, A.cy], DUR, [B.cx, B.cy]);
move(ut.property("ADBE Scale"), 0, [f0, f0], DUR, [f1, f1]);

// --- headline: left column x 280, optical centre y 1080, in at scene + 0.6 s ---
function headlineBaseline(str, centreY) {
  var lines = String(str).split("\n").length;
  return centreY - (lines - 1) * TYPE.headline.leading / 2 + TYPE.headline.size * 0.36;
}

var headline = T("Enter the\ndimensions.", "寸法を入力。");
addHeadline(comp, headline, 280, headlineBaseline(headline, 1080), 0.6, DUR, { justify: "left" });

addStatus(comp, 0.6, DUR);

comp.numLayers;
