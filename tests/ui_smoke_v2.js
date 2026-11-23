/* NEXUS FABRIC — UI smoke test v2 (simplified UI) */
'use strict';

const fs = require('fs');
const path = require('path');
const { chromium, request } = require('playwright');

const BASE = (process.env.NF_BASE || 'http://127.0.0.1:8793').replace(/\/$/, '');
const SETUP_TOKEN = process.env.NF_SETUP_TOKEN || 'ui-smoke-setup-token';
const SHOT_DIR = process.argv[2] || '';
const FIXTURE = path.join(__dirname, '..', 'crates', 'nf-pdf', 'tests', 'fixtures', 'reportlab_lines.pdf');

const USER = 'admin';
const PASS = 'correct-horse-9';

const MODULES = [
  '/app.css',
  '/js/theme.js',
  '/js/app.js',
  '/js/api.js',
  '/js/store.js',
  '/js/ui.js',
  '/js/icons.js',
  '/js/i18n.js',
  '/js/lib/poll.js',
  '/js/views/convert.js',
  '/js/views/login.js',
  '/js/views/result.js',
  '/js/views/workspace.js',
];

const TABS = ['calculations', 'projects', 'amoeba', 'obligations', 'audit', 'users'];
const DROPZONE = '#view .dropzone, #view .drop-zone';

let passed = 0, failed = 0;
const check = (name, ok, detail) => {
  if (ok) { passed++; console.log('ok  - ' + name); }
  else { failed++; console.log('FAIL - ' + name + (detail ? ' [' + detail + ']' : '')); }
};

const msgOf = (e) => (e && e.message) || String(e);

async function main() {
  if (SHOT_DIR) fs.mkdirSync(SHOT_DIR, { recursive: true });

  const consoleErrors = [];
  const pageErrors = [];
  const badRequests = [];
  const convStates = [];
  const bodyJobs = [];
  let loggedIn = false;
  let page = null;
  let browser = null;

  const shot = async (name) => {
    if (!SHOT_DIR || !page) return;
    try { await page.screenshot({ path: path.join(SHOT_DIR, name), fullPage: true }); }
    catch (e) { console.log('note - screenshot ' + name + ': ' + msgOf(e)); }
  };

  /* ---- 1. static assets ---- */
  let api = null;
  try {
    api = await request.newContext({ baseURL: BASE });
    const problems = [];
    const root = await api.get('/');
    if (root.status() !== 200) problems.push('/ -> ' + root.status());
    for (const url of MODULES) {
      const r = await api.get(url);
      const ct = (r.headers()['content-type'] || '').toLowerCase();
      const want = url.endsWith('.css') ? /css/ : /javascript|ecmascript/;
      if (r.status() !== 200) problems.push(url + ' -> ' + r.status());
      else if (!want.test(ct)) problems.push(url + ' -> ' + (ct || 'no content-type'));
    }
    check('static shell + modules served', problems.length === 0, problems.join('; '));
  } catch (e) {
    check('static shell + modules served', false, msgOf(e));
  }

  /* ---- first-run setup (empty data dir) ---- */
  try {
    if (api) {
      const r = await api.post('/api/setup', {
        headers: { 'X-NF-Request': '1' },
        data: { username: USER, password: PASS, setup_token: SETUP_TOKEN },
      });
      if (!r.ok() && r.status() !== 400 && r.status() !== 409) {
        console.log('note - setup returned ' + r.status() + ': ' + (await r.text()).slice(0, 200));
      }
    }
  } catch (e) {
    console.log('note - setup: ' + msgOf(e));
  }

  try {
    browser = await chromium.launch({ headless: true });
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    page = await ctx.newPage();
    page.setDefaultTimeout(15000);

    // network failures are judged by status in the response handler, not by the console echo
    page.on('console', (m) => { if (m.type() === 'error' && !m.text().startsWith('Failed to load resource')) consoleErrors.push(m.text()); });
    page.on('pageerror', (e) => pageErrors.push(msgOf(e)));
    page.on('response', (res) => {
      const url = res.url();
      const status = res.status();
      if (status >= 400) {
        if (!loggedIn && status === 401 && url.includes('/api/me')) return;
        // 422 from pdf-to-dxf is the converter's refusal, a valid outcome the UI must show
        if (status !== 422 || !/pdf-to-dxf/.test(url)) { badRequests.push(status + ' ' + url); return; }
      }
      if (/\/api\/(pdf-conversions\/|documents\/[^/]+\/pdf-to-dxf)/.test(url)) {
        bodyJobs.push(res.json().then((j) => {
          const c = (j && (j.conversion || j)) || null;
          if (c && typeof c === 'object') {
            if (c.release_state) convStates.push(String(c.release_state));
            if (c.status) convStates.push(String(c.status));
          }
        }).catch(() => { /* non-JSON payload */ }));
      }
    });

    /* ---- 2. unauthenticated -> login ---- */
    try {
      await page.goto(BASE + '/', { waitUntil: 'load' });
      await page.waitForSelector('#login-form', { state: 'visible', timeout: 20000 });
      const hash = await page.evaluate(() => location.hash);
      const userVisible = await page.isVisible('#login-user');
      const passVisible = await page.isVisible('#login-pass');
      await shot('01-login.png');
      check('unauthenticated load shows login', hash === '#/login' && userVisible && passVisible,
        'hash=' + hash + ' user=' + userVisible + ' pass=' + passVisible);
    } catch (e) {
      await shot('01-login.png');
      check('unauthenticated load shows login', false, msgOf(e));
    }

    /* ---- 3. sign in through the form ---- */
    try {
      await page.fill('#login-user', USER);
      await page.fill('#login-pass', PASS);
      await page.click('#login-form button[type="submit"]');
      await page.waitForFunction(() => ['', '#', '#/'].includes(location.hash), null, { timeout: 20000 });
      await page.waitForSelector(DROPZONE, { state: 'visible', timeout: 20000 });
      loggedIn = true;
      await shot('02-convert.png');
      check('login lands on convert dropzone', true);
    } catch (e) {
      loggedIn = true;
      await shot('02-convert.png');
      check('login lands on convert dropzone', false, msgOf(e));
    }

    /* ---- 4. upload fixture -> result or refusal ---- */
    let badgeText = '';
    try {
      if (!fs.existsSync(FIXTURE)) throw new Error('fixture missing: ' + FIXTURE);
      await page.waitForSelector(DROPZONE, { state: 'visible', timeout: 20000 });
      await page.selectOption('#conv-scale-kind', 'ratio');
      await page.fill('#conv-scale-value', '1');
      await page.setInputFiles('#view input[type=file]', FIXTURE);

      let refused = false;
      try {
        await page.waitForSelector('#view .badge', { state: 'visible', timeout: 60000 });
        badgeText = (await page.locator('#view .badge').first().innerText()).trim();
      } catch {
        await page.waitForSelector('#view .convert-error, #view .alert.bad', { state: 'visible', timeout: 10000 });
        badgeText = (await page.locator('#view .convert-error, #view .alert.bad').first().innerText()).trim();
        refused = true;
      }
      await Promise.allSettled(bodyJobs);
      await shot('03-result.png');
      const shown = badgeText.length > 0;
      check('upload yields result or refusal', shown,
        refused ? 'no badge/refusal text' : 'empty badge');
      if (shown) console.log('note - badge: ' + badgeText.replace(/\s+/g, ' ').slice(0, 160));

      const bodyText = await page.evaluate(() => document.body.innerText || '');
      const claimsApproved = /APPROVED/.test(bodyText) && !/REVIEW REQUIRED|NOT RELEASED/.test(badgeText);
      const serverApproved = convStates.includes('APPROVED');
      check('no unearned APPROVED claim', !claimsApproved || serverApproved,
        'ui=APPROVED server=' + (convStates.join(',') || 'unknown'));
    } catch (e) {
      await shot('03-result.png');
      check('upload yields result or refusal', false, msgOf(e));
      check('no unearned APPROVED claim', false, 'upload step failed: ' + msgOf(e));
    }

    /* ---- 5. workspace tabs ---- */
    try {
      await page.evaluate(() => { location.hash = '#/workspace/calculations'; });
      await page.waitForSelector('.ws-sidebar', { state: 'visible', timeout: 20000 });
      await page.waitForSelector('.ws-content .table, .ws-content .sub, .ws-content p', { timeout: 20000 });
      const problems = [];
      for (const tab of TABS) {
        const sel = '.ws-sidebar [role="tab"][data-tab="' + tab + '"]';
        await page.waitForSelector(sel, { state: 'visible', timeout: 10000 });
        await page.click(sel);
        await page.waitForSelector('.ws-content .table, .ws-content .sub, .ws-content p, .ws-content dl, .ws-content .alert',
          { timeout: 20000 });
        await page.waitForTimeout(300);
        const bad = await page.locator('.ws-content .alert.bad').count();
        if (bad) {
          const txt = (await page.locator('.ws-content .alert.bad').first().innerText()).trim();
          problems.push(tab + ': ' + txt.replace(/\s+/g, ' ').slice(0, 120));
        }
      }
      await shot('04-workspace.png');
      check('workspace sidebar + all tabs render cleanly', problems.length === 0, problems.join('; '));
    } catch (e) {
      await shot('04-workspace.png');
      check('workspace sidebar + all tabs render cleanly', false, msgOf(e));
    }

    /* ---- 6. mobile: no horizontal scroll ---- */
    try {
      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(BASE + '/#/', { waitUntil: 'load' });
      await page.reload({ waitUntil: 'load' });
      await page.waitForSelector(DROPZONE, { state: 'visible', timeout: 20000 });
      const w = await page.evaluate(() => document.documentElement.scrollWidth);
      await shot('05-mobile.png');
      check('mobile 390px has no horizontal scroll', w <= 391, 'scrollWidth=' + w);
    } catch (e) {
      await shot('05-mobile.png');
      check('mobile 390px has no horizontal scroll', false, msgOf(e));
    }

    /* ---- 7. japanese toggle ---- */
    try {
      await page.click('#btn-lang');
      await page.waitForFunction(() => document.documentElement.lang === 'ja', null, { timeout: 15000 });
      await page.waitForTimeout(400);
      const text = await page.evaluate(() => document.body.innerText || '');
      const hasJa = /[぀-ヿ一-鿿]/.test(text);
      await shot('06-ja.png');
      check('language toggle switches to Japanese', hasJa, 'no Japanese characters rendered');
    } catch (e) {
      await shot('06-ja.png');
      check('language toggle switches to Japanese', false, msgOf(e));
    }

    /* ---- 8. safety: never offer approve/release ---- */
    try {
      const offenders = await page.evaluate(() => {
        const out = [];
        const nodes = document.querySelectorAll('button, a, [role="button"], [role="menuitem"]');
        for (const n of nodes) {
          const vis = n.getClientRects().length > 0 &&
            getComputedStyle(n).visibility !== 'hidden' &&
            getComputedStyle(n).display !== 'none';
          if (!vis) continue;
          const t = (n.innerText || n.textContent || '').trim();
          if (/approve|release|承認|リリース/i.test(t)) out.push(t.replace(/\s+/g, ' ').slice(0, 80));
        }
        return out;
      });
      check('no approve/release affordance in UI', offenders.length === 0, offenders.join('; '));
    } catch (e) {
      check('no approve/release affordance in UI', false, msgOf(e));
    }

    /* ---- 9. clean console / network ---- */
    await Promise.allSettled(bodyJobs);
    const issues = []
      .concat(consoleErrors.map((m) => 'console: ' + m))
      .concat(pageErrors.map((m) => 'pageerror: ' + m))
      .concat(badRequests.map((m) => 'request: ' + m));
    check('no console errors / page errors / failed requests', issues.length === 0,
      issues.slice(0, 8).join('; '));
  } catch (e) {
    check('browser session', false, msgOf(e));
  } finally {
    try { if (browser) await browser.close(); } catch { /* ignore */ }
    try { if (api) await api.dispose(); } catch { /* ignore */ }
  }

  console.log(passed + ' passed, ' + failed + ' failed');
  process.exit(failed ? 1 : 0);
}

main();
