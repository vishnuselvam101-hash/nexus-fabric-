// Browser test of every critical workflow. Requires a freshly started server with an EMPTY data directory
// (it performs first-run setup). The server must be started with the same one-time setup token this test uses:
//   NF_SETUP_TOKEN=ui-smoke-setup-token NF_DATA_DIR=$(mktemp -d) NF_LISTEN=127.0.0.1:8793 target/release/nf-server &
// Usage: NF_BASE=http://127.0.0.1:8793 [NF_SETUP_TOKEN=...] node tests/ui_smoke.js [screenshot-dir]
// Exits 0 only if every check passes. Prints each check and the exact pass/fail counts.
const { chromium } = require('playwright');
const fs = require('fs');

const B = process.env.NF_BASE || 'http://127.0.0.1:8793';
const SHOTS = process.argv[2];
const SETUP_TOKEN = process.env.NF_SETUP_TOKEN || 'ui-smoke-setup-token';
const ADMIN = { username: 'admin', password: 'correct-horse-9' }, NEW_PW = 'correct-horse-10';
const results = [];
const check = (name, ok, detail) => { results.push({ name, ok: !!ok, detail }); console.log(`${ok ? 'ok  ' : 'FAIL'} - ${name}${ok || detail === undefined ? '' : `  [${JSON.stringify(detail)}]`}`); };
// The client's ONE display rule for computed numbers, in panels, on the canvas and in the read-out:
// three decimals with thousands separators (round 3; the read-out used to omit the separators).
const n3 = (n) => new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 }).format(Math.abs(n) < 5e-4 ? 0 : n);
const mm = (text) => parseFloat(String(text).replace(/,/g, '')); // a displayed length back to a number

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
  const errs = []; let expected401 = 0, allow401 = false, expected422 = 0, allow422 = false, dialogs = 0, approveControls = 0, approvedTags = 0;
  const watch = (page, who) => {
    page.on('pageerror', (e) => errs.push(`${who} pageerror: ${e.message}`));
    page.on('console', (m) => { if (m.type() !== 'error') return; if (allow401 && /401/.test(m.text())) expected401++; else if (allow422 && /422/.test(m.text())) expected422++; else errs.push(`${who} console: ${m.text()}`); });
    page.on('dialog', (d) => { dialogs++; d.dismiss(); }); // the client must never use blocking dialogs; one would also reveal script injection
  };
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 }, acceptDownloads: true, deviceScaleFactor: SHOTS ? 2 : 1 });
  const p = await ctx.newPage(); watch(p, 'admin');
  const apiGet = async (path) => (await ctx.request.get(B + path, { headers: { 'X-NF-Request': '1' } })).json();
  // navigation is complete when the client marks the view with the hash it rendered
  const go = async (hash, sel) => { await p.goto(`${B}/#/${hash}`); await p.waitForFunction((h) => { const v = document.querySelector('#view'); return v && v.dataset.hash === h; }, `#/${hash}`); await p.waitForSelector(sel || '#view h1'); };
  const sweepControls = async () => { const c = await p.evaluate(controlProbe); approveControls += c.approve; approvedTags += c.approvedTags; return c; };
  const vpos = (ci, vi) => p.evaluate(([c, v]) => { const pl = document.querySelector(`.pv .hit[data-ci="${c}"]`), q = pl.points[v], m = pl.getScreenCTM(); return [q.x * m.a + q.y * m.c + m.e, q.x * m.b + q.y * m.d + m.f]; }, [ci, vi]);
  const transform = () => p.evaluate(() => document.querySelector('.pv g[transform]').getAttribute('transform'));
  const zoom = async () => parseInt(await p.textContent('#zoomv'), 10);

  // ---- 1. first-run setup and sign-in
  await p.goto(B);
  await p.waitForSelector('#f');
  check('setup: first-run screen asks for the administrator', /Create the administrator/.test(await p.textContent('h1')));
  check('setup: sign-in screen carries the development-build notice', /Not validated/.test(await p.textContent('.login-card')));
  // V-36: the first administrator can only be created with the installation's one-time setup token
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', ADMIN.password); await p.click('#f button'); await p.waitForSelector('#f .field-err');
  check('setup: refused inline without the setup token', /setup token/i.test(await p.textContent('#f .field-err')) && (await (await ctx.request.get(`${B}/api/health`)).json()).setup_required === true);
  await p.fill('input[name=t]', SETUP_TOKEN); await p.click('#f button');
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
    check(`calculation ${fam}: marking side stated as a provisional convention, errors labelled as estimates`, (await p.locator('#mside').count()) === 1 && /seen from the outside/.test(await p.textContent('#mside')) && /provisional convention, not reviewed/.test(await p.textContent('#quant')) && /estimate, not a bound/.test(await p.textContent('#quant')) && /PROVISIONAL convention/.test(await p.textContent('#tp-ws-gates')));
    check(`calculation ${fam}: approval gate G16 shown as NOT IMPLEMENTED`, /G16\s*human approval\s*NOT IMPLEMENTED/.test((await p.textContent('#tp-ws-gates')).replace(/\s+/g, ' ')));
    await sweepControls();
  }

  // ---- 3b. a refusal must look like a refusal everywhere
  const jobCount = async () => (await apiGet('/api/calculations')).length;
  const before = await jobCount();
  await go('math/1', '#calc'); await p.selectOption('select[name=fam]', 'header_with_branch');
  check('new calculation: parameter schematic is drawn from the form values', /R 150/.test(await p.textContent('#sketch svg')) && /station 600/.test(await p.textContent('#sketch svg')) && /not the computed pattern/.test(await p.textContent('#sketch')));
  await p.fill('#f_branch_station', '450'); await p.focus('#f_beta_deg');
  check('new calculation: schematic follows the entered values and highlights the focused dimension', /station 450/.test(await p.textContent('#sketch svg')) && (await p.locator('#sketch .hot[data-dim=beta_deg]').count()) === 1);
  check('new calculation: run button stays on screen', await p.evaluate(() => { const r = document.querySelector('#run').getBoundingClientRect(); return r.top >= 0 && r.bottom <= window.innerHeight; }));
  await p.fill('#f_header_radius', '-150'); await p.click('#run'); await p.waitForSelector('#calc .field-err');
  const negMsg = await p.textContent('#calc .field-err');
  await p.fill('#f_header_radius', '150'); await p.fill('#f_branch_radius', '400'); await p.click('#run'); await p.waitForSelector('#calc .field-err');
  const bigMsg = await p.textContent('#calc .field-err');
  check('refusal: negative radius and impossible intersection are stopped at the field, nothing is sent', /must be greater than 0/.test(negMsg) && /must be smaller than the header radius/.test(bigMsg) && /Not sent/.test(await p.textContent('#cmsg')) && (await jobCount()) === before && p.url().includes('#/math'), { negMsg, bigMsg });
  await p.selectOption('select[name=fam]', 'cylinder_shell'); await p.fill('#f_r_dev', '1e9'); await p.click('#run'); await p.waitForSelector('#plaus');
  check('refusal: an absurd magnitude is questioned before anything is sent', /more than 20 m/.test(await p.textContent('#plaus')) && (await jobCount()) === before && /unusual/i.test(await p.textContent('#run')));
  // a request the client cannot judge and the engine refuses (square end inside the saddle): recorded, but as NO RESULT
  // BE-003: the server answers a refused request with 422 (it used to answer 200). That one answer is expected here.
  allow422 = true;
  await p.selectOption('select[name=fam]', 'header_with_branch'); await p.fill('#f_s_end', '150'); await p.click('#run');
  await p.waitForURL(/#\/calc\/\d+$/); await p.waitForSelector('.ws-banner.bad'); allow422 = false;
  check('refusal: the server answered the refused request with 422, once', expected422 === 1, expected422);
  const R = Number(p.url().split('/').pop());
  const rj = { tag: (await p.textContent('.ws-head h1 .tag')).trim(), toastErr: await p.locator('.toast.err').count(), toastOk: await p.locator('.toast.ok').count(), toast: await p.locator('.toast').first().textContent(), banner: await p.textContent('.ws-banner'), dxf: await p.locator('#dxf, #dxf-all, a[href*="/dxf/"]').count(), replay: await p.locator('#replay').count(), corr: await p.locator('#corr-open, [data-correct]').count(), pv: await p.locator('.pv').count() };
  check('refusal: an engine-refused job is shown as NO RESULT with an error notification, never as a computed result', rj.tag === 'No result' && rj.toastErr === 1 && rj.toastOk === 0 && /NO RESULT/.test(rj.toast) && /NO RESULT/.test(rj.banner) && /s_end/.test(rj.banner) && !/REVIEW REQUIRED/.test(await p.textContent('.ws')), rj);
  check('refusal: no DXF, replay or correction is offered on a job without a result', rj.dxf === 0 && rj.replay === 0 && rj.corr === 0 && rj.pv === 0, rj);
  await go('dashboard', '.stats');
  const dashR = { none: (await p.textContent('#st-none b')).trim(), review: (await p.textContent('#st-review b')).trim(), row: (await p.locator('#jobs-recent tbody tr', { hasText: `Job #${R}` }).textContent()) };
  check('refusal: dashboard counts it as no result, not as review required', dashR.none === '1' && dashR.review === String(before) && /No result/.test(dashR.row) && /input refused/.test(dashR.row) && !/Review required/i.test(dashR.row), dashR);
  await go('manufacturing'); const mfg = await p.textContent('#view');
  await go('workspace'); const wsRow = await p.locator('#jobs-all tbody tr', { hasText: `Job #${R}` }).textContent();
  await go('verification'); const verRow = await p.locator('#jobs-ver tbody tr', { hasText: `Job #${R}` }).textContent();
  check('refusal: absent from DXF outputs, labelled NO RESULT in the job lists', (await (async () => { await go('manufacturing'); return p.locator('#tbl-dxf tbody tr', { hasText: `Job #${R}` }).count(); })()) === 0 && /1 job produced no pattern/.test(mfg) && /No result/.test(wsRow) && /No result/.test(verRow) && /none run/.test(verRow), { wsRow, verRow });
  check('manufacturing: one row per part with extents, hash and download', (await p.locator('#tbl-dxf tbody tr').count()) === 6 && (await p.locator('#tbl-dxf a[href*="/dxf/"]').count()) === 6 && /CUT_OUTER/.test(mfg));

  // ---- 4. pattern viewer on the eccentric reducer: real geometry read-out, selection, zoom, pan, fit, measure
  const J = jobs.eccentric_reducer;
  const data = await apiGet(`/api/calculations/${J}`);
  const curve0 = data.patterns[0].curves[0], v100 = curve0.points[100];
  await go(`calc/${J}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' });
  let [x, y] = await vpos(0, 100); await p.mouse.move(x - 30, y - 30); await p.mouse.move(x, y);
  await p.waitForFunction((n) => document.querySelector('#ro-info').textContent.includes(n), curve0.name);
  const ro = await p.textContent('#ro-info');
  check('viewer: hover read-out names the curve, its computed length and the real vertex coordinates', ro.includes(curve0.name) && ro.includes(`length ${n3(curve0.length)} mm`) && ro.includes(`vertex 101/${curve0.points.length} (${n3(v100[0])}, ${n3(v100[1])})`), ro);
  check('viewer: cursor coordinates are pattern coordinates in mm', Math.abs(mm(await p.textContent('#ro-x')) - v100[0]) < 2 && Math.abs(mm(await p.textContent('#ro-y')) - v100[1]) < 2 && / mm$/.test(await p.textContent('#ro-x')));
  await p.mouse.click(x, y);
  const selText = await p.textContent('#sel');
  check('viewer: clicking a curve selects it and shows its properties from the result data', selText.includes(curve0.name) && selText.includes(`${n3(curve0.length)} mm`) && (await p.getAttribute('#sel dd span[title]', 'title')).startsWith(String(Number(curve0.length.toPrecision(12)))) && selText.includes(String(curve0.points.length)) && (await p.getAttribute('#curves tr[data-ci="0"]', 'aria-selected')) === 'true', selText);
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
  check('viewer: measuring between two computed vertices gives the engine length of that line', meas.startsWith(`Distance ${n3(seam.length)} mm`) && (await p.textContent('#meas')).includes(`${n3(seam.length)} mm`) && /vertex/.test(await p.textContent('#meas')), meas);
  await p.keyboard.press('Escape'); await p.keyboard.press('Escape');
  const quant = (await p.textContent('#quant')).replace(/\s+/g, ' ');
  check('quantities: every value has a unit, angles in degrees, one precision', new RegExp(`Developed angle ?developed_angle ?${n3(data.patterns[0].quantities.developed_angle * 180 / Math.PI)} ° \\(${n3(data.patterns[0].quantities.developed_angle)} rad\\)`).test(quant) && quant.includes(`${n3(data.patterns[0].quantities.area)} mm²`) && !/unit not stated/.test(quant), quant);
  check('viewer: canvas exposes an accessible description', /Flat pattern of reducer: 4 curves, extents/.test(await p.getAttribute('.pv', 'aria-label')));

  // ---- 5. DXF download
  const [dl] = await Promise.all([p.waitForEvent('download'), p.click('#dxf')]);
  const dxf = fs.readFileSync(await dl.path(), 'utf8');
  check('DXF: downloads, file name marked NOT-RELEASED, content ends with EOF', dl.suggestedFilename().startsWith('NOT-RELEASED_REVIEW_REQUIRED') && dxf.trimEnd().endsWith('EOF') && dxf.length > 1000, dl.suggestedFilename());

  // ---- 6. replay audit
  await p.click('#replay'); await p.waitForSelector('#tp-ws-records:not([hidden]) >> text=stored records checked');
  check('replay: audit reproduces the closure and is reported with its limitation', /Replay match/i.test(await p.textContent('#replay-state')) && /Replay match/i.test(await p.locator('.toast').first().textContent()) && /same machine/.test(await p.textContent('#tp-ws-records')));
  await p.click('#tab-ws-history');
  check('history: replay audit appears in the job history', /Replay match/.test(await p.textContent('#tp-ws-history')));

  // ---- 7. assembly: parts, interface checks, dependency graph
  const A = jobs.header_with_branch;
  await go(`calc/${A}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' });
  const headerCurves = await p.locator('.pv .c').count(), innerCurves = await p.locator('.pv .c.inner').count();
  await p.keyboard.press('2'); await p.waitForFunction(() => document.querySelector('#curves-part').textContent.includes('branch'));
  const [d1, d2] = await (async () => { const got = []; const on = (d) => got.push(d.suggestedFilename()); p.on('download', on); await p.click('#dxf-all'); await p.waitForFunction(() => /2 DXF files requested/.test(document.querySelector('#toasts').textContent)); await p.waitForTimeout(300); p.off('download', on); return got.sort(); })();
  check('DXF: all parts of an assembly download in one action, each marked NOT-RELEASED', /^NOT-RELEASED.*_branch_/.test(d1 || '') && /^NOT-RELEASED.*_header_/.test(d2 || ''), [d1, d2]);
  check('assembly: header shows its opening, key 2 switches to the branch pattern', headerCurves === 5 && innerCurves === 1 && (await p.locator('.pv .c').count()) === 4 && /branch/.test(await p.getAttribute('#dxf', 'href')));
  await p.click('#tab-ws-checks'); const interfaceChecks = await p.locator('#tp-ws-checks .id:text-is("interface")').count();
  await p.click('#tab-ws-records'); const interfaceRecords = await p.locator('#tp-ws-records .rec-t', { hasText: /^interface ·/ }).count();
  check('assembly: five interface checks (three fold-back: the two 3D cross-checks and the common marking side; two independent 3D point checks of the joint, review RB-001) and the interface record', interfaceChecks === 5 && interfaceRecords === 1, { interfaceChecks, interfaceRecords });
  await p.click('#tp-ws-records .rec >> nth=0'); await p.waitForSelector('#tp-ws-records .rec[open] pre');
  check('records: a record opens to its stored payload; rows name their subject', /header_with_branch/.test(await p.textContent('#tp-ws-records .rec[open] pre')) && (await p.locator('#tp-ws-records .rec-t', { hasText: 'parameter · header_radius = 150 mm' }).count()) === 1);
  await p.click('#tab-ws-graph'); const nodes = await p.locator('#tp-ws-graph .dgl .node').count(), edges = await p.locator('#tp-ws-graph details tbody tr').count();
  const clipped = await p.evaluate(() => { const pane = document.querySelector('#tp-ws-graph').getBoundingClientRect(); return [...document.querySelectorAll('#tp-ws-graph .dgl .node')].filter((n) => { const r = n.getBoundingClientRect(); return r.right > pane.right + 1 || r.left < pane.left - 1; }).length; });
  await p.locator('#tp-ws-graph .dgl .node', { hasText: 'header_radius' }).first().click();
  const reach = await p.textContent('#tp-ws-graph .dg-info');
  check('dependencies: layers built from the edge list, nothing clipped, selecting a record traces what a change reaches', nodes >= 15 && edges >= 20 && clipped === 0 && /header_radius depends on 1 record\. A change to it reaches \d+:.*pattern · header/.test(reach), { nodes, edges, reach });

  // ---- 8. correction -> obligations -> resolution
  await p.click('[data-correct=header_radius]'); await p.waitForSelector('#corrdlg[open]');
  check('correction: opens from the request row with that parameter selected, labelled in words', (await p.inputValue('#corr select[name=p]')) === 'header_radius' && /Header contact radius — header_radius \(now 150 mm\)/.test(await p.textContent('#corr select[name=p]')));
  await p.fill('#corr input[name=v]', '-5'); await p.click('#corr-go'); await p.waitForSelector('#corr .field-err');
  const corrNeg = await p.textContent('#corr .field-err');
  await p.fill('#corr input[name=v]', '158'); await p.click('#corr-go'); await p.waitForSelector('#corr .field-err');
  check('correction: an impossible value and a missing reason are refused inline', /greater than 0/.test(corrNeg) && /reason/i.test(await p.textContent('#corr .field-err')) && p.url().endsWith(`/calc/${A}`), corrNeg);
  await p.fill('#corr input[name=r]', 'drawing dimension is the outside diameter of the thicker pipe'); await p.click('#corr-go');
  await p.waitForURL((u) => /#\/calc\/\d+$/.test(u.hash) && !u.hash.endsWith(`/${A}`)); await p.waitForSelector('#tp-ws-changes:not([hidden]) table');
  const diff = (await p.textContent('#tp-ws-changes')).replace(/\s+/g, ' ');
  check('correction: the new job opens on what changed — parameter before and after, and the computed values that moved', /Header contact radius ?header_radius ?150\.000 ?158\.000 ?\+?8\.000 mm/.test(diff) && /Curve branch_opening length/.test(diff) && /Pattern · branch/.test(diff) && (await p.locator('#tp-ws-changes [data-diff^="curve:"]').count()) >= 2 && (await p.locator('#corrects').getAttribute('href')) === `#/calc/${A}`, diff.slice(0, 400));
  await p.click('#v-prev'); const ghosts = await p.locator('.pv .ghost').count(); await p.click('#v-prev');
  check('correction: the earlier pattern can be overlaid on the canvas', ghosts === 5 && (await p.locator('.pv .ghost').count()) === 0, ghosts);
  await p.click('#tab-ws-scrutiny'); await p.waitForSelector('#obls');
  const N = Number(p.url().split('/').pop());
  const openBefore = await p.locator('button[data-decision=confirmed]').count();
  check('correction: new job opens on its scrutiny obligations, three need an engineer', N > A && openBefore === 3 && (await p.locator('#obls > li').count()) >= 9 && (await p.textContent('#corrects')).trim() === `corrects #${A}`, { N, openBefore });
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
  check('superseded job: marked, DXF withheld for both parts', /SUPERSEDED by job #/.test(await p.textContent('.ws-banner')) && withheld.join() === '1,1' && (await p.locator('#dxf, #dxf-all').count()) === 0, withheld);
  const sup = { tag: (await p.textContent('.ws-head h1 .tag')).trim(), wm: (await p.textContent('#wm')).trim(), dead: await p.locator('.stage.dead').count(), opacity: await p.evaluate(() => getComputedStyle(document.querySelector('.pv .c')).opacity), succ: await p.getAttribute('#successor', 'href'), corr: await p.locator('#corr-open, [data-correct], #corrdlg').count(), replay: await p.locator('#replay').count(), text: await p.textContent('.ws') };
  check('superseded job: unmistakable — own watermark, dimmed pattern, link to its successor, no replay or correction', sup.tag === 'Superseded' && /^Superseded — do not use$/i.test(sup.wm) && !/REVIEW REQUIRED/i.test(sup.text) && sup.dead === 1 && Number(sup.opacity) <= 0.5 && sup.succ === `#/calc/${N}` && sup.corr === 0 && sup.replay === 0, { ...sup, text: undefined });
  await go('dashboard', '.stats');
  const dash = { sup: (await p.textContent('#st-sup b')).trim(), review: (await p.textContent('#st-review b')).trim(), none: (await p.textContent('#st-none b')).trim(), legend: (await p.textContent('#legend')).replace(/\s+/g, ' '), rowsSup: await p.locator('#jobs-recent .tag:text-is("Superseded")').count(), rowsReview: await p.locator('#jobs-recent .tag:text-is("REVIEW REQUIRED")').count(), total: (await apiGet('/api/calculations')).length };
  check('dashboard: counts agree with the rows after a correction (superseded is not review required)', dash.sup === '1' && dash.rowsSup === 1 && dash.none === '1' && Number(dash.review) === dash.total - 2 && dash.rowsReview === Number(dash.review) && dash.legend.includes(`${dash.review} review required`) && dash.legend.includes('1 superseded'), dash);
  await go(`calc/${A}`, '.ws-banner.bad');
  check('superseded job: server refuses the DXF', (await ctx.request.get(`${B}/api/calculations/${A}/dxf/header`, { headers: { 'X-NF-Request': '1' } })).status() === 409);

  // ---- 9. lessons and advisories
  await go('verification');
  const ver = (await p.textContent('#view')).replace(/\s+/g, ' ');
  check('lessons: the correction is recorded as a lesson', /header_radius: 150 was wrong, 158 is right/.test(ver) && /Open scrutiny obligations ?1/.test(ver), ver.slice(0, 200));
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
  await p.fill('#flt-tbl-obl', 'waived'); check('tables: filter narrows the ledger and its count', (await p.locator('#tbl-obl tbody tr:visible').count()) === 1 && /^1 of \d+$/.test((await p.textContent('#tp-am-obligations .panel-h .cnt')).trim()));
  await p.fill('#flt-tbl-obl', 'zzzz'); const nomatch = await p.textContent('#tbl-obl tbody tr.nomatch'); await p.click('#tbl-obl tr.nomatch button');
  check('tables: a filter without matches says so and can be cleared', /Nothing matches “zzzz”/.test(nomatch) && (await p.inputValue('#flt-tbl-obl')) === '' && (await p.locator('#tbl-obl tbody tr:visible').count()) >= 9);
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
  for (const u of ['viewer1', 'engineer1']) { await p.selectOption('#memu', u); await p.click('#mem button'); await p.waitForSelector(`.members li:has-text("${u}")`); }
  check('project: members picked from the active users, shown as entered', (await p.locator('.members li', { hasText: 'viewer1' }).count()) === 1 && (await p.locator('#memu option[value=temp1]').count()) === 0);

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
  const vJob = { replay: await v.locator('#replay').count(), corr: await v.locator('#corr-open, [data-correct]').count(), resolve: await v.locator('[data-resolve]').count(), obligations: await v.locator('#obls > li').count(), dxf: await v.locator('#dxf').count() };
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
  check('login: old password refused with a plain message, username kept', refused.trim() === 'Incorrect username or password.' && (await p.inputValue('input[name=u]')) === ADMIN.username && (await p.evaluate(() => document.activeElement.name)) === 'p' && expected401 === 1, { refused, expected401 });
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', NEW_PW); await p.click('#f button'); await p.waitForSelector('#view h1');
  check('login: new password signs in and returns to the page the user was on', (await p.textContent('#view h1')).trim() === 'Account' && (await p.textContent('.who')).includes('admin'));
  await p.click('#logout'); await p.waitForSelector('#f');
  check('logout: returns to sign-in and the session is invalid', (await ctx.request.get(`${B}/api/projects`, { headers: { 'X-NF-Request': '1' } })).status() === 401);

  // ---- 14a. joints (review RB-001; UI_CHANGES items 1-5, 9-12): a declared set-on joint, an undeclared one, a refused pairing
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', NEW_PW); await p.click('#f button'); await p.waitForSelector('#view h1');
  const SO = await (async () => {
    await go('math/1', '#calc'); await p.selectOption('select[name=fam]', 'header_with_branch');
    await p.selectOption('select[name=surface_reference]', 'outside'); await p.selectOption('select[name=branch_surface_reference]', 'inside'); await p.selectOption('select[name=joint]', 'set_on');
    await p.click('#run'); await p.waitForURL(/#\/calc\/\d+$/); await p.waitForSelector('.pv .c', { state: 'attached' });
    return Number(p.url().split('/').pop());
  })();
  const jointDd = async () => p.locator('#joint dd').allTextContents();
  await go(`calc/${SO}`, '.pv'); await p.waitForSelector('#joint');
  const soDd = (await jointDd()).map((x) => x.trim());
  check('joint: a declared set-on job shows the declared joint and both of its surfaces', /Set-on/.test(soDd[0] || '') && soDd.includes('outside') && soDd.includes('inside') && /^set-on \(Spec 02 section 4\.4/.test((await p.textContent('#joint p')).trim()), soDd);
  await go(`calc/${A}`, '.pv'); await p.waitForSelector('#joint');
  const undDd = (await jointDd()).map((x) => x.trim());
  const aText = await p.textContent('#view');
  check('joint: an undeclared job says the joint type is not declared and is not called set-on', /single contact curve \(joint type not declared\)/.test((await p.textContent('#joint p')).trim()) && undDd[0] === 'not declared' && !/set-on joint assumed/i.test(aText) && !/set-on/i.test(await p.textContent('#joint p')), { undDd });
  await go('math/1', '#calc'); await p.selectOption('select[name=fam]', 'header_with_branch');
  const beforeRefusal = await jobCount();
  check('form: the optional joint fields are left at their defaults, so the request is the same as before', (await p.inputValue('#f_joint')) === '' && (await p.inputValue('#f_header_surface_reference')) === '' && (await p.inputValue('#f_branch_surface_reference')) === '' && (await p.locator('#calc [aria-invalid="true"]').count()) === 0);
  await p.selectOption('select[name=joint]', 'set_on'); await p.click('#run'); await p.waitForSelector('#f_joint[aria-invalid="true"]');
  const jm = await p.evaluate(() => ({ marked: [...document.querySelectorAll('#calc [aria-invalid="true"]')].map((e) => e.name), msg: (document.querySelector('#f_joint').closest('.field').querySelector('.field-err') || {}).textContent || '' }));
  check('refusal: a set-on pairing that is not outside-header and inside-branch marks the joint field, nothing is sent', jm.marked.join() === 'joint' && /INSIDE/.test(jm.msg) && (await jobCount()) === beforeRefusal, jm);
  // the same pairing refused by the engine: the server names the field, and the refused job marks that row
  const refusedApi = await ctx.request.post(`${B}/api/projects/1/calculations`, { headers: { 'X-NF-Request': '1' }, data: { family: 'header_with_branch', header_radius: 150, header_dev_radius: 150, header_length: 1200, branch_station: 600, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400, surface_reference: 'unknown', joint: 'set_on' } });
  const rfb = await refusedApi.json();
  check('refusal: the engine names the field "joint" for a set-on pairing it does not accept', refusedApi.status() === 422 && rfb.field === 'joint', rfb);
  await go(`calc/${rfb.id}`, '#view h1');
  const rowMark = await p.evaluate(() => { const dd = [...document.querySelectorAll('dl.req dd')].find((d) => d.querySelector('.tag')); return dd ? dd.previousElementSibling.textContent : ''; });
  check('refusal: the refused job marks the request row of the field it names', /joint/.test(rowMark), rowMark);

  await p.click('#logout'); await p.waitForSelector('#f');
  // ---- 14b. routing and Japanese: the switch, its persistence, the URL override, every main screen walked for
  // English words, layout at three widths in both languages, and errors in both languages.
  // The walk accepts only English that is English by design. Before it reads any text it skips:
  //   .mono .idk .hash code kbd pre  identifiers, hashes, file and key names (monospace in the client)
  //   .u bdi                         text a person typed (project, user, file and reason names), shown as written
  //   .u-select option               project and user names listed in a select
  //   [lang=en]                      engine or server text the catalogue deliberately leaves in English (engineJa)
  // and it accepts these words wherever they are: the product name and its AMOEBA module, the file formats (DXF, PDF,
  // PNG, JPEG, TIFF, STEP), the hash algorithm name SHA, and the unit rad.
  const JA_ALLOW = ['NEXUS', 'FABRIC', 'AMOEBA', 'DXF', 'PDF', 'PNG', 'JPEG', 'TIFF', 'STEP', 'SHA', 'rad'];
  const jaWalk = (allow) => {
    const skip = '.mono,.idk,.hash,.u,bdi,code,kbd,pre,[lang=en],.u-select option,script,style,noscript';
    const out = [], w = document.createTreeWalker(document.getElementById('app'), NodeFilter.SHOW_TEXT); let n;
    while ((n = w.nextNode())) {
      const el = n.parentElement; if (!el || el.closest(skip) || !el.getClientRects().length) continue;
      const bad = (n.textContent.match(/[A-Za-z]{3,}/g) || []).filter((x) => !allow.includes(x));
      if (bad.length) out.push(`<${el.tagName.toLowerCase()}> "${n.textContent.trim().slice(0, 50)}" [${bad.join(' ')}]`);
    }
    return out;
  };
  const JA_TEXT = /[぀-ヿ一-鿿]/;
  const jaScreen = async (label) => { const found = await p.evaluate(jaWalk, JA_ALLOW); check(`ja: ${label}: no English word of three or more letters outside the allow-list`, found.length === 0, found.slice(0, 6)); };
  const errsBeforeJa = errs.length;

  // the language switch on the sign-in screen: Japanese at once, the same document (no reload)
  // the page is still on the sign-in screen that the logout above left (a reload would probe the session: a 401)
  await p.waitForSelector('#f');
  await p.evaluate(() => { window.__nfSameDocument = 'yes'; });
  await p.click('#lang-ja');
  check('language: the switch on the sign-in screen shows Japanese without a page reload and sets <html lang>', (await p.evaluate(() => window.__nfSameDocument)) === 'yes' && (await p.getAttribute('html', 'lang')) === 'ja' && JA_TEXT.test(await p.textContent('.login-card h1')), await p.getAttribute('html', 'lang'));
  await jaScreen('sign-in');
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', NEW_PW); await p.click('#f button'); await p.waitForSelector('#view h1');

  // routing: the empty route opens the dashboard (it used to say that the page does not exist)
  await p.goto(`${B}/#/`); await p.waitForSelector('.stats');
  check('routing: the empty route #/ opens the dashboard', (await p.getAttribute('#view', 'data-hash')) === '#/' && (await p.locator('.stats').count()) === 1 && !/does not exist/i.test(await p.textContent('#view')));

  // the choice is kept: after a reload the page is still Japanese, and the switch works in place
  await p.goto(`${B}/#/dashboard`); await p.waitForSelector('.stats');
  await p.evaluate(() => { window.__nfSameDocument = 'yes'; });
  await p.click('#lang-en');
  check('language: a switch on a signed-in screen re-renders in place (no reload) and sets <html lang="en">', (await p.evaluate(() => window.__nfSameDocument)) === 'yes' && (await p.getAttribute('html', 'lang')) === 'en' && (await p.textContent('#view h1')).trim() === 'Dashboard');
  await p.click('#lang-ja');
  await p.reload(); await p.waitForSelector('.stats');
  check('language: the Japanese choice survives a reload', (await p.getAttribute('html', 'lang')) === 'ja' && JA_TEXT.test(await p.textContent('#view h1')));

  // the URL decides for one page load: ?lang=en and ?lang=ja both work
  await p.goto(`${B}/?lang=en#/dashboard`); await p.waitForSelector('.stats');
  check('language: ?lang=en opens the page in English', (await p.getAttribute('html', 'lang')) === 'en' && (await p.textContent('#view h1')).trim() === 'Dashboard');
  await p.goto(`${B}/?lang=ja#/dashboard`); await p.waitForSelector('.stats');
  check('language: ?lang=ja opens the page in Japanese', (await p.getAttribute('html', 'lang')) === 'ja' && JA_TEXT.test(await p.textContent('#view h1')));

  // every main screen, in Japanese
  await jaScreen('dashboard');
  for (const fam of ['cylinder_shell', 'cone_frustum', 'eccentric_reducer', 'header_with_branch', 'branch_stub']) { await go('math/1', '#calc'); await p.selectOption('select[name=fam]', fam); await jaScreen(`new calculation, ${fam}`); }
  // engine sentences shown without their Japanese text (the class .eng.raw is set by the client for an untranslated text)
  const rawEng = () => p.evaluate(() => [...document.querySelectorAll('#joint .eng.raw, #assume .eng.raw, #gates li[data-gate="G02"] .eng.raw')].map((e) => e.textContent.trim().slice(0, 160)));
  // the joint in Japanese: the refused pairing, the declared set-on job and the undeclared one (assumptions opened)
  await go('math/1', '#calc'); await p.selectOption('select[name=fam]', 'header_with_branch'); await p.selectOption('select[name=joint]', 'set_on'); await p.click('#run'); await p.waitForSelector('#f_joint[aria-invalid="true"]');
  const jaMark = await p.evaluate(() => ({ marked: [...document.querySelectorAll('#calc [aria-invalid="true"]')].map((e) => e.name), msg: (document.querySelector('#f_joint').closest('.field').querySelector('.field-err') || {}).textContent || '' }));
  check('ja: the refused set-on pairing marks the joint field in Japanese', jaMark.marked.join() === 'joint' && /セットオン/.test(jaMark.msg), jaMark);
  await jaScreen('new calculation, header with branch, joint field marked');
  await go(`calc/${SO}`, '.pv'); await p.waitForSelector('#joint'); await p.evaluate(() => { document.querySelector('#assume').parentElement.open = true; });
  const soJa = await p.textContent('#joint');
  check('ja: the declared set-on joint is shown in Japanese, with both surfaces', /セットオン/.test(soJa) && /外面/.test(soJa) && /内面/.test(soJa), soJa.slice(0, 120));
  check('ja: no engine sentence of the declared set-on job is untranslated (joint, assumptions, gate G02)', (await rawEng()).length === 0, await rawEng());
  await jaScreen('job view, declared set-on joint');
  await go(`calc/${A}`, '.pv'); await p.waitForSelector('#joint'); await p.evaluate(() => { document.querySelector('#assume').parentElement.open = true; });
  const undJa = await p.textContent('#view');
  check('ja: the undeclared job does not say that set-on is assumed, and says the joint type is not declared', !/セットオン継手を前提/.test(undJa) && /未宣言/.test(await p.textContent('#joint')));
  check('ja: no engine sentence of the undeclared job is untranslated (joint, assumptions, gate G02)', (await rawEng()).length === 0, await rawEng());
  await jaScreen('job view, undeclared joint');
  await go(`calc/${J}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' }); await jaScreen('job view, pattern');
  for (const k of ['gates', 'checks', 'scrutiny', 'graph', 'history', 'records']) { await p.click(`#tab-ws-${k}`); await jaScreen(`job view, ${k} tab`); }
  await go(`calc/${N}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' }); await p.click('#tab-ws-changes'); await jaScreen('job view, changes tab');
  await p.click('#tab-ws-records'); await p.click('#tp-ws-records .rec >> nth=0'); await p.waitForSelector('#tp-ws-records .rec[open] pre'); await jaScreen('job view, a record opened');
  await p.click('#corr-open'); await p.waitForSelector('#corrdlg[open]'); await jaScreen('correction dialog'); await p.keyboard.press('Escape');
  await p.locator('#view').focus(); await p.keyboard.press('?'); await p.waitForSelector('dialog#help[open]'); await jaScreen('keyboard help'); await p.keyboard.press('Escape');
  await go('projects', '#np'); await jaScreen('projects');
  await go('project/1', '#up'); await jaScreen('project');
  await go('amoeba/records', '#tp-am-records:not([hidden])'); await jaScreen('AMOEBA records');
  await go('amoeba/obligations', '#tbl-obl'); await jaScreen('AMOEBA obligations');
  await go('amoeba/lessons', '#tp-am-lessons:not([hidden])'); await jaScreen('AMOEBA lessons');
  await go(`amoeba/dependencies/${N}`, '#tp-am-dependencies .dg'); await jaScreen('AMOEBA dependencies');
  await go('amoeba/mechanisms', '#tp-am-mechanisms:not([hidden])'); await jaScreen('AMOEBA mechanisms');
  await go('workspace', '#jobs-all'); await jaScreen('job list');
  await go('manufacturing', '#tbl-dxf'); await jaScreen('manufacturing');
  await go('verification', '#jobs-ver'); await jaScreen('verification');
  await go('admin', '#nu'); await jaScreen('administration, users');
  await p.click('#tab-ad-security'); await jaScreen('administration, security');
  await p.click('#tab-ad-audit'); await jaScreen('administration, audit');
  await go('account', '#pw'); await jaScreen('account');
  const missingJa = await p.evaluate(() => window.__nfI18nMissing());
  check('ja: no message is missing from the Japanese catalogue on the screens walked', missingJa.length === 0, missingJa.slice(0, 8));

  // layout in Japanese: the same probe as the English screens, at the three widths the client must survive
  for (const [w, h] of [[1366, 768], [1024, 768], [768, 1024]]) {
    await p.setViewportSize({ width: w, height: h });
    const over = {};
    for (const r of ['dashboard', 'math/1', `calc/${J}`]) { await go(r); if (r.startsWith('calc/')) await p.waitForSelector('.pv', { state: 'attached' }); await p.waitForTimeout(120); const o = await p.evaluate(overflowProbe); if (o.length) over[r] = o; }
    check(`layout ja ${w}x${h}: no horizontal overflow on the dashboard, the new-calculation form or the job view`, Object.keys(over).length === 0, over);
  }
  await p.setViewportSize({ width: 1366, height: 768 });
  check('ja: zero script errors and zero console errors in Japanese', errs.length === errsBeforeJa, errs.slice(errsBeforeJa, errsBeforeJa + 5));

  // ---- 14c. a dimension is computed exactly as typed, or refused (bug hunt BH-001: a number field turned "1,5" into 15)
  {
    const count = () => p.evaluate(async () => (await (await fetch('/api/calculations')).json()).length);
    for (const lang of ['en', 'ja']) {
      await p.goto(`${B}/?lang=${lang}#/math/1`); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'cone_frustum');
      const before = await count(), refused = {};
      for (const bad of ['1,5', '0x10', '１２３', '1 000', '1.5.2', '12mm', '']) {
        await p.fill('#f_h', ''); await p.type('#f_h', bad); const shown = await p.inputValue('#f_h');
        await p.click('#run'); await p.waitForTimeout(150);
        refused[bad] = { keptAsTyped: shown === bad, fieldError: await p.locator('#calc .field-err').count() > 0, stillOnForm: await p.locator('#calc').count() === 1 };
      }
      const after = await count();
      check(`dimension ${lang}: a decimal comma, hex, full-width digits, a space, two points, a unit suffix and an empty field are shown as typed, marked, and nothing is sent`,
        after === before && Object.values(refused).every((r) => r.keptAsTyped && r.fieldError && r.stillOnForm), { before, after, refused });
    }
    await p.goto(`${B}/?lang=en#/math/1`); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'cone_frustum');
    await p.fill('#f_h', '1.5'); await p.focus('#f_h'); await p.keyboard.press('ArrowUp'); const up = await p.inputValue('#f_h'); await p.keyboard.press('ArrowDown'); await p.keyboard.press('ArrowDown'); const down = await p.inputValue('#f_h');
    check('dimension: ArrowUp and ArrowDown step the field by one and keep its decimals', up === '2.5' && down === '0.5', { up, down });
    await p.fill('#f_h', '301.25'); await p.click('#run'); await p.waitForFunction(() => /^#\/calc\/\d+/.test(location.hash)); const id = Number(/calc\/(\d+)/.exec(await p.evaluate(() => location.hash))[1]);
    const stored = await p.evaluate(async (i) => (await (await fetch(`/api/calculations/${i}`)).json()).request.h, id);
    check('dimension: the value stored with the job is exactly the value typed (301.25)', stored === 301.25, { stored });
  }

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
