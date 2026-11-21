// Renders stills of the film at given times. Usage: node stills.js <outDir> [lang] [t1,t2,...]
const { chromium } = require('playwright'); const path = require('path'); const fs = require('fs');
(async () => {
  const out = process.argv[2]; const lang = process.argv[3] || 'en'; const times = (process.argv[4] || '2.5,8,16,19,24,28,32.5,36,40,46,53').split(',').map(Number);
  fs.mkdirSync(out, { recursive: true });
  const b = await chromium.launch({ args: ['--allow-file-access-from-files', '--ignore-gpu-blocklist'] });
  const p = await b.newPage({ viewport: { width: 1920, height: 1080 } });
  const errs = []; p.on('pageerror', (e) => errs.push(e.message)); p.on('console', (m) => { if (m.type() === 'error') errs.push(m.text()); });
  await p.goto('file://' + path.resolve(__dirname, 'film.html') + '?lang=' + lang);
  await p.waitForFunction('window.READY === true', null, { timeout: 120000 });
  for (const t of times) { const t0 = Date.now(); await p.evaluate((x) => window.seek(x), t); await p.waitForTimeout(250);
    await p.screenshot({ path: path.join(out, `t${String(t).padStart(4, '0')}.png`) }); console.log('t', t, (Date.now() - t0) + ' ms'); }
  if (errs.length) console.log('ERRORS', errs.slice(0, 5)); await b.close();
})().catch((e) => { console.error('FAILED', e.message); process.exit(1); });
