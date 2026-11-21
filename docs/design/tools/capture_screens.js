// Captures every screen at 1920x1080 and 1366x768 and measures load performance.
// Usage: NF_BASE=http://127.0.0.1:8787 node docs/design/tools/capture_screens.js <out-dir>   (server seeded with seed_demo.js)
const { chromium } = require('playwright');
const fs = require('fs');
const B = process.env.NF_BASE || 'http://127.0.0.1:8787';
const out = process.argv[2];
const SCREENS = [['dashboard', '#/dashboard'], ['projects', '#/projects'], ['project', '#/project/1'], ['workspace', '#/workspace'], ['amoeba', '#/amoeba'], ['math-new-calculation', '#/math/1'],
  ['job-single-eccentric', '#/calc/2'], ['job-assembly-corrected', '#/calc/7'], ['job-assembly-superseded', '#/calc/6'], ['job-advisory', '#/calc/8'], ['job-no-pattern', '#/calc/5'],
  ['manufacturing', '#/manufacturing'], ['verification', '#/verification'], ['admin', '#/admin'], ['account', '#/account']];
(async () => {
  const b = await chromium.launch(); const errs = []; const files = [];
  for (const [w, h] of [[1920, 1080], [1366, 768]]) {
    const ctx = await b.newContext({ viewport: { width: w, height: h } }); const p = await ctx.newPage();
    p.on('pageerror', (e) => errs.push('pageerror: ' + e.message)); p.on('console', (m) => { if (m.type() === 'error' && !/401/.test(m.text())) errs.push('console: ' + m.text()); });
    const shot = async (name) => { const f = `${out}/${name}-${w}x${h}.png`; await p.screenshot({ path: f }); files.push(f); };
    await p.goto(B); await p.waitForSelector('input[type=password]'); await shot('login');
    await ctx.request.post(B + '/api/login', { headers: { 'X-NF-Request': '1' }, data: { username: 'admin', password: 'correct-horse-9' } });
    for (const [name, hash] of SCREENS) { await p.goto(B + '/' + hash); await p.reload(); await p.waitForSelector('h1'); await p.waitForTimeout(400); await shot(name); }
    // Interaction states and views that exist only in the redesigned client (skipped for the "before" build).
    await p.goto(B + '/#/calc/7'); await p.reload(); await p.waitForSelector('h1'); await p.waitForTimeout(300);
    if (await p.$('#tab-ws-scrutiny')) {
      const vpos = (ci, vi) => p.evaluate(([c, v]) => { const pl = document.querySelector(`.pv .hit[data-ci="${c}"]`), q = pl.points[v], m = pl.getScreenCTM(); return [q.x * m.a + q.y * m.c + m.e, q.x * m.b + q.y * m.d + m.f]; }, [ci, vi]);
      let [x, y] = await vpos(4, 100); await p.mouse.click(x, y); await p.mouse.move(x, y); await p.click('#tab-ws-scrutiny'); await p.mouse.move(x + 1, y); await p.waitForTimeout(200); await shot('x-workspace-curve-selected-scrutiny');
      await p.click('#t-measure'); [x, y] = await vpos(0, 0); await p.mouse.click(x, y); [x, y] = await vpos(4, 300); await p.mouse.move(x, y); await p.mouse.click(x, y); await p.click('#tab-ws-checks'); await p.mouse.move(x + 1, y); await p.waitForTimeout(200); await shot('x-workspace-measure-checks');
      await p.click('#t-select'); await p.keyboard.press('2'); await p.click('#tab-ws-graph'); await p.locator('#tp-ws-graph .dgl .node').nth(3).click(); await p.waitForTimeout(200); await shot('x-workspace-branch-dependency-graph');
      await p.click('#tab-ws-history'); [x, y] = await vpos(2, 400); await p.mouse.move(x, y); await p.mouse.wheel(0, -500); await p.waitForTimeout(250); await shot('x-workspace-zoomed-history');
      await p.click('#tab-ws-records'); await p.locator('#tp-ws-records .rec').first().click(); await p.waitForTimeout(200); await shot('x-workspace-records-provenance');
      await p.click('#tab-ws-changes'); await p.click('#v-prev'); await p.waitForTimeout(200); await shot('x-workspace-changes-overlay'); await p.click('#v-prev');
      await p.click('#corr-open'); await p.waitForTimeout(200); await shot('x-workspace-correction-dialog'); await p.keyboard.press('Escape');
      await p.goto(B + '/#/math/1'); await p.waitForSelector('#calc'); await p.selectOption('#f_fam', 'header_with_branch'); await p.focus('#f_beta_deg'); await p.waitForTimeout(200); await shot('x-new-calculation-schematic');
      await p.fill('#f_branch_radius', '400'); await p.click('#run'); await p.waitForTimeout(200); await shot('x-new-calculation-refused-inline');
      await p.selectOption('#f_fam', 'eccentric_reducer'); await p.waitForTimeout(200); await shot('x-new-calculation-eccentric');
      await p.goto(B + '/#/calc/7'); await p.waitForSelector('.pv');
      await p.keyboard.press('?'); await p.waitForTimeout(250); await shot('x-help-overlay'); await p.keyboard.press('Escape');
      await p.click('#bell'); await p.waitForTimeout(150); await shot('x-notifications'); await p.keyboard.press('Escape');
      for (const t of ['activities', 'obligations', 'lessons', 'dependencies/7', 'mechanisms']) { await p.goto(B + '/#/amoeba/' + t); await p.waitForSelector('h1'); await p.waitForTimeout(250); await shot('x-amoeba-' + t.replace('/7', '')); }
      for (const t of ['security', 'audit']) { await p.goto(B + '/#/admin/' + t); await p.waitForSelector('h1'); await p.waitForTimeout(250); await shot('x-admin-' + t); }
      await p.click('#theme');
      for (const [n, hsh] of [['dashboard', '#/dashboard'], ['job-assembly-corrected', '#/calc/7'], ['verification', '#/verification']]) { await p.goto(B + '/' + hsh); await p.waitForSelector('h1'); await p.waitForTimeout(350); await shot('x-light-' + n); }
      await p.click('#theme');
      if (w === 1366) for (const nw of [1024, 768]) { await p.setViewportSize({ width: nw, height: 768 }); for (const [n, hsh] of [['dashboard', '#/dashboard'], ['job-assembly-corrected', '#/calc/7'], ['amoeba', '#/amoeba']]) { await p.goto(B + '/' + hsh); await p.waitForSelector('h1'); await p.waitForTimeout(350); const f = `${out}/x-responsive-${n}-${nw}x768.png`; await p.screenshot({ path: f }); files.push(f); } }
    }
    await ctx.close();
  }
  // performance: cold loads of the dashboard and of the heaviest job page, median of 7
  const ctx = await b.newContext({ viewport: { width: 1366, height: 768 } });
  await ctx.request.post(B + '/api/login', { headers: { 'X-NF-Request': '1' }, data: { username: 'admin', password: 'correct-horse-9' } });
  const med = (a) => a.sort((x, y) => x - y)[Math.floor(a.length / 2)];
  const perf = {};
  for (const [name, hash] of [['dashboard', '#/dashboard'], ['job', '#/calc/7']]) {
    const dcl = [], render = [], nodes = [];
    for (let i = 0; i < 7; i++) {
      const p = await ctx.newPage(); await p.goto(B + '/' + hash); await p.waitForSelector('h1');
      const m = await p.evaluate(() => new Promise((res) => requestAnimationFrame(() => res({ dcl: performance.getEntriesByType('navigation')[0].domContentLoadedEventEnd, now: performance.now(), nodes: document.querySelectorAll('*').length }))));
      dcl.push(m.dcl); render.push(m.now); nodes.push(m.nodes); await p.close();
    }
    perf[name] = { domContentLoaded_ms: +med(dcl).toFixed(1), first_view_rendered_ms: +med(render).toFixed(1), dom_nodes: med(nodes) };
  }
  const assets = {};
  for (const a of ['/', '/app.js', '/viewer.js', '/styles.css']) { const r = await ctx.request.get(B + a); if (r.ok()) assets[a] = (await r.body()).length; }
  assets.total = Object.values(assets).reduce((x, y) => x + y, 0);
  const report = { assets_bytes: assets, perf, console_errors: errs, screenshots: files.length };
  fs.writeFileSync(`${out}/metrics.json`, JSON.stringify(report, null, 1)); console.log(JSON.stringify(report, null, 1));
  await b.close();
})().catch((e) => { console.error('CAPTURE FAILED', e.message); process.exit(1); });
