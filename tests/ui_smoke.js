// Browser test of every critical workflow. Requires a freshly started server with an EMPTY data directory
// (it performs first-run setup). Usage: NF_BASE=http://127.0.0.1:8793 node tests/ui_smoke.js [screenshot-dir]
// Exits 0 only if every check passes. Prints each check and the exact pass/fail counts.
const { chromium } = require('playwright');
const fs = require('fs');

const B = process.env.NF_BASE || 'http://127.0.0.1:8793';
const SHOTS = process.argv[2];
const ADMIN = { username: 'admin', password: 'correct-horse-9' }, NEW_PW = 'correct-horse-10';
const results = [];
const check = (name, ok, detail) => { results.push({ name, ok: !!ok, detail }); console.log(`${ok ? 'ok  ' : 'FAIL'} - ${name}${ok || detail === undefined ? '' : `  [${JSON.stringify(detail)}]`}`); };
const f3 = (n) => (Math.abs(n) < 5e-4 ? 0 : n).toFixed(3);

// Elements that stick out of the viewport without a scrollable or clipping ancestor that itself fits.
const overflowProbe = () => {
  const W = window.innerWidth, bad = [];
  if (document.documentElement.scrollWidth > W + 1) bad.push('document');
  for (const el of document.querySelectorAll('#app *')) {
    const r = el.getBoundingClientRect(); if (!r.width || !r.height) continue;
    if (el.matches('a.skip:not(:focus), .vh')) continue; // deliberately off-screen until focused / screen-reader-only text
    if (r.right > W + 1 || r.left < -1) {
      let a = el.parentElement, ok = false;
      while (a && a !== document.body) { if (/(auto|scroll|hidden|clip)/.test(getComputedStyle(a).overflowX)) { const ar = a.getBoundingClientRect(); if (ar.right <= W + 1 && ar.left >= -1) { ok = true; break; } } a = a.parentElement; }
      if (!ok) bad.push(`${el.tagName.toLowerCase()}.${String(el.className.baseVal ?? el.className).split(' ')[0]}`);
    }
  }
  return bad.slice(0, 5);
};
// Visible interactive controls without an accessible name; and any control that would approve or release a result.
const controlProbe = () => {
  const vis = (el) => { const r = el.getBoundingClientRect(); return r.width > 0 && r.height > 0; };
  const name = (el) => (el.getAttribute('aria-label') || el.textContent || '').trim() || (el.labels && el.labels.length ? 'labelled' : '') || el.getAttribute('title') || '';
  const all = [...document.querySelectorAll('button,a[href],input,select,textarea,[role=tab],[role=separator]')].filter(vis);
  return {
    unnamed: all.filter((el) => !name(el)).map((el) => el.outerHTML.slice(0, 80)).slice(0, 3),
    approve: all.filter((el) => /\b(approve|release for manufacturing|mark as approved)\b/i.test(name(el))).length,
    approvedTags: [...document.querySelectorAll('.tag')].filter((t) => /^approved$/i.test(t.textContent.trim())).length,
  };
};

(async () => {
  const browser = await chromium.launch();
  const errs = []; let expected401 = 0, allow401 = false, dialogs = 0, approveControls = 0, approvedTags = 0;
  const watch = (page, who) => {
    page.on('pageerror', (e) => errs.push(`${who} pageerror: ${e.message}`));
    page.on('console', (m) => { if (m.type() !== 'error') return; if (allow401 && /401/.test(m.text())) expected401++; else errs.push(`${who} console: ${m.text()}`); });
    page.on('dialog', (d) => { dialogs++; d.dismiss(); }); // the client must never use blocking dialogs; one would also reveal script injection
  };
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 }, acceptDownloads: true, deviceScaleFactor: SHOTS ? 2 : 1 });
  const p = await ctx.newPage(); watch(p, 'admin');
  const apiGet = async (path) => (await ctx.request.get(B + path, { headers: { 'X-NF-Request': '1' } })).json();
  const go = async (hash, sel) => { await p.goto(`${B}/#/${hash}`); await p.waitForSelector(sel || '#view h1'); };
  const sweepControls = async () => { const c = await p.evaluate(controlProbe); approveControls += c.approve; approvedTags += c.approvedTags; return c; };
  const vpos = (ci, vi) => p.evaluate(([c, v]) => { const pl = document.querySelector(`.pv .hit[data-ci="${c}"]`), q = pl.points[v], m = pl.getScreenCTM(); return [q.x * m.a + q.y * m.c + m.e, q.x * m.b + q.y * m.d + m.f]; }, [ci, vi]);
  const transform = () => p.evaluate(() => document.querySelector('.pv g[transform]').getAttribute('transform'));
  const zoom = async () => parseInt(await p.textContent('#zoomv'), 10);

  // ---- 1. first-run setup and sign-in
  await p.goto(B);
  await p.waitForSelector('#f');
  check('setup: first-run screen asks for the administrator', /Create the administrator/.test(await p.textContent('h1')));
  check('setup: sign-in screen carries the development-build notice', /Not validated/.test(await p.textContent('.login-card')));
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', ADMIN.password); await p.click('#f button');
  await p.waitForSelector('.stats');
  check('login: dashboard shown after setup', (await p.textContent('#view h1')).trim() === 'Dashboard');
  check('shell: landmarks present', await p.evaluate(() => !!document.querySelector('header.topbar') && !!document.querySelector('nav[aria-label="Main"]') && !!document.querySelector('main#view') && !!document.querySelector('footer.statusbar') && !!document.querySelector('a.skip')));
  check('shell: build notice in top bar and status bar', /not validated/i.test(await p.textContent('.devnote')) && /NOT FOR MANUFACTURING/.test(await p.textContent('.statusbar')));
  check('dashboard: honest empty state before any job', /No engineering jobs yet/.test(await p.textContent('#view')));

  // ---- 2. project (with markup in the name: must be shown as text, never executed) and document storage
  await go('projects', '#np');
  await p.fill('#np input[name=n]', 'Header study <img src=x onerror="window.__xss=1">'); await p.fill('#np input[name=d]', '<script>window.__xss=2</script>');
  await p.click('#np button'); await p.waitForURL(/#\/project\/1$/); await p.waitForSelector('#up');
  check('project: created and opened', /Header study <img/.test(await p.textContent('#view h1')));
  await p.setInputFiles('#upf', { name: 'ga-drawing.pdf', mimeType: 'application/pdf', buffer: Buffer.from('%PDF-1.7\nui smoke, not a drawing\n') });
  await p.click('#up button'); await p.waitForSelector('#view td:text-is("pdf")');
  check('project: document stored, snapshot recorded', /ga-drawing\.pdf/.test(await p.textContent('#view')) && !/snapshot: none/.test(await p.textContent('#view')));

  // ---- 3. every calculation family, through the form
  const FAMILIES = [['cylinder_shell', 1, 4], ['cone_frustum', 1, 4], ['eccentric_reducer', 1, 4], ['header_with_branch', 2, 5], ['branch_stub', 1, 4]];
  const jobs = {};
  for (const [fam, parts, curves] of FAMILIES) {
    await go('math/1', '#calc');
    await p.selectOption('select[name=fam]', fam);
    if (fam === 'header_with_branch') await p.selectOption('select[name=surface_reference]', 'outside');
    await p.click('#run'); await p.waitForURL(/#\/calc\/\d+$/); await p.waitForSelector('.pv .c', { state: 'attached' });
    const id = Number(p.url().split('/').pop()); jobs[fam] = id;
    const got = { state: (await p.textContent('.ws-head h1 .tag')).trim(), parts: await p.locator('#parts [role=tab]').count(), curves: await p.locator('.pv .c').count() };
    await p.click('#tab-ws-checks');
    const verdicts = await p.locator('#tp-ws-checks .l1 .tag').allTextContents();
    check(`calculation ${fam}: REVIEW REQUIRED, ${parts} part(s), ${curves} curves drawn, all checks PASS`, got.state === 'REVIEW REQUIRED' && got.parts === parts && got.curves === curves && verdicts.length >= 4 && verdicts.every((v) => v.trim() === 'PASS'), { got, verdicts: verdicts.length });
    await p.click('#tab-ws-gates');
    check(`calculation ${fam}: approval gate G16 shown as NOT IMPLEMENTED`, /G16\s*human approval\s*NOT IMPLEMENTED/.test((await p.textContent('#tp-ws-gates')).replace(/\s+/g, ' ')));
    await sweepControls();
  }

  // ---- 4. pattern viewer on the eccentric reducer: real geometry read-out, selection, zoom, pan, fit, measure
  const J = jobs.eccentric_reducer;
  const data = await apiGet(`/api/calculations/${J}`);
  const curve0 = data.patterns[0].curves[0], v100 = curve0.points[100];
  await go(`calc/${J}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' });
  let [x, y] = await vpos(0, 100); await p.mouse.move(x - 30, y - 30); await p.mouse.move(x, y);
  await p.waitForFunction((n) => document.querySelector('#ro-info').textContent.includes(n), curve0.name);
  const ro = await p.textContent('#ro-info');
  check('viewer: hover read-out names the curve, its computed length and the real vertex coordinates', ro.includes(curve0.name) && ro.includes(`length ${f3(curve0.length)} mm`) && ro.includes(`vertex 101/${curve0.points.length} (${f3(v100[0])}, ${f3(v100[1])})`), ro);
  check('viewer: cursor coordinates are pattern coordinates in mm', Math.abs(Number(await p.textContent('#ro-x')) - v100[0]) < 2 && Math.abs(Number(await p.textContent('#ro-y')) - v100[1]) < 2);
  await p.mouse.click(x, y);
  const selText = await p.textContent('#sel');
  check('viewer: clicking a curve selects it and shows its properties from the result data', selText.includes(curve0.name) && selText.includes(String(Number(curve0.length.toPrecision(12)))) && selText.includes(String(curve0.points.length)) && (await p.getAttribute('#curves tr[data-ci="0"]', 'aria-selected')) === 'true', selText);
  await p.click('#curves button[data-ci="2"]');
  check('viewer: curve table selects a curve by keyboard-reachable control', (await p.textContent('#sel')).includes(data.patterns[0].curves[2].name) && (await p.locator('.pv .c.sel').count()) === 1);
  const t0 = await transform(); await p.click('#z-in'); await p.click('#z-in');
  const zIn = await zoom(); [x, y] = await vpos(0, 100); await p.mouse.move(x, y); await p.mouse.wheel(0, -300);
  await p.waitForFunction((z) => parseInt(document.querySelector('#zoomv').textContent, 10) > z, zIn);
  check('viewer: zoom buttons and wheel zoom in', zIn > 100 && (await zoom()) > zIn, { zIn });
  const t1 = await transform(); const box = await p.locator('#pv').boundingBox();
  await p.mouse.move(box.x + box.width / 2, box.y + box.height / 2); await p.mouse.down(); await p.mouse.move(box.x + box.width / 2 + 90, box.y + box.height / 2 + 40, { steps: 4 }); await p.mouse.up();
  await p.waitForFunction((t) => document.querySelector('.pv g[transform]').getAttribute('transform') !== t, t1);
  check('viewer: drag pans the view', (await transform()) !== t1);
  await p.focus('.pv'); const t2 = await transform(); await p.keyboard.press('ArrowRight');
  await p.waitForFunction((t) => document.querySelector('.pv g[transform]').getAttribute('transform') !== t, t2);
  await p.keyboard.press('f'); await p.waitForFunction(() => document.querySelector('#zoomv').textContent === '100%');
  check('viewer: keyboard pans, F fits back to 100%', (await transform()) === t0, { t0, now: await transform() });
  await p.keyboard.press('m');
  const seam = data.patterns[0].curves[3]; // seam_a: a straight line whose length the engine reports
  [x, y] = await vpos(3, 0); await p.mouse.click(x, y); [x, y] = await vpos(3, 1); await p.mouse.move(x, y); await p.mouse.click(x, y);
  await p.waitForFunction(() => /^Distance/.test(document.querySelector('#ro-info').textContent));
  const meas = await p.textContent('#ro-info');
  check('viewer: measuring between two computed vertices gives the engine length of that line', meas.startsWith(`Distance ${f3(seam.length)} mm`), meas);
  await p.keyboard.press('Escape'); await p.keyboard.press('Escape');
  check('viewer: canvas exposes an accessible description', /Flat pattern of reducer: 4 curves, extents/.test(await p.getAttribute('.pv', 'aria-label')));

  // ---- 5. DXF download
  const [dl] = await Promise.all([p.waitForEvent('download'), p.click('#dxf')]);
  const dxf = fs.readFileSync(await dl.path(), 'utf8');
  check('DXF: downloads, file name marked NOT-RELEASED, content ends with EOF', dl.suggestedFilename().startsWith('NOT-RELEASED_REVIEW_REQUIRED') && dxf.trimEnd().endsWith('EOF') && dxf.length > 1000, dl.suggestedFilename());

  // ---- 6. replay audit
  await p.click('#replay'); await p.waitForSelector('#tp-ws-records:not([hidden]) >> text=stored records checked');
  check('replay: audit reproduces the closure and is reported with its limitation', /PASS/.test(await p.textContent('#tp-ws-records .kv')) && /same machine/.test(await p.textContent('#tp-ws-records')));
  await p.click('#tab-ws-history');
  check('history: replay audit appears in the job history', /Replay audit reproduced the closure/.test(await p.textContent('#tp-ws-history')));

  // ---- 7. assembly: parts, interface checks, dependency graph
  const A = jobs.header_with_branch;
  await go(`calc/${A}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' });
  const headerCurves = await p.locator('.pv .c').count(), innerCurves = await p.locator('.pv .c.inner').count();
  await p.keyboard.press('2'); await p.waitForFunction(() => document.querySelector('#curves-part').textContent.includes('branch'));
  check('assembly: header shows its opening, key 2 switches to the branch pattern', headerCurves === 5 && innerCurves === 1 && (await p.locator('.pv .c').count()) === 4 && /branch/.test(await p.getAttribute('#dxf', 'href')));
  await p.click('#tab-ws-checks'); const interfaceChecks = await p.locator('#tp-ws-checks .id:text-is("interface")').count();
  await p.click('#tab-ws-records'); const interfaceRecords = await p.locator('#tp-ws-records .rec b:text-is("interface")').count();
  check('assembly: two interface checks and the interface record', interfaceChecks === 2 && interfaceRecords === 1, { interfaceChecks, interfaceRecords });
  await p.click('#tp-ws-records .rec >> nth=0'); await p.waitForSelector('#tp-ws-records .rec[open] pre');
  check('records: a record opens to its stored payload', /header_with_branch/.test(await p.textContent('#tp-ws-records .rec[open] pre')));
  await p.click('#tab-ws-graph'); const nodes = await p.locator('#tp-ws-graph .dg .n').count(), edges = await p.locator('#tp-ws-graph .dg .e').count();
  await p.locator('#tp-ws-graph .dg .n', { hasText: 'header_radius' }).first().click();
  const reach = await p.textContent('#tp-ws-graph .dg-info');
  check('dependencies: graph drawn from the edge list, selecting a record traces what a change reaches', nodes >= 15 && edges >= 20 && /header_radius depends on 1 record\. A change to it reaches \d+:.*pattern · header/.test(reach), { nodes, edges, reach });

  // ---- 8. correction -> obligations -> resolution
  await p.selectOption('#corr select[name=p]', 'header_radius'); await p.fill('#corr input[name=v]', '142');
  await p.click('#corr button'); await p.waitForSelector('#corr .field-err');
  check('correction: refused inline without a reason', p.url().endsWith(`/calc/${A}`));
  await p.fill('#corr input[name=r]', 'drawing dimension is the inside diameter'); await p.click('#corr button');
  await p.waitForURL((u) => /#\/calc\/\d+$/.test(u.hash) && !u.hash.endsWith(`/${A}`)); await p.waitForSelector('#obls');
  const N = Number(p.url().split('/').pop());
  const openBefore = await p.locator('button[data-decision=confirmed]').count();
  check('correction: new job opens on its scrutiny obligations, three need an engineer', N > A && openBefore === 3 && (await p.locator('#obls > li').count()) >= 9 && /corrects job #/.test((await p.textContent('.ws-meta')).replace(/\s+/g, ' ')), { N, openBefore });
  if (SHOTS) await p.screenshot({ path: `${SHOTS}/obligations.png` });
  await p.locator('button[data-decision=confirmed]').first().click(); await p.waitForSelector('#obls .field-err');
  check('obligation: decision refused inline without a reason', (await p.locator('button[data-decision=confirmed]').count()) === openBefore);
  await p.locator('input[data-reason]').first().fill('checked against the drawing: this radius is outside');
  await p.locator('button[data-decision=confirmed]').first().click();
  await p.waitForFunction((n) => document.querySelectorAll('button[data-decision=confirmed]').length === n - 1, openBefore);
  const openAfter = await p.locator('button[data-decision=confirmed]').count();
  check('obligation: confirmed with a reason, two remain open', openAfter === 2 && /discharged by decision/i.test(await p.textContent('#obls')));
  await p.locator('input[data-reason]').first().fill('not applicable to this joint; recorded'); await p.locator('button[data-decision=waived]').first().click();
  await p.waitForFunction(() => document.querySelectorAll('button[data-decision=waived]').length === 1);
  check('obligation: waived with a reason, one remains open and the job stays in review', /waived/i.test(await p.textContent('#obls')) && (await p.textContent('.ws-head h1 .tag')).trim() === 'REVIEW REQUIRED' && /1 open scrutiny obligation$/.test((await p.textContent('#sb-obl')).trim()));
  await p.click('#tab-ws-history');
  check('history: decisions listed with who and why', /Obligation #\d+ discharged by decision/.test(await p.textContent('#tp-ws-history')) && /admin: checked against the drawing/.test(await p.textContent('#tp-ws-history')));
  await go(`calc/${A}`, '.ws-banner.bad');
  const withheld = [await p.locator('#dxfseg .tag:text-is("DXF withheld")').count()]; await p.keyboard.press('2'); withheld.push(await p.locator('#dxfseg .tag:text-is("DXF withheld")').count());
  check('superseded job: marked, DXF withheld for both parts, cannot be corrected again', /SUPERSEDED by job #/.test(await p.textContent('.ws-banner')) && withheld.join() === '1,1' && (await p.locator('#dxf').count()) === 0 && (await p.locator('#corr').count()) === 0, withheld);
  check('superseded job: server refuses the DXF', (await ctx.request.get(`${B}/api/calculations/${A}/dxf/header`, { headers: { 'X-NF-Request': '1' } })).status() === 409);

  // ---- 9. lessons and advisories
  await go('verification');
  const ver = (await p.textContent('#view')).replace(/\s+/g, ' ');
  check('lessons: the correction is recorded as a lesson', /header_radius: 150 was wrong, 142 is right/.test(ver) && /Open scrutiny obligations ?1/.test(ver), ver.slice(0, 200));
  await go('math/1', '#calc'); await p.selectOption('select[name=fam]', 'header_with_branch'); await p.selectOption('select[name=surface_reference]', 'outside'); await p.fill('#f_header_length', '900'); await p.fill('#f_branch_station', '450');
  await p.click('#run'); await p.waitForURL(/#\/calc\/\d+$/); await p.waitForSelector('.pv .c', { state: 'attached' });
  await p.click('#tab-ws-scrutiny');
  check('advisory: repeating the corrected value raises an open advisory', /header_radius = 150 repeats a value that was corrected/.test(await p.textContent('#tp-ws-scrutiny')) && (await p.locator('[data-ack]').count()) === 1);
  await p.click('[data-ack]'); await p.waitForSelector('#tp-ws-scrutiny .field-err');
  await p.fill('[data-advreason]', 'different header: this one is dimensioned outside'); await p.click('[data-ack]');
  await p.waitForFunction(() => document.querySelectorAll('[data-ack]').length === 0);
  check('advisory: acknowledged only with a reason; the value was not changed', /acknowledged/i.test(await p.textContent('#tp-ws-scrutiny')) && /150/.test(await p.textContent('.pane .kv')));
  await go('amoeba/lessons', '#tp-am-lessons:not([hidden])');
  check('AMOEBA: lessons view counts the match', /header_radius/.test(await p.textContent('#tp-am-lessons')) && (await p.textContent('#tp-am-lessons tbody td.num')).trim() === '1');
  await p.click('#tab-am-obligations');
  check('AMOEBA: obligation ledger lists every obligation with its state', (await p.locator('#tbl-obl tbody tr').count()) >= 9 && (await p.locator('#tbl-obl .tag:text-is("open")').count()) === 1);
  await p.fill('#flt-tbl-obl', 'waived'); check('tables: filter narrows the ledger', (await p.locator('#tbl-obl tbody tr:visible').count()) === 1);
  await p.click('#tab-am-records'); const recKinds = await p.locator('#tbl-rec tbody tr').count();
  await p.click('#tab-am-dependencies'); await p.waitForSelector('#tp-am-dependencies .dg');
  await p.click('#tab-am-mechanisms');
  check('AMOEBA: records, dependency graph and mechanism lists present', recKinds >= 15 && /engineer approval/.test(await p.textContent('#tp-am-mechanisms')) && /Not implemented/.test(await p.textContent('#tp-am-mechanisms')));

  // ---- 10. administration: users, audit chain
  await go('admin', '#nu');
  for (const [u, pw, r] of [['viewer1', 'read-only-pass', 'viewer'], ['engineer1', 'engineer-pass-1', 'engineer'], ['temp1', 'temporary-pass', 'viewer']]) {
    await p.fill('#nuu', u); await p.fill('#nup', pw); await p.selectOption('#nur', r); await p.click('#nu button'); await p.waitForSelector(`#tbl-users td:text-is("${u}")`);
  }
  check('admin: users added with roles', (await p.locator('#tbl-users tbody tr').count()) === 4);
  const tempBtn = p.locator('#tbl-users button[data-name=temp1]');
  await tempBtn.click();
  const armed = (await tempBtn.textContent()).trim(), stillActive = (await apiGet('/api/users')).find((u) => u.username === 'temp1').active;
  await tempBtn.click(); await p.waitForSelector('#tbl-users .tag:text-is("deactivated")');
  check('admin: deactivation needs a second, explicit confirmation', armed === 'Confirm: deactivate temp1' && stillActive === true);
  await p.click('#tab-ad-audit'); await p.click('#verify'); await p.waitForSelector('.toast >> text=Audit chain intact');
  check('audit: chain verified INTACT from the UI, log lists the correction', /INTACT/.test(await p.textContent('#chain')) && (await p.locator('#tbl-audit tbody tr').count()) > 10 && /correction/.test(await p.textContent('#tbl-audit')));
  await p.click('#tab-ad-security');
  check('admin: security configuration as running is shown', /password hashing/i.test(await p.textContent('#tp-ad-security')) && /Argon2id/.test(await p.textContent('#tp-ad-security')));
  await go('project/1', '#mem');
  for (const u of ['viewer1', 'engineer1']) { await p.fill('#memu', u); await p.click('#mem button >> nth=0'); await p.waitForSelector(`.panel .tag:text-is("${u}")`); }
  check('project: members added', (await p.locator('.panel .tag:text-is("viewer1")').count()) === 1);

  // ---- 11. shell: shortcuts, help overlay, notifications, theme, navigation rail
  await go('dashboard', '.stats'); await p.locator('#view').focus();
  await p.keyboard.press('?'); await p.waitForSelector('dialog#help[open]');
  const helpText = await p.textContent('#help');
  await p.keyboard.press('Escape'); await p.waitForSelector('dialog#help:not([open])', { state: 'attached' });
  check('help: ? opens the shortcut overlay, Esc closes it', /Keyboard shortcuts/.test(helpText) && /Pattern viewer/.test(helpText) && /New calculation/.test(helpText));
  await p.keyboard.press('g'); await p.keyboard.press('p'); await p.waitForURL(/#\/projects$/);
  await p.keyboard.press('n'); await p.waitForURL(/#\/math/); await p.waitForSelector('#calc');
  check('shortcuts: G P goes to projects, N starts a calculation', true);
  await p.locator('#view').focus(); const rail0 = await p.getAttribute('.app', 'data-rail'); await p.keyboard.press('['); const rail1 = await p.getAttribute('.app', 'data-rail'); await p.keyboard.press('[');
  check('shell: [ toggles navigation labels', rail0 !== rail1 && (await p.getAttribute('.app', 'data-rail')) === rail0);
  await p.click('#bell'); await p.waitForSelector('#notes:not([hidden])');
  check('notifications: session log lists what happened, and says there is no server service', /Correction recorded/.test(await p.textContent('#notes')) && /planned, not implemented/.test(await p.textContent('#notes')));
  await p.keyboard.press('Escape');
  await p.click('#theme'); const light = await p.evaluate(() => [document.documentElement.dataset.theme, getComputedStyle(document.body).backgroundColor]);
  await p.click('#theme'); const dark = await p.evaluate(() => [document.documentElement.dataset.theme, getComputedStyle(document.body).backgroundColor]);
  check('theme: light and dark both apply', light[0] === 'light' && dark[0] === 'dark' && light[1] !== dark[1] && dark[1] === 'rgb(14, 17, 22)', { light, dark });
  await go('workspace'); await p.keyboard.press('/');
  check('shortcuts: / focuses the list filter', await p.evaluate(() => document.activeElement.matches('input[data-filter]')));
  await p.keyboard.press('Escape');

  // ---- 12. every screen: heading, no overflow, named controls — at 1366, 1024 and 768 wide
  const ROUTES = ['dashboard', 'projects', 'project/1', 'workspace', 'math/1', `calc/${J}`, `calc/${A}`, `calc/${N}`, 'amoeba', 'amoeba/obligations', `amoeba/dependencies/${N}`, 'manufacturing', 'verification', 'admin', 'admin/audit', 'account'];
  const EXPECT = { dashboard: 'Dashboard', projects: 'Projects & Documents', workspace: 'Engineering Workspace', 'math/1': 'New calculation', amoeba: 'AMOEBA Control Centre', manufacturing: 'Manufacturing', verification: 'Verification & Quality', admin: 'Administration', account: 'Account' };
  for (const [w, h] of [[1366, 768], [1024, 768], [768, 1024]]) {
    await p.setViewportSize({ width: w, height: h });
    const over = {}, unnamed = {}, titles = {}; let unreachable = 0;
    for (const r of ROUTES) {
      await go(r); if (r.startsWith('calc/')) await p.waitForSelector('.pv', { state: 'attached' }); await p.waitForTimeout(120);
      titles[r] = (await p.textContent('#view h1')).trim();
      const o = await p.evaluate(overflowProbe); if (o.length) over[r] = o;
      const c = await sweepControls(); if (c.unnamed.length) unnamed[r] = c.unnamed;
      if (r.startsWith('calc/')) { // every workspace region must be reachable: on screen, or by scrolling the page or its panel
        for (const sel of ['#replay', '#tab-ws-records', '.pv', '.pane .kv']) { if (await p.locator(sel).count()) { await p.locator(sel).first().scrollIntoViewIfNeeded(); const bb = await p.locator(sel).first().boundingBox(); if (!bb || bb.x < -1 || bb.x + bb.width > w + 1 || bb.y < 0 || bb.y > h) unreachable++; } }
      }
    }
    check(`layout ${w}x${h}: no horizontal overflow on any of ${ROUTES.length} screens`, Object.keys(over).length === 0, over);
    check(`layout ${w}x${h}: every visible control has an accessible name`, Object.keys(unnamed).length === 0, unnamed);
    check(`layout ${w}x${h}: workspace controls reachable`, unreachable === 0, unreachable);
    if (w === 1366) check('screens: every route renders its heading', Object.entries(EXPECT).every(([r, t]) => titles[r] === t) && titles[`calc/${J}`].startsWith(`Job #${J}`), titles);
    if (SHOTS) { await go(`calc/${N}`, '.pv'); await p.screenshot({ path: `${SHOTS}/workspace-${w}.png` }); }
  }
  await p.setViewportSize({ width: 1366, height: 768 });
  await go(`calc/${N}`, '.pv');
  const paneW = (await p.locator('.ws-body > .pane').first().boundingBox()).width; const sp = await p.locator('.split[data-split=l]').boundingBox();
  await p.mouse.move(sp.x + 2, sp.y + 200); await p.mouse.down(); await p.mouse.move(sp.x + 62, sp.y + 200, { steps: 3 }); await p.mouse.up();
  const paneW2 = (await p.locator('.ws-body > .pane').first().boundingBox()).width;
  await p.focus('.split[data-split=l]'); await p.keyboard.press('ArrowLeft');
  const paneW3 = (await p.locator('.ws-body > .pane').first().boundingBox()).width;
  check('workspace: panels resize by drag and by keyboard', Math.abs(paneW2 - paneW - 60) <= 2 && Math.abs(paneW3 - paneW2 + 24) <= 2, { paneW, paneW2, paneW3 });
  await p.locator('.split[data-split=l]').dblclick();

  // ---- 13. role restrictions (API sign-in first, so that no page ever provokes a refused request)
  const role = async (cred) => { const c = await browser.newContext({ viewport: { width: 1366, height: 768 } }); await c.request.post(`${B}/api/login`, { headers: { 'X-NF-Request': '1' }, data: cred }); const pg = await c.newPage(); watch(pg, cred.username); return [c, pg]; };
  const [vctx, v] = await role({ username: 'viewer1', password: 'read-only-pass' });
  await v.goto(`${B}/#/dashboard`); await v.waitForSelector('.stats');
  const vDash = { newCalc: await v.locator('#view a:has-text("New calculation")').count(), adminNav: await v.locator('.rail a[data-nav=admin]').count() };
  await v.goto(`${B}/#/math/1`); await v.waitForSelector('#view h1'); const vMath = await v.textContent('#view');
  await v.goto(`${B}/#/calc/${N}`); await v.waitForSelector('.pv .c', { state: 'attached' }); await v.click('#tab-ws-scrutiny');
  const vJob = { replay: await v.locator('#replay').count(), corr: await v.locator('#corr').count(), resolve: await v.locator('[data-resolve]').count(), obligations: await v.locator('#obls > li').count(), dxf: await v.locator('#dxf').count() };
  await v.goto(`${B}/#/admin`); await v.waitForSelector('#view h1'); const vAdmin = await v.textContent('#view');
  await v.goto(`${B}/#/projects`); await v.waitForSelector('#view h1'); const vProj = await v.locator('#np').count();
  check('viewer: can read results, but sees no way to calculate, correct, decide, replay or administer', vDash.newCalc === 0 && vDash.adminNav === 0 && /can view results but not run calculations/.test(vMath) && vJob.replay === 0 && vJob.corr === 0 && vJob.resolve === 0 && vJob.obligations >= 9 && vJob.dxf === 1 && /Administrator role required/.test(vAdmin) && vProj === 0, { vDash, vJob, vProj });
  const post = (c, path, data) => c.request.post(B + path, { headers: { 'X-NF-Request': '1' }, data });
  check('viewer: server refuses calculation, decision and audit log', (await post(vctx, '/api/projects/1/calculations', { family: 'cylinder_shell', r_dev: 1, length: 1 })).status() === 403 && (await post(vctx, '/api/obligations/1/resolve', { decision: 'waived', reason: 'viewer attempt' })).status() === 403 && (await vctx.request.get(`${B}/api/audit`, { headers: { 'X-NF-Request': '1' } })).status() === 403);
  await vctx.close();
  const [ectx, e] = await role({ username: 'engineer1', password: 'engineer-pass-1' });
  await e.goto(`${B}/#/math/1`); await e.waitForSelector('#view h1'); const eCalc = await e.locator('#calc').count();
  await e.goto(`${B}/#/calc/${N}`); await e.waitForSelector('.pv .c', { state: 'attached' }); const eReplay = await e.locator('#replay').count();
  await e.goto(`${B}/#/admin`); await e.waitForSelector('#view h1');
  check('engineer: can calculate and audit, cannot administer', eCalc === 1 && eReplay === 1 && /Administrator role required/.test(await e.textContent('#view')) && (await e.locator('.rail a[data-nav=admin]').count()) === 0 && (await ectx.request.get(`${B}/api/users`, { headers: { 'X-NF-Request': '1' } })).status() === 403);
  await ectx.close();
  check('deactivated user cannot sign in', (await (await browser.newContext()).request.post(`${B}/api/login`, { headers: { 'X-NF-Request': '1' }, data: { username: 'temp1', password: 'temporary-pass' } })).status() === 401);

  // ---- 14. password change (signs out every session), then sign in again
  await go('account', '#pw');
  await p.fill('#pwc', ADMIN.password); await p.fill('#pwn', NEW_PW); await p.fill('#pwn2', 'something-else-10'); await p.click('#pw button'); await p.waitForSelector('#pw .field-err');
  check('password: mismatch refused inline', /differ/.test(await p.textContent('#pw .field-err')));
  await p.fill('#pwn2', NEW_PW); await p.click('#pw button'); await p.waitForSelector('#f');
  check('password: changed, session ended, sign-in screen says so', /Password changed/.test(await p.textContent('.login-card')) && (await ctx.request.get(`${B}/api/me`, { headers: { 'X-NF-Request': '1' } })).status() === 401);
  allow401 = true; // the one refused request this test provokes on purpose
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', ADMIN.password); await p.click('#f button'); await p.waitForFunction(() => { const a = document.querySelector('.login-card [role=alert]'); return a && !/Password changed/.test(a.textContent); });
  const refused = await p.textContent('.login-card [role=alert]'); allow401 = false;
  check('login: old password refused with a message', refused.trim().length > 0 && (await p.locator('#f').count()) === 1 && expected401 === 1, { refused, expected401 });
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', NEW_PW); await p.click('#f button'); await p.waitForSelector('#view h1');
  check('login: new password signs in and returns to the page the user was on', (await p.textContent('#view h1')).trim() === 'Account' && (await p.textContent('.who')).includes('admin'));
  await p.click('#logout'); await p.waitForSelector('#f');
  check('logout: returns to sign-in and the session is invalid', (await ctx.request.get(`${B}/api/projects`, { headers: { 'X-NF-Request': '1' } })).status() === 401);

  // ---- 15. global assertions
  const xss = await p.evaluate(() => window.__xss || 0);
  check('security: injected markup never executed, no blocking dialog used', xss === 0 && dialogs === 0, { xss, dialogs });
  check('integrity: no control anywhere approves or releases a result; no job shown as APPROVED', approveControls === 0 && approvedTags === 0, { approveControls, approvedTags });
  check('console: zero script errors and zero unexpected console errors', errs.length === 0, errs.slice(0, 5));

  await browser.close();
  const failed = results.filter((r) => !r.ok);
  console.log('----'); console.log(`passed: ${results.length - failed.length}  failed: ${failed.length}`);
  process.exit(failed.length ? 1 : 0);
})().catch((e) => { console.error('UI TEST FAILED', e.message); process.exit(1); });
