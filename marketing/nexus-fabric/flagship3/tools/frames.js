// usage: node tools/frames.js <lang> <outdir> <t1,t2,...> [jpeg|png]   (review frames; the caller deletes them after judging)
const { chromium } = require('playwright'); const fs = require('fs'), path = require('path');
(async () => {
  const [lang, dir, list, fmt = 'jpeg'] = process.argv.slice(2);
  const br = await chromium.launch({ args: ['--allow-file-access-from-files'] });
  const page = await br.newPage({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: 1 });
  const errs = []; page.on('pageerror', (e) => errs.push(e.message));
  await page.goto('file://' + path.resolve(__dirname, '..', 'film.html') + '?lang=' + lang);
  await page.waitForFunction('window.READY !== undefined', null, { timeout: 120000 });
  if ((await page.evaluate('window.READY')) !== true) throw new Error('film not ready');
  fs.mkdirSync(dir, { recursive: true });
  for (const t of list.split(',').map(Number)) {
    await page.evaluate((x) => window.seek(x), t);
    const f = path.join(dir, `${lang}-${t.toFixed(2)}.${fmt === 'png' ? 'png' : 'jpg'}`);
    await page.screenshot(fmt === 'png' ? { path: f, type: 'png' } : { path: f, type: 'jpeg', quality: 82 });
  }
  console.log('frames', list.split(',').length, 'errors', errs.length, errs.slice(0, 2).join(' | '));
  await br.close();
})().catch((e) => { console.error('frames failed:', e.message); process.exit(1); });
