// node render.js stills|video [lang]  — stills: key frames; video: 30 fps frames piped to ffmpeg
const { chromium } = require('playwright'); const { spawn } = require('child_process'); const fs = require('fs');
(async () => { const [mode, lang = 'en'] = process.argv.slice(2); const b = await chromium.launch({ args: ['--allow-file-access-from-files'] });
  const p = await b.newPage({ viewport: { width: 1920, height: 1080 } }); const errs = []; p.on('pageerror', (e) => errs.push(e.message));
  await p.goto('file://' + __dirname + '/film.html?lang=' + lang); await p.waitForFunction('window.READY');
  if (mode === 'stills') { fs.mkdirSync('stills', { recursive: true }); for (const t of [3.5, 8, 13, 16.5, 21.5, 24, 28]) { await p.evaluate((x) => window.seek(x), t); await p.screenshot({ path: `stills/${lang}-${String(t).padStart(4, '0')}.png` }); } }
  else { const ff = spawn('ffmpeg', ['-loglevel', 'error', '-y', '-f', 'image2pipe', '-framerate', '30', '-i', '-', '-c:v', 'libx264', '-pix_fmt', 'yuv420p', '-crf', '17', '-movflags', '+faststart', `nexus-fabric-30s-draft-${lang}.mp4`], { stdio: ['pipe', 'inherit', 'inherit'] });
    for (let f = 0; f < 900; f++) { await p.evaluate((x) => window.seek(x), f / 30); ff.stdin.write(await p.screenshot({ type: 'jpeg', quality: 95 })); } ff.stdin.end(); await new Promise((r) => ff.on('close', r)); }
  if (errs.length) console.log('ERRORS', errs); await b.close(); })();
