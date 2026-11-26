// NEXUS FABRIC — capture matrix: theme x language x viewport, 8 screens each.
// Usage: NF_BASE=http://127.0.0.1:8795 OUT=<out-dir> node docs/design/tools/capture_apple.js
// The server must already be seeded (docs/design/tools/seed_demo.js).
'use strict';
const fs = require('fs');
const path = require('path');
const { chromium, request } = require('playwright');

const BASE = (process.env.NF_BASE || 'http://127.0.0.1:8795').replace(/\/$/, '');
const OUT = process.env.OUT || path.join(__dirname, 'apple-shots');
const H = { 'X-NF-Request': '1' };
const CRED = { username: 'admin', password: 'correct-horse-9' };
const VIEWPORTS = [[1440, 900], [390, 844]];
const stateOf = (c) => c.state || c.release_state || '';

// Pick the seeded jobs once: a computed job with a pattern, a refused/withheld job, a stored conversion.
async function discover() {
  const api = await request.newContext({ baseURL: BASE });
  const login = await api.post('/api/login', { headers: H, data: CRED });
  if (!login.ok()) throw new Error('login failed: ' + login.status());
  const body = await (await api.get('/api/calculations', { headers: H })).json();
  const jobs = Array.isArray(body) ? body : (body && body.calculations) || [];
  const calcJob = jobs.find((c) => stateOf(c) === 'REVIEW_REQUIRED' && c.family === 'header_with_branch')
    || jobs.find((c) => stateOf(c) === 'REVIEW_REQUIRED')
    || jobs.find((c) => !['REFUSED', 'NO_RESULT'].includes(stateOf(c))) || jobs[0] || null;
  const refusalJob = jobs.find((c) => ['REFUSED', 'NO_RESULT'].includes(stateOf(c)))
    || jobs.find((c) => ['SUPERSEDED', 'REJECTED'].includes(stateOf(c))) || null;
  let convId = null; // no list endpoint: probe the first few conversion ids
  for (let i = 1; i <= 8 && convId === null; i++) {
    if ((await api.get('/api/pdf-conversions/' + i, { headers: H })).ok()) convId = i;
  }
  await api.dispose();
  return { calcJob, refusalJob, convId };
}

(async () => {
  fs.mkdirSync(OUT, { recursive: true });
  const { calcJob, refusalJob, convId } = await discover();
  console.log(`seed: conversion=${convId === null ? 'none' : convId} calcJob=${calcJob ? calcJob.id : 'none'} refusalJob=${refusalJob ? refusalJob.id : 'none'}`);
  const browser = await chromium.launch({ headless: true });
  const consoleErrors = [], pageErrors = [];
  let amoebaPageErrors = 0;

  for (const [w, h] of VIEWPORTS) for (const theme of ['light', 'dark']) for (const lang of ['en', 'ja']) {
    const tag = `${theme}-${lang}-${w}x${h}`;
    const ctx = await browser.newContext({ viewport: { width: w, height: h } });
    // theme.js / i18n.js read these keys at load; set them before any page script runs
    await ctx.addInitScript(([t, l]) => {
      try { localStorage.setItem('nf.theme', t); localStorage.setItem('nf.lang', l); } catch (e) { /* storage unavailable */ }
    }, [theme, lang]);
    const page = await ctx.newPage();
    page.setDefaultTimeout(20000);
    page.on('console', (m) => { if (m.type() === 'error' && !m.text().startsWith('Failed to load resource')) consoleErrors.push(`[${tag}] ${m.text()}`); });
    page.on('pageerror', (e) => {
      pageErrors.push(`[${tag}] ${page.url()} ${e.message}`);
      if (page.url().includes('#/amoeba')) amoebaPageErrors++;
    });
    const go = async (hash, sel) => {
      await page.goto(BASE + '/' + hash, { waitUntil: 'load' });
      if (sel) await page.waitForSelector(sel, { state: 'visible', timeout: 20000 });
    };
    const shot = async (name, note) => {
      try { await page.waitForLoadState('networkidle', { timeout: 5000 }); } catch (e) { /* keep going if the network stays busy */ }
      await page.waitForTimeout(400);
      const file = path.join(OUT, `${name}-${tag}.png`);
      await page.screenshot({ path: file, fullPage: true });
      console.log(`${name}  ${file}${note ? '  (' + note + ')' : ''}`);
    };
    const screen = async (name, fn, note) => {
      try { await fn(); await shot(name, note); }
      catch (e) { console.log(`${name} SKIP ${String((e && e.message) || e).split('\n')[0]}`); }
    };

    await screen('01-signin', () => go('#/login', '#login-form'));
    await ctx.request.post(BASE + '/api/login', { headers: H, data: CRED });
    await screen('02-home', () => go('#/', '#view .dropzone'));
    if (convId !== null) await screen('03-conversion-result', () => go('#/r/' + convId, '#view .badge-label'));
    else if (calcJob) await screen('03-conversion-result', () => go('#/job/' + calcJob.id, '#gates li'), 'no seeded conversion; closest result view');
    else console.log('03-conversion-result SKIP no conversion or job seeded');
    if (calcJob) await screen('04-calc-pattern', () => go(`#/job/${calcJob.id}/pattern`, '#pattern-host svg'));
    else console.log('04-calc-pattern SKIP no calculation job seeded');
    if (refusalJob) await screen('05-calc-refusal', () => go('#/job/' + refusalJob.id, '#view h1'), 'state ' + stateOf(refusalJob));
    else console.log('05-calc-refusal SKIP no refused or withheld job seeded');
    await screen('06-more-panel', async () => {
      await go('#/', '#view .dropzone');
      const more = '.dropdown > button[data-i18n="nav.more"]';
      if (!(await page.locator(more).isVisible())) await page.click('#hamburger'); // phone: nav is behind the hamburger
      await page.click(more);
      await page.waitForSelector('.dropdown.open .dropdown-menu', { state: 'visible' });
    });
    await page.keyboard.press('Escape');
    await screen('07-amoeba-overview', () => go('#/amoeba', '#view [role="tablist"]'));
    await screen('08-amoeba-tab', () => go('#/amoeba/activities', '#view [role="tablist"]'));
    await ctx.close();
  }

  await browser.close();
  for (const m of consoleErrors) console.log('console-error: ' + m);
  for (const m of pageErrors) console.log('page-error: ' + m);
  console.log(`${consoleErrors.length} console errors, ${pageErrors.length} page errors, ${amoebaPageErrors} page errors on AMOEBA`);
  process.exit(amoebaPageErrors ? 1 : 0);
})().catch((e) => { console.error('CAPTURE FAILED', e.message); process.exit(1); });
