// scenes/S08.jsx — "PDF lines. Your scale."  (film 41.4–46.8 s → local 0.0–5.4 s)
// L2 card: screens/dark/02-convert, crop A 1.85x → crop B 2.18x.
// Both crops frame the "Convert a PDF to DXF" drop panel (title, text, Choose PDF
// button, capture x 1200–2640 y 150–700) plus the scale (required) field below it
// (y 700–900); no empty page margin.
// A plain PDF page card drops 60 px into the drop panel at local 0.6–1.2 (ease-out, no bounce).
// No scan line, no sweep, no sparkle, no accent (accent budget untouched in this scene).

var DUR = 5.4;

var comp = app.project.items.addComp("S08", 3840, 2160, 1, DUR, 60);

var bg = comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);
bg.inPoint = 0;
bg.outPoint = DUR;

comp.layers.addCamera("Cam", [1920, 1080]);

// ------------------------------------------------------------------- card ---

// 16:9 boxes centred on the drop panel + scale field, both within the 2.20x cap.
var cropA = { x0: 880,  y0: 0,  x1: 2960, y1: 1170 }; // 2080x1170, 1.85x, centre 1920,585
var cropB = { x0: 1040, y0: 25, x1: 2800, y1: 1015 }; // 1760x990,  2.18x, centre 1920,520

var shot = addCard(comp, "screens/dark/02-convert.png", cropA, "L2", 0, DUR, {
  cropB: cropB,
  tEnd: DUR
});

// --------------------------------------------------------- PDF page card ---

var lay = LAYOUT.L2;
var nA = normCrop(cropA, lay.w, lay.h);
var nB = normCrop(cropB, lay.w, lay.h);
var fA = Math.min(100, fitScale(nA, lay));
var fB = Math.min(100, fitScale(nB, lay));

// Centre of the drop panel in capture px (x 1200–2640, y 150–700).
var ZONE = [1920, 425];

// Map the drop-panel centre into card-face px at time t, so the page lands inside
// the panel and rides the same push as the capture underneath it.
function zonePoint(t) {
  var u = clamp(t / DUR, 0, 1);
  var cx = nA.cx + (nB.cx - nA.cx) * u;
  var cy = nA.cy + (nB.cy - nA.cy) * u;
  var f = (fA + (fB - fA) * u) / 100;
  return [lay.w / 2 + (ZONE[0] - cx) * f, lay.h / 2 + (ZONE[1] - cy) * f];
}

var T_DROP0 = 0.6;   // film 42.0
var T_DROP1 = 1.2;   // film 42.6
var DROP_PX = 60;
var PAGE_W = 220, PAGE_H = 300;

var pStart = zonePoint(T_DROP0);
var pLand = zonePoint(T_DROP1);
var pEnd = zonePoint(DUR);

var page = shot.inner.layers.addShape();
page.name = "PDF page";
page.inPoint = T_DROP0;
page.outPoint = DUR;

var pg = page.property("ADBE Root Vectors Group");
pg.addProperty("ADBE Vector Shape - Rect").property("ADBE Vector Rect Size").setValue([PAGE_W, PAGE_H]);
pg.addProperty("ADBE Vector Graphic - Fill").property("ADBE Vector Fill Color").setValue(COL.text);
var pStroke = pg.addProperty("ADBE Vector Graphic - Stroke");
pStroke.property("ADBE Vector Stroke Color").setValue(COL.caption);
pStroke.property("ADBE Vector Stroke Width").setValue(2);

var ptr = page.property("ADBE Transform Group");
ptr.property("ADBE Anchor Point").setValue([0, 0]);

var pPos = ptr.property("ADBE Position");
pPos.setValueAtTime(T_DROP0, [pStart[0], pStart[1] - DROP_PX]);
pPos.setValueAtTime(T_DROP1, [pLand[0], pLand[1]]);
pPos.setValueAtTime(DUR, [pEnd[0], pEnd[1]]);
ease(pPos);

var pOp = ptr.property("ADBE Opacity");
pOp.setValueAtTime(T_DROP0, 0);
pOp.setValueAtTime(T_DROP0 + 0.12, 100);
ease(pOp);

// ------------------------------------------------------------------- text ---

addHeadline(comp,
  T("PDF lines.\nYour scale.", "PDFの線を、\n指定の縮尺で。"),
  280, 1048, 0.6, DUR, { justify: "left" });

addSub(comp,
  T("It does not read the drawing.", "図面の内容は読み取りません。"),
  280, 1305, 1.0, DUR, { justify: "left" });

addStatus(comp, 0.6, DUR, "left");

comp.numLayers;
