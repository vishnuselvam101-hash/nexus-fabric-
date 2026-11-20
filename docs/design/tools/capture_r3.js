// Captures every screen and state of the client, per language and viewport, and measures load performance.
//
//   NF_BASE=http://127.0.0.1:8880          server seeded with docs/design/tools/seed_demo.js
//   NF_BASE_EMPTY=http://127.0.0.1:8881    optional: a second, freshly started server with an EMPTY data directory
//                                          (first-run screen and empty states; the script creates its administrator)
//   NF_SETUP_TOKEN=...                     setup token of the empty server
//   node docs/design/tools/capture_r3.js <out-dir> [--langs en,ja] [--sizes 1920x1080,1366x768] [--only name-prefix] [--no-perf]
//
// Files are written to <out-dir>/<lang>/<screen>-<width>x<height>.png, metrics to <out-dir>/metrics.json.
// Works against the round-2 client as well (it ignores ?lang= and has no language switch), which is how the
// "before" set was taken. Nothing is mocked: every screen is the real client on a real server. The only network
// interference is in the two states that need it (a delayed answer for the loading skeleton, an aborted one for
// the error state), and those are named so.
const { chromium } = require('playwright');
const fs = require('fs');
const path = require('path');

const B = process.env.NF_BASE || 'http://127.0.0.1:8880';
const BE = process.env.NF_BASE_EMPTY || '';
const TOKEN = process.env.NF_SETUP_TOKEN || '';
const args = process.argv.slice(2);
const out = args[0];
const opt = (k, d) => { const i = args.indexOf(`--${k}`); return i > 0 ? args[i + 1] : d; };
const LANGS = opt('langs', 'en').split(',');
const SIZES = opt('sizes', '1920x1080,1366x768').split(',').map((s) => s.split('x').map(Number));
const ONLY = opt('only', '');
const PERF = !args.includes('--no-perf');
const ADMIN = { username: 'admin', password: 'correct-horse-9' };
if (!out) { console.error('usage: capture_r3.js <out-dir> [--langs en,ja] [--sizes WxH,...]'); process.exit(2); }

(async () => {
  const browser = await chromium.launch();
  const errs = [], files = [], skipped = [];
  const mkFor = (lang, w, h) => async (base) => {
        const ctx = await browser.newContext({ viewport: { width: w, height: h }, locale: lang === 'ja' ? 'ja-JP' : 'en-GB', timezoneId: 'Asia/Tokyo' });
        const p = await ctx.newPage(); p.setDefaultTimeout(6000);
        p.on('pageerror', (e) => errs.push(`${lang} ${w}: pageerror: ${e.message}`));
        p.on('console', (m) => { if (m.type() === 'error' && !/40[14]|422|ERR_FAILED|net::/.test(m.text())) errs.push(`${lang} ${w}: console: ${m.text()}`); });
        const url = (hash) => `${base}/?lang=${lang}${hash || ''}`;
        const shot = async (name) => { if (ONLY && !name.startsWith(ONLY)) return; const f = path.join(out, lang, `${name}-${w}x${h}.png`); await p.screenshot({ path: f }); files.push(f); };
        const settle = (ms = 260) => p.waitForTimeout(ms);
        // navigation is complete when the client marks the view with the hash it rendered
        const go = async (hash, sel) => {
          await p.goto(url(hash));
          // a hash-only navigation does not reload the document; a page that was showing the sign-in screen must
          if (!(await p.$('.app'))) await p.reload();
          await p.waitForFunction((x) => { const v = document.querySelector('#view'); return v && v.dataset.hash === x; }, hash, { timeout: 15000 });
          if (sel) await p.waitForSelector(sel, { state: 'attached', timeout: 8000 });
          await settle();
        };
        // a step that needs something the build under capture does not have is skipped and reported, never faked
        const step = async (name, fn) => { if (ONLY && !name.startsWith(ONLY)) return; try { await fn(); } catch (e) { skipped.push(`${lang} ${w}x${h} ${name}: ${e.message.split('\n')[0]}`); } };
        return { ctx, p, url, shot, settle, go, step };
  };

  // ---------- empty installation: the first-run screen, for every language and size, before anything creates the administrator
  for (const lang of LANGS) {
    fs.mkdirSync(path.join(out, lang), { recursive: true });
    if (BE && (await (await fetch(`${BE}/api/health`)).json()).setup_required) for (const [w, h] of SIZES) {
      const { ctx, p, url, shot, settle, step } = await mkFor(lang, w, h)(BE);
      await step('setup', async () => { await p.goto(url()); await p.waitForSelector('#f'); await settle(); await shot('setup'); });
      await step('setup-refused-inline', async () => { await p.fill('input[name=u]', 'admin'); await p.fill('input[name=p]', 'short'); await p.click('#f button.pri, #f button'); await p.waitForSelector('#f .field-err'); await settle(120); await shot('setup-refused-inline'); });
      await ctx.close();
    }
  }

  for (const lang of LANGS) {
    for (const [w, h] of SIZES) {
      const mk = mkFor(lang, w, h);
      // ---------- seeded installation
      const { ctx, p, url, shot, settle, go, step } = await mk(B);
      await step('login', async () => { await p.goto(url()); await p.waitForSelector('#f'); await settle(); await shot('login'); });
      await step('login-refused', async () => { await p.fill('input[name=u]', 'admin'); await p.fill('input[name=p]', 'not-the-password'); await p.click('#f button.pri, #f button'); await p.waitForSelector('.login-card [role=alert]'); await settle(120); await shot('login-refused'); });
      await ctx.request.post(`${B}/api/login`, { headers: { 'X-NF-Request': '1' }, data: ADMIN });

      await step('dashboard', async () => { await go('#/dashboard', '.stats'); await shot('dashboard'); });
      await step('projects', async () => { await go('#/projects', '#np'); await shot('projects'); });
      await step('projects-filter-nomatch', async () => { await p.fill('#flt-tbl-projects', 'zzzz'); await settle(120); await shot('projects-filter-nomatch'); });
      await step('project', async () => { await go('#/project/1', '#up'); await shot('project'); });
      await step('workspace', async () => { await go('#/workspace', '#jobs-all'); await shot('workspace'); });

      // new calculation: form and to-scale schematic
      await step('new-calculation-cylinder', async () => { await go('#/math/1', '#calc'); await shot('new-calculation-cylinder'); });
      await step('new-calculation-header-branch', async () => { await p.selectOption('#f_fam', 'header_with_branch'); await p.fill('#f_delta', '25'); await p.focus('#f_delta'); await settle(); await shot('new-calculation-header-branch'); });
      await step('new-calculation-refused-inline', async () => { await p.fill('#f_branch_radius', '400'); await p.click('#run'); await p.waitForSelector('#calc .field-err'); await settle(120); await shot('new-calculation-refused-inline'); });
      await step('new-calculation-eccentric', async () => { await p.selectOption('#f_fam', 'eccentric_reducer'); await p.focus('#f_e'); await settle(); await shot('new-calculation-eccentric'); });
      await step('new-calculation-cone', async () => { await p.selectOption('#f_fam', 'cone_frustum'); await settle(); await shot('new-calculation-cone'); });
      await step('new-calculation-branch-stub', async () => { await p.selectOption('#f_fam', 'branch_stub'); await settle(); await shot('new-calculation-branch-stub'); });
      await step('new-calculation-unusual-size', async () => { await p.selectOption('#f_fam', 'cylinder_shell'); await p.fill('#f_r_dev', '1e9'); await p.click('#run'); await p.waitForSelector('#plaus'); await settle(120); await shot('new-calculation-unusual-size'); });

      // job view: single part
      const vpos = (ci, vi) => p.evaluate(([c, v]) => { const pl = document.querySelector(`.pv .hit[data-ci="${c}"]`), q = pl.points[v], m = pl.getScreenCTM(); return [q.x * m.a + q.y * m.c + m.e, q.x * m.b + q.y * m.d + m.f]; }, [ci, vi]);
      const tab = async (k) => { await p.click(`#tab-ws-${k}`); await settle(160); };
      await step('job-eccentric-gates', async () => { await go('#/calc/2', '.pv .c'); await tab('gates'); await shot('job-eccentric-gates'); });
      await step('job-eccentric-gate-open', async () => { const g = p.locator('#tp-ws-gates [data-gate] summary, #tp-ws-gates .gate summary').first(); await g.click(); await settle(160); await shot('job-eccentric-gate-open'); });
      await step('job-eccentric-checks', async () => { await tab('checks'); await shot('job-eccentric-checks'); });
      await step('job-eccentric-zoomed', async () => { const [x, y] = await vpos(0, 60); await p.mouse.move(x, y); await p.mouse.wheel(0, -700); await settle(300); await p.mouse.move(x + 2, y + 1); await settle(160); await shot('job-eccentric-zoomed'); await p.keyboard.press('f'); });
      await step('job-cone', async () => { await go('#/calc/1', '.pv .c'); await shot('job-cone'); });
      await step('job-cylinder', async () => { await go('#/calc/4', '.pv .c'); await shot('job-cylinder'); });
      await step('job-branch-stub', async () => { await go('#/calc/3', '.pv .c'); await shot('job-branch-stub'); });

      // job view: corrected assembly, every results tab and canvas state
      await step('job-assembly-changes', async () => { await go('#/calc/7', '.pv .c'); await tab('changes'); await shot('job-assembly-changes'); });
      await step('job-assembly-changes-overlay', async () => { await p.click('#v-prev'); await settle(200); await shot('job-assembly-changes-overlay'); await p.click('#v-prev'); });
      await step('job-assembly-gates', async () => { await tab('gates'); await shot('job-assembly-gates'); });
      await step('job-assembly-checks', async () => { await tab('checks'); await shot('job-assembly-checks'); });
      await step('job-assembly-curve-selected-scrutiny', async () => { const [x, y] = await vpos(4, 100); await p.mouse.click(x, y); await tab('scrutiny'); await p.mouse.move(x + 1, y); await settle(200); await shot('job-assembly-curve-selected-scrutiny'); });
      await step('job-assembly-measure', async () => { await p.click('#t-measure'); let [x, y] = await vpos(0, 0); await p.mouse.click(x, y); [x, y] = await vpos(4, 300); await p.mouse.move(x, y); await p.mouse.click(x, y); await p.mouse.move(x + 1, y); await settle(200); await shot('job-assembly-measure'); await p.click('#t-select'); await p.focus('.pv'); await p.keyboard.press('Escape'); await p.keyboard.press('Escape'); });
      await step('job-assembly-branch-graph', async () => { await p.focus('.pv'); await p.keyboard.press('2'); await tab('graph'); await p.locator('#tp-ws-graph .dgl .node').nth(3).click(); await settle(200); await shot('job-assembly-branch-graph'); });
      await step('job-assembly-history', async () => { await tab('history'); await shot('job-assembly-history'); });
      await step('job-assembly-records', async () => { await tab('records'); await p.locator('#tp-ws-records .rec').first().click(); await settle(200); await shot('job-assembly-records'); });
      await step('job-assembly-correction-dialog', async () => { await p.click('#corr-open'); await p.waitForSelector('#corrdlg[open]'); await settle(200); await shot('job-assembly-correction-dialog'); await p.keyboard.press('Escape'); });
      await step('help-overlay', async () => { await p.locator('#view').focus(); await p.keyboard.press('?'); await p.waitForSelector('dialog#help[open]'); await settle(240); await shot('help-overlay'); await p.keyboard.press('Escape'); });
      await step('toast-and-notifications', async () => { await p.click('#replay'); await p.waitForSelector('.toast'); await settle(200); await shot('toast'); await p.click('#bell'); await p.waitForSelector('#notes:not([hidden])'); await settle(160); await shot('notifications'); await p.keyboard.press('Escape'); });
      await step('job-superseded', async () => { await go('#/calc/6', '.ws-banner.bad'); await shot('job-superseded'); });
      await step('job-advisory-scrutiny', async () => { await go('#/calc/8', '.pv .c'); await tab('scrutiny'); await shot('job-advisory-scrutiny'); });
      await step('job-no-result', async () => { await go('#/calc/5', '.ws-banner.bad'); await shot('job-no-result'); });

      await step('manufacturing', async () => { await go('#/manufacturing', '#tbl-dxf'); await shot('manufacturing'); });
      await step('verification', async () => { await go('#/verification', '#jobs-ver'); await shot('verification'); });
      for (const t of ['records', 'activities', 'obligations', 'lessons', 'dependencies/7', 'mechanisms']) await step(`amoeba-${t.replace('/7', '')}`, async () => { await go(`#/amoeba/${t}`, '#view h1'); await shot(`amoeba-${t.replace('/7', '')}`); });
      for (const t of ['users', 'security', 'audit', 'planned']) await step(`admin-${t}`, async () => { await go(`#/admin/${t}`, '#view h1'); await shot(`admin-${t}`); });
      await step('account', async () => { await go('#/account', '#pw'); await shot('account'); });
      await step('account-password-mismatch', async () => { await p.fill('#pwc', 'x'); await p.fill('#pwn', 'one-password-10'); await p.fill('#pwn2', 'another-password'); await p.click('#pw button.pri, #pw button'); await p.waitForSelector('#pw .field-err'); await settle(120); await shot('account-password-mismatch'); });

      // 404, unknown job, error and loading states
      await step('not-found', async () => { await go('#/nonsense', '.empty'); await shot('not-found'); });
      await step('job-not-found', async () => { await go('#/calc/9999', '.empty'); await shot('job-not-found'); });
      await step('error-state', async () => { await ctx.route('**/api/lessons', (r) => r.abort()); await p.goto(url('#/verification')); await p.waitForSelector('#retry'); await settle(); await shot('error-state'); await ctx.unroute('**/api/lessons'); });
      await step('loading-state', async () => {
        await ctx.route('**/api/calculations', async (r) => { await new Promise((res) => setTimeout(res, 1400)); try { await r.continue(); } catch { /* the page moved on */ } });
        try { await p.goto(url('#/workspace')); await p.waitForSelector('.skel', { timeout: 3000 }); await settle(200); await shot('loading-state'); await p.waitForSelector('#jobs-all', { timeout: 5000 }); }
        finally { await ctx.unroute('**/api/calculations'); }
      });

      // light theme
      await step('light', async () => {
        await go('#/dashboard', '.stats'); await p.click('#theme'); await settle(); await shot('light-dashboard');
        await go('#/calc/7', '.pv .c'); await tab('gates'); await shot('light-job-assembly-gates');
        await go('#/math/1', '#calc'); await p.selectOption('#f_fam', 'header_with_branch'); await settle(); await shot('light-new-calculation');
        await go('#/verification', '#jobs-ver'); await shot('light-verification');
        await p.click('#theme');
      });
      await ctx.close();

      // viewer role: what a read-only account sees
      await step('viewer-role', async () => {
        const v = await mk(B);
        await v.ctx.request.post(`${B}/api/login`, { headers: { 'X-NF-Request': '1' }, data: { username: 'viewer1', password: 'read-only-pass' } });
        await v.go('#/math/1', '.empty'); await v.shot('viewer-new-calculation-refused');
        await v.go('#/calc/7', '.pv .c'); await v.shot('viewer-job');
        await v.ctx.close();
      });

      // empty installation after setup: honest empty states
      if (BE) {
        const e = await mk(BE);
        const health = await (await e.ctx.request.get(`${BE}/api/health`)).json();
        if (health.setup_required) await e.ctx.request.post(`${BE}/api/setup`, { headers: { 'X-NF-Request': '1' }, data: { ...ADMIN, setup_token: TOKEN } });
        await e.ctx.request.post(`${BE}/api/login`, { headers: { 'X-NF-Request': '1' }, data: ADMIN });
        for (const [name, hash] of [['empty-dashboard', '#/dashboard'], ['empty-projects', '#/projects'], ['empty-workspace', '#/workspace'], ['empty-new-calculation', '#/math'], ['empty-manufacturing', '#/manufacturing'], ['empty-verification', '#/verification'], ['empty-amoeba', '#/amoeba']])
          await e.step(name, async () => { await e.go(hash, '#view h1'); await e.shot(name); });
        await e.ctx.close();
      }
    }
  }

  // ---------- performance: cold loads of the dashboard and of the heaviest job page, median of 9, per language
  const report = { taken: new Date().toISOString(), base: B, screenshots: files.length, skipped, console_errors: errs };
  if (PERF) {
    const med = (a) => a.slice().sort((x, y) => x - y)[Math.floor(a.length / 2)];
    report.perf = {};
    for (const lang of LANGS) {
      const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 } });
      await ctx.request.post(`${B}/api/login`, { headers: { 'X-NF-Request': '1' }, data: ADMIN });
      report.perf[lang] = {};
      for (const [name, hash] of [['dashboard', '#/dashboard'], ['job', '#/calc/7']]) {
        const dcl = [], render = [], nodes = [];
        for (let i = 0; i < 9; i++) {
          const p = await ctx.newPage(); await p.goto(`${B}/?lang=${lang}${hash}`);
          await p.waitForFunction((x) => { const v = document.querySelector('#view'); return v && v.dataset.hash === x; }, hash);
          const m = await p.evaluate(() => new Promise((res) => requestAnimationFrame(() => res({ dcl: performance.getEntriesByType('navigation')[0].domContentLoadedEventEnd, now: performance.now(), nodes: document.querySelectorAll('*').length }))));
          dcl.push(m.dcl); render.push(m.now); nodes.push(m.nodes); await p.close();
        }
        report.perf[lang][name] = { domContentLoaded_ms: +med(dcl).toFixed(1), first_view_rendered_ms: +med(render).toFixed(1), first_view_min_max_ms: [+Math.min(...render).toFixed(1), +Math.max(...render).toFixed(1)], dom_nodes: med(nodes) };
      }
      await ctx.close();
    }
    const ctx = await browser.newContext();
    const assets = {};
    for (const a of ['/', '/app.js', '/viewer.js', '/i18n.js', '/styles.css']) { const r = await ctx.request.get(B + a); if (r.ok()) assets[a] = (await r.body()).length; }
    assets.total = Object.values(assets).reduce((x, y) => x + y, 0);
    report.assets_bytes = assets;
    await ctx.close();
  }
  fs.writeFileSync(path.join(out, 'metrics.json'), JSON.stringify(report, null, 1));
  console.log(JSON.stringify(report, null, 1));
  await browser.close();
})().catch((e) => { console.error('CAPTURE FAILED', e.message); process.exit(1); });
