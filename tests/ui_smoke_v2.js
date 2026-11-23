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

// Engineering inputs copied from the legacy browser test (tests/ui_smoke.js): the header-with-branch set the
// engine accepts, and the cylinder magnitude the engine refuses (BH-014, magnitude_above_limit).
const FAMILY = 'header_with_branch';
const VALID = {
  header_radius: '150', header_dev_radius: '150', header_length: '1200',
  branch_station: '600', branch_radius: '100', branch_dev_radius: '100',
  beta_deg: '60', delta: '0', s_end: '400',
};
const REFUSED = { family: 'cylinder_shell', r_dev: 100, length: 1e9 };
const ADMIN_TABS = ['users', 'security', 'audit', 'lessons'];
const RAW_ID = /\b[a-z]+(_[a-z0-9]+){2,}\b/;

const decodeName = (n) => { try { return decodeURIComponent(n); } catch { return String(n ?? ''); } };

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
  let projectId = null, pdfName = '', jobId = null;

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

    /* ---- 5a. A: a new calculation through the form, with the legacy valid inputs ---- */
    try {
      if (!page) throw new Error("page unavailable");
      const list = await (await page.request.get(BASE + '/api/projects', { headers: { 'X-NF-Request': '1' } })).json();
      const projects = Array.isArray(list) ? list : (list && list.projects) || [];
      // the project that holds the PDF uploaded above is the one the project check opens later
      for (const p of projects) {
        const detail = await (await page.request.get(BASE + '/api/projects/' + p.id, { headers: { 'X-NF-Request': '1' } })).json();
        const doc = (detail.documents || []).find((d) => d.detected_type === 'pdf');
        if (doc) { projectId = p.id; pdfName = decodeName(doc.filename); break; }
      }
      if (projectId === null && projects.length) projectId = projects[0].id;
      if (projectId === null) throw new Error('no project available');

      await page.goto(BASE + '/#/math', { waitUntil: 'load' });
      await page.waitForSelector('#calc', { state: 'visible', timeout: 20000 });
      await page.selectOption('#f_fam', FAMILY);
      await page.selectOption('#f_project', String(projectId));
      await page.waitForSelector('#f_header_radius', { state: 'visible', timeout: 10000 });
      for (const [k, v] of Object.entries(VALID)) await page.fill('#f_' + k, v);
      await page.selectOption('#f_surface_reference', 'outside');
      await shot('07-math.png');

      await page.click('#run');
      await page.waitForFunction(() => /^#\/job\/\d+/.test(location.hash), null, { timeout: 60000 });
      jobId = Number(/#\/job\/(\d+)/.exec(await page.evaluate(() => location.hash))[1]);
      await page.waitForSelector('#gates li', { timeout: 20000 });
      const stateText = (await page.locator('#view h1 .tag').first().innerText()).trim();
      const gates = await page.locator('#gates li').count();
      await shot('08-job.png');
      check('A new calculation opens a job with a release state and gates',
        Number.isInteger(jobId) && stateText.length > 0 && gates > 0,
        'job=' + jobId + ' state=' + (stateText || 'empty') + ' gates=' + gates);
    } catch (e) {
      await shot('07-math.png');
      check('A new calculation opens a job with a release state and gates', false, msgOf(e));
    }

    /* ---- 5b. B: the pattern tab draws the pattern; F fits without an error ---- */
    try {
      if (!jobId) throw new Error('no job from check A');
      await page.click('#tab-pattern');
      await page.waitForSelector('#pattern-host svg', { timeout: 20000 });
      const curves = await page.locator('#pattern-host [data-curves] table').count();
      const quantities = await page.locator('#pattern-host [data-quant] dl').count();
      const errsBefore = pageErrors.length + consoleErrors.length;
      await page.locator('#pattern-host svg').first().click({ position: { x: 5, y: 5 } });
      await page.evaluate(() => { const s = document.querySelector('#pattern-host svg'); if (s && s.focus) s.focus(); });
      await page.keyboard.press('f');
      await page.waitForTimeout(300);
      const zoomText = (await page.locator('#pattern-host [data-zoomv]').first().innerText()).trim();
      const errsAfter = pageErrors.length + consoleErrors.length;
      await shot('09-pattern.png');
      check('B job pattern tab draws the pattern and fits with F',
        curves + quantities > 0 && /\d+%/.test(zoomText) && errsAfter === errsBefore,
        'curves=' + curves + ' quantities=' + quantities + ' zoom=' + zoomText + ' newErrors=' + (errsAfter - errsBefore));
    } catch (e) {
      await shot('09-pattern.png');
      check('B job pattern tab draws the pattern and fits with F', false, msgOf(e));
    }

    /* ---- 5c. C: the details tab renders without an error alert ---- */
    try {
      if (!jobId) throw new Error('no job from check A');
      await page.click('#tab-details');
      await page.waitForSelector('#tp-details:not([hidden])', { timeout: 20000 });
      await page.waitForSelector('#tp-details dl.kv', { timeout: 20000 });
      await page.waitForTimeout(300);
      const bad = await page.locator('#tp-details .alert.bad').count();
      const txt = bad ? (await page.locator('#tp-details .alert.bad').first().innerText()).trim().replace(/\s+/g, ' ').slice(0, 160) : '';
      check('C job details tab renders without an error alert', bad === 0, txt);
    } catch (e) {
      check('C job details tab renders without an error alert', false, msgOf(e));
    }

    /* ---- 5d. D (BH-014): "new from these values" of a refused job marks the field it names ---- */
    try {
      if (!page) throw new Error("page unavailable");
      if (projectId === null) throw new Error('no project from check A');
      // the refusal is provoked over the API context: the page itself never makes a failed request
      const r = await page.request.post(BASE + '/api/projects/' + projectId + '/calculations', { headers: { 'X-NF-Request': '1' }, data: REFUSED });
      const body = await r.json().catch(() => null);
      const rid = body && body.id;
      if (rid === undefined || rid === null) throw new Error('refusal not recorded: ' + r.status() + ' ' + JSON.stringify(body).slice(0, 160));

      await page.goto(BASE + '/#/math/' + projectId + '/' + rid, { waitUntil: 'load' });
      await page.waitForSelector('#calc', { state: 'visible', timeout: 20000 });
      await page.waitForSelector('#calc [aria-invalid="true"]', { timeout: 20000 });
      const marks = await page.evaluate(() => {
        const out = [];
        for (const i of document.querySelectorAll('#calc [aria-invalid="true"]')) {
          const ids = (i.getAttribute('aria-describedby') || '').split(/\s+/).filter(Boolean);
          const shown = ids.map((id) => document.getElementById(id))
            .filter((m) => m && m.getClientRects().length > 0 && (m.textContent || '').trim().length > 0);
          out.push({
            field: i.name || i.id,
            ids: ids.join(' '),
            visible: shown.length,
            text: shown.map((m) => m.textContent.trim().replace(/\s+/g, ' ').slice(0, 80)).join(' | '),
          });
        }
        return out;
      });
      check('D BH-014 refused values mark a field with a linked visible message',
        marks.length > 0 && marks.some((m) => m.visible > 0), JSON.stringify(marks).slice(0, 240));
    } catch (e) {
      check('D BH-014 refused values mark a field with a linked visible message', false, msgOf(e));
    }

    /* ---- 5e. E: the project screen lists the PDF uploaded above ---- */
    try {
      if (projectId === null) throw new Error('no project from check A');
      await page.goto(BASE + '/#/project/' + projectId, { waitUntil: 'load' });
      await page.waitForSelector('#tbl-docs', { timeout: 20000 });
      const text = (await page.locator('#tbl-docs').innerText()).replace(/\s+/g, ' ');
      const want = pdfName || path.basename(FIXTURE);
      check('E project documents list contains the uploaded PDF', !!pdfName && text.includes(want),
        'want=' + want + ' docs=' + text.slice(0, 160));
    } catch (e) {
      check('E project documents list contains the uploaded PDF', false, msgOf(e));
    }

    /* ---- 5f. F: the account screen shows the user and a password form (never submitted) ---- */
    try {
      await page.goto(BASE + '/#/account', { waitUntil: 'load' });
      await page.waitForSelector('#pwc', { state: 'visible', timeout: 20000 });
      const shown = await page.evaluate(() => document.body.innerText || '');
      const fields = [];
      for (const sel of ['#pwc', '#pwn', '#pwn2']) fields.push(await page.isVisible(sel));
      check('F account shows the signed-in user and a password form',
        shown.includes(USER) && fields.every(Boolean),
        'user=' + shown.includes(USER) + ' fields=' + fields.join(','));
    } catch (e) {
      check('F account shows the signed-in user and a password form', false, msgOf(e));
    }

    /* ---- 5g. G: administration tabs, then the audit chain verification ---- */
    try {
      await page.goto(BASE + '/#/admin', { waitUntil: 'load' });
      await page.waitForSelector('#adpanel', { timeout: 20000 });
      const problems = [];
      for (const k of ADMIN_TABS) {
        const sel = '#adtab-' + k;
        await page.waitForSelector(sel, { state: 'visible', timeout: 10000 });
        await page.click(sel);
        await page.waitForSelector('#adpanel .table, #adpanel .alert, #adpanel .sub', { timeout: 20000 });
        await page.waitForTimeout(300);
        const bad = await page.locator('#view .alert.bad').count();
        if (bad) {
          const txt = (await page.locator('#view .alert.bad').first().innerText()).trim();
          problems.push(k + ': ' + txt.replace(/\s+/g, ' ').slice(0, 120));
        }
      }
      await shot('10-admin.png');
      check('G admin tabs render cleanly', problems.length === 0, problems.join('; '));
    } catch (e) {
      await shot('10-admin.png');
      check('G admin tabs render cleanly', false, msgOf(e));
    }

    try {
      await page.click('#adtab-audit');
      await page.waitForSelector('#adm-audit', { timeout: 20000 });
      await page.click('[data-act="verify"]');
      await page.waitForFunction(() => {
        const s = document.querySelector('#adm-chain');
        return s && (s.textContent || '').trim().length > 0;
      }, null, { timeout: 20000 });
      const chain = (await page.locator('#adm-chain').innerText()).trim().replace(/\s+/g, ' ');
      check('G admin audit verification reports a result', chain.length > 0, chain.slice(0, 160) || 'empty');
    } catch (e) {
      check('G admin audit verification reports a result', false, msgOf(e));
    }

    /* ---- 5g2. I: the reference screens (verification, manufacturing, AMOEBA) render without an error ---- */
    for (const [hash, name] of [['#/verification', 'verification'], ['#/manufacturing', 'manufacturing'], ['#/amoeba', 'AMOEBA']]) {
      try {
        await page.goto(BASE + '/' + hash, { waitUntil: 'load' });
        await page.waitForFunction(() => { const v = document.querySelector('#view'); return v && v.textContent.trim().length > 40; }, null, { timeout: 15000 });
        const bad = await page.$$eval('#view .alert.bad', (a) => a.map((x) => x.textContent.trim().slice(0, 120)));
        check(`I ${name} screen renders without an error`, bad.length === 0, bad);
      } catch (e) { check(`I ${name} screen renders without an error`, false, msgOf(e)); }
    }

    /* ---- 5g3. K: the PDF result screen never claims approval ---- */
    try {
      const rid = await page.evaluate(() => { try { const r = JSON.parse(localStorage.getItem('nf_recent') || '[]'); return r.length ? r[0].id : null; } catch { return null; } });
      if (rid == null) throw new Error('no conversion id in the recent list');
      await page.goto(BASE + '/#/r/' + rid, { waitUntil: 'load' });
      await page.waitForFunction(() => { const l = document.querySelector('#view .badge-label'); return l && l.textContent.trim().length > 0; }, null, { timeout: 15000 });
      const txt = await page.$eval('#view', (v) => v.innerText);
      check('K PDF result screen never claims approval', !/approved|承認/i.test(txt), txt.slice(0, 160));
    } catch (e) { check('K PDF result screen never claims approval', false, msgOf(e)); }

    /* ---- 5g4. L: layout - every screen uses the page width and the pattern viewer is drawn ---- */
    try {
      const jid = await page.evaluate(async (b) => { const r = await fetch(b + '/api/calculations', { headers: { 'X-NF-Request': '1' } }); const j = await r.json(); const a = Array.isArray(j) ? j : (j.calculations || []); const ok = a.find((c) => c.state === 'REVIEW_REQUIRED' || c.release_state === 'REVIEW_REQUIRED'); return ok ? ok.id : (a.length ? a[0].id : null); }, BASE);
      const bad = [];
      for (const hash of ['#/workspace/calculations', `#/job/${jid}`, `#/job/${jid}/pattern`, '#/verification', '#/amoeba', '#/admin', '#/math']) {
        await page.goto(BASE + '/' + hash, { waitUntil: 'load' }); await page.waitForTimeout(1500);
        const narrow = await page.evaluate(() => { for (const el of document.querySelectorAll('#view *')) { const w = el.getBoundingClientRect().width; if (w > 0 && w < 300 && el.textContent.trim().length > 300 && !el.closest('aside,nav,.ws-sidebar,td,th,li,.chip,.btn')) return el.tagName.toLowerCase() + '.' + [...el.classList].join('.') + ' w=' + Math.round(w); } return null; });
        if (narrow) bad.push(hash + ': ' + narrow);
      }
      await page.goto(BASE + `/#/job/${jid}/pattern`, { waitUntil: 'load' }); await page.waitForTimeout(1500);
      const pv = await page.evaluate(() => { const s = document.querySelector('svg.pv'); if (!s) return null; const r = s.getBoundingClientRect(); return { w: r.width, h: r.height, n: s.querySelectorAll('path,polyline,line').length }; });
      if (!pv || pv.w < 600 || pv.h < 300 || pv.n < 5) bad.push('pattern viewer not drawn: ' + JSON.stringify(pv));
      check('L every screen uses the page width and the pattern viewer is drawn', bad.length === 0, bad);
    } catch (e) { check('L every screen uses the page width and the pattern viewer is drawn', false, msgOf(e)); }

    /* ---- 5h. H: the Japanese job screen shows no raw engine identifier ---- */
    try {
      if (!jobId) throw new Error('no job from check A');
      await page.click('#btn-lang');
      await page.waitForFunction(() => document.documentElement.lang === 'ja', null, { timeout: 15000 });
      await page.goto(BASE + '/#/job/' + jobId, { waitUntil: 'load' });
      await page.waitForSelector('#gates li', { timeout: 20000 });
      await page.waitForTimeout(400);
      const offenders = await page.evaluate(() => {
        const re = /\b[a-z]+(_[a-z0-9]+){2,}\b/g;
        const skip = '.mono,.id,script,style,noscript';
        const out = [];
        const w = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
        let n;
        while ((n = w.nextNode())) {
          const el = n.parentElement;
          if (!el || el.closest(skip) || !el.getClientRects().length) continue;
          const found = n.textContent.match(re);
          if (found) for (const x of found) out.push(x + ' in <' + el.tagName.toLowerCase() + '> "' + n.textContent.trim().slice(0, 40) + '"');
        }
        return out.slice(0, 5);
      });
      await shot('11-job-ja.png');
      check('H Japanese job screen shows no raw engine identifier', offenders.length === 0, offenders.join('; '));
    } catch (e) {
      await shot('11-job-ja.png');
      check('H Japanese job screen shows no raw engine identifier', false, msgOf(e));
    } finally {
      // the language toggle check below expects to switch English -> Japanese
      try {
        if (await page.evaluate(() => document.documentElement.lang === 'ja')) {
          await page.click('#btn-lang');
          await page.waitForFunction(() => document.documentElement.lang !== 'ja', null, { timeout: 15000 });
        }
      } catch { /* leave the language as it stands */ }
    }

    /* ---- 5i. J: every screen, in both languages, shows translated text (no message keys) ---- */
    const setLang = async (lang) => {
      const cur = await page.evaluate(() => document.documentElement.lang || '');
      if (cur === lang) return;
      try {
        await page.click('#btn-lang');
        await page.waitForFunction((l) => document.documentElement.lang === l, lang, { timeout: 15000 });
        return;
      } catch { /* fall back to the stored preference */ }
      await page.evaluate((l) => { try { localStorage.setItem('nf.lang', l); } catch { /* ignore */ } }, lang);
      await page.reload({ waitUntil: 'load' });
      await page.waitForFunction((l) => document.documentElement.lang === l, lang, { timeout: 15000 });
    };

    const openScreen = async (hash) => {
      await page.goto(BASE + '/' + hash, { waitUntil: 'load' });
      await page.waitForSelector('#view', { timeout: 20000 });
      await page.waitForTimeout(400);
    };

    // visible text + aria-label/title/placeholder inside #view and the top bar; dotted tokens that are
    // not numbers, file names, domains or versions are treated as untranslated message keys
    const scanScreen = () => page.evaluate(() => {
      const RE = /\b[a-z][a-z0-9]*(?:\.[a-zA-Z0-9_-]+){1,4}\b/g;
      const NUM = /^\d+(\.\d+)*$/;
      const VERSION = /^v?\d+(\.\d+)+$/i;
      const FILE = /\.(pdf|dxf|json|js|css|html)$/i;
      const DOMAIN = /^(www\.|https?\.)/i;
      const TLD = /\.(com|org|net|io|dev|app|ai|edu|gov|co|uk|jp|local|localhost|test|example|internal|lan)$/i;
      const SKIP = '.mono,.id,.eng,[lang="en"],script,style,noscript'; // engine output is English by rule, marked as such
      const TOPBAR = 'header,.topbar,#topbar,.top-bar,.appbar,#appbar,.app-header,#app-header,.navbar,#navbar';
      const roots = [document.querySelector('#view')]
        .concat(Array.from(document.querySelectorAll(TOPBAR)))
        .filter(Boolean);
      const visible = (el) => !!el && !el.closest(SKIP) && el.getClientRects().length > 0 &&
        getComputedStyle(el).visibility !== 'hidden';
      const seen = new Set();
      const tokens = [];
      const take = (text, where) => {
        const found = String(text || '').match(RE) || [];
        for (const raw of found) {
          const t = raw.replace(/[.,]+$/, '');
          if (!t || seen.has(t)) continue;
          if (NUM.test(t) || VERSION.test(t) || FILE.test(t) || DOMAIN.test(t) || TLD.test(t)) continue;
          seen.add(t);
          tokens.push(t + ' ' + where);
        }
      };
      for (const root of roots) {
        const w = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
        let n;
        while ((n = w.nextNode())) {
          const el = n.parentElement;
          if (!visible(el)) continue;
          take(n.textContent, 'in <' + el.tagName.toLowerCase() + '> "' + n.textContent.trim().replace(/\s+/g, ' ').slice(0, 40) + '"');
        }
        const nodes = [root].concat(Array.from(root.querySelectorAll('[aria-label],[title],[placeholder]')));
        for (const el of nodes) {
          if (!visible(el)) continue;
          for (const a of ['aria-label', 'title', 'placeholder']) {
            if (el.hasAttribute(a)) take(el.getAttribute(a), 'in @' + a + ' of <' + el.tagName.toLowerCase() + '>');
          }
        }
      }
      const view = document.querySelector('#view');
      return { tokens, empty: !view || (view.innerText || '').trim().length === 0 };
    });

    try {
      const problems = [];
      const record = (lang, screen, res) => {
        if (res.empty) problems.push(lang + ' ' + screen + ': empty view (missing text)');
        for (const t of res.tokens) problems.push(lang + ' ' + screen + ': ' + t);
      };

      for (const lang of ['en', 'ja']) {
        await setLang(lang);

        await openScreen('#/');
        record(lang, 'convert', await scanScreen());
        if (lang === 'ja') {
          const english = await page.evaluate(() => {
            const v = document.querySelector('#view');
            const t = (v && v.innerText) || '';
            return Array.from(new Set(t.match(/\bverified\b|\bVerify\b/g) || [])).slice(0, 4);
          });
          if (english.length) problems.push('ja convert: English wording ' + english.join(', '));
        }

        for (const [screen, hash] of [
          ['workspace/calculations', '#/workspace/calculations'],
          ['workspace/projects', '#/workspace/projects'],
          ['math', '#/math'],
        ]) {
          await openScreen(hash);
          record(lang, screen, await scanScreen());
        }

        if (jobId) {
          await openScreen('#/job/' + jobId);
          record(lang, 'job', await scanScreen());
          for (const [tab, sel, wait] of [
            ['pattern', '#tab-pattern', '#pattern-host'],
            ['details', '#tab-details', '#tp-details:not([hidden])'],
          ]) {
            await page.click(sel);
            await page.waitForSelector(wait, { timeout: 20000 });
            await page.waitForTimeout(300);
            record(lang, 'job/' + tab, await scanScreen());
          }
        }

        if (projectId !== null) {
          await openScreen('#/project/' + projectId);
          record(lang, 'project', await scanScreen());
        }

        await openScreen('#/account');
        record(lang, 'account', await scanScreen());

        await openScreen('#/admin');
        await page.waitForSelector('#adpanel', { timeout: 20000 });
        record(lang, 'admin', await scanScreen());
        for (const k of ADMIN_TABS) {
          await page.waitForSelector('#adtab-' + k, { state: 'visible', timeout: 10000 });
          await page.click('#adtab-' + k);
          await page.waitForSelector('#adpanel', { timeout: 20000 });
          await page.waitForTimeout(400);
          record(lang, 'admin/' + k, await scanScreen());
        }

        for (const [screen, hash] of [
          ['verification', '#/verification'],
          ['manufacturing', '#/manufacturing'],
          ['amoeba', '#/amoeba'],
        ]) {
          await openScreen(hash);
          record(lang, screen, await scanScreen());
        }
      }

      await shot('12-i18n-keys.png');
      check('J no message key or missing text shown on any screen', problems.length === 0,
        problems.slice(0, 8).join('; '));
    } catch (e) {
      await shot('12-i18n-keys.png');
      check('J no message key or missing text shown on any screen', false, msgOf(e));
    } finally {
      // the mobile + toggle checks below start from English
      try { await setLang('en'); } catch { /* leave the language as it stands */ }
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
