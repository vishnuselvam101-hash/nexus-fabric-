// Browser smoke test. Requires a freshly started server with an EMPTY data directory (it performs first-run setup).
// Usage: NF_BASE=http://127.0.0.1:8787 node tests/ui_smoke.js [screenshot-dir]
const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1360, height: 900 }, deviceScaleFactor: process.argv[2] ? 2 : 1 });
  const errs = []; p.on('pageerror', (e) => errs.push('pageerror: ' + e.message)); p.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text()); });
  p.on('dialog', (d) => { errs.push('unexpected dialog: ' + d.message()); d.dismiss(); }); // the client must not use blocking dialogs; a dialog would also reveal script injection
  const B = process.env.NF_BASE || 'http://127.0.0.1:8793';
  await p.goto(B); await p.fill('input[name=u]', 'admin'); await p.fill('input[name=p]', 'correct-horse-9'); await p.click('button');
  await p.waitForSelector('text=Recent engineering jobs');
  await p.goto(B + '/#/projects'); await p.fill('input[name=n]', 'Header study <img src=x onerror="window.__xss=1">'); await p.fill('input[name=d]', '<script>window.__xss=2</script>'); await p.click('text=Create project');
  await p.waitForSelector('text=Current project snapshot');
  // single part
  await p.goto(B + '/#/math/1'); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'eccentric_reducer'); await p.click('text=Develop, verify and generate DXF');
  await p.waitForSelector('svg.pattern');
  const single = { state: await p.textContent('h1 .badge'), curves: await p.locator('svg.pattern polyline').count() };
  await p.click('#replay'); await p.waitForSelector('text=stored records checked');
  // assembly, correction, obligations
  await p.goto(B + '/#/math/1'); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'header_with_branch'); await p.selectOption('select[name=surface_reference]', 'outside');
  await p.click('text=Develop, verify and generate DXF'); await p.waitForSelector('text=Dependency edges');
  const assembly = { patterns: await p.locator('svg.pattern').count(), interfaceRows: await p.locator('td:text-is("interface")').count() }; // two interface checks + the interface record
  await p.selectOption('#corr select[name=p]', 'header_radius'); await p.fill('#corr input[name=v]', '142'); await p.fill('#corr input[name=r]', 'drawing dimension is the inside diameter');
  await p.click('text=Record correction and propagate'); await p.waitForSelector('text=Scrutiny obligations from the correction');
  const openBefore = await p.locator('button[data-decision=confirmed]').count();
  if (process.argv[2]) { await p.locator('text=Scrutiny obligations from the correction').scrollIntoViewIfNeeded(); await p.screenshot({ path: process.argv[2] + '/obligations.png' }); }
  await p.locator('button[data-decision=confirmed]').first().click(); // no reason yet: must be refused inline
  await p.waitForSelector('#flash'); const refusedWithoutReason = (await p.locator('button[data-decision=confirmed]').count()) === openBefore;
  await p.locator('input[data-reason]').first().fill('checked against the drawing: this radius is outside');
  await p.locator('button[data-decision=confirmed]').first().click(); await p.waitForFunction((n) => document.querySelectorAll('button[data-decision=confirmed]').length === n - 1, openBefore);
  const openAfter = await p.locator('button[data-decision=confirmed]').count();
  await p.goto(B + '/#/calc/2'); await p.waitForSelector('text=SUPERSEDED by');
  const supersededWithheld = await p.locator('text=DXF withheld.').count();
  const seen = {};
  for (const r of ['dashboard', 'projects', 'workspace', 'amoeba', 'manufacturing', 'verification', 'admin', 'account']) { await p.goto(B + '/#/' + r); await p.waitForSelector('h1'); await p.waitForTimeout(250); seen[r] = (await p.textContent('h1')).trim(); }
  await p.setViewportSize({ width: 820, height: 1000 }); await p.goto(B + '/#/calc/3'); await p.waitForSelector('svg.pattern');
  const tabletOverflow = await p.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 2);
  await p.goto(B + '/#/admin'); await p.waitForSelector('text=Security configuration');
  const auditIntact = await p.locator('text=INTACT').count();
  const xss = await p.evaluate(() => window.__xss || 0);
  const result = { single, assembly, openBefore, refusedWithoutReason, openAfter, supersededWithheld, seen, auditIntact, xss, tabletOverflow, errs };
  console.log(JSON.stringify(result, null, 1));
  await b.close();
  const ok = single.state === 'REVIEW REQUIRED' && single.curves === 4 && assembly.patterns === 2 && assembly.interfaceRows === 3 && openBefore === 3 && openAfter === 2 && supersededWithheld === 2 && refusedWithoutReason && auditIntact === 1 && xss === 0 && !tabletOverflow && errs.length === 0;
  process.exit(ok ? 0 : 1);
})().catch((e) => { console.error('UI TEST FAILED', e.message); process.exit(1); });
