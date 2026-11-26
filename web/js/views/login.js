import { api } from '../api.js';
import { getState, setState } from '../store.js';
import { t } from '../i18n.js';

let form = null;
let root = null;
let onSubmit = null;

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

  if (setup) {
    // data-i18n would be applied again after mount and overwrite these texts: drop it in setup mode
    const heading = root.querySelector('#login-heading');
    heading.removeAttribute('data-i18n');
    heading.textContent = t('login.setup');
    // desktop mode: the setup link already carried the token, so no token field is asked for
    if (!getState().setupToken) {
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
      setupFields = [tokenLabel, tokenInput, tokenHelp];
    }
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
    btn.removeAttribute('data-i18n');
    btn.textContent = t('login.createAccount');
  }

  // the installer already ran (for example from another client, or before a network blip): show the normal sign-in
  const leaveSetup = () => {
    setup = false;
    setState({ setupRequired: false });
    if (!root) return;   // the view was left while the request was in flight
    for (const el of setupFields) el.remove();
    setupFields = [];
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
        try {
          await api('/api/setup', { json: linkToken ? { ...data, setup_token: linkToken } : data });
        } catch (err) {
          if (err.status === 409 && err.message === 'setup already completed') {
            leaveSetup();
            return;
          }
          throw err;
        }
        await api('/api/login', { json: { username: data.username, password: data.password } });
      } else {
        await api('/api/login', { json: data });
      }
      const user = await api('/api/me');
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
}

// server answers mapped to text: 401 wrong credentials, 429 throttled; anything else is a generic failure
function loginError(err) {
  if (err.status === 401) return t('login.bad');
  if (err.status === 429) return t('login.throttled');
  return t('login.failed');
}

// setup: the server's own 4xx answer says what to fix (a bad setup token, a weak password). It is shown as text and
// never includes the token, which the form does not echo back.
function setupError(err) {
  const client = err.status >= 400 && err.status < 500 && err.status !== 401 && err.status !== 429;
  return client && err.message ? String(err.message) : loginError(err);
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
  if (root) {
    root.remove();
    root = null;
  }
  form = null;
}
