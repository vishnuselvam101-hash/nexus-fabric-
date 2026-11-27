// Desktop-mode smoke test: starts `nf-server --desktop` on 127.0.0.1 with a fresh data folder and checks
//  - the listening address is loopback only, and foreign host names are refused;
//  - first-run setup through the browser, with the one-time token in the URL fragment (not sent to the server);
//  - the fragment is removed from the address bar, and the setup link is dead after setup;
//  - a second copy on the same data folder is refused;
//  - Ctrl+C (SIGINT) stops the program cleanly, with the database checkpointed.
// Usage: node tests/desktop_smoke.js <path to nf-server binary> [port 8860-8869]
// Only 127.0.0.1 is used. Exits 0 only if every check passes; prints each check and the pass/fail counts.
const { spawn } = require('child_process');
const { chromium } = require('playwright');
const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');

const BIN = process.argv[2];
const PORT = Number(process.argv[3] || 8860);
if (!BIN || !(PORT >= 8860 && PORT <= 8869)) {
  console.error('usage: node tests/desktop_smoke.js <nf-server binary> [port 8860-8869]');
  process.exit(2);
}
const WIN = process.platform === 'win32';
const DATA_ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'nf-desktop-smoke-'));
const ENV = { ...process.env, XDG_DATA_HOME: DATA_ROOT, LOCALAPPDATA: DATA_ROOT, HOME: DATA_ROOT };
const DATA_DIR = WIN ? path.join(DATA_ROOT, 'NexusFabric', 'data') : path.join(DATA_ROOT, 'nexus-fabric');
const results = [];
const check = (name, ok, detail = '') => {
  results.push({ name, ok });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? `  (${detail})` : ''}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Runs the program, collecting its output; resolves to the exit code.
function start(extra) {
  const child = spawn(BIN, ['--desktop', '--port', String(PORT), '--no-browser', ...extra], { env: ENV, windowsHide: true });
  const out = { text: '', code: null };
  child.stdout.on('data', (d) => (out.text += d));
  child.stderr.on('data', (d) => (out.text += d));
  out.exited = new Promise((r) => child.on('exit', (code) => { out.code = code; r(code); }));
  out.child = child;
  return out;
}
async function waitFor(out, pattern, ms = 15000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const m = out.text.match(pattern);
    if (m) return m;
    await sleep(100);
  }
  return null;
}
// A raw request with a chosen Host header; resolves to the status code.
function rawGet(p, host) {
  return new Promise((resolve) => {
    const req = http.request({ host: '127.0.0.1', port: PORT, path: p, method: 'GET', headers: { Host: host } }, (res) => {
      res.resume();
      resolve(res.statusCode);
    });
    req.on('error', () => resolve(0));
    req.end();
  });
}

(async () => {
  const run = start([]);
  const addr = await waitFor(run, /Address: http:\/\/127\.0\.0\.1:\d+\//);
  check('program prints the loopback address', !!addr, addr ? addr[0] : run.text.slice(0, 300));
  const link = await waitFor(run, /Setup link[^\n]*?(http:\/\/127\.0\.0\.1:\d+\/#setup=[0-9a-f]{64})/);
  check('first run prints a one-time setup link with a 64-hex token', !!link);
  const linkUrl = link ? link[1] : `http://127.0.0.1:${PORT}/`;
  const token = linkUrl.split('#setup=')[1] || '';
  check('the token is at least 32 characters', token.length >= 32, `${token.length} characters`);
  check('the data folder exists', fs.existsSync(DATA_DIR), DATA_DIR);
  if (!WIN) {
    const mode = fs.statSync(DATA_DIR).mode & 0o777;
    check('data folder is owner-only (0700)', mode === 0o700, mode.toString(8));
  }
  check('a foreign Host name is refused', [400, 403, 421].includes(await rawGet('/api/health', 'evil.example')) );
  check('the loopback Host name is answered', (await rawGet('/api/health', `127.0.0.1:${PORT}`)) === 200);

  // Second copy on the same data folder: must refuse and say so.
  const second = start([]);
  const code2 = await Promise.race([second.exited, sleep(15000).then(() => 'timeout')]);
  check('second copy on the same data folder is refused', code2 === 2 && /already running/.test(second.text), `exit ${code2}`);

  // First-run setup through the browser, with the token only in the fragment.
  const browser = await chromium.launch();
  // the catalogue texts the first-run screen shows (English; Japanese for one case)
  const TXT = {
    tokenRefused: 'The setup token was not accepted. Type the token printed by the program, or open the setup link again.',
    throttled: 'Too many failed attempts from this address. Wait a few minutes, then try again.',
    throttledJa: 'このアドレスからの失敗が多すぎます。数分待ってから、もう一度お試しください。',
    shortPw: 'The password must have at least 10 characters.',
    linkBroken: 'This setup link is incomplete — open the link printed by the program again.',
  };
  // A setup link whose token the real server refuses (403): the token is forgotten, the token field is shown.
  const badCtx = await browser.newContext({ viewport: { width: 1366, height: 768 } });
  const badPage = await badCtx.newPage();
  await badPage.goto(`http://127.0.0.1:${PORT}/#setup=${'0'.repeat(64)}`);
  await badPage.waitForSelector('#login-form', { timeout: 15000 });
  await badPage.fill('#login-user', 'admin');
  await badPage.fill('#login-pass', 'correct-horse-9');
  await badPage.click('#login-form button[type="submit"]');
  await badPage.waitForSelector('#login-token', { state: 'visible', timeout: 15000 }).catch(() => {});
  check('a refused setup link: the token field is shown by hand', await badPage.isVisible('#login-token').catch(() => false));
  const refusal = ((await badPage.textContent('#login-form .alert').catch(() => '')) || '').trim();
  check('a refused setup link: the specific message is shown', refusal === TXT.tokenRefused, refusal.slice(0, 100));
  await badCtx.close();
  // The setup answers are intercepted with the real server bodies (no server state changes). A refused token (403) is
  // forgotten; a weak password (400), a conflict (409) and a throttle (429) keep the link token.
  const LINK_TOKEN = 'a'.repeat(64);
  const MANUAL_TOKEN = 'b'.repeat(64);
  const SERVER = {
    throttled: { status: 429, body: { error: 'too many setup attempts with a wrong token; try again in 10 minutes' } },
    shortPw: { status: 400, body: { error: 'password must have at least 10 characters' } },
    tokenRefused: { status: 403, body: { error: 'setup token required: the first administrator can only be created with the one-time setup token of this installation (shown to the installer at first start, or set with NF_SETUP_TOKEN)' } },
    completed: { status: 409, body: { error: 'setup already completed' } },
  };
  const setupCase = async (answer, { password = 'correct-horse-9', ja = false, retry = true } = {}) => {
    const ctx2 = await browser.newContext({ viewport: { width: 1366, height: 768 } });
    const p2 = await ctx2.newPage();
    const bodies = [];
    await p2.route('**/api/setup', (route) => {
      bodies.push(route.request().postData() || '');
      route.fulfill({ status: answer.status, contentType: 'application/json', body: JSON.stringify(answer.body) });
    });
    await p2.goto(`http://127.0.0.1:${PORT}/#setup=${LINK_TOKEN}`);
    await p2.waitForSelector('#login-form', { timeout: 15000 });
    if (ja) await p2.click('#btn-lang');
    await p2.fill('#login-user', 'admin');
    await p2.fill('#login-pass', password);
    await p2.click('#login-form button[type="submit"]');
    for (let i = 0; i < 100 && bodies.length < 1; i++) await sleep(100);
    await p2.waitForSelector('#login-form .alert', { timeout: 5000 }).catch(() => {});
    const alert = ((await p2.textContent('#login-form .alert').catch(() => '')) || '').trim();
    const tokenShown = await p2.isVisible('#login-token').catch(() => false);
    const heading = ((await p2.textContent('#login-heading').catch(() => '')) || '').trim();
    if (retry) {
      if (answer.status === 403) await p2.fill('#login-token', MANUAL_TOKEN);   // the link token is gone: the next try sends the typed token
      await p2.click('#login-form button[type="submit"]');   // otherwise the link token is still held and sent again
      for (let i = 0; i < 100 && bodies.length < 2; i++) await sleep(100);
    }
    await ctx2.close();
    return { alert, tokenShown, heading, second: bodies[1] || '' };
  };
  const wait = await setupCase(SERVER.throttled, { ja: true });
  check('a throttled setup (429) shows the catalogue wait text (Japanese)', wait.alert === TXT.throttledJa, wait.alert.slice(0, 100));
  check('a throttled setup (429) keeps the link token and does not ask for a token by hand',
    !wait.tokenShown && wait.second.includes(`"setup_token":"${LINK_TOKEN}"`), `token field ${wait.tokenShown}`);
  const weak = await setupCase(SERVER.shortPw, { password: 'short-1' });
  check('a weak password (400) shows the catalogue text, not the server English', weak.alert === TXT.shortPw, weak.alert.slice(0, 100));
  check('a weak password (400) keeps the link token and hides the token field',
    !weak.tokenShown && weak.second.includes(`"setup_token":"${LINK_TOKEN}"`), `token field ${weak.tokenShown}`);
  const refused = await setupCase(SERVER.tokenRefused);
  check('a refused token (403) shows the catalogue text and asks for a token by hand',
    refused.alert === TXT.tokenRefused && refused.tokenShown, `${refused.alert.slice(0, 60)} / token field ${refused.tokenShown}`);
  check('a refused token (403) forgets the link token',
    !refused.second.includes(LINK_TOKEN) && refused.second.includes(`"setup_token":"${MANUAL_TOKEN}"`), `second request ${refused.second.length} bytes`);
  const done = await setupCase(SERVER.completed, { retry: false });
  check('setup completed elsewhere (409) returns to the sign-in form', !done.tokenShown && done.heading === 'Sign in' && done.alert === '',
    `heading ${done.heading}`);
  // An incomplete setup link: the fragment is taken out of the address bar and the sign-in page says the link is incomplete.
  const shortCtx = await browser.newContext({ viewport: { width: 1366, height: 768 } });
  const shortPage = await shortCtx.newPage();
  await shortPage.goto(`http://127.0.0.1:${PORT}/#setup=abc`);
  await shortPage.waitForSelector('#login-form .alert', { timeout: 15000 }).catch(() => {});
  const shortHref = await shortPage.evaluate(() => location.href);
  check('an incomplete setup link: the fragment is removed from the address bar', !shortHref.includes('setup='), shortHref.replace(/^https?:\/\/[^/]*/, ''));
  const hint = ((await shortPage.textContent('#login-form .alert').catch(() => '')) || '').trim();
  check('an incomplete setup link: the sign-in page says the link is incomplete', hint === TXT.linkBroken, hint.slice(0, 100));
  await shortCtx.close();
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 } });
  const page = await ctx.newPage();
  const sent = [];
  page.on('request', (r) => sent.push(r.url() + ' ' + (r.postData() || '')));
  await page.goto(linkUrl);
  await page.waitForSelector('#login-form', { timeout: 15000 });
  await page.waitForFunction(() => !location.hash, null, { timeout: 5000 }).catch(() => {});
  check('the fragment is removed from the address bar', !(await page.evaluate(() => location.href)).includes('#'));
  check('the token field is hidden when the token came from the link', (await page.$('#login-token')) === null);
  // task 2.4: the server tells the client it runs in desktop mode, and the first-run screen says what is and is not protected
  const mode = await page.evaluate(async () => (await fetch('/api/health')).json());
  check('the server tells the client it runs in desktop mode', mode.mode === 'desktop', JSON.stringify(mode).slice(0, 160));
  const noticeEn = (await page.textContent('#desktop-notice').catch(() => '')) || '';
  check('first-run screen: the program does not encrypt the database or the whole data folder (English)',
    /does not encrypt its database or the whole data folder/.test(noticeEn) && /own encryption/.test(noticeEn) && /depends on this computer/.test(noticeEn)
    && /disk encryption/.test(noticeEn) && /individual account/.test(noticeEn) && /data folder only/.test(noticeEn) && /protected backups/.test(noticeEn)
    && /account by itself does not encrypt the database/.test(noticeEn), noticeEn.slice(0, 120));
  check('first-run screen: no wording that an account encrypts the database, no claim words',
    !/account (encrypts|protects) the database|\b(secure|approved|certified|validated|production-ready|hack-proof|error-free)\b/i.test(noticeEn), noticeEn.slice(0, 120));
  await page.click('#btn-lang'); // the topbar language toggle: EN to JA
  await page.waitForFunction(() => /暗号化/.test((document.querySelector('#desktop-notice') || {}).textContent || ''), null, { timeout: 5000 }).catch(() => {});
  const noticeJa = (await page.textContent('#desktop-notice').catch(() => '')) || '';
  check('first-run screen: the same notice in Japanese', /データベースもデータフォルダー全体も暗号化しません/.test(noticeJa) && /保護されたバックアップ/.test(noticeJa) && /アカウントだけでは、データベースは暗号化されません/.test(noticeJa), noticeJa.slice(0, 80));
  await page.click('#btn-lang'); // the topbar language toggle: JA to EN
  await page.waitForFunction(() => /Desktop mode/.test((document.querySelector('#desktop-notice') || {}).textContent || ''), null, { timeout: 5000 }).catch(() => {});
  await page.fill('#login-user', 'admin');
  await page.fill('#login-pass', 'correct-horse-9');
  check('first-run screen offers the optional automatic sign-in, off by default', (await page.isVisible('#login-auto').catch(() => false)) && !(await page.isChecked('#login-auto').catch(() => true)));
  await page.click('#login-form button[type="submit"]');
  // AUTH-1: the recovery code is shown once; the program opens after it is confirmed
  await page.waitForSelector('#rc-screen', { state: 'visible', timeout: 20000 }).catch(() => {});
  check('the recovery code is shown after setup', await page.isVisible('#rc-code').catch(() => false));
  await page.check('#rc-saved').catch(() => {});
  await page.click('#rc-continue').catch(() => {});
  await page.waitForFunction(() => !document.querySelector('#login-form') || document.querySelector('#login-form').closest('[hidden]'), null, { timeout: 20000 }).catch(() => {});
  const me = await page.evaluate(async () => (await fetch('/api/me', { headers: { 'X-NF-Request': '1' } })).json().catch(() => null));
  check('first administrator is created from the link', !!(me && (me.user || me.username || me.name || JSON.stringify(me).includes('admin'))), JSON.stringify(me).slice(0, 120));
  // the token travels only in the setup request body (as in server mode), never in a URL or the query string
  check('the token is never in a request URL', !sent.some((u) => u.split(' ')[0].includes(token)) && !sent.some((u) => u.includes('setup=')));
  const health = await page.evaluate(async () => (await fetch('/api/health')).json());
  check('setup is no longer required', health.setup_required === false);
  const again = await page.evaluate(async (t) => (await fetch('/api/setup', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-NF-Request': '1' }, body: JSON.stringify({ username: 'other', password: 'another-pass-1', setup_token: t }) })).status, token);
  check('the setup link no longer creates an account', again === 409, `HTTP ${again}`);
  await browser.close();

  if (!WIN) {
    run.child.kill('SIGINT');
    const code = await Promise.race([run.exited, sleep(20000).then(() => 'timeout')]);
    check('Ctrl+C stops the program with exit 0', code === 0, `exit ${code}`);
    check('the program reports that the database is closed', /database is closed/.test(run.text));
    const walPath = path.join(DATA_DIR, 'nexus-fabric.sqlite-wal');
    const wal = fs.existsSync(walPath) ? fs.statSync(walPath).size : 0;
    check('the write-ahead log was checkpointed at shutdown', wal === 0, `${wal} bytes`);
    check('the port is closed after shutdown', (await rawGet('/api/health', `127.0.0.1:${PORT}`)) === 0);
  } else {
    console.log('NOTE  Ctrl+C shutdown is not exercised by this script on Windows; stop the program with Ctrl+C by hand.');
    run.child.kill();
    await run.exited;
  }
  fs.rmSync(DATA_ROOT, { recursive: true, force: true });
  const failed = results.filter((r) => !r.ok);
  console.log('----');
  console.log(`passed: ${results.length - failed.length}  failed: ${failed.length}`);
  process.exit(failed.length ? 1 : 0);
})().catch((e) => {
  console.error('DESKTOP TEST FAILED', e.message);
  process.exit(1);
});
