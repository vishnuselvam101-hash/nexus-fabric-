import { api, esc } from '../api.js';
import { getState, setState } from '../store.js';
import { t, h, has, lang, engineJa } from '../i18n.js';
import { el, toast, chip, kv, busy } from '../ui.js';

const LOGIN_HASH = '#/login';
const MIN_PASSWORD = 10;
const SESSION_KEYS = { id: '#', username: 'th.username', role: 'th.role', signed_in: 'th.status' };

let root = null;
let alive = false;

/* ---- shared helpers ---- */

const roleText = (r) => (['admin', 'engineer', 'viewer'].includes(r) ? t(`role.${r}`) : String(r ?? ''));

// Computed numbers: 3 decimals with thousands separators, full precision on hover.
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
const num = (v) => el('span', { class: 'mono', title: String(v) }, n3(v));

// Text the server wrote: shown as it stands, never partly translated.
function srvNode(text) {
  const s = String(text ?? '');
  if (lang() !== 'ja') return el('span', {}, s);
  const ja = engineJa(s);
  if (ja) return el('span', { title: s }, ja);
  return el('span', {}, has('srv.tag') ? chip(t('srv.tag'), 'neutral') : null, ' ', el('span', { lang: 'en' }, s));
}

const errNode = (err) => el('div', { class: 'alert bad', role: 'alert' }, srvNode((err && err.message) || err));

const label = (key, fallback) => (has(key) ? t(key) : t(fallback));

function panel(title, body, extra) {
  return el(
    'section',
    { class: 'panel' },
    el('header', { class: 'panel-h' }, el('h2', {}, title), extra || null),
    el('div', { class: 'panel-b' }, body)
  );
}

function clearFieldError(input) {
  const msg = input.parentElement.querySelector(`[id="${input.id}-err"]`);
  if (msg) msg.remove();
  input.removeAttribute('aria-invalid');
  input.removeAttribute('aria-describedby');
}

function fieldError(input, message) {
  clearFieldError(input);
  const id = `${input.id}-err`;
  input.setAttribute('aria-invalid', 'true');
  input.setAttribute('aria-describedby', id);
  input.parentElement.append(el('span', { class: 'alert bad', id, role: 'alert' }, message));
  input.focus();
}

function pwField(id, name, text, autocomplete) {
  const input = el('input', { id, name, type: 'password', autocomplete, required: true });
  input.addEventListener('input', () => clearFieldError(input));
  return { input, wrap: el('div', { class: 'field' }, el('label', { for: id }, text), input) };
}

/* ---- session ---- */

function sessionPairs(src) {
  const pairs = [];
  for (const [k, v] of Object.entries(src || {})) {
    if (v === null || v === undefined || typeof v === 'object') continue;
    if (/token|secret|password/i.test(k)) continue;
    const key = SESSION_KEYS[k];
    let value;
    if (k === 'role') value = roleText(v);
    else if (k === 'username') value = el('bdi', {}, String(v));
    else if (typeof v === 'boolean') value = t(v ? 'yes' : 'no');
    else if (typeof v === 'number' && k !== 'id') value = num(v);
    else value = String(v);
    pairs.push({ label: k === 'id' ? '#' : key && has(key) ? t(key) : k, value, mono: k === 'id' || !key });
  }
  return pairs;
}

async function signOut(btn) {
  busy(btn, true);
  try { await api('/api/logout', { method: 'POST' }); } catch { /* signed out locally either way */ }
  busy(btn, false);
  if (!alive) return;
  setState({ user: null });
  location.hash = LOGIN_HASH;
}

function sessionPanel(session, err, user) {
  const src = session || { id: user.id, username: user.username, role: user.role };
  const out = el('button', { class: 'btn', type: 'button' }, t('shell.signout'));
  out.addEventListener('click', () => signOut(out));
  const body = [];
  if (err) body.push(errNode(err));
  if (session && session.signed_in === false) body.push(el('div', { class: 'alert warn', role: 'status' }, t('login.ended')));
  body.push(kv(sessionPairs(src)));
  body.push(el('div', { class: 'row' }, out));
  return panel(label('acc.session.h', 'nav.account'), body);
}

/* ---- password change ---- */

function passwordPanel() {
  const cur = pwField('pwc', 'c', t('acc.pw.cur'), 'current-password');
  const nw = pwField('pwn', 'n', t('acc.pw.new'), 'new-password');
  const rep = pwField('pwn2', 'n2', t('acc.pw.rep'), 'new-password');
  const msg = el('div', { id: 'pwmsg' });
  const go = el('button', { class: 'btn btn-primary', type: 'submit' }, t('acc.pw.go'));
  const form = el('form', { class: 'calc-form', novalidate: true }, cur.wrap, nw.wrap, rep.wrap, msg, go);

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    [cur.input, nw.input, rep.input].forEach(clearFieldError);
    msg.textContent = '';
    if (!cur.input.value) { fieldError(cur.input, t('acc.e.cur')); return; }
    if (nw.input.value.length < MIN_PASSWORD) { fieldError(nw.input, t('acc.e.len')); return; }
    if (nw.input.value !== rep.input.value) { fieldError(rep.input, t('acc.e.differ')); return; }
    const idle = go.textContent;
    busy(go, true);
    go.textContent = t('acc.pw.busy');
    try {
      await api('/api/password', { method: 'POST', json: { current: cur.input.value, new: nw.input.value } });
      if (!alive) return;
      toast(t('login.changed'), 'ok');
      setState({ user: null });
      location.hash = LOGIN_HASH;
    } catch (error) {
      if (!alive) return;
      msg.append(errNode(error));
    } finally {
      busy(go, false);
      go.textContent = idle;
    }
  });

  return panel(t('acc.pw.h'), [form, el('p', { class: 'sub' }, t('acc.pw.foot'))]);
}

/* ---- capabilities without a backend ---- */

function plannedPanel() {
  const items = [1, 2, 3].map((i) => `acc.planned.${i}`).filter(has).map((k) => el('li', {}, t(k)));
  if (!items.length) return null;
  return panel(
    label('acc.planned.t', 'nav.account'),
    [el('p', { class: 'sub' }, t('planned.sub')), el('ul', { class: 'il' }, items)],
    chip(t('planned.tag'), 'neutral')
  );
}

/* ---- mount / unmount ---- */

export async function mount(container, params) {
  unmount();
  alive = true;
  const user = (getState() || {}).user || {};
  const sessionHost = el('div', {}, el('p', { class: 'sub' }, t('loading')));
  root = el(
    'div',
    {},
    el('div', { class: 'panel-h' }, el('h1', {}, t('nav.account'))),
    el('p', { class: 'sub', html: h('acc.sub', { user: esc(user.username ?? ''), role: esc(roleText(user.role)) }) }),
    sessionHost,
    passwordPanel(),
    plannedPanel()
  );
  container.textContent = '';
  container.append(root);

  let session = null;
  let err = null;
  try { session = await api('/api/session'); } catch (e) { err = e; }
  if (!alive || !root) return;
  sessionHost.replaceChildren(sessionPanel(session, err, user));
}

export function unmount() {
  alive = false;
  if (root) root.remove();
  root = null;
}
