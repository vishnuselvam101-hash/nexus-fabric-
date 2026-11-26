// node render.js stills [lang] [t,t,...]   key frames -> stills/<lang>-<t>.png and a contact sheet stills/sheet-<lang>.png
// node render.js burst  [lang] a,b          10 fps contact sheet between a and b seconds -> stills/burst-<lang>-<a>.png
// node render.js video  [lang]              900 frames at 30 fps -> final/NEXUS-FABRIC-ad-4k-<lang>-picture.mp4 (picture only; audio is added afterwards)
// Captures are 1920x1080 at deviceScaleFactor 2, i.e. 3840x2160. Everything is computed from window.seek(t); no clock is used.
const { chromium } = require('/root/node_modules/playwright'); const { spawn, spawnSync } = require('child_process'); const fs = require('fs'); const path = require('path');
const TIMES = [1.5, 4.5, 8.0, 10.0, 12.2, 15.8, 19.2, 22.2, 24.2, 27.5];
const sheet = (files, out, cols, w, h) => { const a = ['-loglevel', 'error', '-y']; for (const f of files) a.push('-i', f);
  a.push('-filter_complex', files.map((_, i) => `[${i}:v]scale=${w}:${h},pad=${w + 8}:${h + 8}:4:4:color=0x2A313A[v${i}]`).join(';') + ';' + files.map((_, i) => `[v${i}]`).join('') + `xstack=inputs=${files.length}:layout=` + files.map((_, i) => `${(i % cols) * (w + 8)}_${Math.floor(i / cols) * (h + 8)}`).join('|') + ':fill=0x0E1116', '-frames:v', '1', out); spawnSync('ffmpeg', a, { stdio: 'inherit' }); };
(async () => {
  const [mode = 'stills', lang = 'en', list] = process.argv.slice(2); const extra = process.env.NF_QUERY ? '&' + process.env.NF_QUERY : '';
  const b = await chromium.launch({ args: ['--allow-file-access-from-files', '--font-render-hinting=none'] });
  const p = await b.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 2 }); const errs = [];
  p.on('pageerror', (e) => errs.push('pageerror: ' + e.message)); p.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text()); }); p.on('requestfailed', (r) => errs.push('requestfailed: ' + r.url()));
  await p.goto('file://' + __dirname + '/film.html?lang=' + lang + extra); await p.waitForFunction('window.READY', null, { timeout: 120000 });
  const dir = path.join(__dirname, 'stills'); fs.mkdirSync(dir, { recursive: true });
  if (mode === 'stills') { const times = list ? list.split(',').map(Number) : TIMES, files = [];
    for (const t of times) { await p.evaluate((x) => window.seek(x), t); const f = path.join(dir, `${lang}-${t.toFixed(1).padStart(4, '0')}.png`); await p.screenshot({ path: f }); files.push(f); }
    if (!list) sheet(files, path.join(dir, `sheet-${lang}.png`), 4, 960, 540);
  } else if (mode === 'burst') { const [a, c] = list.split(',').map(Number), tmp = path.join(dir, 'tmp'), files = []; fs.mkdirSync(tmp, { recursive: true });
    for (let t = a; t <= c + 1e-6; t += 0.1) { await p.evaluate((x) => window.seek(x), t); const f = path.join(tmp, `${files.length}.png`); await p.screenshot({ path: f }); files.push(f); }
    sheet(files, path.join(dir, `burst-${lang}-${a}.png`), 5, 768, 432); fs.rmSync(tmp, { recursive: true, force: true });
  } else { const out = path.join(__dirname, 'final'); fs.mkdirSync(out, { recursive: true });
    const fps = Number(process.env.NF_FPS || 30), frames = Math.round(30 * fps), every = Math.max(1, Math.round(3 * fps));
    const name = fps === 30 ? `NEXUS-FABRIC-ad-4k-${lang}-picture.mp4` : `NEXUS-FABRIC-ad-4k60-${lang}-picture.mp4`;
    const ff = spawn('ffmpeg', ['-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', String(fps), '-i', '-', '-c:v', 'libx264', '-profile:v', 'high', '-level', fps === 30 ? '5.1' : '5.2', '-r', String(fps), '-s', '3840x2160', '-pix_fmt', 'yuv420p', '-crf', '16', '-preset', 'slow', '-movflags', '+faststart', path.join(out, name)], { stdio: ['pipe', 'inherit', 'inherit'] });
    for (let f = 0; f < frames; f++) { await p.evaluate((x) => window.seek(x), f / fps); if (!ff.stdin.write(await p.screenshot({ type: 'png' }))) await new Promise((r) => ff.stdin.once('drain', r));
      if ((f + 1) % every === 0) console.log(`video ${lang}: ${f + 1}/${frames} frames (${((f + 1) / fps).toFixed(1)}s)`); }
    ff.stdin.end(); await new Promise((r) => ff.on('close', r)); }
  console.log('console errors:', errs.length, errs.slice(0, 5).join(' | ')); await b.close(); if (errs.length) process.exitCode = 1; })();
