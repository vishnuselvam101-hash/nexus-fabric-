import { api } from '../api.js';
import { getState, setState } from '../store.js';

let form = null;
let root = null;
let onSubmit = null;

export function mount(container) {
  const tpl = document.getElementById('t-login');
  root = tpl.content.cloneNode(true).firstElementChild;
  container.appendChild(root);

  form = root.querySelector('#login-form');
  const btn = form.querySelector('button[type="submit"]');
  const setup = getState().setupRequired;

  if (setup) {
    root.querySelector('#login-heading').textContent = 'Setup';
    const tokenLabel = document.createElement('label');
    tokenLabel.setAttribute('for', 'login-token');
    tokenLabel.textContent = 'Setup token';
    const tokenInput = document.createElement('input');
    tokenInput.id = 'login-token';
    tokenInput.name = 'setup_token';
    tokenInput.type = 'text';
    tokenInput.required = true;
    tokenInput.autocomplete = 'off';
    btn.before(tokenLabel, tokenInput);
    btn.textContent = 'Create account';
  }

  onSubmit = async (e) => {
    e.preventDefault();
    const data = Object.fromEntries(new FormData(form));
    btn.classList.add('is-busy');
    btn.disabled = true;
    clearError();

    try {
      const endpoint = setup ? '/api/setup' : '/api/login';
      await api(endpoint, { json: data });
      const user = await api('/api/me');
      setState({ user, view: 'app' });
      location.hash = '#/';
    } catch (err) {
      showError(err.message || 'Login failed');
    } finally {
      btn.classList.remove('is-busy');
      btn.disabled = false;
    }
  };
  form.addEventListener('submit', onSubmit);
}

function showError(msg) {
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
