'use strict';
// NF CAD UI smoke: open a stored DXF in the viewer, then measure and pick on the canvas (real Chromium).
// Run by tests/cad_ui_smoke.sh, which starts its own server on a port in 8796-8829 and passes NF_BASE and the session.
const fs = require('fs');
const path = require('path');
const { chromium } = require('playwright');

const BASE = (process.env.NF_BASE || '').replace(/\/$/, '');
const TOKEN = process.env.NF_SESSION || '';
const DOC = process.env.NF_DOC || '1';
const fails = [];
const check = (name, ok) => { console.log(`${ok ? 'ok  ' : 'FAIL'} - ${name}`); if (!ok) fails.push(name); };

(async () => {
  if (!BASE || !TOKEN) throw new Error('NF_BASE and NF_SESSION are required');
  const url = new URL(BASE);
  const browser = await chromium.launch({ headless: true });
  const ctx = await browser.newContext({ viewport: { width: 1100, height: 700 }, reducedMotion: 'reduce' });
  const [name, value] = TOKEN.split('=');
  await ctx.addCookies([{ name, value, domain: url.hostname, path: '/' }]);
  const page = await ctx.newPage();
  const errors = [];
  page.on('pageerror', (e) => errors.push(String(e)));
  page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });
  await page.goto(`${BASE}/cad?src=doc:${DOC}&lang=en`);

  // open: the status line names the units and the entity count
  await page.waitForFunction(() => /Entities|要素数/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
  const status = await page.textContent('#status');
  check('open: status shows units and entity count', /millimeter/.test(status) && /Entities: 7/.test(status));
  check('open: layers listed with CUT and MARK', (await page.textContent('#layers')).includes('CUT (5)'));

  // measure: two clicks on the canvas show distance, dx/dy and direction
  await page.click('#t-measure');
  const box = await page.locator('#view').boundingBox();
  await page.mouse.click(box.x + box.width * 0.3, box.y + box.height * 0.5);
  await page.mouse.click(box.x + box.width * 0.7, box.y + box.height * 0.3);
  await page.waitForFunction(() => /Distance|距離/.test(document.getElementById('info').textContent), null, { timeout: 10000 });
  const info = await page.textContent('#info');
  check('measure: distance, dx/dy and direction are shown', /Distance/.test(info) && /dx, dy/.test(info) && /Direction/.test(info));

  // pick: a tap on the canvas names the entity under it or says nothing is there
  await page.click('#t-pick');
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.waitForFunction(() => document.getElementById('info').textContent.length > 0, null, { timeout: 10000 });
  check('pick: the result panel answers', (await page.textContent('#info')).length > 0);

  // the quality checks are shown with their rules
  await page.click('#checks');
  await page.waitForFunction(() => document.getElementById('checks-out').textContent.includes('readable'), null, { timeout: 10000 });
  check('checks: all rules listed', (await page.textContent('#checks-out')).includes('units_declared'));

  // Japanese switch keeps the page working
  await page.goto(`${BASE}/cad?src=doc:${DOC}&lang=ja`);
  await page.waitForFunction(() => /要素数/.test(document.getElementById('status').textContent), null, { timeout: 15000 });
  check('EN/JA: Japanese labels render', true);

  // no console or page errors
  check('no page errors', errors.length === 0);
  if (errors.length) console.log(errors.slice(0, 5).join('\n'));
  await browser.close();
  if (fails.length) { console.log(`${fails.length} failed`); process.exit(1); }
  console.log('all UI checks passed');
})().catch((e) => { console.error('FAIL - ui smoke:', e.message); process.exit(1); });
