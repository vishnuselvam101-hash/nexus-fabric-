// node render.js stills|video [lang] [t,t,...]
//   burst : 10 fps contact sheets over the four scene changes -> stills/r07/burst-<lang>-b1..b4.png
//   stills: key frames to stills/r06/<lang>-<t>.png plus contact sheet stills/r06/sheet-<lang>.png
//   video : 900 frames at 30 fps piped to ffmpeg -> nexus-fabric-30s-<lang>.mp4 (picture only; audio is muxed later)
const { chromium } = require('playwright'); const { spawn, spawnSync } = require('child_process'); const fs = require('fs'); const path = require('path');
const TIMES = [2.5, 5.2, 7, 8, 9.5, 11.3, 13, 16.5, 18.5, 20, 23.5, 25.3, 27, 29], OUT = 'r08';
const BURSTS = { b1: [4.4, 5.8], b2: [10.3, 12.0], b3: [17.5, 19.0], b4: [24.1, 26.0] };
const SWAPS = { h1: 5, h2: 11, h3: 18, h4: 24.5 };   // node render.js strip <lang>: every frame at 30 fps across each headline change (top band of the picture)   // node render.js burst <lang>: 10 fps contact sheets of the scene changes
(async () => {
  const [mode = 'stills', lang = 'en', list] = process.argv.slice(2);
  const b = await chromium.launch({ args: ['--allow-file-access-from-files', '--font-render-hinting=none'] });
  const p = await b.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 }); const errs = [];
  p.on('pageerror', (e) => errs.push('pageerror: ' + e.message)); p.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text()); }); p.on('requestfailed', (r) => errs.push('requestfailed: ' + r.url()));
  await p.goto('file://' + __dirname + '/film.html?lang=' + lang); await p.waitForFunction('window.READY');
  if (mode === 'stills') {
    const dir = path.join(__dirname, 'stills', OUT); fs.mkdirSync(dir, { recursive: true }); const times = list ? list.split(',').map(Number) : TIMES; const files = [];
    for (const t of times) { await p.evaluate((x) => window.seek(x), t); const f = path.join(dir, `${lang}-${t.toFixed(1).padStart(4, '0')}.png`); await p.screenshot({ path: f }); files.push(f); }
    if (!list) { const args = ['-loglevel', 'error', '-y']; for (const f of files) args.push('-i', f);
      args.push('-filter_complex', files.map((_, i) => `[${i}:v]scale=768:432,pad=776:440:4:4:color=0x2A313A[v${i}]`).join(';') + ';' + files.map((_, i) => `[v${i}]`).join('') + `xstack=inputs=${files.length}:layout=` + files.map((_, i) => `${(i % 5) * 776}_${Math.floor(i / 5) * 440}`).join('|'), '-frames:v', '1', path.join(dir, `sheet-${lang}.png`));
      spawnSync('ffmpeg', args, { stdio: 'inherit' }); }
    console.log('lengths (mm)', JSON.stringify(await p.evaluate('window.LENGTHS')));
  } else if (mode === 'strip') {
    const dir = path.join(__dirname, 'stills', OUT), tmp = path.join(dir, 'tmp'); fs.mkdirSync(tmp, { recursive: true });
    for (const [name, a] of Object.entries(SWAPS)) { const files = []; for (let k = -10; k <= 13; k++) { await p.evaluate((x) => window.seek(x), a + k / 30); const f = path.join(tmp, `${name}-${files.length}.png`); await p.screenshot({ path: f, clip: { x: 0, y: 80, width: 1920, height: 180 } }); files.push(f); }
      const args = ['-loglevel', 'error', '-y']; for (const f of files) args.push('-i', f); const cols = 2;
      args.push('-filter_complex', files.map((_, i) => `[${i}:v]scale=1280:120,pad=1288:124:4:2:color=0x2A313A[v${i}]`).join(';') + ';' + files.map((_, i) => `[v${i}]`).join('') + `xstack=inputs=${files.length}:layout=` + files.map((_, i) => `${(i % cols) * 1288}_${Math.floor(i / cols) * 124}`).join('|'), '-frames:v', '1', path.join(dir, `strip-${lang}-${name}.png`));
      spawnSync('ffmpeg', args, { stdio: 'inherit' }); }
    fs.rmSync(tmp, { recursive: true, force: true });
  } else if (mode === 'burst') {
    const dir = path.join(__dirname, 'stills', OUT), tmp = path.join(dir, 'tmp'); fs.mkdirSync(tmp, { recursive: true });
    for (const [name, [a, b]] of Object.entries(BURSTS)) { const files = []; for (let t = a; t <= b + 1e-6; t += 0.1) { await p.evaluate((x) => window.seek(x), t); const f = path.join(tmp, `${name}-${files.length}.png`); await p.screenshot({ path: f }); files.push(f); }
      const args = ['-loglevel', 'error', '-y']; for (const f of files) args.push('-i', f); const cols = 4;
      args.push('-filter_complex', files.map((_, i) => `[${i}:v]scale=960:540,pad=968:548:4:4:color=0x2A313A[v${i}]`).join(';') + ';' + files.map((_, i) => `[v${i}]`).join('') + `xstack=inputs=${files.length}:layout=` + files.map((_, i) => `${(i % cols) * 968}_${Math.floor(i / cols) * 548}`).join('|') + ':fill=0x0E1116', '-frames:v', '1', path.join(dir, `burst-${lang}-${name}.png`));
      spawnSync('ffmpeg', args, { stdio: 'inherit' }); }
    fs.rmSync(tmp, { recursive: true, force: true });
  } else {
    const ff = spawn('ffmpeg', ['-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', '30', '-i', '-', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '15', '-preset', 'slow', '-movflags', '+faststart', path.join(__dirname, `nexus-fabric-30s-${lang}.mp4`)], { stdio: ['pipe', 'inherit', 'inherit'] });
    for (let f = 0; f < 900; f++) { await p.evaluate((x) => window.seek(x), f / 30); if (!ff.stdin.write(await p.screenshot({ type: 'png' }))) await new Promise((r) => ff.stdin.once('drain', r)); }
    ff.stdin.end(); await new Promise((r) => ff.on('close', r));
  }
  console.log(errs.length ? 'ERRORS ' + JSON.stringify(errs, null, 1) : 'console errors: 0'); await b.close();
})();
