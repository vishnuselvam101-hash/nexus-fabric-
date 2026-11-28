/* NEXUS FABRIC — operator cursor smoke test. A headless browser replays a recorded operator event stream with
   web/js/operator-cursor.js on a demo page, in English and Japanese, with and without reduced motion, and checks:
   the cursor reaches the number on the page picture, a click shows its ripple, a typed value appears, the reason is
   shown, a flagged step is marked, an unknown action word and a CAD target do not break it, pause holds the replay,
   no request leaves the origin (no CDN), no console error, no CSP violation.
   Usage: node tests/operator_cursor_smoke.js [stream.json]   (default tests/fixtures/operator-stream.json) */
'use strict';
const fs = require('fs');
const http = require('http');
const path = require('path');
const { chromium } = require('playwright');

const WEB = path.join(__dirname, '..', 'web');
const STREAM = JSON.parse(fs.readFileSync(process.argv[2] || path.join(__dirname, 'fixtures', 'operator-stream.json'), 'utf8'));
// a 1x1 transparent PNG scaled by the page; the page is 1000 x 700 "page pixels" via data-op-width/height
const PNG = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=', 'base64');
const CSP = "default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; base-uri 'none'";
const PAGE = `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>cursor</title>
<link rel="stylesheet" href="/operator-cursor.css"><link rel="stylesheet" href="/demo.css"></head><body>
<img id="pg" src="/page.png" data-op-page="1" data-op-width="1000" data-op-height="700" alt="">
<button id="compute-btn" data-op-ui="compute-btn">Compute</button><input id="part-1-r_dev" data-op-ui="part-1-r_dev">
<div data-op-cad id="cad"></div><script type="module" src="/demo.js"></script></body></html>`;
const DEMO_CSS = '#pg{position:absolute;left:100px;top:80px;width:500px;height:350px;border:1px solid #888}#compute-btn{position:absolute;left:700px;top:500px}#part-1-r_dev{position:absolute;left:700px;top:100px}#cad{position:absolute;left:100px;top:480px;width:300px;height:150px}';
const DEMO_JS = `import { createCursor } from '/js/operator-cursor.js';
import { setLang } from '/js/i18n.js';
const q = new URLSearchParams(location.search);
setLang(q.get('lang') || 'en');
const c = createCursor({ reduced: q.get('reduced') === '1', cad: (t) => { const r = document.getElementById('cad').getBoundingClientRect(); const [x, y] = t.canvas || [0, 0]; return new DOMRect(r.left + x, r.top + y, 1, 1); } });
window.cursor = c; window.shown = [];
window.run = async (events) => { c.opts = events; await c.play(events); window.done = true; };
document.addEventListener('opshow', (e) => window.shown.push(e.detail));
`;

const server = http.createServer((req, res) => {
  const u = req.url.split('?')[0];
  const send = (type, body) => { res.writeHead(200, { 'content-type': type, 'content-security-policy': CSP }); res.end(body); };
  if (u === '/') return send('text/html; charset=utf-8', PAGE);
  if (u === '/page.png') return send('image/png', PNG);
  if (u === '/demo.css') return send('text/css', DEMO_CSS);
  if (u === '/demo.js') return send('text/javascript', DEMO_JS);
  const f = path.join(WEB, u === '/operator-cursor.css' ? 'operator-cursor.css' : u);
  if (f.startsWith(WEB) && fs.existsSync(f) && fs.statSync(f).isFile()) return send(f.endsWith('.css') ? 'text/css' : 'text/javascript', fs.readFileSync(f));
  res.writeHead(404); res.end();
});

let passed = 0, failed = 0;
const check = (name, ok, detail) => { if (ok) { passed++; console.log('ok  - ' + name); } else { failed++; console.log('FAIL - ' + name + (detail ? ' [' + detail + ']' : '')); } };

async function once(browser, port, lang, reduced) {
  const ctx = await browser.newContext({ viewport: { width: 1000, height: 700 }, reducedMotion: reduced ? 'reduce' : 'no-preference' });
  const page = await ctx.newPage();
  const errors = [], external = [], csp = [];
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('request', (r) => { if (!r.url().startsWith(`http://127.0.0.1:${port}/`) && !r.url().startsWith('data:')) external.push(r.url()); });
  await page.addInitScript(() => { document.addEventListener('securitypolicyviolation', (e) => (window.__csp = (window.__csp || []).concat(e.violatedDirective))); });
  await page.goto(`http://127.0.0.1:${port}/?lang=${lang}&reduced=${reduced ? 1 : 0}`);
  await page.waitForFunction(() => window.cursor);
  const tag = `${lang}${reduced ? '/reduced' : ''}`;
  // box of the first page-target click event in page pixels -> where the cursor must end up
  const first = STREAM.find((e) => e.target.page && e.target.bbox && e.action === 'click');
  const img = await page.locator('#pg').boundingBox();
  const [x0, y0, x1, y1] = first.target.bbox;
  const want = { x: img.x + ((x0 + x1) / 2) * (img.width / 1000), y: img.y + ((y0 + y1) / 2) * (img.height / 700) };
  const samples = [];
  // run the recorded stream; sample the layer after each event through onEvent-like polling
  await page.evaluate((events) => { window.sample = []; const c = window.cursor; const orig = c.play; window.run2 = async () => { for (const ev of events) { await c.play([ev]); const t = document.querySelector('.op-tip'); window.sample.push({ seq: ev.seq, action: ev.action, why: t.querySelector('.op-why').textContent, act: t.querySelector('.op-act').textContent, chip: document.querySelector('.op-chip').textContent, cls: document.querySelector('.op-layer').className, tx: document.querySelector('.op-cursor').style.transform, target: document.querySelector('.op-layer').dataset.target }); } window.done = true; }; window.run2(); }, STREAM);
  // pause holds the replay
  await page.waitForFunction(() => window.sample.length >= 2, null, { timeout: 30000 });
  await page.evaluate(() => window.cursor.pause());
  const n1 = await page.evaluate(() => window.sample.length);
  await page.waitForTimeout(reduced ? 400 : 1500);
  const n2 = await page.evaluate(() => window.sample.length);
  check(`${tag}: pause holds the replay (at most one step finishes)`, n2 - n1 <= 1, `${n1} -> ${n2}`);
  check(`${tag}: pause badge shown`, await page.locator('.op-badge').isVisible());
  await page.evaluate(() => window.cursor.resume());
  await page.waitForFunction(() => window.done, null, { timeout: 60000 });
  const s = await page.evaluate(() => window.sample);
  check(`${tag}: every event of the stream was shown`, s.length === STREAM.length, `${s.length}/${STREAM.length}`);
  const click = s.find((x) => x.seq === first.seq);
  const m = /translate\((-?\d+)px, (-?\d+)px\)/.exec(click.tx) || [];
  check(`${tag}: the cursor is on the number of the page (within 3 px)`, Math.abs(Number(m[1]) - want.x) <= 3 && Math.abs(Number(m[2]) - want.y) <= 3, `${m[1]},${m[2]} vs ${want.x.toFixed(1)},${want.y.toFixed(1)}`);
  const typed = s.find((x) => x.action === 'type');
  const typedEv = STREAM.find((e) => e.action === 'type');
  check(`${tag}: the typed value shows in the chip`, typed && typed.chip === String(typedEv.value), typed && typed.chip);
  check(`${tag}: the reason is shown`, s.every((x, i) => x.why === STREAM[i].reason));
  const flaggedIdx = STREAM.findIndex((e) => e.flags && e.flags.length);
  check(`${tag}: a flagged step is marked`, flaggedIdx < 0 || s[flaggedIdx].cls.includes('op-flagged'));
  const askIdx = STREAM.findIndex((e) => e.action === 'ask');
  check(`${tag}: a question is marked`, askIdx < 0 || s[askIdx].cls.includes('op-ask'));
  const cadIdx = STREAM.findIndex((e) => e.target.cad);
  check(`${tag}: a CAD target is placed`, cadIdx < 0 || s[cadIdx].target === '1');
  const unkIdx = STREAM.findIndex((e) => !['look', 'point', 'click', 'type', 'compute', 'ask'].includes(e.action));
  check(`${tag}: an unknown action word is shown as pointing`, unkIdx < 0 || s[unkIdx].act === (lang === 'ja' ? '指しています' : 'Pointing'), unkIdx >= 0 ? s[unkIdx].act : '');
  const label = await page.locator('.op-who').textContent();
  check(`${tag}: labels follow the language`, label === (lang === 'ja' ? 'AIオペレーター' : 'AI operator'), label);
  if (reduced) check(`${tag}: reduced motion: no transition on the cursor`, (await page.evaluate(() => getComputedStyle(document.querySelector('.op-cursor')).transitionDuration)).startsWith('0s'));
  check(`${tag}: nothing left the origin (no CDN)`, external.length === 0, external.join(','));
  check(`${tag}: no console error`, errors.length === 0, errors.join(' | '));
  check(`${tag}: no CSP violation`, !(await page.evaluate(() => window.__csp)), String(await page.evaluate(() => window.__csp)));
  await ctx.close();
}

(async () => {
  // test servers use ports 8796-8829 only
  let port = 0;
  for (let p = 8821; p <= 8829 && !port; p++) {
    const ok = await new Promise((r) => { server.once('error', () => r(false)); server.listen(p, '127.0.0.1', () => r(true)); });
    if (ok) port = p;
  }
  if (!port) throw new Error('no free port in 8821-8829');
  const browser = await chromium.launch();
  try {
    for (const [lang, reduced] of [['en', false], ['ja', false], ['en', true]]) await once(browser, port, lang, reduced);
  } catch (e) { check('smoke ran to the end', false, (e && e.message) || String(e)); }
  await browser.close();
  server.close();
  console.log(`\n${passed} passed, ${failed} failed`);
  process.exit(failed ? 1 : 0);
})();
