// node render.js stills [lang] [t,t,...]   key frames -> stills/<lang>-<t>.png and a contact sheet stills/sheet-<lang>.png
// node render.js burst  [lang] a,b          10 fps contact sheet between a and b seconds -> stills/burst-<lang>-<a>.png
// node render.js video  [lang]              900 frames at 30 fps -> nexus-fabric-notch-30s-<lang>.mp4 (picture only; audio is added afterwards)
// Fonts are read from ../launch30/node_modules (run `npm install` there once). Everything is computed from time; no clock is used.
const { chromium } = require('playwright'); const { spawn, spawnSync } = require('child_process'); const fs = require('fs'); const path = require('path');
const TIMES = [1.6, 5.6, 9.2, 13.0, 17.2, 20.6, 23.9, 28.0];
const sheet = (files, out, cols, w, h) => { const a = ['-loglevel', 'error', '-y']; for (const f of files) a.push('-i', f);
  a.push('-filter_complex', files.map((_, i) => `[${i}:v]scale=${w}:${h},pad=${w + 8}:${h + 8}:4:4:color=0x2A313A[v${i}]`).join(';') + ';' + files.map((_, i) => `[v${i}]`).join('') + `xstack=inputs=${files.length}:layout=` + files.map((_, i) => `${(i % cols) * (w + 8)}_${Math.floor(i / cols) * (h + 8)}`).join('|') + ':fill=0x0E1116', '-frames:v', '1', out); spawnSync('ffmpeg', a, { stdio: 'inherit' }); };
(async () => {
  const [mode = 'stills', lang = 'en', list] = process.argv.slice(2); const extra = process.env.NF_QUERY ? '&' + process.env.NF_QUERY : '';
  const b = await chromium.launch({ args: ['--allow-file-access-from-files', '--font-render-hinting=none'] });
  const p = await b.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 }); const errs = [];
  p.on('pageerror', (e) => errs.push('pageerror: ' + e.message)); p.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text()); }); p.on('requestfailed', (r) => errs.push('requestfailed: ' + r.url()));
  await p.goto('file://' + __dirname + '/film.html?lang=' + lang + extra); await p.waitForFunction('window.READY');
  const dir = path.join(__dirname, 'stills'); fs.mkdirSync(dir, { recursive: true });
  if (mode === 'stills') { const times = list ? list.split(',').map(Number) : TIMES, files = [];
    for (const t of times) { await p.evaluate((x) => window.seek(x), t); const f = path.join(dir, `${lang}-${t.toFixed(1).padStart(4, '0')}.png`); await p.screenshot({ path: f }); files.push(f); }
    if (!list) sheet(files, path.join(dir, `sheet-${lang}.png`), 4, 960, 540);
  } else if (mode === 'burst') { const [a, c] = list.split(',').map(Number), tmp = path.join(dir, 'tmp'), files = []; fs.mkdirSync(tmp, { recursive: true });
    for (let t = a; t <= c + 1e-6; t += 0.1) { await p.evaluate((x) => window.seek(x), t); const f = path.join(tmp, `${files.length}.png`); await p.screenshot({ path: f }); files.push(f); }
    sheet(files, path.join(dir, `burst-${lang}-${a}.png`), 5, 768, 432); fs.rmSync(tmp, { recursive: true, force: true });
  } else { const ff = spawn('ffmpeg', ['-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', '30', '-i', '-', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '15', '-preset', 'slow', '-movflags', '+faststart', path.join(__dirname, `nexus-fabric-notch-30s-${lang}.mp4`)], { stdio: ['pipe', 'inherit', 'inherit'] });
    for (let f = 0; f < 900; f++) { await p.evaluate((x) => window.seek(x), f / 30); if (!ff.stdin.write(await p.screenshot({ type: 'png' }))) await new Promise((r) => ff.stdin.once('drain', r)); }
    ff.stdin.end(); await new Promise((r) => ff.on('close', r)); }
  console.log('console errors:', errs.length, errs.slice(0, 5).join(' | ')); await b.close(); })();
