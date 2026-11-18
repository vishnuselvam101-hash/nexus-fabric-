// Browser smoke test. Requires a freshly started server with an EMPTY data directory (it performs first-run setup).
// Usage: NF_BASE=http://127.0.0.1:8787 node tests/ui_smoke.js [screenshot-dir]
const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch(); const p = await b.newPage({ viewport: { width: 1360, height: 900 } });
  const errs = []; p.on('pageerror', e => errs.push('pageerror: ' + e.message)); p.on('console', m => { if (m.type() === 'error') errs.push('console: ' + m.text()); });
  const B = process.env.NF_BASE || 'http://127.0.0.1:8793';
  await p.goto(B); await p.fill('input[name=u]', 'admin'); await p.fill('input[name=p]', 'correct-horse-9'); await p.click('button');
  await p.waitForSelector('text=Dashboard'); 
  await p.goto(B + '/#/projects'); await p.fill('input[name=n]', 'Header study'); await p.fill('input[name=d]', 'UI test'); await p.click('text=Create project');
  await p.waitForSelector('text=Current project snapshot');
  await p.goto(B + '/#/math/1'); try { await p.waitForSelector('#calc', {timeout: 4000}); } catch (e) { console.log('URL', p.url(), 'VIEW', (await p.textContent('#view')).replace(/\s+/g,' ').slice(0,300), 'ERRS', JSON.stringify(errs)); throw e; } await p.selectOption('select[name=fam]', 'eccentric_reducer'); await p.click('text=Develop, verify and generate DXF');
  await p.waitForSelector('svg.pattern'); 
  const state = await p.textContent('h1 .badge'); const polys = await p.locator('svg.pattern polyline').count();
  const checks = await p.locator('text=PASS').count();
  await p.click('#replay'); await p.waitForSelector('text=stored records checked');
  if (process.argv[2]) await p.screenshot({ path: process.argv[2] + '/calc.png' });
  const seen = {};
  for (const r of ['dashboard','workspace','amoeba','manufacturing','verification','admin']) { await p.goto(B + '/#/' + r); await p.waitForSelector('h1'); await p.waitForTimeout(250); seen[r] = (await p.textContent('h1')).trim(); }
  await p.setViewportSize({ width: 820, height: 1000 }); await p.goto(B + '/#/calc/1'); await p.waitForSelector('svg.pattern');
  const overflow = await p.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 2);
  console.log(JSON.stringify({ state, polys, checks, seen, tabletOverflow: overflow, errs }, null, 1));
  await b.close();
})().catch(e => { console.error('UI TEST FAILED', e.message); process.exit(1); });
