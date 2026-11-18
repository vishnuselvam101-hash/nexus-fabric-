// Records the animatic to WebM with a real browser; ffmpeg then trims and encodes it.
const { chromium } = require('playwright'); const path = require('path'); const fs = require('fs');
(async () => {
  const lang = process.argv[2]; const outDir = process.argv[3];
  const b = await chromium.launch();
  const ctx = await b.newContext({ viewport: { width: 1280, height: 720 }, recordVideo: { dir: outDir, size: { width: 1280, height: 720 } } });
  const p = await ctx.newPage();
  await p.goto('file://' + path.resolve(__dirname, 'animatic.html') + '?lang=' + lang);
  await p.waitForTimeout(57500);
  const v = p.video(); await ctx.close(); const f = await v.path();
  fs.renameSync(f, path.join(outDir, `raw-${lang}.webm`)); await b.close();
})().catch((e) => { console.error(e.message); process.exit(1); });
