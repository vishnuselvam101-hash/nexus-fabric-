/* Recovery code screen and the administrator panels for recovery codes, automatic sign-in and password reset.
   Shared by the sign-in view (login.js) and the Administration view (admin.js). Every text comes from the catalogue. */
import { api } from '../api.js';
import { t } from '../i18n.js';
import { el, toast, busy } from '../ui.js';

// request field names of the server contract (AUTH-1): kept in one place
const F = {
  recoverCode: 'recovery_code',
  newPassword: 'new_password',
  adminPassword: 'password',
};

const MIN_PASSWORD = 10;

// the code as the server sent it: 20 characters are shown in four groups of five
export function groupCode(code) {
  const raw = String(code ?? '');
  if (/[-\s]/.test(raw)) return raw.trim();
  return raw.length === 20 ? raw.match(/.{5}/g).join('-') : raw;
}

// password-rule answers of the server, shown from the catalogue
const RULES = {
  'password must have at least 10 characters': 'login.pw.short',
  'password is too long (at most 1024 bytes)': 'login.pw.long',
  'password must not be the user name': 'login.pw.user',
  'this password is on the list of commonly used passwords; choose another': 'login.pw.common',
};
export function failText(err, fallback = 'adm.e.fail') {
  const s = err && err.status;
  if (s === 429) return t('login.throttled');
  if (s === 403 || s === 401) return t('adm.e.wrongpw');
  if (s === 400 && err.message) {
    const m = String(err.message).replace(/^new /, '');
    if (RULES[m]) return t(RULES[m]);
  }
  return t(s ? fallback : 'login.failed');
}

function printSheet(code, user) {
  const sheet = el(
    'div',
    { id: 'rc-print', class: 'rc-print' },
    el('h1', {}, t('rc.print.h')),
    user ? el('p', {}, t('rc.print.user', { user })) : null,
    el('p', {}, t('rc.print.date', { date: new Date().toISOString().slice(0, 10) })),
    el('p', { class: 'rc-code mono', translate: 'no' }, groupCode(code)),
    el('p', {}, t('rc.print.note'))
  );
  document.body.append(sheet);
  document.body.classList.add('rc-printing');
  const clean = () => {
    document.body.classList.remove('rc-printing');
    sheet.remove();
    window.removeEventListener('afterprint', clean);
  };
  window.addEventListener('afterprint', clean);
  try { window.print(); } catch { /* no printer dialog */ }
  // browsers that do not report afterprint: remove after the dialog returned
  setTimeout(() => { if (sheet.isConnected) clean(); }, 1500);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/* The code is shown once: Print / Copy, and a box that must be ticked before Continue is possible. */
export function codeScreen({ code, user, title, lead, onDone }) {
  const shown = groupCode(code);
  const codeBox = el('output', { id: 'rc-code', class: 'rc-code mono', translate: 'no', lang: 'en', tabindex: 0, 'aria-label': t('rc.codeLabel') }, shown);
  const copyBtn = el('button', { id: 'rc-copy', class: 'btn btn-secondary', type: 'button' }, t('rc.copy'));
  const printBtn = el('button', { id: 'rc-print-btn', class: 'btn btn-secondary', type: 'button' }, t('rc.print'));
  const status = el('p', { class: 'sub', id: 'rc-status', role: 'status' });
  const saved = el('input', { id: 'rc-saved', type: 'checkbox' });
  const go = el('button', { id: 'rc-continue', class: 'btn-primary', type: 'button', disabled: true }, t('rc.continue'));
  saved.addEventListener('change', () => { go.disabled = !saved.checked; });
  copyBtn.addEventListener('click', async () => {
    status.textContent = (await copyText(shown)) ? t('rc.copied') : t('rc.copyfail');
  });
  printBtn.addEventListener('click', () => printSheet(code, user));
  go.addEventListener('click', () => { if (saved.checked) onDone(); });
  const screen = el(
    'section',
    { id: 'rc-screen', class: 'card card-login rc-card', 'aria-labelledby': 'rc-heading' },
    el('h1', { id: 'rc-heading' }, title || t('rc.title')),
    el('p', {}, lead || t('rc.lead')),
    codeBox,
    el('div', { class: 'row rc-actions' }, copyBtn, printBtn),
    status,
    el('label', { class: 'rc-saved', for: 'rc-saved' }, saved, ' ', t('rc.saved')),
    go
  );
  return screen;
}

/* ---- administrator panels (Administration > Security) ---- */

const panel = (title, ...body) => el('section', { class: 'panel' }, el('header', { class: 'panel-h' }, el('h2', {}, title)), el('div', { class: 'panel-b' }, ...body));

function pwRow(id, text, autocomplete) {
  const input = el('input', { id, type: 'password', autocomplete, required: true });
  return { input, wrap: el('div', { class: 'field' }, el('label', { for: id }, text), input) };
}

function showErr(host, text) {
  host.replaceChildren(el('div', { class: 'alert bad', role: 'alert' }, text));
}

function recoveryPanel(onCode) {
  const pw = pwRow('rcn-pw', t('adm.rc.pw'), 'current-password');
  const msg = el('div', { id: 'rcn-msg' });
  const go = el('button', { id: 'rcn-go', class: 'btn btn-primary', type: 'submit' }, t('adm.rc.go'));
  const form = el('form', { id: 'rcn-form', class: 'calc-form', novalidate: true }, pw.wrap, msg, go);
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    msg.replaceChildren();
    if (!pw.input.value) { showErr(msg, t('acc.e.cur')); pw.input.focus(); return; }
    busy(go, true);
    try {
      const res = await api('/api/me/recovery-code', { json: { [F.adminPassword]: pw.input.value, current: pw.input.value } });
      pw.input.value = '';
      onCode(res && res.recovery_code);
    } catch (err) {
      showErr(msg, failText(err));
    } finally {
      busy(go, false);
    }
  });
  return panel(t('adm.rc.h'), el('p', { class: 'sub' }, t('adm.rc.p')), form);
}

function autoPanel(session) {
  const body = [];
  if (!session || session.mode !== 'desktop') {
    body.push(el('p', { class: 'sub', id: 'auto-server' }, t('adm.auto.server')));
    return panel(t('adm.auto.h'), ...body);
  }
  const on = !!session.auto_signin;
  const box = el('input', { id: 'auto-on', type: 'checkbox', checked: on });
  const pw = pwRow('auto-pw', t('adm.rc.pw'), 'current-password');
  const msg = el('div', { id: 'auto-msg' });
  const go = el('button', { id: 'auto-go', class: 'btn btn-primary', type: 'submit' }, t('adm.auto.save'));
  const form = el(
    'form', { id: 'auto-form', class: 'calc-form', novalidate: true },
    el('p', { class: 'sub' }, t('login.autoHelp')),
    el('label', { class: 'rc-saved', for: 'auto-on' }, box, ' ', t('login.auto')),
    pw.wrap, msg, go
  );
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    msg.replaceChildren();
    if (!pw.input.value) { showErr(msg, t('acc.e.cur')); pw.input.focus(); return; }
    busy(go, true);
    try {
      await api('/api/settings/auto-signin', { json: { enabled: box.checked, [F.adminPassword]: pw.input.value, current: pw.input.value } });
      pw.input.value = '';
      session.auto_signin = box.checked;
      toast(t(box.checked ? 'adm.auto.done.on' : 'adm.auto.done.off'), 'ok');
    } catch (err) {
      showErr(msg, failText(err));
    } finally {
      busy(go, false);
    }
  });
  return panel(t('adm.auto.h'), form);
}

/* Fills `host` with the panels. `showCode(code)` is called with a new recovery code (the caller shows it once). */
export async function securityPanels(host, showCode) {
  let session = null;
  try { session = await api('/api/session'); } catch { /* the panels explain what they can */ }
  host.replaceChildren(recoveryPanel(showCode), autoPanel(session));
}

/* ---- reset of another user's password (Administration > Users) ---- */

export function resetForm({ user, name, onDone, onCancel }) {
  const nw = pwRow('rst-new', t('adm.reset.new'), 'new-password');
  const adm = pwRow('rst-admin', t('adm.reset.admin'), 'current-password');
  const msg = el('div', { id: 'rst-msg' });
  const go = el('button', { id: 'rst-go', class: 'btn btn-primary', type: 'submit' }, t('adm.reset.go'));
  const cancel = el('button', { id: 'rst-cancel', class: 'btn btn-secondary', type: 'button' }, t('adm.reset.cancel'));
  cancel.addEventListener('click', onCancel);
  const form = el(
    'form', { id: 'rst-form', class: 'calc-form', novalidate: true },
    el('p', { class: 'sub' }, t('adm.reset.lead')),
    nw.wrap, adm.wrap, msg, el('div', { class: 'row' }, go, cancel)
  );
  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    msg.replaceChildren();
    if (nw.input.value.length < MIN_PASSWORD) { showErr(msg, t('login.pw.short')); nw.input.focus(); return; }
    if (!adm.input.value) { showErr(msg, t('acc.e.cur')); adm.input.focus(); return; }
    busy(go, true);
    try {
      await api(`/api/users/${user}/reset-password`, { json: { [F.newPassword]: nw.input.value, [F.adminPassword]: adm.input.value, current: adm.input.value } });
      onDone();
    } catch (err) {
      showErr(msg, failText(err));
      busy(go, false);
    }
  });
  return el('section', { id: 'rst-panel', class: 'panel' }, el('header', { class: 'panel-h' }, el('h2', {}, t('adm.reset.h', { user: name }))), el('div', { class: 'panel-b' }, form));
}
