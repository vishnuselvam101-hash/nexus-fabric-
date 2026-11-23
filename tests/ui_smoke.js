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
// the full-precision form of a number in the client (web/app.js fmt), and Japanese script (task 1.1 checks)
const fmtFull = (n) => (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3));
const JA_TEXT_11 = /[぀-ヿ一-鿿]/;
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
  const errs = []; let expected401 = 0, allow401 = false, expected422 = 0, allow422 = false, expected400 = 0, allow400 = false, dialogs = 0, approveControls = 0, approvedTags = 0;
  const watch = (page, who) => {
    page.on('pageerror', (e) => errs.push(`${who} pageerror: ${e.message}`));
    page.on('console', (m) => { if (m.type() !== 'error') return; if (allow401 && /401/.test(m.text())) expected401++; else if (allow422 && /422/.test(m.text())) expected422++; else if (allow400 && /400/.test(m.text())) expected400++; else errs.push(`${who} console: ${m.text()}`); });
    page.on('dialog', (d) => { dialogs++; d.dismiss(); }); // the client must never use blocking dialogs; one would also reveal script injection
  };
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 }, acceptDownloads: true, deviceScaleFactor: SHOTS ? 2 : 1 });
  // V-45: every content-policy violation in any page of this context is recorded (an inline style the policy blocks
  // is one), in addition to the console errors that the browser also reports for it
  const cspViolations = [];
  await ctx.exposeBinding('__nfCsp', (_src, v) => { cspViolations.push(v); });
  await ctx.addInitScript(() => document.addEventListener('securitypolicyviolation', (e) => window.__nfCsp(`${e.violatedDirective} ${e.blockedURI || 'inline'} ${location.hash} ${e.sourceFile || ''}:${e.lineNumber || 0}`)));
  // elements that carry a style attribute: none may come from markup, and the client sets none through the CSSOM
  // except the panel widths of the workspace splitter (custom properties on .ws-body)
  const styledProbe = () => [...document.querySelectorAll('[style]')].filter((el) => !(el.matches('.ws-body') && [...el.style].every((k) => k === '--lw' || k === '--rw'))).map((el) => el.outerHTML.slice(0, 80)).slice(0, 3);
  const p = await ctx.newPage(); watch(p, 'admin');
  const apiGet = async (path) => (await ctx.request.get(B + path, { headers: { 'X-NF-Request': '1' } })).json();
  // navigation is complete when the client marks the view with the hash it rendered
  // goOn: the same wait for any page (work plan 3.5: the role checks of section 13 waited on '#view h1', which the previous
  // view still matched after a hash-only navigation; that was the intermittent failure)
  const goOn = async (pg, hash, sel, opt) => { await pg.goto(`${B}/#/${hash}`); await pg.waitForFunction((h) => { const v = document.querySelector('#view'); return v && v.dataset.hash === h; }, `#/${hash}`); await pg.waitForSelector(sel || '#view h1', opt); };
  const go = (hash, sel) => goOn(p, hash, sel);
  const sweepControls = async () => { const c = await p.evaluate(controlProbe); approveControls += c.approve; approvedTags += c.approvedTags; return c; };
  const vpos = (ci, vi) => p.evaluate(([c, v]) => { const pl = document.querySelector(`.pv .hit[data-ci="${c}"]`), q = pl.points[v], m = pl.getScreenCTM(); return [q.x * m.a + q.y * m.c + m.e, q.x * m.b + q.y * m.d + m.f]; }, [ci, vi]);
  const transform = () => p.evaluate(() => document.querySelector('.pv g[transform]').getAttribute('transform'));
  const zoom = async () => parseInt(await p.textContent('#zoomv'), 10);

  // ---- 1. first-run setup and sign-in
  const policy = (await ctx.request.get(`${B}/`)).headers()['content-security-policy'] || '';
  check('V-45: the page is served with a style policy of its own files only (no \'unsafe-inline\')', /(^|;)\s*style-src 'self'\s*(;|$)/.test(policy) && !/unsafe-inline/.test(policy), policy);
  await p.goto(B);
  await p.waitForSelector('#f');
  // #14: the page declares an inline icon, so the browser does not ask the server for /favicon.ico (a 404 on every page)
  check('favicon: the page declares an inline icon (no request for /favicon.ico)', !!(await p.$('link[rel~=icon][href^="data:"]')));
  // #12: the first-run screen counts the characters still needed, then confirms the minimum (client only; the rule is unchanged)
  await p.fill('input[name=p]', 'short'); const hint5 = await p.textContent('#pwhint');
  await p.fill('input[name=p]', ADMIN.password); const hint15 = await p.textContent('#pwhint');
  check('setup: the password hint counts the characters still needed, then confirms the minimum', /^5 more characters needed/.test(hint5) && /Meets the 10-character minimum/.test(hint15), { hint5, hint15 });
  check('setup: first-run screen asks for the administrator', /Create the administrator/.test(await p.textContent('h1')));
  check('setup: sign-in screen carries the development-build notice', /Not validated/.test(await p.textContent('.login-card')));
  check('setup: server mode shows no desktop-mode notice (task 2.4: the server says it is not in desktop mode)', (await p.$('#desktop-notice')) === null && (await (await ctx.request.get(`${B}/api/health`)).json()).mode === 'server');
  // V-36: the first administrator can only be created with the installation's one-time setup token
  await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', ADMIN.password); await p.click('#f button'); await p.waitForSelector('#f .field-err');
  check('setup: refused inline without the setup token', /setup token/i.test(await p.textContent('#f .field-err')) && (await (await ctx.request.get(`${B}/api/health`)).json()).setup_required === true);
  await p.fill('input[name=t]', SETUP_TOKEN); await p.click('#f button');
  await p.waitForSelector('.stats');
  check('login: dashboard shown after setup', (await p.textContent('#view h1')).trim() === 'Dashboard');
  check('shell: landmarks present', await p.evaluate(() => !!document.querySelector('header.topbar') && !!document.querySelector('nav[aria-label="Main"]') && !!document.querySelector('main#view') && !!document.querySelector('footer.statusbar') && !!document.querySelector('a.skip')));
  check('shell: build notice in top bar and status bar', /not validated/i.test(await p.textContent('.devnote')) && /NOT FOR MANUFACTURING/.test(await p.textContent('.statusbar')));
  check('dashboard: honest empty state before any job', /No engineering jobs yet/.test(await p.textContent('#view')));
  check('dashboard: the empty state offers a one-step start, "Create a project", to an administrator', (await p.locator('#view a[href="#/projects"]', { hasText: 'Create a project' }).count()) === 1);

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
  // task 1.2 (review; TOLERANCE_POLICY section 8.2): the INCONCLUSIVE product checks are the eccentric reducer's two
  // circumference checks, whose eps_rep is not certified. Every other check of every family must be PASS; none may FAIL.
  // RC-10 (engine 0.4.8, TOLERANCE_POLICY section 11): the reducer's drawn-area check joins them (chord-deficit
  // allowance eps_rep x perimeter, rep_check demotes it the same way).
  const INCONCLUSIVE_CHECKS = { eccentric_reducer: ['large_edge_equals_circumference', 'small_edge_equals_circumference', 'area'] };
  // task 1.2: the error wording is per family, as job.rs eps_assumption states it and the page shows it in #assume.
  // The sentences are copied from job.rs (EPS_NUM_BOUND, EPS_NUM_REDUCER, EPS_REP_CERTIFIED, EPS_REP_NOT_CERTIFIED).
  const EPS_NUM_SENTENCE = 'eps_num is an a-priori rounding bound, not interval-certified.';
  const EPS_WORDING = {
    cylinder_shell: { has: [EPS_NUM_SENTENCE], not: ['eps_rep is'] },
    cone_frustum: { has: [EPS_NUM_SENTENCE], not: ['eps_rep is'] },
    branch_stub: { has: [EPS_NUM_SENTENCE, 'eps_rep is a certified upper bound of the chord error of the drawn polyline'], not: ['NOT certified', 'ESTIMATE'] },
    header_with_branch: { has: [EPS_NUM_SENTENCE, 'eps_rep is a certified upper bound of the chord error of the drawn polyline'], not: ['NOT certified', 'ESTIMATE'] },
    eccentric_reducer: { has: [EPS_NUM_SENTENCE, 'For an eccentric reducer its quadrature term is an ESTIMATE, not a bound.', 'eps_rep is NOT certified for this pattern'], not: ['is a certified upper bound'] },
  };
  // task 1.2 follow-up: the eps_rep row (#epsrep-note) states the status the engine gives per family: a certified bound for
  // a branch and a header, NOT certified (an estimate for the eccentric reducer), and not applicable where eps_rep is 0.
  const EPS_REP_ROW = {
    cylinder_shell: /^not applicable: eps_rep is 0/, cone_frustum: /^not applicable: eps_rep is 0/,
    branch_stub: /^certified upper bound of the chord error of the drawn polyline, eps_num included/,
    header_with_branch: /^certified upper bound of the chord error of the drawn polyline, eps_num included/,
    eccentric_reducer: /^NOT certified: for an eccentric reducer its quadrature term is an estimate, not a bound/,
  };
  const jobs = {};
  for (const [fam, parts, curves] of FAMILIES) {
    await go('math/1', '#calc');
    await p.selectOption('select[name=fam]', fam);
    // task 1.4: declare the surface so the branch happy-path still verifies to all-PASS (an `unknown` surface makes the
    // radius checks INCONCLUSIVE; that path is covered end to end by e2e C6 and by compat 7/157/191/196, surface_declaration.rs).
    if (fam === 'header_with_branch' || fam === 'branch_stub') await p.selectOption('select[name=surface_reference]', 'outside');
    await p.click('#run'); await p.waitForURL(/#\/calc\/\d+$/); await p.waitForSelector('.pv .c', { state: 'attached' });
    const id = Number(p.url().split('/').pop()); jobs[fam] = id;
    const got = { state: (await p.textContent('.ws-head h1 .tag')).trim(), parts: await p.locator('#parts [role=tab]').count(), curves: await p.locator('.pv .c').count() };
    await p.click('#tab-ws-checks');
    const verdicts = await p.locator('#tp-ws-checks .l1 .tag').allTextContents();
    const cks = (await apiGet(`/api/calculations/${id}`)).checks || [];
    const expectInc = (INCONCLUSIVE_CHECKS[fam] || []).slice().sort();
    const inconc = cks.filter((c) => c.verdict === 'INCONCLUSIVE').map((c) => c.name).sort();
    const verdictLabel = expectInc.length ? `INCONCLUSIVE ${expectInc.join(' and ')}, all others PASS` : 'all checks PASS';
    check(`calculation ${fam}: REVIEW REQUIRED, ${parts} part(s), ${curves} curves drawn, ${verdictLabel}`,
      got.state === 'REVIEW REQUIRED' && got.parts === parts && got.curves === curves && cks.length >= 4 &&
      JSON.stringify(inconc) === JSON.stringify(expectInc) &&
      cks.every((c) => c.verdict === (expectInc.includes(c.name) ? 'INCONCLUSIVE' : 'PASS')) &&
      verdicts.length >= 4 && verdicts.filter((v) => v.trim() === 'INCONCLUSIVE').length === expectInc.length && verdicts.every((v) => ['PASS', 'INCONCLUSIVE'].includes(v.trim())),
      { got, inconc, verdicts: verdicts.length });
    await p.click('#tab-ws-gates');
    const assume = (await p.textContent('#assume')).replace(/\s+/g, ' ');
    const wording = EPS_WORDING[fam];
    check(`calculation ${fam}: marking side stated as a provisional convention; error wording per family (${wording.has.length} sentence(s) present, ${wording.not.length} absent)`,
      (await p.locator('#mside').count()) === 1 && /seen from the outside/.test(await p.textContent('#mside')) && /provisional convention, not reviewed/.test(await p.textContent('#quant')) && /a-priori bound, not interval-certified/.test(await p.textContent('#quant')) && /PROVISIONAL convention/.test(await p.textContent('#tp-ws-gates')) &&
      wording.has.every((s) => assume.includes(s)) && wording.not.every((s) => !assume.includes(s)), { assume: assume.slice(0, 400) });
    const epsRepNote = (await p.textContent('#epsrep-note')).replace(/\s+/g, ' ').trim();
    check(`calculation ${fam}: the eps_rep row states the engine's status for this family (only the reducer is NOT certified)`,
      EPS_REP_ROW[fam].test(epsRepNote) && /NOT certified/.test(epsRepNote) === (fam === 'eccentric_reducer'), { epsRepNote });
    check(`calculation ${fam}: approval gate G16 shown as NOT IMPLEMENTED`,/G16\s*human approval\s*NOT IMPLEMENTED/.test((await p.textContent('#tp-ws-gates')).replace(/\s+/g, ' ')));
    await sweepControls();
  }

  // task 1.2 follow-up, Japanese: the eps_rep row of a branch is a certified bound; the reducer's is not certified
  for (const [fam, re] of [['branch_stub', /描かれた折れ線の弦の誤差について、認証された上界です/], ['eccentric_reducer', /^認証されていません。偏心レデューサ/]]) {
    await p.goto(`${B}/?lang=ja#/calc/${jobs[fam]}`); await p.waitForSelector('.pv .c', { state: 'attached' });
    await p.click('#tab-ws-gates');
    const epsRepJa = (await p.textContent('#epsrep-note')).replace(/\s+/g, ' ').trim();
    check(`calculation ${fam} ja: the eps_rep row says ${fam === 'eccentric_reducer' ? 'not certified (an estimate)' : 'certified'} in Japanese`, re.test(epsRepJa), { epsRepJa });
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
  // B4 (user simulation): the engine's smallest dimension (MIN_DIMENSION_MM 1e-6, nf-core job.rs) is checked at the field,
  // before anything is sent, instead of a round trip that ends in a refused job
  await p.selectOption('select[name=fam]', 'cone_frustum'); await p.fill('#f_h', '0.0000001'); await p.click('#run'); await p.waitForSelector('#calc .field-err');
  const tinyMsg = await p.textContent('#calc .field-err');
  check('B4: a dimension below the engine\'s smallest (1e-6 mm) is marked at the field, and nothing is sent', /below 1e-6 mm/.test(tinyMsg) && (await jobCount()) === before && /Not sent/.test(await p.textContent('#cmsg')), { tinyMsg });
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
  // B5 (user simulation): the NO RESULT banner states the engine's numeric threshold, which the toast shows only briefly;
  // and the reason ends with one full stop (#15)
  const rOutcome = (await apiGet(`/api/calculations/${R}`)).outcome || '';
  const rThr = /s_end\s*>\s*([0-9][0-9.eE+-]*)/.exec(rOutcome);
  const rBanner = (await p.locator('.ws-banner.bad').first().textContent()).replace(/\s+/g, ' ');
  check('NO RESULT banner states the numeric threshold of the toast, and the reason ends with one full stop', !!rThr && rBanner.includes(rThr[1]) && !/\.\./.test(rBanner), { threshold: rThr && rThr[1], rBanner: rBanner.slice(0, 260) });
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
  // the status bar is repainted by its own fetch of /api/dashboard after the decision (refreshSummary), which can
  // finish after the obligation list: give it up to 5 s, then check as before (a count that never updates still fails)
  await p.waitForFunction(() => /1 open scrutiny obligation$/.test((document.querySelector('#sb-obl') || {}).textContent || ''), null, { timeout: 5000 }).catch(() => {});
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
    const over = {}, unnamed = {}, titles = {}, styled = {}; let unreachable = 0;
    for (const r of ROUTES) {
      await go(r); if (r.startsWith('calc/')) await p.waitForSelector('.pv', { state: 'attached' }); await p.waitForTimeout(120);
      titles[r] = (await p.textContent('#view h1')).trim();
      const o = await p.evaluate(overflowProbe); if (o.length) over[r] = o;
      const st = await p.evaluate(styledProbe); if (st.length) styled[r] = st;
      const c = await sweepControls(); if (c.unnamed.length) unnamed[r] = c.unnamed;
      if (r.startsWith('calc/')) { // every workspace region must be reachable: on screen, or by scrolling the page or its panel
        for (const sel of ['#replay', '#tab-ws-records', '.pv', '.pane .kv']) { if (await p.locator(sel).count()) { await p.locator(sel).first().scrollIntoViewIfNeeded(); const bb = await p.locator(sel).first().boundingBox(); if (!bb || bb.x < -1 || bb.x + bb.width > w + 1 || bb.y < 0 || bb.y > h) unreachable++; } }
      }
    }
    check(`layout ${w}x${h}: no horizontal overflow on any of ${ROUTES.length} screens`, Object.keys(over).length === 0, over);
    check(`V-45 ${w}x${h}: no element on any of ${ROUTES.length} screens carries an inline style attribute (dark theme)`, Object.keys(styled).length === 0, styled);
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

  // ---- 12b. V-45: the dashboard state bar keeps its proportions without inline styles, and every screen is walked
  // again in the light theme (headings, overflow, no inline style, and no content-policy violation: section 15)
  const barProbe = () => {
    const bar = document.querySelector('.bar'); const m = /(\d+)\D+(\d+)\D+(\d+)\D+(\d+)\D+(\d+)/.exec(bar.getAttribute('aria-label')); // in the order of dash.bar.aria
    const n = m.slice(1).map(Number), counts = [n[0], n[1], n[2] + n[3], n[4]];
    return { counts, widths: [...bar.children].map((el) => el.getBoundingClientRect().width), styled: bar.querySelectorAll('[style]').length };
  };
  const barOk = (b) => {
    if (b.styled || b.widths.length !== 4) return false;
    const live = b.counts.map((c, i) => [c, b.widths[i]]).filter(([c]) => c > 0);
    if (b.counts.some((c, i) => (c === 0) !== (b.widths[i] === 0)) || live.some(([, w]) => w < 3 - 0.01)) return false;
    // segments wider than the 3 px minimum have the same width per job, within 3 %
    const wide = live.filter(([, w]) => w > 3.5), per = wide.map(([c, w]) => w / c);
    return per.every((x) => Math.abs(x / per[0] - 1) < 0.03);
  };
  const barThemes = {};
  for (const theme of ['dark', 'light']) {
    await go('dashboard', '.bar');
    if ((await p.evaluate(() => document.documentElement.dataset.theme)) !== theme) { await p.click('#theme'); await p.waitForTimeout(150); }
    barThemes[theme] = await p.evaluate(barProbe);
  }
  check('V-45: the dashboard state bar shows each state in proportion to its jobs without an inline style (dark and light)', Object.values(barThemes).every(barOk) && barThemes.dark.counts.filter((c) => c > 0).length >= 2, barThemes);
  {
    const lightTitles = {}, lightOver = {}, lightStyled = {}; const bg = await p.evaluate(() => getComputedStyle(document.body).backgroundColor);
    for (const r of ROUTES) {
      await go(r); if (r.startsWith('calc/')) await p.waitForSelector('.pv', { state: 'attached' }); await p.waitForTimeout(120);
      lightTitles[r] = (await p.textContent('#view h1')).trim();
      const o = await p.evaluate(overflowProbe); if (o.length) lightOver[r] = o;
      const st = await p.evaluate(styledProbe); if (st.length) lightStyled[r] = st;
    }
    const theme = await p.evaluate(() => document.documentElement.dataset.theme);
    check(`light theme: all ${ROUTES.length} screens render their heading, without overflow or inline style attributes`, theme === 'light' && bg !== 'rgb(14, 17, 22)' && Object.entries(EXPECT).every(([r, t]) => lightTitles[r] === t) && Object.keys(lightOver).length === 0 && Object.keys(lightStyled).length === 0, { theme, bg, lightOver, lightStyled });
    await p.click('#theme'); await p.waitForTimeout(150);
    check('light theme: switched back to dark for the rest of the run', (await p.evaluate(() => document.documentElement.dataset.theme)) === 'dark');
  }

  // ---- 13. role restrictions (API sign-in first, so that no page ever provokes a refused request)
  const role = async (cred) => { const c = await browser.newContext({ viewport: { width: 1366, height: 768 } }); await c.request.post(`${B}/api/login`, { headers: { 'X-NF-Request': '1' }, data: cred }); const pg = await c.newPage(); watch(pg, cred.username); return [c, pg]; };
  const [vctx, v] = await role({ username: 'viewer1', password: 'read-only-pass' });
  await goOn(v, 'dashboard', '.stats');
  const vDash = { newCalc: await v.locator('#view a:has-text("New calculation")').count(), adminNav: await v.locator('.rail a[data-nav=admin]').count() };
  await goOn(v, 'math/1'); const vMath = await v.textContent('#view');
  await goOn(v, `calc/${N}`, '.pv .c', { state: 'attached' }); await v.click('#tab-ws-scrutiny');
  const vJob = { replay: await v.locator('#replay').count(), corr: await v.locator('#corr-open, [data-correct]').count(), resolve: await v.locator('[data-resolve]').count(), obligations: await v.locator('#obls > li').count(), dxf: await v.locator('#dxf').count() };
  await goOn(v, 'admin'); const vAdmin = await v.textContent('#view');
  await goOn(v, 'projects'); const vProj = await v.locator('#np').count();
  check('viewer: can read results, but sees no way to calculate, correct, decide, replay or administer', vDash.newCalc === 0 && vDash.adminNav === 0 && /can view results but not run calculations/.test(vMath) && vJob.replay === 0 && vJob.corr === 0 && vJob.resolve === 0 && vJob.obligations >= 9 && vJob.dxf === 1 && /Administrator role required/.test(vAdmin) && vProj === 0, { vDash, vJob, vProj });
  const post = (c, path, data) => c.request.post(B + path, { headers: { 'X-NF-Request': '1' }, data });
  check('viewer: server refuses calculation, decision and audit log', (await post(vctx, '/api/projects/1/calculations', { family: 'cylinder_shell', r_dev: 1, length: 1 })).status() === 403 && (await post(vctx, '/api/obligations/1/resolve', { decision: 'waived', reason: 'viewer attempt' })).status() === 403 && (await vctx.request.get(`${B}/api/audit`, { headers: { 'X-NF-Request': '1' } })).status() === 403);
  await vctx.close();
  const [ectx, e] = await role({ username: 'engineer1', password: 'engineer-pass-1' });
  await goOn(e, 'math/1'); const eCalc = await e.locator('#calc').count();
  await goOn(e, `calc/${N}`, '.pv .c', { state: 'attached' }); const eReplay = await e.locator('#replay').count();
  await goOn(e, 'admin');
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
  check('joint: an undeclared job says the joint type is not declared and is not called set-on', /not declared \(the branch end cut and the header opening are cut on one curve/.test((await p.textContent('#joint p')).trim()) && undDd[0] === 'not declared' && !/set-on joint assumed/i.test(aText) && !/set-on/i.test(await p.textContent('#joint p')), { undDd });
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

  // ---- 14a. task 1.1: a part near 180 degrees shows eps_num first and in full, in English and Japanese; an ordinary part does not
  const nearApi = await ctx.request.post(`${B}/api/projects/1/calculations`, { headers: { 'X-NF-Request': '1' }, data: { family: 'branch_stub', header_radius: 150, branch_radius: 100, branch_dev_radius: 100, beta_deg: 179.997, delta: 0, s_end: 5.2e6, surface_reference: 'outside' } });
  const near = await nearApi.json();
  const nearJob = await apiGet(`/api/calculations/${near.id}`);
  const nearEps = nearJob.patterns && nearJob.patterns[0] ? nearJob.patterns[0].eps_num : undefined;
  await go(`calc/${near.id}`, '.pv'); await p.waitForSelector('#quant dl');
  const epsEn = (await p.locator('#epsgov').count()) ? (await p.textContent('#epsgov')).replace(/\s+/g, ' ') : '';
  check('task 1.1: a branch at 179.997 deg is accepted and shows eps_num first, in full, as an a-priori rounding bound, not interval-certified', nearApi.ok() && nearJob.patterns[0].eps_num_governs === true && epsEn.includes(`eps_num = ${fmtFull(nearEps)} mm`) && /a-priori rounding bound, not interval-certified/.test(epsEn) && (await p.evaluate(() => document.querySelector('#quant').firstElementChild.id)) === 'epsgov', { epsEn, nearEps });
  await p.click('#lang-ja'); await p.waitForSelector('#quant dl');
  const epsJa = (await p.locator('#epsgov').count()) ? (await p.textContent('#epsgov')).replace(/\s+/g, ' ') : '';
  check('task 1.1: the same notice in Japanese', JA_TEXT_11.test(epsJa) && epsJa.includes(fmtFull(nearEps)) && /先験的な上界であり、区間演算による保証はありません/.test(epsJa), epsJa);
  await p.click('#lang-en');
  await go(`calc/${jobs.branch_stub}`, '.pv'); await p.waitForSelector('#quant dl');
  check('task 1.1: a branch at an ordinary angle carries no such notice', (await p.locator('#epsgov').count()) === 0);

  await p.click('#logout'); await p.waitForSelector('#f');
  // ---- 14b. routing and Japanese: the switch, its persistence, the URL override, every main screen walked for
  // English words, layout at three widths in both languages, and errors in both languages.
  // The walk accepts only English that is English by design. Before it reads any text it skips:
  //   .mono .idk .hash code kbd pre  identifiers, hashes, file and key names (monospace in the client)
  //   .u bdi                         text a person typed (project, user, file and reason names), shown as written
  //   .u-select option               project and user names listed in a select
  //   [lang=en]                      engine or server text the catalogue deliberately leaves in English (engineJa)
  // and it accepts these words wherever they are: the product name and its AMOEBA module, the file formats (DXF, PDF,
  // PNG, JPEG, TIFF, STEP), the hash algorithm name SHA, the unit rad, and the two error identifiers eps_num and eps_rep
  // (deliberate technical names, shown identically in both languages; the walk's word rule splits them at '_', so each is
  // removed whole before the words are counted; no other identifier is allowed).
  const JA_ALLOW = ['NEXUS', 'FABRIC', 'AMOEBA', 'DXF', 'PDF', 'PNG', 'JPEG', 'TIFF', 'STEP', 'SHA', 'rad', 'eps_num', 'eps_rep'];
  const jaWalk = (allow) => {
    const skip = '.mono,.idk,.hash,.u,bdi,code,kbd,pre,[lang=en],.u-select option,script,style,noscript';
    const out = [], w = document.createTreeWalker(document.getElementById('app'), NodeFilter.SHOW_TEXT); let n;
    while ((n = w.nextNode())) {
      const el = n.parentElement; if (!el || el.closest(skip) || !el.getClientRects().length) continue;
      let text = n.textContent;
      for (const id of allow.filter((w) => w.includes('_'))) text = text.split(id).join(' ');
      const bad = (text.match(/[A-Za-z]{3,}/g) || []).filter((x) => !allow.includes(x));
      if (bad.length) out.push(`<${el.tagName.toLowerCase()}> "${n.textContent.trim().slice(0, 50)}" [${bad.join(' ')}]`);
    }
    return out;
  };
  const JA_TEXT = /[぀-ヿ一-鿿]/;
  // Work plan 3.3: an identifier the engine or the server sends (gate, check, record kind, relation, parameter, quantity,
  // curve ...) is shown in Japanese with its label and the identifier beside it (.idk), never alone. The probe reads every
  // visible monospace element that is not beside a label, whose text is shaped like an identifier (lower-case words joined
  // by '_' or '.', or a gate id), and fails on any that has no Japanese label in the client's identifier table and is not
  // in JA_IDENT_ALLOW: identifiers deliberately shown without a label, each documented in docs/design/LOCALISATION.md
  // ("Identifiers shown without a Japanese label").
  const JA_IDENT_ALLOW = [
    // the two error quantities: deliberate technical names, shown identically in both languages (as in JA_ALLOW)
    'eps_num', 'eps_rep',
    // warning codes of the PDF converter: each is shown with the converter's own message beside it (crates/nf-pdf)
    'annotations_not_converted', 'clipping_not_applied', 'cropbox_outside_mediabox', 'ext_gstate_missing', 'font_missing', 'invalid_dash',
    'optional_content_ignored', 'pattern_fill_outline_only', 'rotate_invalid', 'shading_not_converted', 'text_chars_replaced', 'unbalanced_restore',
    'unknown_operator', 'userunit_invalid', 'xobject_ignored', 'xobject_missing',
    // the kind of replay audit, quoted as an identifier inside the server's G19 sentence (translated by template)
    'separate_process_replay',
  ];
  const jaIdents = async (allow) => {
    const m = await import('/i18n.js'); const known = new Set();
    for (const kind of Object.values(m.ID_TABLE)) for (const [k, v] of Object.entries(kind)) if (v && v[1]) known.add(k);
    const out = [];
    for (const el of document.querySelectorAll('#app .mono:not(.idk), #app .idk')) {
      // the gate's machine name, in the expanded gate row under the gate's Japanese label (`名称 provenance_complete`)
      // and the refusal codes of the help page, each in the row of its Japanese message
      if (!el.getClientRects().length || el.closest('pre,.hash,script,style,li[data-gate] .g-meta,#help-refusals td:first-child')) continue;
      const text = el.textContent.trim();
      if (!/^(?:[a-z][a-z0-9]*(?:[_.][a-z0-9]+)+|G\d\d)$/.test(text)) continue;
      if (el.classList.contains('idk')) { if (!known.has(text) && !allow.includes(text)) out.push(`${text} (beside a label that is not in the table)`); continue; }
      if (!known.has(text) && !allow.includes(text)) out.push(text);
    }
    return [...new Set(out)];
  };
  const jaScreen = async (label) => {
    const found = await p.evaluate(jaWalk, JA_ALLOW); check(`ja: ${label}: no English word of three or more letters outside the allow-list`, found.length === 0, found.slice(0, 6));
    const ids = await p.evaluate(jaIdents, JA_IDENT_ALLOW); check(`ja: ${label}: no identifier without a Japanese label outside the documented list`, ids.length === 0, ids.slice(0, 8));
  };
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
  // C7: in Japanese every gate and check shows a label. A label that is missing, or equal to the identifier, fails here
  // (the client then shows the raw identifier in place of the label). Counts the rows it saw: zero rows is a failure too.
  const rawLabelsJa = async (where) => {
    const r = await p.evaluate(() => {
      const bad = []; let n = 0;
      for (const row of document.querySelectorAll('#gates li[data-gate] .l1, #checks li > .l1')) {
        const b = row.querySelector(':scope > b'); if (!b) continue; n++;
        const idk = b.querySelector('.idk'); const ident = idk ? idk.textContent.trim() : row.querySelector(':scope > .id').textContent.trim();
        const label = [...b.childNodes].filter((c) => !(c.nodeType === 1 && c.classList.contains('idk'))).map((c) => c.textContent).join('').trim();
        if (!label || label === ident) bad.push(ident);
      }
      return { n, bad };
    });
    check(`ja: ${where}: every gate and check has a Japanese label, none shown as its raw identifier`, r.n > 0 && r.bad.length === 0, r);
  };
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
  await rawLabelsJa('job view, declared set-on joint, gates and checks');
  await go(`calc/${A}`, '.pv'); await p.waitForSelector('#joint'); await p.evaluate(() => { document.querySelector('#assume').parentElement.open = true; });
  const undJa = await p.textContent('#view');
  check('ja: the undeclared job does not say that set-on is assumed, and says the joint type is not declared', !/セットオン継手を前提/.test(undJa) && /未宣言/.test(await p.textContent('#joint')));
  check('ja: no engine sentence of the undeclared job is untranslated (joint, assumptions, gate G02)', (await rawEng()).length === 0, await rawEng());
  await jaScreen('job view, undeclared joint');
  await go(`calc/${J}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' }); await jaScreen('job view, pattern');
  for (const k of ['gates', 'checks', 'scrutiny', 'graph', 'history', 'records']) { await p.click(`#tab-ws-${k}`); await jaScreen(`job view, ${k} tab`); if (k === 'gates' || k === 'checks') await rawLabelsJa(`job view, ${k} tab`); }
  await go(`calc/${jobs.cone_frustum}`, '.pv'); await p.waitForSelector('.pv .c', { state: 'attached' }); await p.click('#tab-ws-checks'); await jaScreen('job view, concentric reducer, checks tab'); await rawLabelsJa('job view, concentric reducer, checks tab');
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
      for (const bad of ['1,5', '0x10', '1 000', '1.5.2', '12mm', '']) {
        await p.fill('#f_h', ''); await p.type('#f_h', bad); const shown = await p.inputValue('#f_h');
        await p.click('#run'); await p.waitForTimeout(150);
        refused[bad] = { keptAsTyped: shown === bad, fieldError: await p.locator('#calc .field-err').count() > 0, stillOnForm: await p.locator('#calc').count() === 1 };
      }
      const after = await count();
      check(`dimension ${lang}: a decimal comma, hex, a space, two points, a unit suffix and an empty field are shown as typed, marked, and nothing is sent`,
        after === before && Object.values(refused).every((r) => r.keptAsTyped && r.fieldError && r.stillOnForm), { before, after, refused });
    }
    await p.goto(`${B}/?lang=en#/math/1`); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'cone_frustum');
    await p.fill('#f_h', '1.5'); await p.focus('#f_h'); await p.keyboard.press('ArrowUp'); const up = await p.inputValue('#f_h'); await p.keyboard.press('ArrowDown'); await p.keyboard.press('ArrowDown'); const down = await p.inputValue('#f_h');
    check('dimension: ArrowUp and ArrowDown step the field by one and keep its decimals', up === '2.5' && down === '0.5', { up, down });
    await p.fill('#f_h', '301.25'); await p.click('#run'); await p.waitForFunction(() => /^#\/calc\/\d+/.test(location.hash)); const id = Number(/calc\/(\d+)/.exec(await p.evaluate(() => location.hash))[1]);
    const stored = await p.evaluate(async (i) => (await (await fetch(`/api/calculations/${i}`)).json()).request.h, id);
    check('dimension: the value stored with the job is exactly the value typed (301.25)', stored === 301.25, { stored });
    // B3 (user simulation): full-width digits and point (IME output, 全角) are read as their ASCII forms: the same value is computed
    await p.goto(`${B}/?lang=ja#/math/1`); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'cone_frustum');
    await p.fill('#f_h', '１５０'); await p.click('#run'); await p.waitForURL(/#\/calc\/\d+$/);
    const fwId = Number(/calc\/(\d+)/.exec(await p.evaluate(() => location.hash))[1]);
    const fwStored = await p.evaluate(async (i) => (await (await fetch(`/api/calculations/${i}`)).json()).request.h, fwId);
    check('B3: full-width digits (１５０) are computed exactly as 150', fwStored === 150, { fwStored });
    await p.goto(`${B}/?lang=en#/math/1`); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'cone_frustum');
    await p.fill('#f_h', '２．５'); await p.press('#f_h', 'Tab');
    check('B3: leaving a field that holds full-width point and digits shows the ASCII value 2.5', (await p.inputValue('#f_h')) === '2.5', await p.inputValue('#f_h'));
    // B4 in Japanese: the same client check, in the Japanese message
    await p.goto(`${B}/?lang=ja#/math/1`); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'cone_frustum');
    const beforeTiny = await count();
    await p.fill('#f_h', '0.0000001'); await p.click('#run'); await p.waitForSelector('#calc .field-err');
    const tinyJa = await p.textContent('#calc .field-err');
    check('B4 ja: the below-limit value is marked at the field in Japanese, and nothing is sent', /1e-6 mm未満/.test(tinyJa) && (await count()) === beforeTiny, tinyJa);
    await p.goto(`${B}/?lang=en#/dashboard`); await p.waitForSelector('.stats');
  }

  // ---- 14d. bug hunt round 7 (docs/qa/BUG_HUNT_R7.md, browser part). Each check reproduces its defect before the fix.
  {
    // BH-008: the sign-in screen loads with no failed request and no console error
    const sctx = await browser.newContext({ viewport: { width: 1366, height: 768 } }); const sp = await sctx.newPage();
    const signInProblems = [];
    sp.on('console', (m) => { if (m.type() === 'error') signInProblems.push(`console: ${m.text()}`); });
    sp.on('pageerror', (e) => signInProblems.push(`pageerror: ${e.message}`));
    sp.on('response', (r) => { if (r.status() >= 400) signInProblems.push(`response ${r.status()} ${new URL(r.url()).pathname}`); });
    await sp.goto(`${B}/#/dashboard`); await sp.waitForSelector('input[name=u]'); await sp.waitForTimeout(600);
    check('BH-008: the sign-in screen loads with no failed request and no console error', signInProblems.length === 0, signInProblems.slice(0, 5));
    await sctx.close();

    // BH-003: a language switch keeps the field error and the note, in the new language
    await p.goto(`${B}/?lang=en#/math/1`); await p.waitForSelector('#calc'); await p.selectOption('select[name=fam]', 'header_with_branch');
    await p.fill('#f_branch_radius', '200'); await p.click('#run'); await p.waitForSelector('#calc .field-err');
    const enNote = await p.textContent('#cmsg');
    await p.click('#lang-ja'); await p.waitForTimeout(400);
    const jaNote = (await p.textContent('#cmsg').catch(() => '')) || '';
    check('BH-003: after the language switch the field error still marks the field, and the note is in Japanese', (await p.locator('#calc .field-err').count()) === 1 && (await p.locator('#calc [aria-invalid=true]').count()) === 1 && /[぀-ヿ一-龯]/.test(jaNote) && !/Not sent/.test(jaNote) && /Not sent/.test(enNote), { enNote, jaNote });
    await p.click('#lang-en'); await p.waitForTimeout(300);

    // BH-006: the status card of a job: no value runs into the next cell (English dark, English light, Japanese)
    const reviewJob = (await apiGet('/api/calculations')).find((j) => j.state === 'REVIEW_REQUIRED' && !j.superseded);
    const cardOverlaps = () => p.evaluate(() => {
      const bad = [];
      document.querySelectorAll('#tblock .tb').forEach((cell, i) => {
        const c = cell.getBoundingClientRect();
        cell.querySelectorAll('*').forEach((el) => { const r = el.getBoundingClientRect(); if (r.width && (r.right > c.right + 1 || r.left < c.left - 1)) bad.push(`cell ${i}: ${el.textContent.trim().slice(0, 30)}`); });
      });
      return bad.slice(0, 4);
    });
    await go(`calc/${reviewJob.id}`, '#tblock');
    const cardEn = await cardOverlaps();
    await p.click('#theme'); await p.waitForTimeout(200); const cardLight = await cardOverlaps(); await p.click('#theme'); await p.waitForTimeout(200);
    await p.click('#lang-ja'); await p.waitForTimeout(300); const cardJa = await cardOverlaps(); await p.click('#lang-en'); await p.waitForTimeout(200);
    check('BH-006: the status card keeps each value inside its cell (English dark, English light, Japanese)', cardEn.length === 0 && cardLight.length === 0 && cardJa.length === 0, { cardEn, cardLight, cardJa });

    // BH-013: no horizontal page scroll at 360 px wide
    await p.setViewportSize({ width: 360, height: 780 });
    const wide = {};
    for (const h of [`calc/${reviewJob.id}`, 'amoeba', 'math/1', 'verification', 'manufacturing']) { await go(h); wide[h] = await p.evaluate(() => document.documentElement.scrollWidth - window.innerWidth); }
    await p.setViewportSize({ width: 1366, height: 768 });
    check('BH-013: no horizontal page scroll at 360 px wide on the five main screens', Object.values(wide).every((w) => w <= 1), wide);

    // BH-018: a splitter that has keyboard focus shows a visible focus indicator
    await go('math/1', '#calc'); await p.focus('select[name=fam]');
    let splitFocus = null;
    for (let i = 0; i < 14 && !splitFocus; i++) { await p.keyboard.press('Tab'); if (await p.evaluate(() => document.activeElement.classList.contains('split'))) splitFocus = await p.evaluate(() => { const cs = getComputedStyle(document.activeElement); return { outline: cs.outlineStyle, width: parseFloat(cs.outlineWidth), shadow: cs.boxShadow }; }); }
    check('BH-018: a focused workspace splitter shows an outline or a box-shadow', !!splitFocus && ((splitFocus.outline !== 'none' && splitFocus.width >= 2) || splitFocus.shadow !== 'none'), splitFocus);

    // BH-007: the Activities badge counts activities: the sum of the table's distinct-activity column
    await go('amoeba');
    const act = await p.evaluate(() => {
      const tab = [...document.querySelectorAll('[role=tab]')].find((b) => b.dataset.tab === 'activities');
      const sum = [...document.querySelectorAll('#tp-am-activities tbody tr')].reduce((s, r) => s + Number(r.children[1].textContent.replace(/[,\s]/g, '')), 0);
      return { badge: Number(tab.querySelector('.cnt').textContent.trim()), sum };
    });
    check('BH-007: the Activities badge equals the number of activities in its table', act.badge === act.sum && act.sum > 0, act);

    // BH-011: a malformed job id in the dependency graph gives the readable not-found message, not an HTTP status
    await go('amoeba/dependencies/abc');
    const depText = await p.evaluate(() => document.querySelector('#tp-am-dependencies').innerText.replace(/\s+/g, ' '));
    check('BH-011: a malformed job id shows the not-found message, not an HTTP status', /not found/i.test(depText) && !/400|Request failed/.test(depText), depText.slice(0, 160));

    // BH-012: browser Back and Forward step through the AMOEBA tabs
    await go('verification'); await go('amoeba');
    await p.click('[role=tab][data-tab=lessons]'); await p.click('[role=tab][data-tab=mechanisms]');
    await p.goBack(); await p.waitForTimeout(300);
    const back1 = await p.evaluate(() => ({ hash: location.hash, tab: (document.querySelector('[role=tab][aria-selected=true]') || { dataset: {} }).dataset.tab || null }));
    await p.goForward(); await p.waitForTimeout(300);
    const fwd1 = await p.evaluate(() => ({ hash: location.hash, tab: (document.querySelector('[role=tab][aria-selected=true]') || { dataset: {} }).dataset.tab || null }));
    check('BH-012: browser Back returns to the previous AMOEBA tab and Forward returns to the next', back1.tab === 'lessons' && back1.hash.startsWith('#/amoeba/lessons') && fwd1.tab === 'mechanisms', { back1, fwd1 });

    // BH-016: the joint row reads as a sentence, without the parameter key
    const asmJobs = (await apiGet('/api/calculations')).filter((j) => j.family === 'header_with_branch' && j.state !== 'REFUSED' && !j.superseded);
    const details = await Promise.all(asmJobs.slice(0, 8).map((j) => apiGet(`/api/calculations/${j.id}`)));
    const plainJoint = details.find((d) => d.request && d.request.joint === undefined);
    if (plainJoint) await go(`calc/${plainJoint.id}`, '#joint');
    const jointView = await p.evaluate(() => { const j = document.querySelector('#joint'); return j ? { dt: j.querySelector('dt') ? j.querySelector('dt').innerText.replace(/\s+/g, ' ').trim() : null, text: j.innerText.replace(/\s+/g, ' ') } : null; });
    check('BH-016: the joint row shows its label without the key, and no sentence prints a key next to a value', !!jointView && jointView.dt === 'Joint type' && !/\bjoint (not declared|Set-on)\b/i.test(jointView.text) && !/wall surface (header|branch)_surface_reference/.test(jointView.text), jointView);

    // BH-010: the values of an open form survive a forced sign-in
    await go('math/1', '#calc');
    await p.selectOption('select[name=fam]', 'cone_frustum'); await p.fill('#f_h', '333');
    await ctx.clearCookies(); allow401 = true; // the refused request this provokes is expected
    await p.click('#run'); await p.waitForSelector('input[name=u]');
    await p.fill('input[name=u]', ADMIN.username); await p.fill('input[name=p]', NEW_PW); await p.click('#f button'); await p.waitForTimeout(1200);
    allow401 = false;
    const restored = await p.evaluate(() => ({ hash: location.hash, fam: (document.querySelector('select[name=fam]') || {}).value || null, h: (document.querySelector('#f_h') || {}).value || null }));
    check('BH-010: after a forced sign-in the form comes back with its family and values', restored.fam === 'cone_frustum' && restored.h === '333', restored);

    // BH-014: the engine stays the judge of a square end inside the saddle (its limit is sampled, so the client cannot state
    // it exactly). Its refusal is recorded as a job, and that job names the field and the engine's reason.
    await go('math/1', '#calc');
    await p.selectOption('select[name=fam]', 'header_with_branch'); await p.fill('#f_s_end', '150');
    // the form page has its own .ws-banner (the provisional-models notice); the job's banner is the one with role=status,
    // so wait for that one: the URL can change before the job view replaces the form
    allow422 = true; await p.click('#run'); await p.waitForURL(/#\/calc\/\d+$/); await p.waitForSelector('.ws-banner[role=status]'); allow422 = false;
    const refusal = (await p.textContent('.ws-banner[role=status]')).replace(/\s+/g, ' ');
    check('BH-014: the engine refusal of a square end names the s_end field and the reason on the refused job', /s_end/.test(refusal) && /saddle/.test(refusal), refusal.slice(0, 200));

    // BH-002: obligations on a superseded result are counted apart from current work, and none is lost.
    // A job created by a correction has open obligations; correcting that job again supersedes it and leaves them open.
    const asmCurrent = (await apiGet('/api/calculations')).find((j) => j.family === 'header_with_branch' && j.state === 'REVIEW_REQUIRED' && !j.superseded);
    const hr = (await apiGet(`/api/calculations/${asmCurrent.id}`)).request.header_radius;
    const corrOn = (id, v) => p.evaluate(async ([i, val]) => { const r = await fetch(`/api/calculations/${i}/corrections`, { method: 'POST', headers: { 'X-NF-Request': '1', 'Content-Type': 'application/json' }, body: JSON.stringify({ parameter: 'header_radius', new_value: val, reason: 'bug hunt round 7 check' }) }); return { status: r.status, body: await r.json().catch(() => null) }; }, [id, v]);
    const c1 = await corrOn(asmCurrent.id, hr + 1);
    const c2 = c1.status === 200 ? await corrOn(c1.body.new_calculation, hr + 2) : { status: 0 };
    check('BH-002 setup: two corrections in a row are accepted (the second supersedes the job the first one created)', c1.status === 200 && c2.status === 200, { c1: c1.status, c2: c2.status });
    const sum = await apiGet('/api/amoeba/summary');
    const allObl = await apiGet('/api/obligations');
    const openAll = allObl.filter((o) => o.state === 'open').length;
    const supIds = new Set((await apiGet('/api/calculations')).filter((c) => c.superseded).map((c) => c.id));
    const onSup = allObl.filter((o) => o.state === 'open' && supIds.has(o.target_calc)).length;
    check('BH-002: open obligations are split into current work and superseded results, and the split adds up', onSup >= 1 && sum.obligations.open + sum.obligations.open_on_superseded === openAll && sum.obligations.open_on_superseded === onSup, { open: sum.obligations.open, onSuperseded: sum.obligations.open_on_superseded, openAll, onSup });
    await go('amoeba');
    const badgeObl = await p.evaluate(() => [...document.querySelectorAll('[role=tab]')].find((b) => b.dataset.tab === 'obligations').querySelector('.cnt').textContent.trim());
    check('BH-002: the obligations badge counts current work only', Number(badgeObl) === openAll - onSup, { badgeObl, currentWork: openAll - onSup });
  }

  // ---- 14e. BH-014 refusal codes (docs/backend/REFUSAL_CODES.md). A refused job names its fields with the message for
  // the code, and "New from these values" marks the same fields; both in English and in Japanese.
  {
    // a refused calculation is answered 422 by design: that answer is expected, as in the other refusal checks
    const post = async (body) => {
      allow422 = true;
      const r = await p.evaluate(async (b) => {
        const res = await fetch('/api/projects/1/calculations', { method: 'POST', headers: { 'content-type': 'application/json', 'x-nf-request': '1' }, body: JSON.stringify(b) });
        return { st: res.status, v: await res.json() };
      }, body);
      await p.waitForTimeout(150); allow422 = false;
      return r;
    };
    // the value of the row whose name is exactly `name` (its label is a small monospace element)
    const fieldText = (name) => p.evaluate((n) => {
      const dt = [...document.querySelectorAll('dt')].find((d) => { const s = d.querySelector('small'); return s && s.textContent.trim() === n; });
      return dt && dt.nextElementSibling ? dt.nextElementSibling.textContent : '';
    }, name);
    const sq = await post({ family: 'header_with_branch', header_radius: 150, header_dev_radius: 150, header_length: 1200, branch_station: 600,
      branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 0.5, surface_reference: 'unknown' });
    check('BH-014: a square end too short is refused with its code and field', sq.st === 422 && sq.v.code === 'square_end_too_short' && sq.v.fields[0] === 's_end', sq.v);
    await p.goto(`${B}/#/calc/${sq.v.id}`); await p.waitForTimeout(700);
    const sqEn = await fieldText('s_end');
    check('BH-014: the refused s_end row shows the English message for square_end_too_short', /does not lie beyond the saddle/.test(sqEn) && /make it longer/.test(sqEn), sqEn.slice(0, 160));
  check('plain words (user simulation): the square-end refusal says what is wrong in plain words, the engine term in brackets', /is too short/.test(sqEn) && /reach past the saddle/.test(sqEn), sqEn.slice(0, 160));
    await p.goto(`${B}/?lang=ja#/calc/${sq.v.id}`); await p.waitForTimeout(700);
    const sqJa = await fieldText('s_end');
    check('BH-014: the refused s_end row shows the Japanese message for square_end_too_short', /相貫線の先まで届いていません/.test(sqJa), sqJa.slice(0, 160));

    const big = await post({ family: 'cylinder_shell', r_dev: 100, length: 1e9 });
    check('BH-014: a magnitude above the limit is refused with its code, field and exact limit', big.st === 422 && big.v.code === 'magnitude_above_limit' && big.v.detail.limit === 1e8, big.v);
    await p.goto(`${B}/#/calc/${big.v.id}`); await p.waitForTimeout(700);
    const bigEn = await fieldText('length');
    check('BH-014: the refused length row shows the English message with the limit and the value', bigEn.includes('1000000000 mm') && bigEn.includes('100000000 mm'), bigEn.slice(0, 160));
    await p.goto(`${B}/?lang=ja#/calc/${big.v.id}`); await p.waitForTimeout(700);
    const bigJa = await fieldText('length');
    check('BH-014: the refused length row shows the Japanese message with the limit and the value', bigJa.includes('100000000mm') && bigJa.includes('1000000000mm'), bigJa.slice(0, 160));
    // "New from these values": the same fields are marked with the message, in each language
    await p.goto(`${B}/#/math/1/${big.v.id}`); await p.waitForTimeout(900);
    const markedEn = await p.evaluate(() => { const el = document.querySelector('form [name=length]'); const m = el && el.closest('.field') && el.closest('.field').querySelector('.field-err'); return { aria: el && el.getAttribute('aria-invalid'), msg: m ? m.textContent : '' }; });
    check('BH-014: New from these values marks the length field with the English message', markedEn.aria === 'true' && markedEn.msg.includes('100000000 mm'), markedEn);
    await p.goto(`${B}/?lang=ja#/math/1/${big.v.id}`); await p.waitForTimeout(900);
    const markedJa = await p.evaluate(() => { const el = document.querySelector('form [name=length]'); const m = el && el.closest('.field') && el.closest('.field').querySelector('.field-err'); return { aria: el && el.getAttribute('aria-invalid'), msg: m ? m.textContent : '' }; });
    check('BH-014: New from these values marks the length field with the Japanese message', markedJa.aria === 'true' && markedJa.msg.includes('100000000mm'), markedJa);
  }

  // ---- 14c. PDF to DXF (work plan 3.1): the action on a PDF, the form (scale required), a refusal with its code, the
  // result labelled "converted from PDF - not verified - not released", its preview and downloads, in both languages.
  // Needs the converter nf-pdf2dxf beside the server binary (or NF_PDF2DXF for the server).
  {
    const pdfBytes = fs.readFileSync(require('path').join(__dirname, '..', 'crates', 'nf-pdf', 'tests', 'fixtures', 'reportlab_lines.pdf'));
    const up = await ctx.request.post(`${B}/api/projects/1/documents`, { headers: { 'X-NF-Request': '1', 'X-Filename': 'lines.pdf' }, data: pdfBytes });
    check('pdf: a PDF is stored as a document', up.status() === 200, up.status());
    const proj = await apiGet('/api/projects/1');
    const doc = proj.documents.filter((d) => d.detected_type === 'pdf').pop();
    for (const lang of ['en', 'ja']) {
      await p.goto(`${B}/?lang=${lang}#/project/1`); await p.waitForSelector('#view h1');
      const action = await p.$(`a[href="#/convert/1/${doc.id}"]`);
      check(`pdf ${lang}: the PDF row offers the conversion`, !!action);
      await p.goto(`${B}/?lang=${lang}#/convert/1/${doc.id}`); await p.waitForSelector('#cv');
      if (lang === 'ja') await jaScreen('PDF to DXF form');
      // no scale: refused in the form, nothing sent
      const before = (await apiGet('/api/projects/1')).conversions.length;
      await p.click('#cv button.pri'); await p.waitForSelector('#cvmsg .note.bad');
      check(`pdf ${lang}: without a scale the form refuses and sends nothing`, (await apiGet('/api/projects/1')).conversions.length === before);
      // a page past the end: the converter's refusal, with its code, is shown
      await p.fill('#cv_page', '7'); await p.fill('#cv_scale', '1');
      allow422 = true; await p.click('#cv button.pri'); await p.waitForFunction(() => /page_out_of_range/.test(document.querySelector('#cvmsg') ? document.querySelector('#cvmsg').textContent : '')); allow422 = false;
      const refusalShown = ((await p.textContent('#cvmsg')) || '').trim();
      check(`pdf ${lang}: a page past the end shows the converter's code`, refusalShown.includes('page_out_of_range'), refusalShown);
      // a real conversion
      await p.fill('#cv_page', '1'); await p.fill('#cv_scale', '10');
      await p.click('#cv button.pri'); await p.waitForSelector('#cv-label');
      const label = (await p.textContent('#cv-label')).trim();
      check(`pdf ${lang}: the result is labelled converted from PDF, not verified, not released`, lang === 'en' ? label === 'Converted from PDF - not verified - not released' : label === 'PDFから変換・未検証・未リリース', label);
      const paths = await p.$$eval('#cv-preview path', (ps) => ps.length);
      check(`pdf ${lang}: the preview draws the converted lines, not the pattern viewer`, paths > 0 && !(await p.$('#view svg.pv')), paths);
      const href = await p.getAttribute('#cv-dxf', 'href');
      const dxf = await ctx.request.get(B + href);
      check(`pdf ${lang}: the DXF downloads with the NOT RELEASED note inside`, dxf.status() === 200 && (await dxf.text()).includes('CONVERTED FROM PDF - NOT VERIFIED - NOT RELEASED'), dxf.status());
      check(`pdf ${lang}: no flat pattern and no calculation comes from a conversion`, !/flat pattern view/i.test(await p.textContent('#view')) && (await apiGet('/api/calculations')).every((c) => !String(c.family || '').includes('pdf')));
      if (lang === 'ja') await jaScreen('PDF to DXF result');
    }
  }

  // ---- 14f. user-simulation drawbacks (phone navigation B1, threshold B5 in Japanese, gate summary #8, id chips #13)
  {
    await p.goto(`${B}/?lang=en#/dashboard`); await p.waitForSelector('.stats');
    await p.setViewportSize({ width: 390, height: 844 });
    await go('dashboard', '.stats');
    const menu = p.locator('#navtoggle');
    const start = { shown: await menu.isVisible(), expanded: await menu.getAttribute('aria-expanded'), railVisible: await p.locator('#rail').isVisible(), label: await menu.getAttribute('aria-label') };
    check('B1 phone: a labelled menu button at 390 px, and the navigation starts closed', start.shown && start.expanded === 'false' && !!start.label && !start.railVisible, start);
    await menu.click();
    const opened = { expanded: await menu.getAttribute('aria-expanded'), railVisible: await p.locator('#rail').isVisible(), focusInRail: await p.evaluate(() => document.getElementById('rail').contains(document.activeElement)), sections: await Promise.all(['manufacturing', 'verification', 'amoeba', 'admin'].map((k) => p.locator(`#rail a[data-nav=${k}]`).isVisible())) };
    check('B1 phone: the menu button reveals Manufacturing, Verification, AMOEBA and Admin, and moves focus into the menu', opened.expanded === 'true' && opened.railVisible && opened.focusInRail && opened.sections.every(Boolean), opened);
    await p.locator('#rail a[data-nav=verification]').click(); await p.waitForURL(/#\/verification/); await p.waitForFunction(() => /Verification/.test(document.querySelector('#view h1')?.textContent || ''));
    check('B1 phone: a link in the menu is clickable, and choosing it opens the page and closes the menu', (await menu.getAttribute('aria-expanded')) === 'false' && !(await p.locator('#rail').isVisible()) && (await p.textContent('#view h1')).trim() === 'Verification & Quality');
    await menu.click(); await p.keyboard.press('Escape');
    check('B1 phone: Escape closes the menu and returns focus to its button', (await menu.getAttribute('aria-expanded')) === 'false' && (await p.evaluate(() => document.activeElement.id)) === 'navtoggle');
    await p.setViewportSize({ width: 1366, height: 768 });
    check('B1: the menu button is not shown on a desktop-width screen', !(await menu.isVisible()) && (await p.locator('#rail').isVisible()));

    // #8: the REVIEW REQUIRED banner names the gates that hold the result, beside the pointer to the Gates tab
    await go(`calc/${J}`, '.pv'); await p.waitForSelector('.ws-banner');
    const revBanner = (await p.locator('.ws-banner').first().textContent()).replace(/\s+/g, ' ');
    check('#8 the REVIEW banner names the gates that hold the result (G16 among them), beside the Gates-tab pointer', /Not passed: .*G16/.test(revBanner) && /Gates tab/.test(revBanner), revBanner.slice(0, 220));

    // B5 in Japanese: the NO RESULT banner keeps the threshold, and no sentence ends in two full stops
    await p.goto(`${B}/?lang=ja#/calc/${R}`); await p.waitForSelector('.ws-banner.bad');
    const jaOutcome = (await apiGet(`/api/calculations/${R}`)).outcome || '';
    const jaThr = /s_end\s*>\s*([0-9][0-9.eE+-]*)/.exec(jaOutcome);
    const banJa = (await p.locator('.ws-banner.bad').first().textContent()).replace(/\s+/g, ' ');
    check('B5 ja: the NO RESULT banner states the threshold in Japanese, once, with no double full stop', !!jaThr && banJa.includes(jaThr[1]) && /閾値/.test(banJa) && !/。。/.test(banJa), banJa.slice(0, 220));

    // #13: on a phone in Japanese the English identifier chips keep the 16 px side gutter
    await p.setViewportSize({ width: 390, height: 844 });
    await p.goto(`${B}/?lang=ja#/calc/${J}`); await p.waitForSelector('.pv .c', { state: 'attached' });
    const chips = await p.evaluate(() => [...document.querySelectorAll('#view .idk')].map((el) => { const r = el.getBoundingClientRect(); return [Math.round(r.left), Math.round(r.right), el.textContent.trim(), r.width > 0 && el.getClientRects().length > 0]; }).filter((c) => c[3]));
    check('#13 phone, Japanese: the English identifier chips on screen stay inside the 16 px gutter', chips.length > 0 && chips.every(([l, r]) => l >= 15 && r <= 375), chips.slice(0, 6));
    await p.setViewportSize({ width: 1366, height: 768 });
    await p.goto(`${B}/?lang=en#/dashboard`); await p.waitForSelector('.stats');
  }

  // ---- 14g. work plan 3.2: BH-014 for every refusal code that names a field. A refused job, opened with "New from these
  // values", marks every field the refusal names with the message of its code, in English and in Japanese; a field the
  // form has no control for is named in the form's message area. The requests are those of crates/nf-core/tests/
  // refusal_codes.rs (no new value). The expected text is the catalogue's own message with the refusal's values.
  {
    const CAT = await p.evaluate(async () => { const m = await import('/i18n.js'); return { en: m.CATALOGUE.en, ja: m.CATALOGUE.ja, codes: m.REFUSAL_CODES }; });
    const plain = (lang, key, params) => String(CAT[lang][key] || '').replace(/\{(\w+)\}/g, (x, k) => (k in params ? params[k] : x)).replace(/\*\*/g, '').replace(/`/g, '').trim();
    const refusalText = (lang, r) => plain(lang, `refusal.${r.code}`, { fields: (r.fields || []).join(', '), limit: r.detail && r.detail.limit !== undefined ? String(r.detail.limit) : '', measured: r.detail && r.detail.measured !== undefined ? String(r.detail.measured) : '' });
    const post = async (body) => {
      allow422 = true;
      const r = await p.evaluate(async (b) => { const res = await fetch('/api/projects/1/calculations', { method: 'POST', headers: { 'content-type': 'application/json', 'x-nf-request': '1' }, body: JSON.stringify(b) }); return { st: res.status, v: await res.json() }; }, body);
      await p.waitForTimeout(100); allow422 = false;
      return r;
    };
    const stub = (x) => ({ family: 'branch_stub', header_radius: 150, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400, ...x });
    const hwb = (x) => ({ family: 'header_with_branch', header_radius: 161.95, header_dev_radius: 156.795, header_length: 1200, branch_station: 600, branch_radius: 84.15, branch_dev_radius: 80.6, beta_deg: 90, delta: 0, s_end: 400, surface_reference: 'unknown', ...x });
    const so = (hs, bs, j) => ({ family: 'header_with_branch', header_radius: 161.95, header_dev_radius: 156.795, header_length: 1200, branch_station: 600, branch_radius: 77.095, branch_dev_radius: 80.6, beta_deg: 90, delta: 0, s_end: 400, header_surface_reference: hs, branch_surface_reference: bs, joint: j });
    const CASES = [
      ['value_not_positive', { family: 'cylinder_shell', r_dev: 0, length: 250 }],
      ['magnitude_above_limit', { family: 'cylinder_shell', r_dev: 100, length: 1e8 * 1.0001 }],
      ['dimension_below_limit', { family: 'cylinder_shell', r_dev: 5e-7, length: 250 }],
      ['offset_negative', { family: 'eccentric_reducer', r1: 200, r2: 100, h: 300, e: -5 }],
      ['reducer_radii_order', { family: 'cone_frustum', r1: 100, r2: 200, h: 300 }],
      ['reducer_near_cylindrical', { family: 'cone_frustum', r1: 1e6, r2: 1e6 - 1, h: 1e6 }],
      ['developed_angle_overlaps', { family: 'cone_frustum', r1: 300, r2: 50, h: 1e-5 }],
      ['branch_angle_out_of_range', stub({ beta_deg: 0 })],
      ['branch_angle_not_verifiable', stub({ beta_deg: 0.002, s_end: Math.ceil(250 / Math.sin(0.002 * Math.PI / 180) + 150) })],
      ['branch_tangent_or_partial', stub({ branch_radius: 150, branch_dev_radius: 150 })],
      ['square_end_too_short', stub({ s_end: 0.5 })],
      ['opening_outside_shell', hwb({ branch_station: 1199.9 })],
      ['chord_error_unreachable', { family: 'branch_stub', header_radius: 1e6, branch_radius: 1e6 * (1 - 1.1e-6), branch_dev_radius: 1e6 * (1 - 1.1e-6), beta_deg: 15, delta: 0, s_end: 1e7 }],
      ['radius_dev_exceeds_outside', stub({ surface_reference: 'outside', branch_dev_radius: 100.5 })],
      ['radius_dev_below_inside', stub({ surface_reference: 'inside', branch_dev_radius: 99.5 })],
      ['radius_neutral_mismatch', stub({ surface_reference: 'neutral', branch_dev_radius: 100.5 })],
      ['surface_reference_invalid', stub({ surface_reference: 'bogus' })],
      // the form of a cylindrical shell has no surface selector (UI_CHANGES_NEEDED item 18): the field is named in the message area
      ['surface_reference_invalid', { family: 'cylinder_shell', r_dev: 500, length: 1000, surface_reference: 'bogus' }],
      ['joint_pairing_invalid', so('inside', 'inside', 'set_on')],
      ['joint_type_unknown', so('outside', 'inside', 'set-on')],
    ];
    const seen = new Set();
    for (const [code, body] of CASES) {
      const r = await post(body);
      const ok0 = r.st === 422 && r.v.code === code && r.v.state === 'REFUSED' && Number.isInteger(r.v.id) && r.v.fields.length > 0;
      const got = {};
      for (const lang of ['en', 'ja']) {
        await p.goto(`${B}/?lang=${lang}#/math/1/${r.v.id}`);
        await p.waitForFunction((h) => { const v = document.querySelector('#view'); return v && v.dataset.hash === h; }, `#/math/1/${r.v.id}`); await p.waitForSelector('#calc');
        got[lang] = await p.evaluate((fields) => {
          const form = document.querySelector('#calc'), names = { family: 'fam' };
          return {
            fields: fields.map((f) => { const el = form.elements.namedItem(names[f] || f); if (!el) return { f, control: false }; const m = el.closest('.field') && el.closest('.field').querySelector('.field-err'); const ids = (el.getAttribute('aria-describedby') || '').split(' '); return { f, control: true, aria: el.getAttribute('aria-invalid'), msg: m ? m.textContent.trim() : '', described: !!m && ids.includes(m.id) }; }),
            note: (document.querySelector('#cmsg [role=alert]') || {}).textContent || '',
          };
        }, r.v.fields);
      }
      const want = { en: refusalText('en', r.v), ja: refusalText('ja', r.v) };
      const okLang = (lang) => got[lang].fields.every((x) => (x.control ? x.aria === 'true' && x.msg === want[lang] && x.described : got[lang].note.includes(x.f)))
        && got[lang].note.includes(want[lang]) && got[lang].note.includes(String(r.v.id));
      const unmapped = got.en.fields.filter((x) => !x.control).map((x) => x.f);
      seen.add(code);
      check(`BH-014 ${code}${unmapped.length ? ` (no control for ${unmapped.join(', ')})` : ''}: New from these values marks ${r.v.fields.join(', ')} with the message of the code, in English and in Japanese, and says so in the message area`,
        ok0 && okLang('en') && okLang('ja') && /[぀-ヿ一-鿿]/.test(want.ja), { st: r.st, code: r.v.code, want, got });
    }
    // every code of the build that a stored job can carry with a field is in the list above (the others are refused
    // before a job exists: 400, below; or concern a correction: the dialog, below)
    const NOT_STORED = ['request_not_object', 'request_malformed', 'family_unknown', 'field_missing', 'field_wrong_type', 'field_unknown', 'surface_reference_required', 'value_not_finite', 'bend_angle_out_of_range', 'no_pattern_produced'];
    const CORRECTION = CAT.codes.filter((c) => c.startsWith('correction_') || c.startsWith('reason_') || c.startsWith('stored_') || c === 'corrected_result_failed_verification');
    const uncovered = CAT.codes.filter((c) => !seen.has(c) && !NOT_STORED.includes(c) && !CORRECTION.includes(c));
    check('BH-014: every stored refusal code of the build is exercised above (catalogue codes minus the 400 and correction codes)', uncovered.length === 0 && CAT.codes.length === 42, uncovered);

    // refusals answered before a job exists (400): the form marks the named field, or names it in the message area.
    // The request is changed on its way to the server, as only the API can send it.
    const sendChanged = async (fam, change, lang = 'en') => {
      await p.goto(`${B}/?lang=${lang}#/math/1`); await p.waitForFunction(() => document.querySelector('#view') && document.querySelector('#view').dataset.hash === '#/math/1'); await p.waitForSelector('#calc');
      await p.selectOption('select[name=fam]', fam);
      await p.route('**/api/projects/1/calculations', async (route) => { const b = JSON.parse(route.request().postData()); change(b); await route.continue({ postData: JSON.stringify(b) }); }, { times: 1 });
      allow400 = true; await p.click('#run'); await p.waitForSelector('#cmsg [role=alert]'); await p.waitForTimeout(100); allow400 = false;
      await p.unroute('**/api/projects/1/calculations');
      return p.evaluate(() => ({ marked: [...document.querySelectorAll('#calc [aria-invalid="true"]')].map((e) => [e.name, (e.closest('.field').querySelector('.field-err') || {}).textContent || '']), note: document.querySelector('#cmsg [role=alert]').textContent, url: location.hash }));
    };
    const before400 = (await apiGet('/api/calculations')).length;
    const u = await sendChanged('cylinder_shell', (b) => { b.thickness = 12; });
    check('BH-014 field_unknown (400): the field the form does not have is named in the message area; nothing is marked, nothing is stored', u.marked.length === 0 && u.note.includes(plain('en', 'refusal.field_unknown', { fields: 'thickness' })) && u.note.includes('thickness') && u.url === '#/math/1', u);
    const m = await sendChanged('cylinder_shell', (b) => { delete b.length; });
    check('BH-014 field_missing (400): the length field is marked with the message of the code', m.marked.length === 1 && m.marked[0][0] === 'length' && m.marked[0][1] === plain('en', 'refusal.field_missing', { fields: 'length' }), m);
    const w = await sendChanged('cylinder_shell', (b) => { b.r_dev = 'abc'; });
    check('BH-014 field_wrong_type (400): the r_dev field is marked with the message of the code', w.marked.length === 1 && w.marked[0][0] === 'r_dev' && w.marked[0][1] === plain('en', 'refusal.field_wrong_type', { fields: 'r_dev' }), w);
    const fu = await sendChanged('cylinder_shell', (b) => { b.family = 'teapot'; }, 'ja');
    check('BH-014 family_unknown (400), Japanese: the family selector is marked with the Japanese message of the code', fu.marked.length === 1 && fu.marked[0][0] === 'fam' && fu.marked[0][1] === plain('ja', 'refusal.family_unknown', { fields: 'family' }), fu);
    const sr = await sendChanged('header_with_branch', (b) => { delete b.surface_reference; });
    check('BH-014 surface_reference_required (400): the three surface selectors are marked', sr.marked.map((x) => x[0]).sort().join() === 'branch_surface_reference,header_surface_reference,surface_reference' && sr.marked.every((x) => x[1] === plain('en', 'refusal.surface_reference_required', {})), sr);
    check('BH-014: a request refused before a job exists stores no job', (await apiGet('/api/calculations')).length === before400);
  }

  // ---- 14h. work plan 3.2: every length rule counts characters, as the server does (Unicode scalar values after the
  // server's trim): a two-character Japanese reason, a reason of three characters outside the BMP (six UTF-16 units)
  // and a reason of ideographic spaces around two characters are refused in the form; a five-character Japanese reason
  // passes, and so does a project name of 150 characters outside the BMP (300 UTF-16 units, above the old maxlength).
  // A coded refusal of the correction (reason_required, correction_unknown_parameter) marks the field it names.
  {
    const hb = { family: 'header_with_branch', header_radius: 150, header_dev_radius: 150, header_length: 1200, branch_station: 600, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400, surface_reference: 'outside' };
    const created = await p.evaluate(async (b) => (await (await fetch('/api/projects/1/calculations', { method: 'POST', headers: { 'content-type': 'application/json', 'x-nf-request': '1' }, body: JSON.stringify(b) })).json()).id, hb);
    const corrections = async () => (await apiGet('/api/calculations')).length;
    await p.goto(`${B}/?lang=en#/calc/${created}`); await p.waitForFunction((h) => document.querySelector('#view') && document.querySelector('#view').dataset.hash === h, `#/calc/${created}`); await p.waitForSelector('#corr-open');
    const n0 = await corrections();
    const tryReason = async (reason) => {
      await p.click('#corr-open'); await p.waitForSelector('#corrdlg[open]');
      await p.selectOption('#corr select[name=p]', 'header_radius'); await p.fill('#corr input[name=v]', '150.5'); await p.fill('#corr input[name=r]', reason); await p.click('#corr-go');
      await p.waitForSelector('#corr .field-err');
      const r = await p.evaluate(() => { const el = document.querySelector('#corrr'); const m = el.closest('.field').querySelector('.field-err'); return { aria: el.getAttribute('aria-invalid'), msg: m ? m.textContent : '', described: !!m && (el.getAttribute('aria-describedby') || '').split(' ').includes(m.id) }; });
      await p.keyboard.press('Escape');
      return r;
    };
    const ja2 = await tryReason('日本');
    const astral3 = await tryReason('𠮷𠮷𠮷');
    const spaced = await tryReason('　　日本　　');
    check('3.2 lengths: a reason of 2 Japanese characters, of 3 characters outside the BMP (6 UTF-16 units) and of 2 characters between ideographic spaces is refused in the form, linked to the field, nothing sent',
      [ja2, astral3, spaced].every((x) => x.aria === 'true' && /at least 5 characters/.test(x.msg) && x.described) && (await corrections()) === n0, { ja2, astral3, spaced });
    // the server's own refusal of a reason, by code: the field it names is marked (the request is changed on its way)
    const coded = async (change, sel) => {
      await p.click('#corr-open'); await p.waitForSelector('#corrdlg[open]');
      await p.selectOption('#corr select[name=p]', 'header_radius'); await p.fill('#corr input[name=v]', '150.5'); await p.fill('#corr input[name=r]', '図面寸法を確認した');
      await p.route(`**/api/calculations/${created}/corrections`, async (route) => { const b = JSON.parse(route.request().postData()); change(b); await route.continue({ postData: JSON.stringify(b) }); }, { times: 1 });
      allow400 = true; allow422 = true; await p.click('#corr-go'); await p.waitForSelector('#corrmsg [role=alert]'); await p.waitForTimeout(100); allow400 = false; allow422 = false;
      await p.unroute(`**/api/calculations/${created}/corrections`);
      const r = await p.evaluate((s) => { const el = document.querySelector(s); const m = el.closest('.field').querySelector('.field-err'); return { aria: el.getAttribute('aria-invalid'), msg: m ? m.textContent : '', note: document.querySelector('#corrmsg').textContent }; }, sel);
      await p.keyboard.press('Escape');
      return r;
    };
    const rq = await coded((b) => { b.reason = 'abc'; }, '#corrr');
    const up = await coded((b) => { b.parameter = 'teapot'; }, '#corrp');
    check('3.2/BH-014: the server refusals reason_required and correction_unknown_parameter mark the reason and the parameter fields with their messages',
      rq.aria === 'true' && /at least 5 characters/.test(rq.msg) && /Every character counts as one/.test(rq.msg) && up.aria === 'true' && /not a numeric parameter of this job/.test(up.msg) && (await corrections()) === n0, { rq, up });
    // five Japanese characters are enough for the client and for the server: the correction is made
    await p.click('#corr-open'); await p.waitForSelector('#corrdlg[open]');
    await p.selectOption('#corr select[name=p]', 'header_radius'); await p.fill('#corr input[name=v]', '150.5'); await p.fill('#corr input[name=r]', '図面を確認');
    await p.click('#corr-go'); await p.waitForURL((x) => /#\/calc\/\d+$/.test(x.hash) && !x.hash.endsWith(`/${created}`));
    const corrected = Number(p.url().split('/').pop());
    check('3.2 lengths: a reason of exactly 5 Japanese characters is accepted by the form and by the server', corrected > created && (await corrections()) === n0 + 1, { corrected });
    // an obligation reason of 2 Japanese characters is refused in the form; the obligation stays open
    await p.waitForSelector('#tp-ws-changes:not([hidden]), #obls'); await p.click('#tab-ws-scrutiny'); await p.waitForSelector('#obls');
    const oid = await p.getAttribute('[data-reason]', 'data-reason');
    await p.fill(`#reason${oid}`, '日本'); await p.click(`[data-resolve="${oid}"][data-decision=confirmed]`); await p.waitForSelector(`#reason${oid}[aria-invalid=true]`);
    const obl = (await apiGet('/api/obligations')).find((o) => String(o.id) === String(oid));
    check('3.2 lengths: an obligation reason of 2 Japanese characters is refused in the form, and the obligation stays open', obl && obl.state === 'open', obl);
    // project name: 150 characters outside the BMP are 300 UTF-16 units; the server's limit is 200 characters
    await p.goto(`${B}/?lang=en#/projects`); await p.waitForSelector('#np');
    await p.fill('#np input[name=n]', '日'.repeat(201)); await p.click('#np button'); await p.waitForSelector('#np .field-err');
    const longName = await p.textContent('#np .field-err');
    const projectsBefore = (await apiGet('/api/projects')).length;
    await p.fill('#np input[name=n]', '𠮷'.repeat(150)); await p.click('#np button'); await p.waitForURL(/#\/project\/\d+$/);
    const projectsAfter = await apiGet('/api/projects');
    check('3.2 lengths: a project name of 201 characters is refused in the form (200 at most); one of 150 characters outside the BMP (300 UTF-16 units) is created',
      /at most 200 characters \(201 now\)/.test(longName) && projectsAfter.length === projectsBefore + 1 && projectsAfter.some((x) => x.name === '𠮷'.repeat(150)), { longName });
  }

  // ---- 14i. work plan 3.5: keyboard and screen reader on the new-calculation form, the job tabs and the PDF conversion
  // screen: every control named, units and hints in the description, a visible focus on every stop, the focus order of
  // the page, no keyboard trap, live regions for refusals and results.
  {
    const focusWalk = async (start, stopSel, max = 60) => {
      // reach the first stop by the keyboard (Shift+Tab, Tab), so that the browser shows focus as it does for a keyboard user
      await p.focus(start); await p.keyboard.down('Shift'); await p.keyboard.press('Tab'); await p.keyboard.up('Shift'); await p.keyboard.press('Tab');
      const stops = [];
      for (let i = 0; i < max; i++) {
        const s = await p.evaluate((stop) => {
          const el = document.activeElement; const cs = getComputedStyle(el);
          const visible = (cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0) || cs.boxShadow !== 'none';
          return { id: el.id || el.getAttribute('name') || el.tagName.toLowerCase(), tag: el.tagName.toLowerCase(), visible, inStop: !!el.closest(stop) };
        }, stopSel);
        stops.push(s);
        if (!s.inStop) break;
        await p.keyboard.press('Tab');
      }
      return stops;
    };
    await p.goto(`${B}/?lang=en#/math/1`); await p.waitForFunction(() => document.querySelector('#view') && document.querySelector('#view').dataset.hash === '#/math/1'); await p.waitForSelector('#calc');
    await p.selectOption('select[name=fam]', 'header_with_branch');
    const named = await p.evaluate(() => [...document.querySelectorAll('#calc input, #calc select, #calc button')].map((el) => {
      const desc = (el.getAttribute('aria-describedby') || '').split(' ').filter(Boolean).map((id) => (document.getElementById(id) || {}).textContent || '').join(' ').trim();
      return { name: el.name || el.id, labelled: (el.labels && el.labels.length > 0) || !!el.getAttribute('aria-label') || (el.tagName === 'BUTTON' && !!el.textContent.trim()), dim: el.hasAttribute('data-dim'), desc };
    }));
    check('3.5 form: every control of the new-calculation form has a label; every dimension field has its unit in its description', named.length > 10 && named.every((x) => x.labelled) && named.filter((x) => x.dim).every((x) => /^(mm|degrees|deg|°)$/.test(x.desc)), named.filter((x) => !x.labelled || (x.dim && !x.desc)));
    const walk = await focusWalk('#f_project', '#calc');
    const order = walk.filter((s) => s.inStop && ['input', 'select', 'button'].includes(s.tag)).map((s) => s.id);
    const domOrder = await p.evaluate(() => [...document.querySelectorAll('#calc input, #calc select, #calc button')].map((el) => el.id || el.name));
    check('3.5 form: Tab visits every control in the order of the page, each with a visible focus, and leaves the form after the run button (no trap)',
      order.join() === domOrder.join() && walk.every((s) => s.visible || !s.inStop) && !walk[walk.length - 1].inStop, { order, domOrder, invisible: walk.filter((s) => !s.visible && s.inStop).map((s) => s.id) });
    await p.keyboard.down('Shift'); await p.keyboard.press('Tab'); await p.keyboard.up('Shift');
    check('3.5 form: Shift+Tab from the next stop returns into the form', await p.evaluate(() => !!document.activeElement.closest('#calc')));
    await p.fill('#f_header_radius', '-1'); await p.click('#run'); await p.waitForSelector('#cmsg [role=alert]');
    const live = await p.evaluate(() => { const el = document.querySelector('#f_header_radius'); const m = el.closest('.field').querySelector('.field-err'); return { alert: !!document.querySelector('#cmsg [role=alert]'), described: !!m && el.getAttribute('aria-describedby').split(' ').includes(m.id) && el.getAttribute('aria-describedby').split(' ').includes('u_header_radius'), focused: document.activeElement === el, toasts: document.querySelector('#toasts').getAttribute('aria-live') };
    });
    check('3.5 form: a refused value is announced (role alert), the message joins the unit in the field description, focus moves to the field; results are announced in the polite toast region', live.alert && live.described && live.focused && live.toasts === 'polite', live);

    // the job tabs: a tab list with arrow keys, Home and End; each tab controls its panel; Tab moves from the list into the panel
    await p.goto(`${B}/?lang=en#/calc/${J}`); await p.waitForFunction((h) => document.querySelector('#view') && document.querySelector('#view').dataset.hash === h, `#/calc/${J}`); await p.waitForSelector('.pv .c', { state: 'attached' });
    await p.focus('#tab-ws-gates');
    const tabState = () => p.evaluate(() => { const a = document.activeElement; const panel = a && document.getElementById(a.getAttribute('aria-controls')); const cs = getComputedStyle(a); return { id: a.id, selected: a.getAttribute('aria-selected'), panelShown: !!panel && !panel.hidden, labelled: !!panel && panel.getAttribute('aria-labelledby') === a.id, visible: cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0 }; });
    await p.keyboard.press('ArrowRight'); const right = await tabState();
    await p.keyboard.press('End'); const end = await tabState();
    await p.keyboard.press('Home'); const home = await tabState();
    await p.keyboard.press('Tab'); const intoPanel = await p.evaluate(() => document.activeElement.getAttribute('role') === 'tabpanel' && !document.activeElement.hidden);
    check('3.5 job tabs: arrow keys, End and Home move and select the tab with a visible focus; each tab controls a labelled panel; Tab enters the shown panel',
      right.id === 'tab-ws-checks' && right.selected === 'true' && right.panelShown && right.labelled && right.visible && end.id === 'tab-ws-records' && end.panelShown && home.id === 'tab-ws-gates' && home.panelShown && intoPanel, { right, end, home, intoPanel });
    const seps = await p.evaluate(() => [...document.querySelectorAll('#view [role=separator][tabindex="0"]')].map((s) => ({ now: Number(s.getAttribute('aria-valuenow')), min: Number(s.getAttribute('aria-valuemin')), max: Number(s.getAttribute('aria-valuemax')), label: !!s.getAttribute('aria-label') })));
    await p.focus('#view [role=separator][data-split=l]'); const w0 = Number(await p.getAttribute('#view [role=separator][data-split=l]', 'aria-valuenow'));
    await p.keyboard.press('ArrowRight'); const w1 = Number(await p.getAttribute('#view [role=separator][data-split=l]', 'aria-valuenow'));
    await p.keyboard.press('ArrowLeft');
    check('3.5 job view: each panel divider is a labelled separator that states its value, and the arrow keys change it', seps.length === 2 && seps.every((s) => s.label && s.now > 0 && s.min <= s.now && s.now <= s.max) && w1 > w0, { seps, w0, w1 });

    // the PDF conversion screen
    const doc = (await apiGet('/api/projects/1')).documents.filter((d) => d.detected_type === 'pdf').pop();
    await p.goto(`${B}/?lang=en#/convert/1/${doc.id}`); await p.waitForSelector('#cv');
    const cvNamed = await p.evaluate(() => [...document.querySelectorAll('#cv input, #cv select, #cv button')].map((el) => ({ name: el.name || el.tagName, labelled: (el.labels && el.labels.length > 0) || (el.tagName === 'BUTTON' && !!el.textContent.trim()), desc: (el.getAttribute('aria-describedby') || '').split(' ').filter(Boolean).map((id) => (document.getElementById(id) || {}).textContent || '').join(' ') })));
    const cvWalk = await focusWalk('#cv_page', '#cv');
    const cvOrder = cvWalk.filter((s) => s.inStop).map((s) => s.id);
    check('3.5 conversion: every control labelled, the scale and tolerance hints in their descriptions; Tab visits page, scale kind, scale, tolerance, the two options and the button, each with a visible focus, then leaves the form',
      cvNamed.every((x) => x.labelled) && cvNamed.find((x) => x.name === 'scale').desc.length > 10 && cvNamed.find((x) => x.name === 'tol').desc.length > 10
        && cvOrder.join() === 'cv_page,cv_kind,cv_scale,cv_tol,fill,text,button' && cvWalk.filter((s) => s.inStop).every((s) => s.visible) && !cvWalk[cvWalk.length - 1].inStop, { cvNamed, cvOrder, cvWalk });
    await p.click('#cv button.pri'); await p.waitForSelector('#cvmsg [role=alert]');
    const cvErr = await p.evaluate(() => { const el = document.querySelector('#cv_scale'); const m = el.closest('.field').querySelector('.field-err'); const ids = el.getAttribute('aria-describedby').split(' '); return { aria: el.getAttribute('aria-invalid'), focused: document.activeElement === el, described: !!m && ids.includes(m.id) && ids.includes('cv_scale_h'), alert: document.querySelector('#cvmsg [role=alert]').textContent }; });
    check('3.5 conversion: a missing scale is announced in the message area, marks the scale field (message and hint in its description) and takes the focus there', cvErr.aria === 'true' && cvErr.focused && cvErr.described && cvErr.alert.length > 0, cvErr);
  }

  // ---- 14j. work plan 3.5: the offline help, in English and in Japanese. Its inputs are the form's own fields, its gate
  // names the labels of the job view and its refusal texts the messages of a refused field; it says what is not
  // modelled and that every result stays REVIEW REQUIRED and every DXF NOT RELEASED; it asks nothing of the network.
  {
    const CAT = await p.evaluate(async () => { const m = await import('/i18n.js'); return { en: m.CATALOGUE.en, ja: m.CATALOGUE.ja, codes: m.REFUSAL_CODES, gates: m.GATE_IDS, ids: m.ID_TABLE }; });
    const requests = []; const onReq = (r) => requests.push(r.url()); p.on('request', onReq);
    await p.goto(`${B}/?lang=en#/help`); await p.waitForFunction(() => document.querySelector('#view') && document.querySelector('#view').dataset.hash === '#/help'); await p.waitForSelector('#help-gates');
    const helpEn = await p.evaluate(() => ({
      text: document.querySelector('#view').textContent.replace(/\s+/g, ' '),
      fams: Object.fromEntries([...document.querySelectorAll('table[id^=help-fam-]')].map((tb) => [tb.id.slice(9), [...tb.querySelectorAll('tbody tr td:nth-child(2)')].map((td) => td.textContent.trim())])),
      gates: [...document.querySelectorAll('#help-gates tbody tr')].map((tr) => [tr.cells[0].textContent.trim(), tr.cells[1].textContent.trim()]),
      refusals: [...document.querySelectorAll('#help-refusals tbody tr')].map((tr) => [tr.cells[0].textContent.trim(), tr.cells[1].textContent.trim()]),
      nav: !!document.querySelector('.rail a[data-nav=help][aria-current=page]'),
    }));
    // the form's own fields, family by family
    const formFields = {};
    await p.goto(`${B}/?lang=en#/math/1`); await p.waitForFunction(() => document.querySelector('#view') && document.querySelector('#view').dataset.hash === '#/math/1'); await p.waitForSelector('#calc');
    for (const fam of ['cylinder_shell', 'cone_frustum', 'eccentric_reducer', 'header_with_branch', 'branch_stub']) { await p.selectOption('select[name=fam]', fam); formFields[fam] = await p.evaluate(() => [...document.querySelectorAll('#fields input, #fields select')].map((el) => el.name)); }
    p.off('request', onReq);
    const ph = { fields: 'the named field', limit: 'the limit', measured: 'the measured value' };
    const refText = (code) => String(CAT.en[`refusal.${code}`]).replace(/\{(\w+)\}/g, (x, k) => ph[k]).replace(/\*\*/g, '').replace(/`/g, '');
    check('3.5 help: the help is a page of the program (Help in the navigation), and its inputs are exactly the fields of the new-calculation form, family by family',
      helpEn.nav && Object.keys(formFields).every((f) => JSON.stringify(helpEn.fams[f]) === JSON.stringify(formFields[f])), { fams: helpEn.fams, formFields });
    check('3.5 help: every gate G00 to G22 is listed with the name the job view uses; G03, G04, G07, G09, G12, G13 and G17 are marked as not evaluated by this build',
      helpEn.gates.map((g) => g[0]).join() === Array.from({ length: 23 }, (_, i) => `G${String(i).padStart(2, '0')}`).join() && helpEn.gates.every(([id, name]) => name === CAT.ids.gate[id][0])
        && (helpEn.text.match(/not evaluated by this build/g) || []).length === 7, helpEn.gates);
    check('3.5 help: every refusal code of the build is listed with the message a refused field shows (one source)', helpEn.refusals.length === CAT.codes.length && helpEn.refusals.every(([code, text], i) => code === CAT.codes[i] && text === refText(code)), helpEn.refusals.filter(([code, text]) => text !== refText(code)).slice(0, 3));
    check('3.5 help: it says what is not modelled (thickness, bend allowance, weld gap, kerf), that it is a development build, that results stay REVIEW REQUIRED and DXF files NOT RELEASED, and keeps the EV-1 disclosure',
      /Plate thickness/.test(helpEn.text) && /Bend allowance/.test(helpEn.text) && /Weld gap/.test(helpEn.text) && /Kerf/.test(helpEn.text) && /Development build\./.test(helpEn.text) && /REVIEW REQUIRED/.test(helpEn.text) && /NOT RELEASED/.test(helpEn.text) && /EV-1 \(not independently reviewed\)/.test(helpEn.text), helpEn.text.slice(0, 300));
    const external = requests.filter((u) => !u.startsWith(B) && !u.startsWith('data:'));
    check('3.5 help: no request leaves the program (every request of the help page is to the program itself)', requests.length > 0 && external.length === 0, external);
    await p.goto(`${B}/?lang=ja#/help`); await p.waitForFunction(() => document.querySelector('#view') && document.querySelector('#view').dataset.hash === '#/help'); await p.waitForSelector('#help-gates');
    const helpJa = await p.evaluate(() => ({ gates: [...document.querySelectorAll('#help-gates tbody tr')].map((tr) => tr.cells[1].textContent.trim()), h1: document.querySelector('#view h1').textContent }));
    check('3.5 help ja: the page is in Japanese, with the Japanese gate labels of the job view', /ヘルプ/.test(helpJa.h1) && helpJa.gates.length === 23 && helpJa.gates.every((g, i) => g === CAT.ids.gate[CAT.gates[i]][1]), helpJa);
    await jaScreen('help');
    await p.goto(`${B}/?lang=ja#/help/refusals`); await p.waitForSelector('#help-refusals');
    const focusedSection = await p.waitForFunction(() => document.activeElement && document.activeElement.id === 'help-refusals-h', null, { timeout: 5000 }).then(() => true, () => false);
    check('3.5 help: a section named in the address takes the focus (keyboard and screen reader)', focusedSection, await p.evaluate(() => document.activeElement && document.activeElement.id));
  }

  // ---- 14k. work plan 3.5: three sample projects, each opened or created with one action. Each computes the example
  // values of the new-calculation form through the normal engine path, is recorded and audited like any job, and ends
  // REVIEW REQUIRED. A second action opens the same project. A viewer is offered no action.
  {
    const formExamples = {};
    await p.goto(`${B}/?lang=en#/math/1`); await p.waitForFunction(() => document.querySelector('#view') && document.querySelector('#view').dataset.hash === '#/math/1'); await p.waitForSelector('#calc');
    for (const fam of ['cylinder_shell', 'cone_frustum', 'eccentric_reducer', 'header_with_branch', 'branch_stub']) {
      await p.selectOption('select[name=fam]', fam);
      formExamples[fam] = await p.evaluate(() => Object.fromEntries([...document.querySelectorAll('#fields input')].map((el) => [el.name, Number(el.value)]).concat([...document.querySelectorAll('#fields select[name=surface_reference]')].map((el) => [el.name, el.value]))));
    }
    const results = [];
    for (const [n, fams] of [[1, ['cylinder_shell']], [2, ['cone_frustum', 'eccentric_reducer']], [3, ['header_with_branch', 'branch_stub']]]) {
      await p.goto(`${B}/?lang=en#/help/samples`); await p.waitForSelector(`[data-sample="${n}"]`);
      const label = await p.getAttribute(`[data-sample="${n}"]`, 'aria-label');
      await p.click(`[data-sample="${n}"]`); await p.waitForURL(/#\/project\/\d+$/, { timeout: 60000 });
      const pid = Number(p.url().split('/').pop());
      const proj = await apiGet(`/api/projects/${pid}`);
      const jobs = await Promise.all(proj.calculations.map((c) => apiGet(`/api/calculations/${c.id}`)));
      const sameValues = jobs.length === fams.length && fams.every((fam) => { const j = jobs.find((x) => x.request.family === fam); const want = { family: fam, ...formExamples[fam] }; return !!j && JSON.stringify(Object.keys(want).sort().map((k) => [k, want[k]])) === JSON.stringify(Object.keys(j.request).sort().map((k) => [k, j.request[k]])); });
      results.push({ n, pid, name: proj.name, label, states: jobs.map((j) => j.state), sameValues, patterns: jobs.map((j) => j.patterns.length) });
    }
    check('3.5 samples: three sample projects, each created with one action, compute the example values of the form through the engine and every job ends REVIEW REQUIRED with a pattern',
      results.length === 3 && results.every((r) => r.sameValues && r.states.every((s) => s === 'REVIEW_REQUIRED') && r.patterns.every((x) => x > 0) && /^Sample [123]: /.test(r.name) && /^Open or create Sample/.test(r.label)), results);
    const audit = await apiGet('/api/audit?limit=1000');
    const audited = results.map((r) => ({ project: audit.filter((a) => a.action === 'project_created' && a.detail.startsWith(`#${r.pid} `)).length, jobs: audit.filter((a) => a.action === 'calculation' && a.detail.includes(` project #${r.pid} -> REVIEW_REQUIRED`)).length }));
    check('3.5 samples: they are audited like any job (one project creation, and one calculation entry per job)', audited.every((a, i) => a.project === 1 && a.jobs === [1, 2, 2][i]), audited);
    const projectsBefore = (await apiGet('/api/projects')).length;
    await p.goto(`${B}/?lang=ja#/help/samples`); await p.waitForSelector('[data-sample="2"]');
    await p.click('[data-sample="2"]'); await p.waitForURL(/#\/project\/\d+$/);
    check('3.5 samples: the action on an existing sample opens it (in either language) and creates nothing', Number(p.url().split('/').pop()) === results[1].pid && (await apiGet('/api/projects')).length === projectsBefore);
    const [sctx, sv] = await role({ username: 'viewer1', password: 'read-only-pass' });
    await goOn(sv, 'help/samples', '#samples');
    const vSamples = { buttons: await sv.locator('[data-sample]').count(), note: await sv.textContent('#help-samples') };
    check('3.5 samples: a viewer is offered no sample action and is told who can create one', vSamples.buttons === 0 && /viewer cannot create projects/.test(vSamples.note), vSamples);
    await sctx.close();
    await p.goto(`${B}/?lang=en#/dashboard`); await p.waitForSelector('.stats');
  }

  // ---- 15. global assertions
  const xss = await p.evaluate(() => window.__xss || 0);
  check('security: injected markup never executed, no blocking dialog used', xss === 0 && dialogs === 0, { xss, dialogs });
  check('integrity: no control anywhere approves or releases a result; no job shown as APPROVED', approveControls === 0 && approvedTags === 0, { approveControls, approvedTags });
  check('console: zero script errors and zero unexpected console errors', errs.length === 0, errs.slice(0, 5));
  check('V-45: no content-policy violation on any screen, in either theme or language', cspViolations.length === 0, cspViolations.slice(0, 5));

  await browser.close();
  const failed = results.filter((r) => !r.ok);
  console.log('----'); console.log(`passed: ${results.length - failed.length}  failed: ${failed.length}`);
  process.exit(failed.length ? 1 : 0);
})().catch((e) => { console.error('UI TEST FAILED', e.message); process.exit(1); });
