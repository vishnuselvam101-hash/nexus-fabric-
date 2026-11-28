/* NEXUS FABRIC job live + pro view smoke (lanes F2/F3): live operator events with the blob cursor, pause / undo / take over,
   Watch / Fast / Instant, inline question cards (type a number, confirm), the pro view (tabs, chips, review rows) and the
   person's hold override (a short reason is refused, the AI operator is refused, a person's is accepted and undoable).
   Usage: NF_BASE=http://127.0.0.1:8797 node tests/ui_job_f2_smoke.js [shot-dir]   (fresh server; ports 8796-8829)
   Headless Chromium, EN and JA, 1440 and 390, reduced motion. */
'use strict';

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const { chromium, request } = require('playwright');

const BASE = (process.env.NF_BASE || 'http://127.0.0.1:8797').replace(/\/$/, '');
const SETUP_TOKEN = process.env.NF_SETUP_TOKEN || 'ui-job-f1-setup-token';
const SHOT_DIR = process.argv[2] || '';
// a real white PNG page (the PDF renderer is not part of every build, so the page is an image upload)
const crc = (() => { const t = []; for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } return (b) => { let c = 0xffffffff; for (const x of b) c = t[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; }; })();
function pagePng(w, h) {
  const chunk = (type, data) => { const body = Buffer.concat([Buffer.from(type), data]); const len = Buffer.alloc(4); len.writeUInt32BE(data.length); const c = Buffer.alloc(4); c.writeUInt32BE(crc(body)); return Buffer.concat([len, body, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 0;
  const raw = Buffer.alloc((w + 1) * h, 255); for (let y = 0; y < h; y++) raw[y * (w + 1)] = 0;
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}
const USER = 'admin', PASS = 'correct-horse-9', AGENT = 'op-bot', AGENT_PASS = 'agent-horse-battery-9';
const HDR = { 'X-NF-Request': '1' };
const RAW_KEY = /\bjf[12]\.[a-z0-9.]+\b/;

let passed = 0, failed = 0;
const check = (name, ok, detail) => {
  if (ok) { passed++; console.log('ok  - ' + name); } else { failed++; console.log('FAIL - ' + name + (detail ? ' [' + detail + ']' : '')); }
};
const msgOf = (e) => (e && e.message) || String(e);
const ev = (step, action, target, reason, extra) => ({ step, action, target, value: null, reason, confidence: 0.95, flags: [], ...extra });

async function main() {
  if (SHOT_DIR) fs.mkdirSync(SHOT_DIR, { recursive: true });
  const api = await request.newContext({ baseURL: BASE, extraHTTPHeaders: HDR });
  const agent = await request.newContext({ baseURL: BASE, extraHTTPHeaders: HDR });
  const cspViolations = [], pageErrors = [];
  let browser = null;
  try {
    const health = await (await api.get('/api/health')).json();
    if (health.setup_required) check('first-run setup', (await api.post('/api/setup', { data: { username: USER, password: PASS, setup_token: SETUP_TOKEN } })).ok());
    check('sign in', (await api.post('/api/login', { data: { username: USER, password: PASS } })).ok());

    // project, order, one page, three parts: C1 holds with source_inferred (overridable), C2 and C3 wait for an answer
    const proj = await (await api.post('/api/projects', { data: { name: 'f2 smoke' } })).json();
    const pid = (proj.project ?? proj).id;
    const oid = (await (await api.post(`/api/projects/${pid}/orders`, { data: { name: 'F2 job' } })).json()).id;
    const up = await api.post(`/api/orders/${oid}/files`, { data: pagePng(1240, 1754), headers: { 'X-Filename': 'a.png' } });
    check('page uploaded', up.ok(), 'status ' + up.status());
    const put = await api.put(`/api/orders/${oid}/parts`, { data: { parts: [
      { no: 'C1', name: 'pipe', shape: 'cylinder', qty: 1, page: 1 },
      { no: 'C2', name: 'sleeve', shape: 'cylinder', qty: 1, page: 1 },
      { no: 'C3', name: 'ring', shape: 'cylinder', qty: 1, page: 1 } ] } });
    check('parts stored', put.ok(), 'status ' + put.status());
    const v = (part, key, value, source = 'entered', bbox) => ({ part, key, value, text: String(value), source, ...(bbox ? { bbox } : {}) });
    const vals = await api.put(`/api/orders/${oid}/values`, { data: { values: [
      v('C1', 'D', 216.3, 'inferred', [100, 100, 220, 140]), v('C1', 't', 12), v('C1', 'L', 2000),
      v('C2', 'D', 165.2, 'entered', [100, 200, 220, 240]), v('C2', 't', 7.1), v('C2', 'L', 800),
      v('C3', 'D', 114.3), v('C3', 't', 6), v('C3', 'L', 500) ] } });
    check('values stored', vals.ok(), 'status ' + vals.status());
    // confirm only C1 (the other values stay PROPOSED: they become question cards)
    for (const k of ['D', 't', 'L']) {
      const r = await api.post(`/api/orders/${oid}/decisions`, { data: { kind: 'confirm', part: 'C1', key: k, reason: 'smoke' } });
      if (!r.ok()) check('confirm C1 ' + k, false, 'status ' + r.status() + ' ' + (await r.text()).slice(0, 120));
    }
    const comp = await api.post(`/api/orders/${oid}/compute`, { data: {} });
    check('compute stored', comp.ok(), 'status ' + comp.status());
    const resParts = (await (await api.get(`/api/orders/${oid}/compute`)).json())?.current?.result?.parts || [];
    const c1 = resParts.find((p) => p.no === 'C1');
    check('C1 holds with source_inferred', !!c1 && (c1.holds || []).some((h) => h.code === 'source_inferred'), JSON.stringify(c1 && c1.holds));

    // the AI operator: an agent account on the project
    const mk = await api.post('/api/users', { data: { username: AGENT, password: AGENT_PASS, role: 'engineer', kind: 'agent' } });
    check('agent account made', mk.ok(), 'status ' + mk.status() + ' ' + (await mk.text()).slice(0, 100));
    check('agent joined the project', (await api.post(`/api/projects/${pid}/members`, { data: { username: AGENT } })).ok());
    check('agent signs in', (await agent.post('/api/login', { data: { username: AGENT, password: AGENT_PASS } })).ok());
    const send = (list) => agent.post(`/api/orders/${oid}/operator/events`, { data: { events: list } });
    const first = await send([ev('look-1', 'look', { page: 1, bbox: [100, 100, 220, 140] }, 'reading the outer diameter'),
      ev('type-1', 'type', { page: 1, bbox: [100, 200, 220, 240] }, 'typing a value', { value: 165.2 })]);
    check('operator events accepted', first.ok(), 'status ' + first.status() + ' ' + (await first.text()).slice(0, 120));

    browser = await chromium.launch();
    const state = await api.storageState();
    const open = async (lang, width, extra = {}) => {
      const ctx = await browser.newContext({ storageState: state, viewport: { width, height: 900 }, ...extra });
      const page = await ctx.newPage();
      page.on('pageerror', (e) => pageErrors.push(msgOf(e)));
      page.on('console', (m) => { if (/Content Security Policy/i.test(m.text())) cspViolations.push(m.text()); });
      await page.addInitScript((l) => { try { localStorage.setItem('nf.lang', l); } catch { /* */ } }, lang);
      return { ctx, page };
    };

    /* ---- simple view: live blob cursor, controls, question cards ---- */
    const en = await open('en', 1440);
    const p = en.page;
    await p.goto(BASE + '/#/start/' + oid);
    await p.waitForSelector('.lv-bar', { timeout: 20000 });
    await p.click('[data-sp="instant"]');
    check('Instant speed selected', (await p.getAttribute('[data-sp="instant"]', 'aria-pressed')) === 'true');
    await p.waitForSelector('.op-blob', { state: 'attached', timeout: 15000 });
    check('the living blob is the cursor', (await p.locator('.op-cursor.op-blob .op-blob-body').count()) === 1);
    await p.waitForFunction(() => document.querySelectorAll('[data-log] li').length >= 2, null, { timeout: 15000 });
    const logText = await p.textContent('[data-log]');
    check('live events reach the log with their reasons', /reading the outer diameter/.test(logText) && /typing a value/.test(logText), logText);
    check('status says AMOEBA is working', /working/i.test(await p.textContent('.lv-status')));
    await send([ev('look-2', 'look', { page: 1 }, 'a third event arrives while the page is open')]);
    await p.waitForFunction(() => /third event/.test(document.querySelector('[data-log]').textContent), null, { timeout: 15000 });
    check('a later event arrives without a reload', true);

    // pause: the server then refuses the operator's next write
    await p.click('[data-lv="pause"]');
    await p.waitForSelector('[data-lv="resume"]');
    let st = await (await api.get(`/api/orders/${oid}/operator/events?after=0`)).json();
    check('Pause is stored on the server', st.paused === true);
    const refused = await send([ev('look-3', 'look', { page: 1 }, 'tries to act while paused')]);
    check('a paused operator cannot write (409)', refused.status() === 409, 'status ' + refused.status());
    // take over keeps it paused; hand back resumes
    await p.click('[data-lv="resume"]');
    await p.waitForSelector('[data-lv="pause"]');
    await p.click('[data-lv="take"]');
    await p.waitForSelector('[data-lv="hand"]');
    check('Take over says it is your turn', /your turn/i.test(await p.textContent('.lv-status')));
    st = await (await api.get(`/api/orders/${oid}/operator/events?after=0`)).json();
    check('Take over pauses the operator', st.paused === true);
    await p.click('[data-lv="hand"]');
    await p.waitForSelector('[data-lv="pause"]');
    st = await (await api.get(`/api/orders/${oid}/operator/events?after=0`)).json();
    check('Hand back resumes it', st.paused === false);
    // undo with nothing of the operator's to undo: the server's answer is shown, nothing breaks
    await p.click('[data-lv="undo"]');
    await p.waitForSelector('.lv-err:not([hidden])', { timeout: 10000 });
    check('Undo shows the server answer when there is nothing to undo', (await p.textContent('.lv-err')).length > 3);
    await p.click('[data-sp="fast"]');
    await p.click('[data-sp="watch"]');
    check('Watch is selectable again', (await p.getAttribute('[data-sp="watch"]', 'aria-pressed')) === 'true');

    // question cards: C2 and C3 values wait for an answer
    const cards = await p.locator('.qc').count();
    check('question cards are listed from the server', cards >= 2, String(cards));
    check('a card has the plain question, a crop and big buttons', (await p.locator('.qc').first().locator('.qc-q').textContent()).length > 10 && (await p.locator('.qc-crop').count()) === cards && (await p.locator('.qc .qc-btn').count()) >= 4);
    // type a number on the C2 L card
    const cL = p.locator('.qc[data-part="C2"][data-key="L"]');
    await cL.locator('[data-qa="type"]').click();
    await cL.locator('.qc-num').fill('820');
    await cL.locator('.qc-form button[type="submit"]').click();
    await p.waitForFunction(() => !document.querySelector('.qc[data-part="C2"][data-key="L"]'), null, { timeout: 15000 });
    let order = await (await api.get(`/api/orders/${oid}`)).json();
    const l2 = order.parts.find((x) => x.no === 'C2').values.find((x) => x.key === 'L');
    check('typed answer stored as a confirmed value', l2.value === 820 && l2.state === 'CONFIRMED', JSON.stringify(l2));
    // "Yes, that's right" on C3 D
    const cD = p.locator('.qc[data-part="C3"][data-key="D"]');
    await cD.locator('[data-qa="yes"]').click();
    await p.waitForFunction(() => !document.querySelector('.qc[data-part="C3"][data-key="D"]'), null, { timeout: 15000 });
    order = await (await api.get(`/api/orders/${oid}`)).json();
    const d3 = order.parts.find((x) => x.no === 'C3').values.find((x) => x.key === 'D');
    check('Yes answer confirms the value through the decisions API', d3.state === 'CONFIRMED', JSON.stringify(d3));
    check('no approve or release control on the simple view', (await p.locator('button', { hasText: /approve|release|承認|リリース/i }).count()) === 0);
    if (SHOT_DIR) await p.screenshot({ path: path.join(SHOT_DIR, 'f2-simple-en-1440.png'), fullPage: true });

    /* ---- pro view: sidebar, tabs, chips, review rows, override ---- */
    await p.goto(BASE + '/#/pro/' + oid);
    await p.waitForSelector('.jp-part', { timeout: 20000 });
    check('sidebar lists the parts', (await p.locator('.jp-part').count()) === 3);
    await p.locator('.jp-part[data-part="C1"]').click();
    await p.waitForSelector('.jp-img');
    check('Drawing tab shows the page picture with data-op-page', (await p.locator('.jp-img[data-op-page]').count()) === 1);
    await p.locator('.jp-chip[data-chip="D"]').click();
    await p.waitForSelector('.jp-box.is-lit');
    check('a value chip lights its box on the drawing', (await p.locator('.jp-box.is-lit').count()) === 1);
    check('review row names the source_inferred hold', /source_inferred/.test(await p.textContent('.jp-rows')));
    await p.click('[data-tab="cut"]');
    check('Cut pattern tab shows a pattern or the plain note', (await p.locator('.jp-svg, .jf1-note').count()) >= 1);
    await p.click('[data-tab="3d"]');
    await p.waitForTimeout(600);
    check('3D tab shows the model or the no-WebGL note', (await p.locator('.jp-3d canvas, [role="status"], .jf1-note').count()) >= 1);
    await p.click('[data-tab="cad"]');
    await p.waitForSelector('.jp-view p, .jp-view a');
    const cadNote = await p.locator('.jp-view').textContent();
    check('NF CAD tab says it is not installed on a 404', /not installed/i.test(cadNote) || (await p.locator('.jp-view a[href^="/cad?src=doc:"]').count()) === 1, cadNote);
    await p.click('[data-tab="drawing"]');
    await p.waitForSelector('.jp-rows');
    // override: refused for the agent, refused for a short reason, refused outside the allow-list, accepted for a person
    const asAgent = await agent.post(`/api/orders/${oid}/overrides`, { data: { part: 'C1', code: 'source_inferred', reason: 'the agent tries to wave this hold through' } });
    check('the AI operator cannot override (403)', asAgent.status() === 403, 'status ' + asAgent.status());
    const bad = await api.post(`/api/orders/${oid}/overrides`, { data: { part: 'C1', code: 'missing', reason: 'a long enough reason for the list check' } });
    check('a code outside the allow-list is refused (409)', bad.status() === 409, 'status ' + bad.status());
    check('an agent-free page: override button only where allowed', (await p.locator('[data-ov-open]').count()) === 1);
    await p.click('[data-ov-open="source_inferred"]');
    await p.fill('.jp-ta', 'too short');
    await p.click('.jp-ov button[type="submit"]');
    check('a short reason is refused with a message', (await p.locator('.jp-ov .jf1-err:not([hidden])').count()) === 1);
    let list = await (await api.get(`/api/orders/${oid}/overrides`)).json();
    check('nothing recorded for the short reason', list.overrides.length === 0);
    await p.fill('.jp-ta', 'Checked against the paper drawing: D is 216.3.');
    await p.click('.jp-ov button[type="submit"]');
    await p.waitForSelector('[data-ov-undo]', { timeout: 15000 });
    list = await (await api.get(`/api/orders/${oid}/overrides`)).json();
    check('a person override is recorded with name and reason', list.overrides.length === 1 && list.overrides[0].active && !!list.overrides[0].by && /Checked against/.test(list.overrides[0].reason), JSON.stringify(list.overrides));
    check('the override list shows the mark text', /PERSON OVERRIDE — NOT VERIFIED BY NF/.test(await p.textContent('[data-insp]')));
    await p.click('[data-ov-undo]');
    await p.waitForFunction(() => !document.querySelector('[data-ov-undo]'), null, { timeout: 15000 });
    list = await (await api.get(`/api/orders/${oid}/overrides`)).json();
    check('the override can be undone', list.overrides.length === 1 && !list.overrides[0].active);
    // rules: a preview only
    await p.click('[data-pm="rules"]');
    await p.waitForSelector('.jp-tbl');
    await p.selectOption('.jp-sel', 'allow');
    await p.fill('.jp-in', '3');
    await p.click('.jp-rform button[type="submit"]');
    await p.waitForSelector('[data-impact] p', { timeout: 20000 });
    check('rules panel shows the preview result', (await p.textContent('[data-impact]')).length > 5);
    const rulesAfter = await (await api.get(`/api/orders/${oid}/rules`)).json();
    check('the preview saved nothing', JSON.stringify(rulesAfter.rules || {}) === JSON.stringify((await (await api.get(`/api/orders/${oid}/rules`)).json()).rules || {}) && !('allow' in (rulesAfter.rules || {})) );
    check('no approve or release control on the pro view', (await p.locator('button', { hasText: /approve|release|承認|リリース/i }).count()) === 0);
    if (SHOT_DIR) await p.screenshot({ path: path.join(SHOT_DIR, 'f2-pro-en-1440.png'), fullPage: true });
    await en.ctx.close();

    /* ---- JA 390: no raw keys, no horizontal scroll ---- */
    const ja = await open('ja', 390);
    const q = ja.page;
    await q.goto(BASE + '/#/pro/' + oid);
    await q.waitForSelector('.jp-part', { timeout: 20000 });
    await q.locator('.jp-part[data-part="C1"]').click();
    const jaText = await q.textContent('#view');
    check('JA pro view: Japanese title', jaText.includes('ジョブの確認'));
    check('JA pro view: no raw translation keys', !RAW_KEY.test(jaText), (jaText.match(RAW_KEY) || [''])[0]);
    check('390px pro view: no horizontal scroll', await q.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
    await q.goto(BASE + '/#/start/' + oid);
    await q.waitForSelector('.lv-bar', { timeout: 20000 });
    const jaS = await q.textContent('#view');
    check('JA simple view: controls in Japanese', jaS.includes('一時停止') && jaS.includes('引き継ぐ'));
    check('JA simple view: no raw translation keys', !RAW_KEY.test(jaS), (jaS.match(RAW_KEY) || [''])[0]);
    check('390px simple view: no horizontal scroll', await q.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
    if (SHOT_DIR) await q.screenshot({ path: path.join(SHOT_DIR, 'f2-simple-ja-390.png'), fullPage: true });
    await ja.ctx.close();

    /* ---- reduced motion: the blob stands still ---- */
    const rm = await open('en', 1440, { reducedMotion: 'reduce' });
    await rm.page.goto(BASE + '/#/start/' + oid);
    await rm.page.waitForSelector('.op-blob-body', { state: 'attached', timeout: 15000 });
    const anim = await rm.page.$eval('.op-blob-body', (e) => getComputedStyle(e).animationName);
    check('reduced motion: the blob does not animate', anim === 'none', anim);
    await rm.ctx.close();

    check('no CSP violations', cspViolations.length === 0, cspViolations.slice(0, 2).join(' | '));
    check('no page errors', pageErrors.length === 0, pageErrors.slice(0, 2).join(' | '));
  } catch (e) {
    check('job f2 flow', false, msgOf(e));
  } finally {
    try { if (browser) await browser.close(); } catch { /* */ }
    try { await api.dispose(); await agent.dispose(); } catch { /* */ }
  }
  console.log(passed + ' passed, ' + failed + ' failed');
  process.exit(failed ? 1 : 0);
}

main();
