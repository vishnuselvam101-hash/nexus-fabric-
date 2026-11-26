// Release verification: desktop-mode check of the SHIPPED Linux launcher (nexus-fabric.sh) in a browser.
// Derived from tests/desktop_smoke.js. Differences, all deliberate:
//  - the port must be in 8870-8879 (release verification uses only that range; desktop_smoke.js uses 8860-8869);
//  - the program is started through the launcher script (which runs `nexus-fabric --desktop`), not the binary;
//  - Linux only (the Windows branch is removed; Windows checks are run under wine by packaging/verify/windows_checks.sh).
// Checks: loopback address; owner-only data folder; foreign host name refused; first-run setup in the browser with
// the token only in the URL fragment; fragment removed from the address bar; setup link dead after setup;
// second copy on the same data folder refused; Ctrl+C stops the program with exit 0; WAL checkpointed; port closed.
// Usage: node tests/release_verify.js <path to nexus-fabric.sh> <port 8870-8879>
// Only 127.0.0.1 is used. Prints one PASS/FAIL line per check and "passed: N  failed: M". Exit 0 only if all pass.
// Development build. Not validated. Outputs must not be used for manufacturing.
const { spawn } = require('child_process');
const { chromium } = require('playwright');
const fs = require('fs');
const os = require('os');
const path = require('path');
const http = require('http');

const LAUNCHER = process.argv[2];
const PORT = Number(process.argv[3] || 0);
if (!LAUNCHER || !(PORT >= 8870 && PORT <= 8879)) {
  console.error('usage: node tests/release_verify.js <nexus-fabric.sh> <port 8870-8879>');
  process.exit(2);
}
if (process.platform === 'win32') {
  console.error('this check is for the Linux package only');
  process.exit(2);
}
const DATA_ROOT = fs.mkdtempSync(path.join(os.tmpdir(), 'nf-release-verify-'));
const ENV = { ...process.env, XDG_DATA_HOME: DATA_ROOT, HOME: DATA_ROOT };
const DATA_DIR = path.join(DATA_ROOT, 'nexus-fabric');
const results = [];
const check = (name, ok, detail = '') => {
  results.push({ name, ok });
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${detail ? `  (${detail})` : ''}`);
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// Runs the launcher, collecting its output; out.exited resolves to the exit code.
function start(extra) {
  const child = spawn(LAUNCHER, ['--port', String(PORT), '--no-browser', ...extra], { env: ENV });
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
// A raw request with a chosen Host header; resolves to the status code (0 if no connection).
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
  const mode = fs.statSync(DATA_DIR).mode & 0o777;
  check('data folder is owner-only (0700)', mode === 0o700, mode.toString(8));
  check('a foreign Host name is refused', [400, 403, 421].includes(await rawGet('/api/health', 'evil.example')));
  check('the loopback Host name is answered', (await rawGet('/api/health', `127.0.0.1:${PORT}`)) === 200);

  // Second copy on the same data folder: must refuse and say so.
  const second = start([]);
  const code2 = await Promise.race([second.exited, sleep(15000).then(() => 'timeout')]);
  check('second copy on the same data folder is refused', code2 === 2 && /already running/.test(second.text), `exit ${code2}`);

  // First-run setup through the browser, with the token only in the fragment.
  const browser = await chromium.launch();
  const ctx = await browser.newContext({ viewport: { width: 1366, height: 768 } });
  const page = await ctx.newPage();
  const sent = [];
  page.on('request', (r) => sent.push(r.url() + ' ' + (r.postData() || '')));
  await page.goto(linkUrl);
  await page.waitForSelector('#login-form', { timeout: 15000 });
  await page.waitForFunction(() => !location.hash, null, { timeout: 5000 }).catch(() => {});
  check('the fragment is removed from the address bar', !(await page.evaluate(() => location.href)).includes('#'));
  check('the token field is hidden when the token came from the link', (await page.$('#login-token')) === null);
  await page.fill('#login-user', 'admin');
  await page.fill('#login-pass', 'correct-horse-9');
  await page.click('#login-form button[type="submit"]');
  await page.waitForFunction(() => !document.querySelector('#login-form'), null, { timeout: 20000 }).catch(() => {});
  const me = await page.evaluate(async () => (await fetch('/api/me', { headers: { 'X-NF-Request': '1' } })).json().catch(() => null));
  check('first administrator is created from the link', !!(me && JSON.stringify(me).includes('admin')), JSON.stringify(me).slice(0, 120));
  // The token may appear in the setup request body (as in server mode), but never in a URL.
  check('the token is never in a request URL', !sent.some((u) => u.split(' ')[0].includes(token)) && !sent.some((u) => u.split(' ')[0].includes('setup=')));
  const health = await page.evaluate(async () => (await fetch('/api/health')).json());
  check('setup is no longer required', health.setup_required === false);
  const again = await page.evaluate(async (t) => (await fetch('/api/setup', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-NF-Request': '1' }, body: JSON.stringify({ username: 'other', password: 'another-pass-1', setup_token: t }) })).status, token);
  check('the setup link no longer creates an account', again === 409, `HTTP ${again}`);
  await browser.close();

  run.child.kill('SIGINT');
  const code = await Promise.race([run.exited, sleep(20000).then(() => 'timeout')]);
  check('Ctrl+C stops the program with exit 0', code === 0, `exit ${code}`);
  check('the program reports that the database is closed', /database is closed/.test(run.text));
  const walPath = path.join(DATA_DIR, 'nexus-fabric.sqlite-wal');
  const wal = fs.existsSync(walPath) ? fs.statSync(walPath).size : 0;
  check('the write-ahead log was checkpointed at shutdown', wal === 0, `${wal} bytes`);
  check('the port is closed after shutdown', (await rawGet('/api/health', `127.0.0.1:${PORT}`)) === 0);

  fs.rmSync(DATA_ROOT, { recursive: true, force: true });
  const failed = results.filter((r) => !r.ok);
  console.log('----');
  console.log(`passed: ${results.length - failed.length}  failed: ${failed.length}`);
  process.exit(failed.length ? 1 : 0);
})().catch((e) => {
  console.error('FAIL  release verification script error: ' + e.message);
  console.log('passed: 0  failed: 1');
  process.exit(1);
});
