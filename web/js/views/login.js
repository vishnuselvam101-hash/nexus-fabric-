import { api } from '../api.js';
import { getState, setState } from '../store.js';
import { t } from '../i18n.js';
import { el } from '../ui.js';
import { codeScreen, failText } from './recovery.js';

let form = null;
let root = null;
let onSubmit = null;
let extra = [];   // elements this view added next to the sign-in card (recovery form, code screen, password change)

export function mount(container) {
  const tpl = document.getElementById('t-login');
  root = tpl.content.cloneNode(true).firstElementChild;
  container.appendChild(root);

  form = root.querySelector('#login-form');
  const btn = form.querySelector('button[type="submit"]');
  let setup = getState().setupRequired;
  let setupFields = [];

  // the session was signed in and has ended: say so once, here
  if (getState().sessionEnded) {
    setState({ sessionEnded: false });
    showError(t('login.ended'));
  }
  // the setup link was incomplete or altered: say so once, here
  if (getState().setupLinkBroken) {
    setState({ setupLinkBroken: false });
    showError(t('login.linkBroken'));
  }

  if (setup) {
    // data-i18n would be applied again after mount and overwrite these texts: drop it in setup mode
    const heading = root.querySelector('#login-heading');
    heading.removeAttribute('data-i18n');
    heading.textContent = t('login.setup');
    // desktop mode: the setup link already carried the token, so no token field is asked for
    if (!getState().setupToken) setupFields = addTokenField(btn);
    // desktop mode: say what the program does and does not protect, before the first account is made
    if (getState().health?.mode === 'desktop') {
      const notice = document.createElement('div');
      notice.className = 'note desk';
      notice.id = 'desktop-notice';
      const body = document.createElement('div');
      const title = document.createElement('b');
      title.textContent = t('login.desktop.h');
      body.append(title);
      for (const key of ['login.desktop.p1', 'login.desktop.p2', 'login.desktop.p3']) {
        const p = document.createElement('p');
        p.textContent = t(key);
        body.append(p);
      }
      notice.append(body);
      form.before(notice);
      setupFields.push(notice);
    }
    // desktop mode: optional automatic sign-in of the person using this computer
    if (getState().health?.mode === 'desktop') setupFields.push(...addAutoField(btn));
    btn.removeAttribute('data-i18n');
    btn.textContent = t('login.createAccount');
  } else {
    addForgot(container);
  }

  // the installer already ran (for example from another client, or before a network blip): show the normal sign-in
  const leaveSetup = () => {
    setup = false;
    setState({ setupRequired: false });
    if (!root) return;   // the view was left while the request was in flight
    for (const node of setupFields) node.remove();
    setupFields = [];
    addForgot(container);
    const heading = root.querySelector('#login-heading');
    heading.setAttribute('data-i18n', 'login.title');
    heading.textContent = t('login.title');
    btn.setAttribute('data-i18n', 'login.submit');
    btn.textContent = t('login.submit');
  };

  onSubmit = async (e) => {
    e.preventDefault();
    const data = Object.fromEntries(new FormData(form));
    const linkToken = getState().setupToken;   // from the setup link's fragment; travels in the body only
    btn.classList.add('is-busy');
    btn.disabled = true;
    clearError();

    try {
      if (setup) {
        // the first administrator is created, then signed in with the same credentials
        let made = null;
        try {
          const auto = root.querySelector('#login-auto');
          const body = { ...data, ...(linkToken ? { setup_token: linkToken } : {}), ...(auto ? { auto_signin: auto.checked } : {}) };
          made = await api('/api/setup', { json: body });
        } catch (err) {
          if (err.status === 409 && err.message === 'setup already completed') {
            leaveSetup();
            return;
          }
          // only a refused token (403) is forgotten: the link token is asked for by hand on the next try.
          // A weak password or user name (400), a conflict (409) and a throttle (429) keep the link token:
          // desktop mode never shows the token again, so forgetting a valid one would lock the factory out.
          if (err.status === 403) {
            setState({ setupToken: null });
            if (root && !root.querySelector('#login-token')) setupFields.push(...addTokenField(btn));
          }
          throw err;
        }
        await api('/api/login', { json: { username: data.username, password: data.password } });
        if (made && made.recovery_code) {
          // the first account exists and is signed in; the code is shown once before the program opens
          setState({ pendingCode: { code: made.recovery_code, user: data.username, then: 'app' }, setupToken: null, setupRequired: false });
          showCode(container, getState().pendingCode);
          return;
        }
      } else {
        await api('/api/login', { json: data });
      }
      const user = await api('/api/me');
      if (user?.must_change_password) {
        setState({ user });
        showChange(container);
        return;
      }
      setState({ user, view: 'app', setupToken: null });
      location.hash = '#/';
    } catch (err) {
      showError(setup ? setupError(err) : loginError(err));
    } finally {
      btn.classList.remove('is-busy');
      btn.disabled = false;
    }
  };
  form.addEventListener('submit', onSubmit);

  // the recovery code is shown once and kept in memory only (a language switch must not lose it)
  const pending = getState().pendingCode;
  if (pending) showCode(container, pending);
  // an administrator reset the password: the new one is chosen before anything else
  else if (getState().user?.must_change_password) showChange(container);
}

// optional automatic sign-in (desktop mode only), off unless the person ticks it
function addAutoField(btn) {
  const box = document.createElement('input');
  box.id = 'login-auto';
  box.type = 'checkbox';
  box.setAttribute('aria-describedby', 'login-auto-help');
  const label = document.createElement('label');
  label.className = 'rc-saved';
  label.setAttribute('for', 'login-auto');
  label.append(box, ' ', t('login.auto'));
  const help = document.createElement('p');
  help.className = 'sub';
  help.id = 'login-auto-help';
  help.textContent = t('login.autoHelp');
  btn.before(label, help);
  return [label, help];
}

// "Forgot your password?" opens the recovery form
function addForgot(container) {
  if (!root || root.querySelector('#forgot-link')) return;
  const link = document.createElement('button');
  link.type = 'button';
  link.id = 'forgot-link';
  link.className = 'btn-text';
  link.textContent = t('login.forgot');
  link.addEventListener('click', () => showRecover(container));
  form.after(link);
  extra.push(link);
}

// the recovery form: account name, recovery code, new password
function showRecover(container) {
  root.hidden = true;
  const user = el('input', { id: 'rec-user', name: 'username', type: 'text', required: true, autocomplete: 'username', autocapitalize: 'none', spellcheck: false });
  const code = el('input', { id: 'rec-code', name: 'code', type: 'text', required: true, autocomplete: 'off', autocapitalize: 'characters', spellcheck: false, 'aria-describedby': 'rec-help' });
  const pw = el('input', { id: 'rec-new', name: 'new', type: 'password', required: true, autocomplete: 'new-password' });
  const pw2 = el('input', { id: 'rec-new2', name: 'new2', type: 'password', required: true, autocomplete: 'new-password' });
  const go = el('button', { id: 'rec-go', class: 'btn-primary', type: 'submit' }, t('rec.go'));
  const back = el('button', { id: 'rec-back', class: 'btn-text', type: 'button' }, t('rec.back'));
  const f = el('form', { id: 'recover-form', autocomplete: 'off', novalidate: true },
    el('label', { for: 'rec-user' }, t('login.user')), user,
    el('label', { for: 'rec-code' }, t('rec.code')), code,
    el('p', { class: 'sub', id: 'rec-help' }, t('rec.help')),
    el('label', { for: 'rec-new' }, t('rec.new')), pw,
    el('label', { for: 'rec-new2' }, t('rec.rep')), pw2,
    go);
  const card = el('section', { id: 'recover-card', class: 'card card-login', 'aria-labelledby': 'rec-heading' },
    el('h1', { id: 'rec-heading' }, t('rec.title')), f, back);
  const leave = (notice) => {
    card.remove();
    extra = extra.filter((n) => n !== card);
    root.hidden = false;
    if (notice) showNotice(notice);
    document.getElementById('forgot-link')?.focus();
  };
  back.addEventListener('click', () => leave());
  const err = (msg) => {
    f.querySelector('.alert')?.remove();
    f.prepend(el('div', { class: 'alert', role: 'alert' }, msg));
  };
  f.addEventListener('submit', async (e) => {
    e.preventDefault();
    f.querySelector('.alert')?.remove();
    if (pw.value.length < 10) { err(t('login.pw.short')); pw.focus(); return; }
    if (pw.value !== pw2.value) { err(t('rec.differ')); pw2.focus(); return; }
    go.classList.add('is-busy'); go.disabled = true;
    try {
      const res = await api('/api/recover', { json: { username: user.value.trim(), recovery_code: code.value.trim(), new_password: pw.value } });
      card.remove();
      extra = extra.filter((n) => n !== card);
      if (res && res.recovery_code) {
        setState({ pendingCode: { code: res.recovery_code, user: user.value.trim(), then: 'login', recovered: true } });
        showCode(container, getState().pendingCode);
      } else {
        leave('rec.done');
      }
    } catch (error) {
      err(recoverError(error));
    } finally {
      go.classList.remove('is-busy'); go.disabled = false;
    }
  });
  container.append(card);
  extra.push(card);
  user.focus();
}

function recoverError(error) {
  if (error.status === 429) return t('login.throttled');
  if (error.status === 400) {
    const key = SETUP_400[String(error.message).replace(/^new /, '')];
    if (key) return t(key);
  }
  return t(error.status ? 'rec.failed' : 'login.failed');
}

// a short note above the sign-in form
function showNotice(key) {
  if (!form) return;
  clearError();
  form.prepend(el('div', { class: 'alert ok', role: 'status' }, t(key)));
}

// the recovery code, shown once
function showCode(container, pending) {
  root.hidden = true;
  const screen = codeScreen({
    code: pending.code,
    user: pending.user,
    title: pending.recovered ? t('rc.new.title') : undefined,
    lead: pending.recovered ? t('rc.new.lead') : undefined,
    onDone: async () => {
      setState({ pendingCode: null });
      screen.remove();
      extra = extra.filter((n) => n !== screen);
      if (pending.then === 'app') {
        try {
          const user = await api('/api/me');
          setState({ user, view: 'app', setupToken: null });
          const same = location.hash === '#/';
          location.hash = '#/';
          if (same) window.dispatchEvent(new HashChangeEvent('hashchange'));
          return;
        } catch { /* fall through to the sign-in form */ }
      }
      root.hidden = false;
      if (pending.recovered) showNotice('rec.done');
      document.getElementById('login-user')?.focus();
    },
  });
  container.append(screen);
  extra.push(screen);
  screen.querySelector('#rc-code')?.focus();
}

// forced password change after an administrator reset
function showChange(container) {
  root.hidden = true;
  const cur = el('input', { id: 'chg-cur', type: 'password', required: true, autocomplete: 'current-password' });
  const pw = el('input', { id: 'chg-new', type: 'password', required: true, autocomplete: 'new-password' });
  const pw2 = el('input', { id: 'chg-new2', type: 'password', required: true, autocomplete: 'new-password' });
  const go = el('button', { id: 'chg-go', class: 'btn-primary', type: 'submit' }, t('chg.go'));
  const f = el('form', { id: 'change-form', novalidate: true },
    el('label', { for: 'chg-cur' }, t('chg.cur')), cur,
    el('label', { for: 'chg-new' }, t('acc.pw.new')), pw,
    el('label', { for: 'chg-new2' }, t('acc.pw.rep')), pw2,
    go);
  const card = el('section', { id: 'change-card', class: 'card card-login', 'aria-labelledby': 'chg-heading' },
    el('h1', { id: 'chg-heading' }, t('chg.title')), el('p', {}, t('chg.lead')), f);
  const err = (msg) => {
    f.querySelector('.alert')?.remove();
    f.prepend(el('div', { class: 'alert', role: 'alert' }, msg));
  };
  f.addEventListener('submit', async (e) => {
    e.preventDefault();
    f.querySelector('.alert')?.remove();
    if (!cur.value) { err(t('acc.e.cur')); cur.focus(); return; }
    if (pw.value.length < 10) { err(t('login.pw.short')); pw.focus(); return; }
    if (pw.value !== pw2.value) { err(t('acc.e.differ')); pw2.focus(); return; }
    go.classList.add('is-busy'); go.disabled = true;
    try {
      await api('/api/password', { json: { current: cur.value, new: pw.value } });
      // the server ended every session of this account: sign in again with the new password
      setState({ user: null });
      card.remove();
      extra = extra.filter((n) => n !== card);
      root.hidden = false;
      showNotice('login.changed');
      document.getElementById('login-user')?.focus();
    } catch (error) {
      err(error.status === 403 ? t('adm.e.wrongpw') : failText(error, 'login.failed'));
    } finally {
      go.classList.remove('is-busy'); go.disabled = false;
    }
  });
  container.append(card);
  extra.push(card);
  cur.focus();
}

// the one-time setup token typed in by hand (used when the setup link did not carry it); returns the added elements
function addTokenField(btn) {
  const tokenLabel = document.createElement('label');
  tokenLabel.setAttribute('for', 'login-token');
  tokenLabel.textContent = t('login.token');
  const tokenInput = document.createElement('input');
  tokenInput.id = 'login-token';
  tokenInput.name = 'setup_token';
  tokenInput.type = 'text';
  tokenInput.required = true;
  tokenInput.autocomplete = 'off';
  const tokenHelp = document.createElement('p');
  tokenHelp.className = 'sub';
  tokenHelp.id = 'login-token-help';
  tokenHelp.textContent = t('login.tokenHelp');
  tokenInput.setAttribute('aria-describedby', 'login-token-help');
  btn.before(tokenLabel, tokenInput, tokenHelp);
  return [tokenLabel, tokenInput, tokenHelp];
}

// server answers mapped to text: 401 wrong credentials, 429 throttled; anything else is a generic failure
function loginError(err) {
  if (err.status === 401) return t('login.bad');
  if (err.status === 429) return t('login.throttled');
  return t('login.failed');
}

// setup: each refusal the server gives is shown as a catalogue text in the user's language, never as the server's English.
// The 400 texts are the server's password and user name rules (main.rs password_policy and valid_username).
const SETUP_400 = {
  'password must have at least 10 characters': 'login.pw.short',
  'password is too long (at most 1024 bytes)': 'login.pw.long',
  'password must not be the user name': 'login.pw.user',
  'this password is on the list of commonly used passwords; choose another': 'login.pw.common',
  'username must be 3-64 characters: letters, digits, . _ -': 'login.user.rule',
};
function setupError(err) {
  if (err.status === 429) return t('login.throttled');
  if (err.status === 403) return t('login.tokenRefused');
  if (err.status === 400 && SETUP_400[err.message]) return t(SETUP_400[err.message]);
  return err.status ? t('login.setupFailed') : t('login.failed');
}

function showError(msg) {
  if (!form) return;   // the view was left while the request was in flight
  clearError();
  const el = document.createElement('div');
  el.className = 'alert';
  el.setAttribute('role', 'alert');
  el.textContent = msg;
  form.prepend(el);
}

function clearError() {
  const prev = form?.querySelector('.alert');
  if (prev) prev.remove();
}

export function unmount() {
  if (form && onSubmit) {
    form.removeEventListener('submit', onSubmit);
    onSubmit = null;
  }
  for (const node of extra) node.remove();
  extra = [];
  if (root) {
    root.remove();
    root = null;
  }
  form = null;
}
