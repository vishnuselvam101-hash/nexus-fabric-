// node render.js stills|video [lang] [t,t,...]
//   stills: key frames to stills/r06/<lang>-<t>.png plus contact sheet stills/r06/sheet-<lang>.png
//   video : 900 frames at 30 fps piped to ffmpeg -> nexus-fabric-30s-<lang>.mp4 (picture only; audio is muxed later)
const { chromium } = require('playwright'); const { spawn, spawnSync } = require('child_process'); const fs = require('fs'); const path = require('path');
const TIMES = [2.5, 4.5, 7, 10, 13, 16.5, 20, 23.5, 26.5, 29];
(async () => {
  const [mode = 'stills', lang = 'en', list] = process.argv.slice(2);
  const b = await chromium.launch({ args: ['--allow-file-access-from-files', '--font-render-hinting=none'] });
  const p = await b.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 }); const errs = [];
  p.on('pageerror', (e) => errs.push('pageerror: ' + e.message)); p.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text()); }); p.on('requestfailed', (r) => errs.push('requestfailed: ' + r.url()));
  await p.goto('file://' + __dirname + '/film.html?lang=' + lang); await p.waitForFunction('window.READY');
  if (mode === 'stills') {
    const dir = path.join(__dirname, 'stills', 'r06'); fs.mkdirSync(dir, { recursive: true }); const times = list ? list.split(',').map(Number) : TIMES; const files = [];
    for (const t of times) { await p.evaluate((x) => window.seek(x), t); const f = path.join(dir, `${lang}-${t.toFixed(1).padStart(4, '0')}.png`); await p.screenshot({ path: f }); files.push(f); }
    if (!list) { const args = ['-loglevel', 'error', '-y']; for (const f of files) args.push('-i', f);
      args.push('-filter_complex', files.map((_, i) => `[${i}:v]scale=768:432,pad=776:440:4:4:color=0x2A313A[v${i}]`).join(';') + ';' + files.map((_, i) => `[v${i}]`).join('') + `xstack=inputs=${files.length}:layout=` + files.map((_, i) => `${(i % 5) * 776}_${Math.floor(i / 5) * 440}`).join('|'), '-frames:v', '1', path.join(dir, `sheet-${lang}.png`));
      spawnSync('ffmpeg', args, { stdio: 'inherit' }); }
    console.log('lengths (mm)', JSON.stringify(await p.evaluate('window.LENGTHS')));
  } else {
    const ff = spawn('ffmpeg', ['-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', '30', '-i', '-', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '15', '-preset', 'slow', '-movflags', '+faststart', path.join(__dirname, `nexus-fabric-30s-${lang}.mp4`)], { stdio: ['pipe', 'inherit', 'inherit'] });
    for (let f = 0; f < 900; f++) { await p.evaluate((x) => window.seek(x), f / 30); if (!ff.stdin.write(await p.screenshot({ type: 'png' }))) await new Promise((r) => ff.stdin.once('drain', r)); }
    ff.stdin.end(); await new Promise((r) => ff.on('close', r));
  }
  console.log(errs.length ? 'ERRORS ' + JSON.stringify(errs, null, 1) : 'console errors: 0'); await b.close();
})();
