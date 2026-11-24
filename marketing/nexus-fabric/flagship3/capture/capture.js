#!/usr/bin/env node
// Live product footage for the flagship3 film. One command per language, no hand edits:
//
//   node capture/capture.js --lang en --server /path/to/nf-server --theme light
//   node capture/capture.js --lang ja --server /path/to/nf-server --theme light
//
// The script starts its own copy of the application (a fresh data directory, a free local port, a random setup token),
// creates the jobs the film shows through the application's own API (four family jobs) and through its form (the
// header with a branch, job 5), drives the takes in the interface language asked for and the light theme, then makes the
// correction in the application (job 6, job 5 superseded), reads every value the film uses from the API, writes
// raw data files to data/<lang>/, packs the frames with capture/pack.py into capture/footage/<lang>, removes the raw
// frames and stops the server it started.
//
//   options: --server <binary> | --url <base url>      --lang en|ja      --theme light|dark (default light)
//            --out <dir>        raw frames (default capture/frames/<lang>)     --port <n>   (default: a free port)
//            --keep-frames      --no-pack      --python <python with numpy and Pillow>
//
// Determinism: the page clock is installed and paused; one frame is 1/30 s of page time; CSS animations are paused and
// stepped by the same amount. Requests to the server are real and the script waits for their results by condition, never
// by a fixed time. The pointer is not in the screenshots: its position, button state and CSS cursor are recorded per frame.
//
// Selectors: ids and data attributes only (table SEL below). When the interface changes, change SEL; a missing hook fails.
const { chromium } = require('playwright');
const fs = require('fs'), path = require('path'), os = require('os'), net = require('net'), crypto = require('crypto');
const { spawn, spawnSync } = require('child_process');

const arg = (k, d) => { const i = process.argv.indexOf('--' + k); return i > 0 && process.argv[i + 1] !== undefined ? process.argv[i + 1] : d; };
const flag = (k) => process.argv.includes('--' + k);
const LANG = arg('lang', 'en');
if (!['en', 'ja'].includes(LANG)) { console.error('--lang must be en or ja'); process.exit(2); }
const SERVER = arg('server', ''), URL0 = arg('url', '');
if (!SERVER && !URL0) { console.error('give --server <nf-server binary> (the script starts a copy) or --url <running server>'); process.exit(2); }
const DSF = 2, STEP = 1000 / 30, HERE = path.resolve(__dirname, '..');
const OUT = path.resolve(arg('out', path.join(HERE, 'capture', 'frames')), LANG);
const DATA = path.join(OUT, 'server-data');
const DATA_OUT = path.join(HERE, 'data', LANG);
const USER = 'admin', PASS = 'correct-horse-9';
const TOKEN = arg('setup-token', crypto.randomBytes(16).toString('hex'));
const THEME = arg('theme', 'light'); if (!['light', 'dark'].includes(THEME)) { console.error('--theme must be light or dark'); process.exit(2); }
const PROJECT = LANG === 'ja' ? 'H-204 ヘッダー スプール検討' : 'Header H-204 spool study';
const DEFAULT_PY = path.join(os.homedir(), '.venvs', 'nf-flagship2', 'bin', 'python');
const PY = arg('python', fs.existsSync(DEFAULT_PY) ? DEFAULT_PY : 'python3');
// the five family jobs the film draws (their vertex lists come from the API); created first, so the form job is number 5
const FAMILIES = [
  { family: 'cylinder_shell', r_dev: 500, length: 1000 },
  { family: 'cone_frustum', r1: 200, r2: 100, h: 300 },
  { family: 'eccentric_reducer', r1: 200, r2: 100, h: 300, e: 100 },
  { family: 'branch_stub', header_radius: 150, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400 },
];
const JOB = { family: 'header_with_branch', header_radius: 150, header_dev_radius: 150, header_length: 1200, branch_station: 600, branch_radius: 100, branch_dev_radius: 100, beta_deg: 60, delta: 0, s_end: 400, surface_reference: 'outside' };
const CORR = { parameter: 'header_radius', new_value: 158, reason: 'drawing dimension is the outside diameter of the thicker pipe' };
const H = { 'X-NF-Request': '1' };
const log = (...a) => console.log('[capture ' + LANG + ']', ...a);

// Stable hooks of the application (ids and data attributes). Lists are tried in order.
const SEL = {
  view: '#view', theme: '#theme', langBtn: (l) => `#lang-${l}`,
  form: { root: '#calc', fam: '#f_fam', sref: '#f_sref', station: '#f_branch_station', beta: '#f_beta_deg', run: '#run', sketchSvg: ['#sketch svg', '#sk-canvas svg'] },
  job: { pv: '#pv', fit: '#z-fit', tabHeader: '#parts [role=tab][data-part="0"]', tabBranch: '#parts [role=tab][data-part="1"]', tabsWs: '[data-tabs=ws]',
    tabGates: '#tab-ws-gates', tabChecks: '#tab-ws-checks', tabRecords: '#tab-ws-records', tabScrutiny: '#tab-ws-scrutiny', tabChanges: '#tab-ws-changes', tabGraph: '#tab-ws-graph',
    paneGates: '#tp-ws-gates', paneChecks: '#tp-ws-checks', paneRecords: '#tp-ws-records',
    checks: '#checks > li', checkChipIface: '#tp-ws-checks [data-scope=interface]', gateItems: '#tp-ws-gates [data-gate]', gateNotImpl: '#tp-ws-gates [data-gate][data-result=NOT_IMPLEMENTED]',
    records: '#tp-ws-records [data-rec]', replay: '#replay', replayState: '#replay-state', dxf: '#dxf', dxfAll: '#dxf-all', state: '.ws-head h1 .tag', head: '.ws-head',
    ro: '#ro-info', rox: '#ro-x', roy: '#ro-y', zoomv: '#zoomv', toasts: '#toasts', corrOpen: '#corr-open', corrDlg: '#corrdlg', corrP: '#corrp', corrV: '#corrv', corrR: '#corrr', corrGo: '#corr-go', successor: '#successor' },
  am: { tabRecords: '#tab-am-records', tabActivities: '#tab-am-activities', tabObligations: '#tab-am-obligations', tabLessons: '#tab-am-lessons', tabDependencies: '#tab-am-dependencies', tabMechanisms: '#tab-am-mechanisms', depSelect: '#depjob', dg: 'svg.dg' },
};
const JOB_SEL = { pv: SEL.job.pv, parts: '#parts', ro: SEL.job.ro, rox: SEL.job.rox, roy: SEL.job.roy, zoomv: SEL.job.zoomv, head: SEL.job.head, state: SEL.job.state,
  banner: '.ws-banner', statusbar: '#sb-engine', tabs: SEL.job.tabsWs, tab_gates: SEL.job.tabGates, tab_checks: SEL.job.tabChecks, tab_records: SEL.job.tabRecords,
  gates: SEL.job.paneGates, checks: SEL.job.paneChecks, records: SEL.job.paneRecords, replay: SEL.job.replay, replay_state: SEL.job.replayState, dxf: SEL.job.dxf, dxfall: SEL.job.dxfAll, fit: SEL.job.fit };

// seeded randomness for pointer paths (the same path on every run)
let seed = 20261119; const rnd = () => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 4294967296; };
const minJerk = (u) => u * u * u * (10 - 15 * u + 6 * u * u);
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const freePort = () => new Promise((res, rej) => { const s = net.createServer(); s.on('error', rej); s.listen(0, '127.0.0.1', () => { const p = s.address().port; s.close(() => res(p)); }); });
async function waitHealth(base, ms = 30000) { const t0 = Date.now(); for (;;) { try { const r = await fetch(base + '/api/health'); if (r.ok) return; } catch { /* not up yet */ } if (Date.now() - t0 > ms) throw new Error('the server did not answer at ' + base); await sleep(150); } }

let child = null;
const stopServer = () => { if (child && child.exitCode === null) child.kill('SIGTERM'); };
process.on('exit', stopServer);
process.on('SIGINT', () => { stopServer(); process.exit(130); });
process.on('SIGTERM', () => { stopServer(); process.exit(143); });

async function main() {
  fs.rmSync(OUT, { recursive: true, force: true }); fs.mkdirSync(path.join(OUT, 'downloads'), { recursive: true });
  fs.rmSync(DATA_OUT, { recursive: true, force: true }); fs.mkdirSync(DATA_OUT, { recursive: true });
  let base = URL0;
  if (SERVER) {
    const port = Number(arg('port', 0)) || (await freePort()); base = `http://127.0.0.1:${port}`;
    fs.mkdirSync(DATA, { recursive: true });
    const logFd = fs.openSync(path.join(OUT, 'server.log'), 'w');
    child = spawn(path.resolve(SERVER), [], { cwd: DATA, env: { ...process.env, NF_SETUP_TOKEN: TOKEN, NF_DATA_DIR: DATA, NF_LISTEN: `127.0.0.1:${port}` }, stdio: ['ignore', logFd, logFd] });
    log('started', path.basename(SERVER), 'at', base, '(fresh data directory)');
    await waitHealth(base);
  }
  try { await capture(base); } finally { stopServer(); }
  fs.rmSync(DATA, { recursive: true, force: true });
  if (!flag('no-pack')) {
    const r = spawnSync(PY, [path.join(HERE, 'capture', 'pack.py'), LANG, '--frames', OUT], { stdio: 'inherit' });
    if (r.status !== 0) throw new Error('pack.py failed');
    if (!flag('keep-frames')) { fs.rmSync(OUT, { recursive: true, force: true }); try { fs.rmdirSync(path.dirname(OUT)); } catch { /* not empty */ } log('raw frames removed'); }
  }
}

async function capture(base) {
  const browser = await chromium.launch({ args: ['--font-render-hinting=none', '--force-color-profile=srgb', '--disable-smooth-scrolling'] });
  try { await run(browser, base); } finally { await browser.close(); }
}

async function run(browser, base) {
  const ctx = await browser.newContext({ viewport: { width: 1920, height: 1080 }, deviceScaleFactor: DSF, timezoneId: 'UTC', locale: LANG === 'ja' ? 'ja-JP' : 'en-GB', acceptDownloads: true, colorScheme: THEME === 'light' ? 'light' : 'dark' });
  const man = { lang: LANG, url: base, viewport: [1920, 1080], dsf: DSF, fps: 30, started: new Date().toISOString(), takes: {}, notes: [], skipped: [] };

  // ---- account, project, and the four family jobs, through the API
  const health = await (await ctx.request.get(base + '/api/health')).json();
  if (health.setup_required) { const r = await ctx.request.post(base + '/api/setup', { headers: H, data: { username: USER, password: PASS, setup_token: TOKEN } }); if (!r.ok()) throw new Error('setup failed: ' + r.status()); log('first-run setup done'); }
  const lr = await ctx.request.post(base + '/api/login', { headers: H, data: { username: USER, password: PASS } }); if (!lr.ok()) throw new Error('login failed: ' + lr.status());
  const pr = await ctx.request.post(base + '/api/projects', { headers: H, data: { name: PROJECT, description: LANG === 'ja' ? '管台付きヘッダー' : 'Header with set-on branch' } });
  const projId = (await pr.json()).id; man.project = { id: projId, name: PROJECT };
  for (const req of FAMILIES) { const r = await ctx.request.post(`${base}/api/projects/${projId}/calculations`, { headers: H, data: req }); if (!r.ok()) throw new Error('family job refused: ' + req.family + ' ' + r.status()); }
  const famIds = (await (await ctx.request.get(base + '/api/calculations', { headers: H })).json()).map((c) => c.id).sort((a, b) => a - b);
  log('family jobs', famIds.join(','));

  const page = await ctx.newPage(); const errs = [];
  page.on('pageerror', (e) => errs.push('pageerror: ' + e.message));
  page.on('console', (m) => { if (m.type() === 'error') errs.push('console: ' + m.text()); });
  page.on('requestfailed', (r) => errs.push('requestfailed: ' + r.url().slice(0, 120)));
  const downloads = []; page.on('download', async (d) => { const f = path.join(OUT, 'downloads', d.suggestedFilename()); await d.saveAs(f); downloads.push(d.suggestedFilename()); });
  await page.clock.install({ time: new Date('2026-11-19T09:00:00Z') });
  const cdp = await ctx.newCDPSession(page);

  // ---- the interface language: the URL switch (?lang=), verified against the document and the language button
  await page.goto(`${base}/?lang=${LANG}#/`); await page.waitForSelector(SEL.view);
  const li = await page.evaluate((l) => ({ html: document.documentElement.lang, pressed: document.querySelector('#lang-' + l).getAttribute('aria-pressed') }), LANG);
  if (!String(li.html).startsWith(LANG) || li.pressed !== 'true') throw new Error(`the interface did not open in ${LANG} (html lang "${li.html}", button pressed "${li.pressed}")`);
  man.html_lang = li.html; man.lang_applied = 'query';
  // the application's own theme (the top-bar toggle); the choice is kept in localStorage by the application
  const ensureTheme = async () => {
    const cur = await page.evaluate(() => document.documentElement.getAttribute('data-theme'));
    if (cur !== THEME) { await page.click(SEL.theme); await page.waitForFunction((t) => document.documentElement.getAttribute('data-theme') === t, THEME, { timeout: 10000 }); }
    return page.evaluate(() => document.documentElement.getAttribute('data-theme'));
  };
  man.theme = await ensureTheme();

  // ---- frame recorder
  let take = null, cur = { x: 1180, y: 640, down: false };
  const stepAnims = (dt) => { const seen = (window.__nfAnims ||= new Map()); for (const a of document.getAnimations()) { if (!seen.has(a)) { a.pause(); a.currentTime = 0; seen.set(a, 0); } else { const t = seen.get(a) + dt; seen.set(a, t); const end = a.effect && a.effect.getComputedTiming().endTime; if (Number.isFinite(end) && t >= end) a.finish(); else a.currentTime = t; } } };
  const probe = ({ x, y, sel }) => {
    const r4 = (e) => { const r = e.getBoundingClientRect(); return [+r.x.toFixed(2), +r.y.toFixed(2), +r.width.toFixed(2), +r.height.toFixed(2)]; };
    const o = { rects: {} }; const at = document.elementFromPoint(x, y); o.cursor = at ? getComputedStyle(at).cursor : 'auto';
    for (const [k, s] of Object.entries(sel || {})) { if (typeof s !== 'string') continue; const e = document.querySelector(s); if (e && e.getClientRects().length) o.rects[k] = r4(e); }
    const g = document.querySelector('#pv g[transform]') || document.querySelector('.pv g[transform]'), pv = document.querySelector('#pv') || document.querySelector('.pv');
    if (g && pv) { const m = /matrix\(([^)]+)\)/.exec(g.getAttribute('transform') || ''); if (m) { const v = m[1].split(/[ ,]+/).map(Number); o.view = [v[0], v[4], v[5], ...r4(pv)]; } }
    const ro = document.querySelector('#ro-info'); if (ro) o.ro = [ro.textContent.trim(), (document.querySelector('#ro-x') || {}).textContent || '', (document.querySelector('#ro-y') || {}).textContent || ''];
    const zv = document.querySelector('#zoomv'); if (zv) o.zoom = zv.textContent.trim();
    return o;
  };
  const begin = (name, sel) => { take = { name, sel: sel || {}, frames: [], marks: {} }; man.takes[name] = take; fs.mkdirSync(path.join(OUT, name), { recursive: true }); log('take', name); };
  const frame = async (mark) => {
    await page.clock.runFor(STEP); await page.evaluate(stepAnims, STEP);
    const i = take.frames.length, file = `${take.name}/${String(i).padStart(5, '0')}.png`;
    const shot = await cdp.send('Page.captureScreenshot', { format: 'png', optimizeForSpeed: true, clip: { x: 0, y: 0, width: 1920, height: 1080, scale: DSF } });
    fs.writeFileSync(path.join(OUT, file), Buffer.from(shot.data, 'base64'));
    const st = await page.evaluate(probe, { x: cur.x, y: cur.y, sel: take.sel });
    take.frames.push({ f: file, c: [+cur.x.toFixed(1), +cur.y.toFixed(1), cur.down ? 1 : 0, st.cursor], r: st.rects, v: st.view, ro: st.ro, z: st.zoom });
    if (mark) take.marks[mark] = i;
    return i;
  };
  const idle = async () => { await page.clock.runFor(STEP); await page.evaluate(stepAnims, STEP); };
  const hold = async (n, mark) => { for (let k = 0; k < n; k++) await frame(k === 0 ? mark : undefined); };
  const moveTo = async (x, y, n, mark) => { // minimum-jerk path with a slight seeded arc
    const x0 = cur.x, y0 = cur.y, d = Math.hypot(x - x0, y - y0); n = n || Math.max(10, Math.min(34, Math.round(9 + d / 38)));
    const bow = (rnd() - 0.5) * 0.16 * d, nx = d ? -(y - y0) / d : 0, ny = d ? (x - x0) / d : 0;
    for (let k = 1; k <= n; k++) { const u = minJerk(k / n), w = Math.sin(Math.PI * u) * bow; cur.x = x0 + (x - x0) * u + nx * w; cur.y = y0 + (y - y0) * u + ny * w; await page.mouse.move(cur.x, cur.y); await frame(k === 1 ? mark : undefined); }
  };
  const centre = async (sel, fx = 0.5, fy = 0.5) => { const b = await page.locator(sel).first().boundingBox(); if (!b) throw new Error('not visible: ' + sel); return [b.x + b.width * fx, b.y + b.height * fy]; };
  const down = async (mark) => { await page.mouse.down(); cur.down = true; await frame(mark); };
  const up = async (mark) => { await page.mouse.up(); cur.down = false; await frame(mark); };
  const click = async (sel, opt = {}) => { const [x, y] = await centre(sel, opt.fx, opt.fy); await moveTo(x, y, opt.n, opt.mark && opt.mark + ':move'); await hold(opt.pre ?? 3); await down(opt.mark && opt.mark + ':down'); await hold(1); await up(opt.mark && opt.mark + ':up'); };
  const typeInto = async (sel, text, perFrame, mark) => { await click(sel, { mark }); for (let i = 0; i < text.length; i += perFrame) { await page.keyboard.type(text.slice(i, i + perFrame)); await frame(i === 0 && mark ? mark + ':typing' : undefined); } await hold(4); };
  const key = async (k, mark) => { await page.keyboard.press(k); return frame(mark); };
  const gotoHash = async (hash, waitSel) => { await page.evaluate((h) => { location.hash = h; }, hash); await page.waitForSelector(waitSel, { state: 'attached' }); await idle(); };
  // pattern-space vertex -> screen (the viewer's own transform, read from its hit element)
  const vpos = (ci, vi) => page.evaluate(([c, v]) => { const pl = document.querySelector(`#pv .hit[data-ci="${c}"], .pv .hit[data-ci="${c}"]`); const q = pl.points[((v % pl.points.length) + pl.points.length) % pl.points.length], m = pl.getScreenCTM(); return [q.x * m.a + q.y * m.c + m.e, q.x * m.b + q.y * m.d + m.f]; }, [ci, vi]);
  const curveCount = (ci) => page.evaluate((c) => document.querySelector(`#pv .hit[data-ci="${c}"], .pv .hit[data-ci="${c}"]`).points.length, ci);
  // the curve with the most vertices in the visible part: the opening of the header, or the saddle cut of the branch
  const bigCi = () => page.evaluate(() => { let best = 0, n = 0; document.querySelectorAll('#pv .hit, .pv .hit').forEach((h) => { if (h.points.length > n) { n = h.points.length; best = Number(h.dataset.ci); } }); return best; });
  // a point inside the visible part of the tab panel (the panel itself can be taller than the window)
  const paneSpot = async (fy = 0.6) => { const b = await page.locator(SEL.job.tabsWs).first().boundingBox(); return [b.x + b.width * 0.5, b.y + b.height + (1080 - 30 - (b.y + b.height)) * fy]; };
  const rowInfo = (sel) => page.evaluate((sel) => { const r4 = (e) => { const r = e.getBoundingClientRect(); return [+r.x.toFixed(2), +r.y.toFixed(2), +r.width.toFixed(2), +r.height.toFixed(2)]; }; return [...document.querySelectorAll(sel)].filter((li) => !li.hidden && li.getClientRects().length).map((li) => ({ scope: li.dataset.scope || '', id: (li.querySelector('.l1 .id') || {}).textContent || '', rect: r4(li), name: (li.querySelector('.l1 b') || {}).textContent || '', tag: li.querySelector('.l1 .tag') ? { text: li.querySelector('.l1 .tag').textContent.trim(), rect: r4(li.querySelector('.l1 .tag')) } : null, cells: [...li.querySelectorAll('.quad dd')].map((d) => ({ text: d.textContent.trim(), rect: r4(d) })) })); }, sel);
  // steps the page clock (no frames) until the application's notifications have closed by themselves
  const toastsGone = async () => { for (let k = 0; k < 400; k++) { const n = await page.evaluate((s) => document.querySelectorAll(s + ' > *').length, SEL.job.toasts); if (!n) return; await idle(); } throw new Error('a notification did not close'); };
  const paneTop = async () => { const [x, y] = await paneSpot(0.5); await page.mouse.move(x, y); await page.mouse.wheel(0, -20000); await idle(); await page.mouse.move(cur.x, cur.y); await idle(); };
  const sketchGeom = () => page.evaluate((list) => { const sel = list.find((s) => document.querySelector(s)); const s = sel && document.querySelector(sel); if (!s) return null; const m = s.getScreenCTM(); return { ctm: [m.a, m.b, m.c, m.d, m.e, m.f], paths: [...s.querySelectorAll(':scope > path')].map((p) => [p.getAttribute('class') || '', p.getAttribute('d')]) }; }, SEL.form.sketchSvg);
  const sketchSel = await page.evaluate((list) => list.find((s) => document.querySelector(s)) || list[0], SEL.form.sketchSvg);
  const readJSON = async (p) => (await ctx.request.get(base + p, { headers: H })).json();

  // ================================================================= take 1: the form (job 5, typed values)
  await page.goto(`${base}/?lang=${LANG}#/math/${projId}`); await page.waitForSelector(SEL.form.root);
  if ((await ensureTheme()) !== THEME) throw new Error('the theme did not switch to ' + THEME);
  await page.clock.pauseAt(new Date('2026-11-19T09:00:10Z'));
  await page.selectOption(SEL.form.fam, JOB.family); await page.selectOption(SEL.form.sref, JOB.surface_reference);
  for (const [k, v] of Object.entries(JOB)) if (typeof v === 'number') await page.fill('#f_' + k, String(v));
  // start from a perpendicular branch at another station, so that entering this job's values is visible in the to-scale schematic
  await page.fill(SEL.form.beta, '90'); await page.fill(SEL.form.station, '450');
  await page.evaluate(() => { if (document.activeElement) document.activeElement.blur(); });
  await page.mouse.move(cur.x, cur.y); for (let k = 0; k < 6; k++) await idle();
  const FORM_SEL = { form: SEL.form.root, fields: '#fields', sketch_svg: sketchSel, run: SEL.form.run, station: SEL.form.station, beta: SEL.form.beta, fam: SEL.form.fam, statusbar: '#sb-engine' };
  begin('form', FORM_SEL);
  await hold(8, 'start'); take.sketch0 = await sketchGeom();
  await click(SEL.form.station, { fx: 0.3, mark: 'station' });
  await key('Control+A', 'station:select'); await hold(3);
  for (const [i, ch] of [...'600'].entries()) { await page.keyboard.type(ch); await frame('station:key' + i); await hold(2); }
  await hold(6, 'station:done');
  await click(SEL.form.beta, { fx: 0.3, mark: 'beta' });
  await hold(5);
  const from = Number(await page.inputValue(SEL.form.beta));
  for (let b = from; b > JOB.beta_deg; b--) await key('ArrowDown', b === from ? 'sweep:start' : undefined);
  await hold(10, 'sweep:end'); take.sketch1 = await sketchGeom();
  const req = await page.evaluate(() => { const o = {}; for (const e of document.querySelectorAll('#fields input, #fields select')) o[e.name] = e.type === 'number' ? Number(e.value) : e.value; return o; });
  for (const [k, v] of Object.entries(JOB)) if (k !== 'family' && req[k] !== v) throw new Error(`form value ${k}=${req[k]} differs from the reference job (${v})`);
  let release, serverMs = 0; const gate = new Promise((r) => { release = r; });
  await page.route('**/api/projects/*/calculations', async (route) => { const t0 = Date.now(); const resp = await route.fetch(); serverMs = Date.now() - t0; await gate; await route.fulfill({ response: resp }); });
  const [rx, ry] = await centre(SEL.form.run); await moveTo(rx, ry, undefined, 'run:move'); await hold(5);
  await down('run:down'); await hold(2); await up('run:up');
  for (let k = 0; k < 400 && !serverMs; k++) await sleep(25);
  if (!serverMs) throw new Error('the calculation request did not reach the server');
  const busy = Math.max(2, Math.ceil(serverMs / STEP)); take.server_ms = serverMs; take.busy_frames = busy;
  await hold(busy, 'run:busy');
  release(); await page.waitForURL(/#\/calc\/\d+$/); await page.waitForSelector('#pv .c, .pv .c', { state: 'attached' }); await page.unroute('**/api/projects/*/calculations');
  const jobId = Number(page.url().split('/').pop()); man.job_id = jobId;
  if (jobId !== 5) throw new Error('the form job is number ' + jobId + ', expected 5');

  // ================================================================= take 2: the result arrives
  begin('result', JOB_SEL); await hold(34, 'start');

  // ================================================================= take 3: the canvas, opening, zoom, fit, branch part, saddle cut
  begin('canvas', JOB_SEL);
  const hole = await bigCi(); take.hole_ci = hole; let n = await curveCount(hole);
  await hold(4, 'start');
  let [x, y] = await vpos(hole, Math.round(n * 0.86)); await moveTo(x, y, 28, 'approach');
  for (let k = 0; k <= 22; k++) { [x, y] = await vpos(hole, Math.round(n * (0.86 - 0.11 * minJerk(k / 22)))); cur.x = x; cur.y = y; await page.mouse.move(x, y); await frame(k === 0 ? 'trace:start' : k === 22 ? 'trace:end' : undefined); }
  await hold(4);
  for (let k = 0; k < 7; k++) { await page.mouse.wheel(0, -100); await frame(k === 0 ? 'zoom:start' : undefined); [x, y] = await vpos(hole, Math.round(n * 0.75)); cur.x = x; cur.y = y; await page.mouse.move(x, y); await frame(); }
  await hold(12, 'zoom:end');
  await click(SEL.job.fit, { mark: 'fit', pre: 2 }); await hold(6, 'fitted');
  const tb = await (await page.locator(SEL.job.tabBranch).first().boundingBox());
  await moveTo(tb.x + tb.width / 2, tb.y + tb.height / 2, 16, 'tab:move'); await hold(2); await down('tab:down'); await up('tab:up'); await hold(10, 'shown');
  const saddle = await bigCi(); n = await curveCount(saddle); take.saddle_ci = saddle; take.saddle_n = n;
  [x, y] = await vpos(saddle, 0); await moveTo(x, y, 24, 'saddle:approach');
  await hold(4, 'saddle:on');
  for (let k = 0; k <= 150; k++) { const vi = Math.round((n - 1) * k / 150); [x, y] = await vpos(saddle, vi); cur.x = x; cur.y = y; await page.mouse.move(x, y); const i = await frame(k === 0 ? 'saddle:start' : k === 150 ? 'saddle:end' : undefined); take.frames[i].vi = vi; }
  await hold(10, 'saddle:hold');

  // ================================================================= take 4: checks
  begin('checks', { ...JOB_SEL, chips: SEL.job.paneChecks + ' [data-scope]', list: '#checks', sum: SEL.job.paneChecks + ' .tag' });
  await hold(3, 'start');
  await click(SEL.job.tabChecks, { mark: 'tab' }); await hold(14, 'shown');
  take.rows_all = await rowInfo(SEL.job.checks);
  const [lx, ly] = await paneSpot(0.55); await moveTo(lx, ly, 14, 'scroll:move');
  for (let k = 0; k < 18; k++) { await page.mouse.wheel(0, 80); await frame(k === 0 ? 'scroll:start' : undefined); }
  await hold(6, 'scroll:end');
  for (let k = 0; k < 4; k++) { await page.mouse.wheel(0, -480); await frame(k === 0 ? 'scroll:back' : undefined); }
  await hold(3, 'scroll:top');
  await click(SEL.job.checkChipIface, { mark: 'iface' }); await hold(20, 'iface:shown');
  const rows = await rowInfo(SEL.job.checks); take.rows = rows;
  const ifaceRows = rows.map((r, i) => [r, i]).filter(([r]) => r.scope === 'interface');
  const jobChecks = (await readJSON(`/api/calculations/${jobId}`)).checks || [];
  const want = jobChecks.filter((c) => c.scope === 'interface').findIndex((c) => c.name === 'branch_cut_lies_on_header_opening');
  if (want < 0 || !ifaceRows[want]) throw new Error('the interface check branch_cut_lies_on_header_opening is not in the checks list');
  const target = ifaceRows[want];
  take.iface_row = target[1]; take.iface_rows = ifaceRows.map(([r]) => r.name);
  const c = target[0].cells[2] || { rect: target[0].rect };
  await moveTo(c.rect[0] + c.rect[2] * 0.5, c.rect[1] + c.rect[3] + 16, 18, 'iface:point'); await hold(14, 'iface:hold');
  await paneTop();

  // ================================================================= take 5: replay audit and records (job 5)
  begin('records', { ...JOB_SEL, recs: SEL.job.paneRecords, rec0: SEL.job.records });
  await hold(3, 'start');
  const [rx2, ry2] = await centre(SEL.job.replay); await moveTo(rx2, ry2, undefined, 'replay:move'); await hold(4); await down('replay:down'); await up('replay:up');
  await page.waitForFunction((s) => { const e = document.querySelector(s); return e && /\S/.test(e.textContent); }, SEL.job.replayState, { timeout: 60000 });
  await hold(26, 'replayed');
  const [lx2, ly2] = await paneSpot(0.6); await moveTo(lx2, ly2, 16, 'scroll:move');
  for (let k = 0; k < 22; k++) { await page.mouse.wheel(0, 70); await frame(k === 0 ? 'scroll:start' : undefined); }
  await hold(6, 'scroll:end');
  take.record_count = await page.locator(SEL.job.records).count();
  const idx = await page.evaluate((s) => { const rs = [...document.querySelectorAll(s)]; const vis = rs.map((r, i) => [i, r.getBoundingClientRect()]).filter(([, b]) => b.top > 240 && b.bottom < innerHeight - 260); return vis.length ? vis[Math.floor(vis.length / 3)][0] : 0; }, SEL.job.records);
  const sb = await page.locator(SEL.job.records).nth(idx).locator('summary').boundingBox();
  await moveTo(sb.x + sb.width * 0.3, sb.y + sb.height / 2, 14, 'rec:move'); await hold(3); await down('rec:down'); await up('rec:up'); await hold(20, 'rec:open');
  await toastsGone(); await paneTop();

  // ================================================================= take 6: gates (after the replay, so the summary is the complete one)
  begin('gates', { ...JOB_SEL, sum: SEL.job.paneGates });
  await hold(3, 'start');
  await click(SEL.job.tabGates, { mark: 'tab' }); await hold(18, 'shown');
  const tags = () => page.evaluate((s) => [...document.querySelectorAll(s + ' [data-gate]')].map((li) => { const t = li.querySelector('.tag'); const r = (t || li).getBoundingClientRect(); return { id: (li.querySelector('.id') || {}).textContent || li.dataset.gate, result: li.dataset.result, text: t ? t.textContent.trim() : '', rect: [+r.x.toFixed(1), +r.y.toFixed(1), +r.width.toFixed(1), +r.height.toFixed(1)] }; }), SEL.job.paneGates);
  take.gate_rows = await tags();
  const [gx, gy] = await paneSpot(0.6); await moveTo(gx, gy, 18, 'scroll:move');
  for (let k = 0; k < 40; k++) {
    const y0 = await page.evaluate((s) => { const li = document.querySelector(s); return li ? li.getBoundingClientRect().top : null; }, SEL.job.gateNotImpl);
    if (y0 !== null && y0 < 560) break;
    await page.mouse.wheel(0, 60); await frame(k === 0 ? 'scroll:start' : undefined);
  }
  await hold(14, 'scroll:end'); take.gate_rows_end = (await tags()).filter((t) => t.rect[1] > 0 && t.rect[1] < 1080);
  const [sx, sy] = await centre(SEL.job.state); await moveTo(sx + 4, sy + 30, 24, 'state:move'); await hold(16, 'state');
  await paneTop();

  // ================================================================= take 7: DXF (job 5)
  begin('dxf', JOB_SEL);
  await hold(3, 'start');
  const tha = await page.locator(SEL.job.tabHeader).first().boundingBox();
  await moveTo(tha.x + tha.width / 2, tha.y + tha.height / 2, 20, 'header:move'); await down(); await up('header:shown'); await hold(8);
  const [dx, dy] = await centre(SEL.job.dxf); await moveTo(dx, dy, undefined, 'dxf:move'); await hold(6, 'dxf:hover');
  await down('dxf:down'); await hold(1); await up('dxf:up');
  for (let k = 0; k < 60 && !downloads.length; k++) await sleep(50);
  if (!downloads.length) throw new Error('the DXF download did not start');
  await hold(14, 'dxf:written');
  const [ax, ay] = await centre(SEL.job.dxfAll); await moveTo(ax, ay, 12, 'all:move'); await hold(3); await down('all:down'); await up('all:up');
  for (let k = 0; k < 40; k++) { await frame(k === 0 ? 'all:run' : undefined); if (downloads.length >= 3 && k > 24) break; }
  await hold(20, 'all:done');

  // ================================================================= take 8: the interface language, by the top-bar switch (job 5 page)
  const other = LANG === 'ja' ? 'en' : 'ja';
  await page.mouse.move(cur.x, cur.y);
  await gotoHash(`#/calc/${jobId}`, SEL.job.state);
  begin('lang', { ...JOB_SEL, lang_en: SEL.langBtn('en'), lang_ja: SEL.langBtn('ja') });
  await hold(26, 'start');
  await click(SEL.langBtn(other), { mark: 'switch' }); await hold(42, 'switched');
  await click(SEL.langBtn(LANG), { mark: 'back' }); await hold(42, 'back');
  man.lang_switch_ok = await page.evaluate(() => document.documentElement.lang);

  // ================================================================= take 9: AMOEBA Control Centre (records, activities), then the dependency graph of job 5 (before the correction)
  begin('am_pre', { ...JOB_SEL, am_tabs: '[role=tablist]', dg: SEL.am.dg });
  await gotoHash('#/amoeba', SEL.am.tabRecords); await hold(40, 'start');
  await click(SEL.am.tabActivities, { mark: 'activities' }); await hold(34, 'activities:shown');
  await click(SEL.am.tabDependencies, { mark: 'deps' }); await hold(14, 'deps:shown');
  await page.waitForSelector(SEL.am.dg, { state: 'attached' });
  await hold(4);
  await click(SEL.am.depSelect, { mark: 'pick', pre: 4 });
  await page.selectOption(SEL.am.depSelect, String(jobId)); await hold(8, 'picked');
  await hold(60, 'graph');

  // ================================================================= take 10: the correction, made in the application (job 5 -> job 6)
  begin('correct', { ...JOB_SEL, dialog: SEL.job.corrDlg, corrp: SEL.job.corrP, corrv: SEL.job.corrV, corrr: SEL.job.corrR, go: SEL.job.corrGo });
  await gotoHash(`#/calc/${jobId}`, SEL.job.corrOpen); await hold(30, 'start');
  await click(SEL.job.corrOpen, { mark: 'open' }); await hold(20, 'dialog');
  await page.selectOption(SEL.job.corrP, CORR.parameter); await hold(12, 'param');
  await typeInto(SEL.job.corrV, String(CORR.new_value), 1, 'value');
  await typeInto(SEL.job.corrR, CORR.reason, 7, 'reason');
  await hold(10, 'filled');
  await click(SEL.job.corrGo, { mark: 'go', pre: 4 });
  await page.waitForURL(/#\/calc\/6$/, { timeout: 60000 });
  await page.waitForSelector(SEL.job.tabChanges, { state: 'attached' });
  await hold(20, 'created');
  await toastsGone();
  const corrJob = await readJSON(`/api/calculations/6`);
  if (!corrJob.supersedes || corrJob.supersedes !== jobId) throw new Error('the correction did not create job 6 from job 5');
  man.correction = { new_calculation: 6, obligations: (corrJob.obligations || []).length, open: (corrJob.obligations || []).filter((o) => o.state === 'open').length };

  // ================================================================= take 11: the superseded job, the new job's changes and scrutiny obligations (job 6)
  begin('superseded', JOB_SEL);
  await gotoHash(`#/calc/${jobId}`, SEL.job.state); await hold(50, 'start');
  begin('changes', JOB_SEL);
  await gotoHash('#/calc/6', SEL.job.tabChanges); await hold(40, 'start');
  await click(SEL.job.tabScrutiny, { mark: 'scrutiny' }); await hold(18, 'scrutiny:shown');
  const sp = await paneSpot(0.5); await moveTo(sp[0], sp[1], 16, 'scroll:move');
  for (let k = 0; k < 14; k++) { await page.mouse.wheel(0, 60); await frame(k === 0 ? 'scroll:start' : undefined); }
  await hold(40, 'scrutiny:end');
  await paneTop();

  // ================================================================= take 12: AMOEBA after the correction: obligations, lessons, the graph of job 6
  begin('am_post', { ...JOB_SEL, dg: SEL.am.dg });
  await gotoHash('#/amoeba/obligations', SEL.am.tabObligations); await hold(60, 'obligations');
  await click(SEL.am.tabLessons, { mark: 'lessons' }); await hold(44, 'lessons:shown');
  await click(SEL.am.tabDependencies, { mark: 'deps' }); await hold(10, 'deps:shown');
  await page.waitForSelector(SEL.am.dg, { state: 'attached' });
  await click(SEL.am.depSelect, { mark: 'pick', pre: 4 });
  await page.selectOption(SEL.am.depSelect, '6'); await hold(8, 'picked');
  await hold(80, 'graph6');

  // ================================================================= take 13: the replay audit of the corrected job (job 6)
  begin('replay6', JOB_SEL);
  await gotoHash('#/calc/6', SEL.job.replay); await hold(30, 'start');
  await click(SEL.job.replay, { mark: 'replay', pre: 4 });
  await page.waitForFunction((s) => { const e = document.querySelector(s); return e && /\S/.test(e.textContent); }, SEL.job.replayState, { timeout: 60000 });
  await hold(60, 'replayed');
  const rState = await page.evaluate((s) => document.querySelector(s).textContent.trim(), SEL.job.replayState); man.replay6_text = rState;

  // ---- the job's data through the API: every number the film shows is read from here
  const jobs = {}; for (const id of [1, 2, 3, 4, 5, 6]) jobs[id] = await readJSON(`/api/calculations/${id}`);
  const deps = { 5: await readJSON('/api/calculations/5/dependencies'), 6: await readJSON('/api/calculations/6/dependencies') };
  const obligations = await readJSON('/api/obligations');
  const amoeba = await readJSON('/api/amoeba/summary');
  const lessons = await readJSON('/api/lessons');
  const dashboard = await readJSON('/api/dashboard');
  const dxf = {};
  for (const id of [5, 6]) for (const p of jobs[id].patterns || []) {
    const r = await ctx.request.get(`${base}/api/calculations/${id}/dxf/${encodeURIComponent(p.part)}`, { headers: H });
    if (!r.ok()) { dxf[`${id}-${p.part}`] = { status: r.status() }; continue; }
    const cd = r.headers()['content-disposition'] || ''; const name = (/filename="?([^";]+)/.exec(cd) || [])[1] || `${p.part}.dxf`;
    const text = await r.text(); dxf[`${id}-${p.part}`] = { name, bytes: Buffer.byteLength(text), text };
  }
  for (const [k, v] of Object.entries(dxf)) if (v.text) { fs.writeFileSync(path.join(DATA_OUT, `dxf-${k}.dxf`), v.text); (man.dxf ||= {})[k] = { name: v.name, bytes: v.bytes }; }
  for (const [id, j] of Object.entries(jobs)) fs.writeFileSync(path.join(DATA_OUT, `job-${id}.json`), JSON.stringify(j, null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'deps-5.json'), JSON.stringify(deps[5], null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'deps-6.json'), JSON.stringify(deps[6], null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'obligations.json'), JSON.stringify(obligations, null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'checks-5.json'), JSON.stringify(jobs[5].checks, null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'gates-5.json'), JSON.stringify(jobs[5].gates, null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'gates-6.json'), JSON.stringify(jobs[6].gates, null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'amoeba.json'), JSON.stringify(amoeba, null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'lessons.json'), JSON.stringify(lessons, null, 1));
  fs.writeFileSync(path.join(DATA_OUT, 'dashboard.json'), JSON.stringify(dashboard, null, 1));
  // the film's bundle: what pack.py embeds in the footage file
  const product = { lang: LANG, family_ids: { cylinder_shell: 1, cone_frustum: 2, eccentric_reducer: 3, branch_stub: 4, header_with_branch: 5 }, jobs: Object.fromEntries(Object.entries(jobs).map(([id, j]) => [id, j])),
    deps: { 5: deps[5], 6: deps[6] }, obligations, amoeba, lessons, dashboard, dxf: Object.fromEntries(Object.entries(dxf).map(([k, v]) => [k, { name: v.name, bytes: v.bytes, text: v.text }])), replay6_text: rState, correction: man.correction, html_lang: man.html_lang };
  fs.writeFileSync(path.join(OUT, 'product.json'), JSON.stringify(product));
  fs.writeFileSync(path.join(DATA_OUT, 'capture.json'), JSON.stringify({ lang: LANG, html_lang: man.html_lang, theme: man.theme, project: man.project, job_id: jobId, correction: man.correction, replay6_text: rState, engine: jobs[5].engine, closure_hash: { 5: jobs[5].closure_hash, 6: jobs[6].closure_hash }, server: 'nf-server (copy started by the capture)', finished: new Date().toISOString() }, null, 1));
  man.downloads = downloads; man.console_errors = errs; man.engine = jobs[5].engine; man.job_state = jobs[5].state; man.finished = new Date().toISOString();
  man.frames_total = Object.values(man.takes).reduce((k, t) => k + t.frames.length, 0);
  fs.writeFileSync(path.join(OUT, 'manifest.json'), JSON.stringify(man));
  fs.copyFileSync(path.join(OUT, 'manifest.json'), path.join(DATA_OUT, 'manifest.json'));
  log('frames', man.frames_total, '| job', jobId, jobs[5].state, '| correction job 6', jobs[6].state, '| engine', jobs[5].engine, '| html lang', man.html_lang, '| downloads', downloads.length, '| page errors', errs.length, errs.slice(0, 3));
}

main().catch((e) => { console.error('[capture ' + LANG + '] FAILED:', e && e.stack || e); process.exit(1); });
