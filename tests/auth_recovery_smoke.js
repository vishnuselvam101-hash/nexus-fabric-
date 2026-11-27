// Account recovery smoke test (AUTH-1): starts nf-server on 127.0.0.1 with a fresh data folder and walks the browser through
//  - first-run setup: the recovery code is shown once, Continue stays disabled until "I have saved it" is ticked;
//  - "Forgot your password?": a wrong code gives one generic message, the right code sets a new password and a NEW code;
//    the old code is dead afterwards;
//  - Administration > Users > Reset password, and the forced password change at the next sign-in;
//  - Administration > Security > new recovery code (password required), old code dead;
//  - the same screens in Japanese.
// Usage: node tests/auth_recovery_smoke.js <path to nf-server binary> [port 8845-8849] [screenshot dir]
// Only 127.0.0.1 is used. Exits 0 only if every check passes.
const { spawn } = require('child_process');
const { chromium } = require('playwright');
const fs = require('fs');
const os = require('os');
const path = require('path');

const BIN = process.argv[2];
const PORT = Number(process.argv[3] || 8845);
const SHOTS = process.argv[4] || '';
if (!BIN || !(PORT >= 8845 && PORT <= 8849)) {
  console.error('usage: node tests/auth_recovery_smoke.js <nf-server binary> [port 8845-8849] [screenshot dir]');
  process.exit(2);
}
if (SHOTS) fs.mkdirSync(SHOTS, { recursive: true });
const BASE = `http://127.0.0.1:${PORT}`;
const DATA = fs.mkdtempSync(path.join(os.tmpdir(), 'nf-auth-smoke-'));
const TOKEN = 'auth-smoke-setup-token';
const USER = 'admin';
const PASS1 = 'correct-horse-9';
const PASS2 = 'battery-staple-77';
const PASS3 = 'orbit-lantern-31';
const WORKER = 'worker1';
const TEMP = 'temporary-pass-42';
const WORKER_PW = 'worker-final-pass-55';

let passed = 0, failed = 0;
const check = (name, ok, detail = '') => {
  if (ok) passed++; else failed++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} - ${name}${detail && !ok ? ` [${detail}]` : ''}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const msgOf = (e) => (e && e.message) || String(e);
const CODE_RE = /^[A-Z2-9]{5}(-[A-Z2-9]{5}){3}$/;

async function main() {
  const log = [];
  const child = spawn(BIN, [], { env: { ...process.env, NF_SETUP_TOKEN: TOKEN, NF_DATA_DIR: DATA, NF_LISTEN: `127.0.0.1:${PORT}` } });
  child.stdout.on('data', (d) => log.push(String(d)));
  child.stderr.on('data', (d) => log.push(String(d)));
  let browser = null;
  let page = null;
  try {
    for (let i = 0; i < 100; i++) {
      try { if ((await fetch(`${BASE}/api/health`)).ok) break; } catch { /* not yet */ }
      await sleep(150);
    }
    browser = await chromium.launch();
    const ctx = await browser.newContext({ viewport: { width: 1280, height: 900 } });
    page = await ctx.newPage();
    page.setDefaultTimeout(15000);
    const errors = [];
    page.on('pageerror', (e) => errors.push(msgOf(e)));
    const shot = async (name) => { if (SHOTS) await page.screenshot({ path: path.join(SHOTS, name) }); };
    const text = async (sel) => (((await page.textContent(sel).catch(() => '')) || '').trim());
    const post = (url, body) => page.evaluate(async ([u, b]) => {
      const r = await fetch(u, { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-NF-Request': '1' }, body: JSON.stringify(b) });
      return { status: r.status, body: await r.json().catch(() => ({})) };
    }, [url, body]);
    const fillLogin = async (user, pass) => {
      await page.fill('#login-user', user);
      await page.fill('#login-pass', pass);
      await page.click('#login-form button[type="submit"]');
    };
    const DROP = '#view .dropzone, #view .drop-zone';

    /* ---- 1. first-run setup shows the code once ---- */
    await page.goto(`${BASE}/`);
    await page.waitForSelector('#login-token', { state: 'visible' });
    await page.fill('#login-user', USER);
    await page.fill('#login-pass', PASS1);
    await page.fill('#login-token', TOKEN);
    await page.click('#login-form button[type="submit"]');
    await page.waitForSelector('#rc-screen', { state: 'visible', timeout: 20000 });
    const code1 = (await text('#rc-code')).replace(/\s/g, '');
    await shot('auth-1-setup-code.png');
    check('setup: a recovery code of 4 groups of 5 is shown', CODE_RE.test(code1), code1);
    check('setup: Continue is disabled until the box is ticked', await page.isDisabled('#rc-continue'));
    check('setup: Print and Copy buttons are present', (await page.isVisible('#rc-print-btn')) && (await page.isVisible('#rc-copy')));
    check('setup: the code is reachable by keyboard', await page.$eval('#rc-code', (e) => e.tabIndex >= 0));
    await page.check('#rc-saved');
    check('setup: ticking the box enables Continue', !(await page.isDisabled('#rc-continue')));
    await page.click('#rc-continue');
    await page.waitForSelector(DROP, { state: 'visible', timeout: 20000 });
    check('setup: Continue opens the program signed in', true);
    check('the code is not left in the page afterwards', !(await page.content()).includes(code1.slice(0, 5) + '-' + code1.slice(5, 10)));

    /* ---- 2. forgot password ---- */
    await post('/api/logout', {});
    await page.goto(`${BASE}/#/login`);
    await page.reload();
    await page.waitForSelector('#forgot-link', { state: 'visible' });
    await shot('auth-2-login-forgot.png');
    await page.click('#forgot-link');
    await page.waitForSelector('#recover-form', { state: 'visible' });
    await shot('auth-3-recover-form.png');
    await page.fill('#rec-user', USER);
    await page.fill('#rec-code', 'AAAAA-BBBBB-CCCCC-DDDDD');
    await page.fill('#rec-new', PASS2);
    await page.fill('#rec-new2', PASS2);
    await page.click('#rec-go');
    await page.waitForSelector('#recover-form .alert', { state: 'visible' });
    const wrong = await text('#recover-form .alert');
    check('recover: a wrong code gives one generic message', /Recovery failed/.test(wrong) && !/not found|no such|exist/i.test(wrong), wrong);
    await page.fill('#rec-user', 'nobody-here');
    await page.fill('#rec-code', code1);
    await page.click('#rec-go');
    await page.waitForFunction(() => /Recovery failed/.test((document.querySelector('#recover-form .alert') || {}).textContent || ''));
    check('recover: an unknown account gives the same message', (await text('#recover-form .alert')) === wrong);
    await page.fill('#rec-user', USER);
    await page.fill('#rec-code', code1.toLowerCase());
    await page.fill('#rec-new', 'short');
    await page.fill('#rec-new2', 'short');
    await page.click('#rec-go');
    check('recover: a short new password is refused from the catalogue',
      (await text('#recover-form .alert')) === 'The password must have at least 10 characters.');
    await page.fill('#rec-new', PASS2);
    await page.fill('#rec-new2', PASS2 + 'x');
    await page.click('#rec-go');
    check('recover: two different passwords are refused', /not the same/.test(await text('#recover-form .alert')));
    await page.fill('#rec-new2', PASS2);
    await page.click('#rec-go');
    await page.waitForSelector('#rc-screen', { state: 'visible', timeout: 20000 });
    const code2 = (await text('#rc-code')).replace(/\s/g, '');
    await shot('auth-4-new-code.png');
    check('recover: the right code gives a NEW recovery code', CODE_RE.test(code2) && code2 !== code1, code2);
    check('recover: the new-code screen says the old one stops working', /no longer works/.test(await text('#rc-screen')));
    await page.check('#rc-saved');
    await page.click('#rc-continue');
    await page.waitForSelector('#login-form', { state: 'visible' });
    check('recover: the sign-in form follows with a notice', /new password is set/.test(await text('#login-form .alert')));
    await fillLogin(USER, PASS1);
    await page.waitForSelector('#login-form .alert.bad, #login-form [role="alert"]');
    check('recover: the old password no longer signs in', /Incorrect/.test(await text('#login-form .alert')));
    await fillLogin(USER, PASS2);
    await page.waitForSelector(DROP, { state: 'visible', timeout: 20000 });
    check('recover: the new password signs in', true);
    const replay = await post('/api/recover', { username: USER, recovery_code: code1, new_password: PASS3 });
    check('recover: the old code is dead (generic refusal)', replay.status >= 400 && replay.status !== 500, 'HTTP ' + replay.status);

    /* ---- 3. admin resets another user's password ---- */
    const add = await post('/api/users', { username: WORKER, password: 'initial-pass-1234', role: 'engineer' });
    check('an engineer account is created for the reset test', add.status === 200 || add.status === 201, 'HTTP ' + add.status);
    await page.goto(`${BASE}/#/admin/users`);
    await page.waitForSelector('#adm-users');
    const row = page.locator('#adm-users tr', { hasText: WORKER });
    await row.locator('[data-act="reset"]').click();
    await page.waitForSelector('#rst-form', { state: 'visible' });
    check('reset: there is no Reset button on the administrator\'s own row',
      (await page.locator('#adm-users tr', { hasText: USER }).first().locator('[data-act="reset"]').count()) === 0);
    await shot('auth-5-reset-form.png');
    await page.fill('#rst-new', TEMP);
    await page.fill('#rst-admin', 'wrong-admin-pass');
    await page.click('#rst-go');
    await page.waitForSelector('#rst-msg .alert');
    check('reset: a wrong administrator password is refused', /not correct/.test(await text('#rst-msg .alert')), await text('#rst-msg'));
    await page.fill('#rst-admin', PASS2);
    await page.click('#rst-go');
    await page.waitForFunction(() => /was reset/.test(document.body.innerText));
    check('reset: the administrator is told the password was reset', true);
    await post('/api/logout', {});

    /* ---- 4. the user must change the temporary password ---- */
    await page.goto(`${BASE}/#/login`);
    await page.reload();
    await page.waitForSelector('#login-form', { state: 'visible' });
    await fillLogin(WORKER, TEMP);
    await page.waitForSelector('#change-form', { state: 'visible', timeout: 20000 });
    await shot('auth-6-must-change.png');
    check('must change: the temporary password leads to the change screen', true);
    await page.goto(`${BASE}/#/workspace/history`);
    await page.waitForSelector('#change-form', { state: 'visible' });
    check('must change: other pages are not reachable meanwhile', true);
    await page.fill('#chg-cur', TEMP);
    await page.fill('#chg-new', WORKER_PW);
    await page.fill('#chg-new2', WORKER_PW);
    await page.click('#chg-go');
    await page.waitForSelector('#login-form', { state: 'visible', timeout: 20000 });
    await fillLogin(WORKER, WORKER_PW);
    await page.waitForSelector(DROP, { state: 'visible', timeout: 20000 });
    check('must change: after the change the new password signs in to the program', true);
    await post('/api/logout', {});

    /* ---- 5. Administration > Security: a new recovery code ---- */
    await page.goto(`${BASE}/#/login`);
    await page.reload();
    await page.waitForSelector('#login-form', { state: 'visible' });
    await fillLogin(USER, PASS2);
    await page.waitForSelector(DROP, { state: 'visible', timeout: 20000 });
    await page.goto(`${BASE}/#/admin/security`);
    await page.waitForSelector('#rcn-form', { state: 'visible' });
    check('security: the automatic sign-in panel explains it is for desktop mode only', /desktop mode/.test(await text('#auto-server')), await text('#adm-secpanels'));
    await shot('auth-7-security.png');
    await page.fill('#rcn-pw', 'not-my-password');
    await page.click('#rcn-go');
    await page.waitForSelector('#rcn-msg .alert');
    check('security: a wrong password creates no code', /not correct/.test(await text('#rcn-msg .alert')));
    await page.fill('#rcn-pw', PASS2);
    await page.click('#rcn-go');
    await page.waitForSelector('#rc-screen', { state: 'visible' });
    const code3 = (await text('#rc-code')).replace(/\s/g, '');
    check('security: a new recovery code is shown', CODE_RE.test(code3) && code3 !== code2, code3);
    await page.check('#rc-saved');
    await page.click('#rc-continue');
    await page.waitForSelector('#rcn-form', { state: 'visible' });
    await post('/api/logout', {});
    const dead = await post('/api/recover', { username: USER, recovery_code: code2, new_password: PASS3 });
    check('security: the previous code is dead', dead.status >= 400 && dead.status !== 500, 'HTTP ' + dead.status);

    /* ---- 6. Japanese ---- */
    await page.goto(`${BASE}/#/login`);
    await page.reload();
    await page.waitForSelector('#forgot-link', { state: 'visible' });
    await page.click('#btn-lang');
    await page.waitForFunction(() => /リカバリーコード/.test((document.querySelector('#forgot-link') || {}).textContent || ''));
    await shot('auth-8-login-ja.png');
    check('Japanese: the forgot-password link', /パスワードを忘れた場合/.test(await text('#forgot-link')));
    await page.click('#forgot-link');
    await page.waitForSelector('#recover-form', { state: 'visible' });
    await shot('auth-9-recover-ja.png');
    await page.fill('#rec-user', USER);
    await page.fill('#rec-code', code3);
    await page.fill('#rec-new', PASS3);
    await page.fill('#rec-new2', PASS3);
    await page.click('#rec-go');
    await page.waitForSelector('#rc-screen', { state: 'visible' });
    await shot('auth-10-new-code-ja.png');
    check('Japanese: the code screen', /リカバリーコード/.test(await text('#rc-heading')) && /続ける/.test(await text('#rc-continue')));
    const body = await page.evaluate(() => document.body.innerText);
    check('Japanese: no English sentence on the code screen', !/recovery code|Continue|Print|Copy/.test(body.replace(/NEXUS FABRIC/g, '')), body.slice(0, 120));
    await page.check('#rc-saved');
    await page.click('#rc-continue');
    await page.waitForSelector('#login-form', { state: 'visible' });
    await fillLogin(USER, PASS3);
    await page.waitForSelector(DROP, { state: 'visible', timeout: 20000 });
    check('Japanese: the recovered account signs in', true);

    /* ---- 7. secrets stay out of the server log ---- */
    const all = log.join('');
    check('the server log carries no recovery code or password', ![code1, code2, code3, PASS1, PASS2, PASS3, TEMP, WORKER_PW].some((s) => all.includes(s)));
    check('no uncaught page error', errors.length === 0, errors.join(' | ').slice(0, 200));
  } catch (e) {
    const seen = page ? await page.evaluate(() => location.hash + ' | ' + (document.getElementById('view') || document.body).innerText.slice(0, 300)).catch(() => '') : '';
    if (page && SHOTS) await page.screenshot({ path: path.join(SHOTS, 'auth-failure.png') }).catch(() => {});
    check('the run completed', false, msgOf(e).split('\n')[0] + ' | at ' + seen);
  } finally {
    if (browser) await browser.close();
    child.kill('SIGINT');
    await Promise.race([new Promise((r) => child.on('exit', r)), sleep(10000)]);
    fs.rmSync(DATA, { recursive: true, force: true });
  }
  console.log('----');
  console.log(`passed: ${passed}  failed: ${failed}`);
  process.exit(failed ? 1 : 0);
}
main();
