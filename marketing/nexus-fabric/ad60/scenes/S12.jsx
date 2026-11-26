// scenes/S12.jsx — S12 · 504f · film 62.4–70.8 s · local 0 = film 62.4.
// L1 end card: #000, cyan hairline, centred stack, fade to black. No software, no status caption.

var DUR = 8.4;                 // 504 frames @ 60 fps
var T_FADE0 = 7.2;             // film 69.6 — hold ends
var T_FADE1 = 8.4;             // film 70.8 — black

var comp = app.project.items.addComp("S12", 3840, 2160, 1, DUR, 60);

comp.layers.addSolid(COL.stage, "BG", 3840, 2160, 1);

// Cyan hairline 2 x 640 px centred at (1920, 700); trim 0->100% over 0.0–0.6.
addHairline(comp, 1600, 700, 2240, 700, 0.0, 0.6, COL.accent);

// Centred stack. Each element holds at full opacity to the end of the comp;
// the 1.2 s fade to black is carried by the top black solid only.
function endText(str, role, y, tIn) {
  var L = textLayer(comp, str, role, 1920, y, tIn, DUR,
                    { justify: "center", fade: 0.30 });
  L.property("ADBE Transform Group").property("ADBE Opacity").setValueAtTime(DUR, 100);
  return L;
}

endText(T("NEXUS FABRIC", "NEXUS FABRIC"), "wordmark", 880, 0.6);

endText(T("Flat patterns you can check.", "検算できる板展開。"), "headline", 1120, 1.2);

endText(T("Request a demonstration", "デモのご依頼はこちら"), "cta", 1330, 3.0);

endText(T("chuo-sogo.net · sekishion@chuo-sogo.net · vishnu.s@chuo-sogo.net",
          "chuo-sogo.net · sekishion@chuo-sogo.net · vishnu.s@chuo-sogo.net"),
        "contacts", 1500, 4.2);

endText(T("Development build · not validated · not for manufacturing.",
          "開発版・未検証・製造には使用できません。"),
        "endstatus", 1620, 5.4);

// Fade to black 69.6–70.8 (local 7.2–8.4), then black tail.
var fade = comp.layers.addSolid(COL.stage, "FADE", 3840, 2160, 1);
fade.inPoint = 0;
fade.outPoint = DUR;
move(fade.property("ADBE Transform Group").property("ADBE Opacity"), T_FADE0, 0, T_FADE1, 100);

comp.numLayers;
