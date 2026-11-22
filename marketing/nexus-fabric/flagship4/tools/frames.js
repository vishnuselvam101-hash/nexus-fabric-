// Review frames from the page. usage: node tools/frames.js <en|ja> <outdir> <t1,t2,...|t0-t1:step> [--labels 0] [--scale 2] [--png]
// Frames are written as <lang>-<t>.jpg (or .png). The caller deletes them after judging.
const { chromium } = require('playwright'); const fs = require('fs'), path = require('path');
const opt = (k, d) => { const i = process.argv.indexOf('--' + k); return i > 0 ? process.argv[i + 1] : d; };
(async () => {
  const [lang, dir, spec] = process.argv.slice(2);
  let times; if (/^[\d.]+-[\d.]+:[\d.]+$/.test(spec)) { const [r, st] = spec.split(':'); const [a, b] = r.split('-').map(Number); times = []; for (let x = a; x <= b + 1e-9; x += Number(st)) times.push(Math.round(x * 1000) / 1000); } else times = spec.split(',').map(Number);
  const png = process.argv.includes('--png'), scale = Number(opt('scale', '1')), labels = opt('labels', '1');
  const br = await chromium.launch({ args: ['--allow-file-access-from-files', '--ignore-gpu-blocklist', '--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
  const page = await br.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errs = []; page.on('pageerror', (e) => errs.push(e.message)); page.on('console', (m) => { if (m.type() === 'error') errs.push(m.text()); });
  await page.goto('file://' + path.resolve(__dirname, '..', 'film.html') + `?lang=${lang}&labels=${labels}&scale=${scale}`);
  await page.waitForFunction('window.READY !== undefined', null, { timeout: 300000 });
  if ((await page.evaluate('window.READY')) !== true) throw new Error('film not ready: ' + (await page.evaluate('window.READY_ERROR')) + ' ' + errs.join(' | '));
  fs.mkdirSync(dir, { recursive: true });
  for (const t of times) {
    const url = await page.evaluate(([x, p]) => { window.seek(x); return document.getElementById('gfx').toDataURL(p ? 'image/png' : 'image/jpeg', 0.9); }, [t, png]);
    fs.writeFileSync(path.join(dir, `${lang}-${t.toFixed(2)}.${png ? 'png' : 'jpg'}`), Buffer.from(url.split(',')[1], 'base64'));
  }
  console.log('frames', times.length, 'errors', errs.length, errs.slice(0, 3).join(' | '));
  await br.close();
})().catch((e) => { console.error('frames failed:', e.message); process.exit(1); });
