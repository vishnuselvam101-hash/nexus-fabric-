/* NEXUS FABRIC — job simple view smoke (lane F1): drop 2 PDFs -> order -> pages -> parts -> compute -> result.
   Usage: NF_BASE=http://127.0.0.1:8801 node tests/ui_job_f1_smoke.js [shot-dir]
   The server must be a fresh one (first-run setup creates the admin). Headless Chromium, EN and JA, 1440 and 390. */
'use strict';

const fs = require('fs');
const path = require('path');
const { chromium, request } = require('playwright');

const BASE = (process.env.NF_BASE || 'http://127.0.0.1:8801').replace(/\/$/, '');
const SETUP_TOKEN = process.env.NF_SETUP_TOKEN || 'ui-job-f1-setup-token';
const SHOT_DIR = process.argv[2] || '';
const FIXTURE = path.join(__dirname, '..', 'crates', 'nf-pdf', 'tests', 'fixtures', 'reportlab_lines.pdf');
const USER = 'admin';
const PASS = 'correct-horse-9';
const HDR = { 'X-NF-Request': '1' };
// the parts the order holds (same shape the server tests use): a cylinder and a branch on it
const PARTS = [
  { no: '1', name: 'main', shape: 'cylinder', qty: 2, page: 1 },
  { no: '2', name: 'branch', shape: 'tee_branch', page: 1, attach_part: '1' },
];
const RAW_KEY = /\bjf1\.[a-z.]+\b/;

let passed = 0, failed = 0;
const check = (name, ok, detail) => {
  if (ok) { passed++; console.log('ok  - ' + name); }
  else { failed++; console.log('FAIL - ' + name + (detail ? ' [' + detail + ']' : '')); }
};
const msgOf = (e) => (e && e.message) || String(e);

async function main() {
  if (SHOT_DIR) fs.mkdirSync(SHOT_DIR, { recursive: true });
  const api = await request.newContext({ baseURL: BASE, extraHTTPHeaders: HDR });
  const cspViolations = [];
  const pageErrors = [];
  let browser = null;
  try {
    const health = await (await api.get('/api/health')).json();
    if (health.setup_required) {
      const r = await api.post('/api/setup', { data: { username: USER, password: PASS, setup_token: SETUP_TOKEN } });
      check('first-run setup', r.ok(), 'status ' + r.status());
    }
    const login = await api.post('/api/login', { data: { username: USER, password: PASS } });
    check('sign in through the API', login.ok(), 'status ' + login.status());
    const pdfs = [
      { name: 'drawing-a.pdf', mimeType: 'application/pdf', buffer: fs.readFileSync(FIXTURE) },
      { name: 'drawing-b.pdf', mimeType: 'application/pdf', buffer: fs.readFileSync(FIXTURE) },
    ];

    browser = await chromium.launch();
    const state = await api.storageState();
    const mk = async (lang, width) => {
      const ctx = await browser.newContext({ storageState: state, viewport: { width, height: 900 } });
      const page = await ctx.newPage();
      page.on('pageerror', (e) => pageErrors.push(msgOf(e)));
      page.on('console', (m) => { if (/Content Security Policy/i.test(m.text())) cspViolations.push(m.text()); });
      await page.addInitScript((l) => { try { localStorage.setItem('nf.lang', l); } catch { /* */ } }, lang);
      return { ctx, page };
    };

    /* ---- EN, 1440: intake -> order -> pages -> compute -> result ---- */
    const en = await mk('en', 1440);
    const p = en.page;
    await p.goto(BASE + '/#/start');
    await p.waitForSelector('.jf1-dz', { timeout: 15000 });
    check('start route renders the drop zone', await p.isVisible('.jf1-dz'));
    const startBtn = p.locator('[data-start]');
    check('start disabled until a PDF is chosen', await startBtn.isDisabled());
    await p.setInputFiles('#jf1-files', pdfs);
    await p.waitForFunction(() => document.querySelectorAll('[data-picked] li').length === 2);
    check('two PDFs listed', (await p.locator('[data-picked] li').count()) === 2);
    await startBtn.click();
    await p.waitForFunction(() => /#\/start\/\d+/.test(location.hash), null, { timeout: 30000 });
    const oid = (await p.evaluate(() => location.hash)).match(/(\d+)$/)[1];
    check('order created and its id in the address', !!oid, oid);
    await p.waitForSelector('.jf1-pages', { timeout: 15000 });
    const order = await (await api.get(`/api/orders/${oid}`)).json();
    check('two files on the order', (order.files || []).length === 2, String((order.files || []).length));
    const pageItems = await p.locator('.jf1-pages li').count();
    check('pages listed from the server', pageItems === (order.pages || []).length && pageItems >= 2, `ui ${pageItems}, api ${(order.pages || []).length}`);
    check('calculate disabled with no parts', await p.locator('[data-act="calc"]').isDisabled());
    check('no-parts message shown', (await p.textContent('.jf1-panel')).includes('nothing to calculate'));

    const put = await api.put(`/api/orders/${oid}/parts`, { data: { parts: PARTS } });
    check('parts stored through the orders API', put.ok(), 'status ' + put.status());
    await p.reload();
    await p.waitForSelector('[data-act="calc"]:not([disabled])', { timeout: 15000 });
    await p.locator('[data-act="calc"]').click();
    await p.waitForSelector('.jf1-counts', { timeout: 30000 });
    const counts = await p.$$eval('.jf1-counts dd', (els) => els.map((e) => Number(e.textContent)));
    const parts = (await (await api.get(`/api/orders/${oid}`)).json()).parts || [];
    check('result counts add up to the parts', counts.reduce((a, b) => a + b, 0) === parts.length, JSON.stringify(counts) + ' vs ' + parts.length);
    check('current step is 3', (await p.getAttribute('.jf1-panel', 'data-cur')) === '3');
    const dl = p.locator('#jf1-outputs');
    const dlHref = (await dl.count()) ? await dl.getAttribute('href') : '';
    check('download links to the job package of the order', dlHref === `/api/orders/${oid}/outputs.zip`, dlHref);
    const pkg = await api.get(dlHref);
    check('the job package downloads as a ZIP', pkg.status() === 200 && (pkg.headers()['content-type'] || '').includes('application/zip'));
    check('no approve or release control on the page', (await p.locator('button', { hasText: /approve|release|承認|リリース/i }).count()) === 0);
    check('held parts never show a DXF link', (await p.locator('a[href*="dxf"]').count()) === 0);
    if (SHOT_DIR) await p.screenshot({ path: path.join(SHOT_DIR, 'f1-result-en-1440.png'), fullPage: true });
    await en.ctx.close();

    /* ---- JA, 390: same order, Japanese wording, no raw keys, no horizontal scroll ---- */
    const ja = await mk('ja', 390);
    const q = ja.page;
    await q.goto(BASE + '/#/start/' + oid);
    await q.waitForSelector('.jf1-steps', { timeout: 15000 });
    const jaText = await q.textContent('#view');
    check('JA: Japanese title', jaText.includes('図面を入れてください'));
    check('JA: no raw translation keys on screen', !RAW_KEY.test(jaText), (jaText.match(RAW_KEY) || [''])[0]);
    check('390px: no horizontal page scroll', await q.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
    if (SHOT_DIR) await q.screenshot({ path: path.join(SHOT_DIR, 'f1-result-ja-390.png'), fullPage: true });
    await ja.ctx.close();

    /* ---- reduced motion: transitions and animations off ---- */
    const rm = await browser.newContext({ storageState: state, viewport: { width: 1440, height: 900 }, reducedMotion: 'reduce' });
    const r = await rm.newPage();
    r.on('pageerror', (e) => pageErrors.push(msgOf(e)));
    await r.goto(BASE + '/#/start');
    await r.waitForSelector('.jf1-dz');
    const dur = await r.$eval('.jf1-btn', (b) => getComputedStyle(b).transitionDuration);
    check('reduced motion: no transitions', /^(0s)(, 0s)*$/.test(dur.replace(/\s/g, '')) || dur === '0s', dur);
    await rm.close();

    check('no CSP violations', cspViolations.length === 0, cspViolations.slice(0, 2).join(' | '));
    check('no page errors', pageErrors.length === 0, pageErrors.slice(0, 2).join(' | '));
  } catch (e) {
    check('job f1 flow', false, msgOf(e));
  } finally {
    try { if (browser) await browser.close(); } catch { /* */ }
    try { await api.dispose(); } catch { /* */ }
  }
  console.log(passed + ' passed, ' + failed + ' failed');
  process.exit(failed ? 1 : 0);
}

main();
