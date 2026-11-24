// scenes/S10.jsx — "Less on screen." · film 52.2–57.0 s · local 0–4.8 s · 288 f @ 60 fps.
// L2 dark workspace card, crop C -> crop B pushing onto the left "More" tool list.
// Requires GEOM + scenes/lib.jsx (concatenated by the build step) and global LANG.

var DUR = 4.8;

var comp = app.project.items.addComp("S10", 3840, 2160, 1, DUR, 60);
comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1); // stage stays #000

// --------------------------------------------------------------- the card ---

// Dark capture: the light-theme workspace reads as a blank white card on the
// black stage. No JA capture exists for this screen (the real EN capture is
// kept in both cuts, JA carried by overlay text only).
var CROP_A = { x0: 640, y0: 360, x1: 3200, y1: 1800 }; // content box C, 1.50x
// "More" tool list (Calculations, Projects, AMOEBA, Obligations, Audit, Users)
// plus the users panel to its right. normCrop widens this to the 2.20x cap.
var CROP_B = { x0: 480, y0: 260, x1: 1430, y1: 800 };

addCard(comp, "screens/dark/04-workspace.png", CROP_A, "L2", 0, DUR, {
  scaleFrom: 100,
  scaleTo: 100,
  xFrom: 2660,
  xTo: 2600,
  rotYFrom: LAYOUT.L2.rotYFrom,
  rotYTo: LAYOUT.L2.rotYTo,
  cropB: CROP_B,
  tEnd: DUR
});

comp.layers.addCamera("Cam", [1920, 1080]);

// ------------------------------------------------------------- headline -----

var HEAD = T("Less on screen.", "画面は、\nシンプルに。");
var SUB = T("Everything else behind More.", "ほかのツールは「その他」の中に。");

// Left column x 280, block optically centred on y 1080; sub-line 40 px below.
var HS = TYPE.headline.size;
var HL = TYPE.headline.leading;
var SS = TYPE.sub.size;
var nLines = HEAD.split("\n").length;
var ascent = HS * 0.72;
var descent = SS * 0.22;
var gap = 40;
var blockH = ascent + (nLines - 1) * HL + gap + SS + descent;
var headY = Math.round(1080 + ascent - blockH / 2);
var subY = headY + (nLines - 1) * HL + gap + SS;

addHeadline(comp, HEAD, 280, headY, 0.6, DUR, { justify: "left" });
addSub(comp, SUB, 280, subY, 0.9, DUR, { justify: "left" });

// "More" callout above the card, fades up at film 53.4 -> local 1.2.
// No accent in this shot (accent budget §3): secondary grey, no hairline.
textLayer(comp, T("More", "その他"), "caption", 2630, 540, 1.2, DUR,
          { justify: "center", color: COL.sub });

addStatus(comp, 0.9, DUR, "left");

comp.numLayers;
