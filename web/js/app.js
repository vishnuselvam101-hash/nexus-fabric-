/* NEXUS FABRIC — boot + router + chrome */
import { api, setAuthHandler } from './api.js';
import { getState, setState } from './store.js';
import { initialLang, setLang, applyPage, lang, t } from './i18n.js';
import { initFx, enterMotion } from './lib/motion.js';

const TITLES = { convert: 'Convert', login: 'Sign in', result: 'Result', workspace: 'Workspace', job: 'Job', math: 'New calculation', project: 'Project', account: 'Account', admin: 'Administration', manufacturing: 'Manufacturing', help: 'Help', verification: 'Verification', amoeba: 'AMOEBA' };
const THEMES = ['dark', 'light', ''];
const LANGS = ['en', 'ja'];
const QUICK_PROJECT = 'Quick conversions';

let currentView = null;
let currentRoute = null;
let renderToken = 0;

const $ = (sel, root = document) => root.querySelector(sel);
const byId = (id) => document.getElementById(id);
const errText = (e) => e?.message || (typeof e === 'string' ? e : 'Unexpected error');

/* ---------- toasts + error boundary ---------- */

export function showToast(msg, type = 'error') {
  const el = document.createElement('div');
  el.className = `toast toast-${type}`;
  el.setAttribute('role', type === 'error' ? 'alert' : 'status');
  el.textContent = String(msg ?? 'Unknown error');
  document.body.appendChild(el);
  setTimeout(() => { el.classList.add('out'); setTimeout(() => el.remove(), 300); }, 5000);
}

window.onerror = (message) => { showToast(message); return false; };
window.onunhandledrejection = (ev) => { showToast(errText(ev?.reason)); };

/* ---------- navigation ---------- */

export function navigate(hash) {
  const target = hash.startsWith('#') ? hash : `#${hash}`;
  if (location.hash === target) renderRoute();
  else location.hash = target;
}

function resolveRoute(hash) {
  const path = (hash || '').replace(/^#/, '') || '/';
  if (path === '/' || path === '') return { view: 'convert', mod: './views/convert.js', params: {} };
  if (path === '/login') return { view: 'login', mod: './views/login.js', params: {} };
  if (path === '/history') return { redirect: '#/workspace/history' };
  let m = path.match(/^\/r\/([A-Za-z0-9_-]+)\/?$/);
  if (m) return { view: 'result', mod: './views/result.js', params: { id: m[1] } };
  m = path.match(/^\/workspace\/([A-Za-z0-9_-]+)\/?$/);
  if (m && m[1] === 'amoeba') return { redirect: '#/amoeba' };   // AMOEBA lives at its own top-level page
  if (m) return { view: 'workspace', mod: './views/workspace.js', params: { tab: m[1] } };
  if (path === '/workspace') return { redirect: '#/workspace/history' };
  m = path.match(/^\/job\/(\d+)(?:\/([a-z]+))?\/?$/);
  if (m) return { view: 'job', mod: './views/job.js', params: { id: m[1], tab: m[2] } };
  m = path.match(/^\/math(?:\/(\d+))?(?:\/(\d+))?\/?$/);
  if (m) return { view: 'math', mod: './views/math.js', params: { projectId: m[1], fromId: m[2] } };
  m = path.match(/^\/project\/(\d+)\/?$/);
  if (m) return { view: 'project', mod: './views/project.js', params: { id: m[1] } };
  if (path === '/account') return { view: 'account', mod: './views/account.js', params: {} };
  if (path === '/help') return { view: 'help', mod: './views/help.js', params: {} };
  if (path === '/manufacturing') return { view: 'manufacturing', mod: './views/manufacturing.js', params: {} };
  if (path === '/verification') return { view: 'verification', mod: './views/verification.js', params: {} };
  m = path.match(/^\/amoeba(?:\/([a-z]+)(?:\/([^/]+))?)?\/?$/);   // the id is checked by the view
  if (m) return { view: 'amoeba', mod: './views/amoeba.js', params: { tab: m[1], id: m[2] } };
  m = path.match(/^\/admin(?:\/([a-z]+))?\/?$/);
  if (m) return { view: 'admin', mod: './views/admin.js', params: { tab: m[1] } };
  return { view: 'convert', mod: './views/convert.js', params: {} };
}

async function renderRoute() {
  const route = resolveRoute(location.hash);
  if (route.redirect) { navigate(route.redirect); return; }
  const token = ++renderToken;
  const viewEl = byId('view');
  if (!viewEl) return;

  if (currentView?.unmount) {
    try { await currentView.unmount(); } catch (e) { showToast(errText(e)); }
  }
  currentView = null;
  viewEl.innerHTML = '';

  try {
    const mod = await import(route.mod);
    if (token !== renderToken) return; // stale
    viewEl.setAttribute('tabindex', '-1');
    await mod.mount(viewEl, route.params || {});
    if (token !== renderToken) return;
    applyPage(viewEl);
    enterMotion(viewEl);
    currentView = { unmount: mod.unmount };
  } catch (e) {
    if (e?.status === 401) { navigate('#/login'); return; }
    viewEl.innerHTML = '<div class="error-box"><h2>Something went wrong</h2><p></p></div>';
    $('p', viewEl).textContent = errText(e);
    showToast(errText(e));
  }

  currentRoute = route;
  const suffix = route.params?.tab ? ` · ${cap(route.params.tab)}` : '';
  document.title = route.view === 'convert'
    ? 'NEXUS FABRIC'
    : `NEXUS FABRIC — ${TITLES[route.view] || ''}${suffix}`;
  setState({ view: route.view });
  // the top bar marks the page it leads to
  for (const a of document.querySelectorAll('.topbar-nav > a')) {
    const href = a.getAttribute('href') || '';
    if (location.hash === href || location.hash.startsWith(href + '/')) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current');
  }
  updateUserButton();
  closeMenus();
  viewEl.focus({ preventScroll: true });
}

const cap = (s) => s ? s.charAt(0).toUpperCase() + s.slice(1) : '';

function startRouter() {
  initFx();
  window.addEventListener('hashchange', renderRoute);
  renderRoute();
}

/* ---------- chrome ---------- */

function applyTheme(theme) {
  document.documentElement.dataset.theme = theme || '';
  try { localStorage.setItem('nf.theme', theme || ''); } catch { /* private mode */ }
}

function applyLang(l, remount = true) {
  setLang(l);
  try { localStorage.setItem('nf.lang', l); } catch { /* private mode */ }
  applyPage();
  const btn = byId('btn-lang');
  if (btn) btn.textContent = lang().toUpperCase();
  setState({ lang: l });
  if (remount && currentRoute) renderRoute();
}

function closeMenus() {
  const nav = byId('nav-links');
  const burger = byId('hamburger');
  if (nav) nav.classList.remove('open');
  if (burger) burger.setAttribute('aria-expanded', 'false');
  document.querySelectorAll('.dropdown.open').forEach((d) => {
    d.classList.remove('open');
    $('button', d)?.setAttribute('aria-expanded', 'false');
  });
  $('.user-menu')?.remove();
  byId('btn-user')?.setAttribute('aria-expanded', 'false');
}

function updateUserButton() {
  const btn = byId('btn-user');
  if (!btn) return;
  const user = getState().user;
  const label = user?.name || user?.username || '';
  btn.textContent = label ? label.trim().charAt(0).toUpperCase() : '';
  btn.title = label || 'Not signed in';
}

function toggleUserMenu() {
  const btn = byId('btn-user');
  const existing = $('.user-menu');
  if (existing) { existing.remove(); btn.setAttribute('aria-expanded', 'false'); return; }

  const menu = document.createElement('div');
  menu.className = 'user-menu';
  menu.setAttribute('role', 'menu');

  const user = getState().user;
  const who = document.createElement('span');
  who.className = 'user-menu-name';
  who.textContent = user?.name || user?.username || 'Guest';

  const link = (hash, key) => { const a = document.createElement('a'); a.href = hash; a.setAttribute('role', 'menuitem'); a.textContent = t(key); a.addEventListener('click', closeMenus); return a; };
  const acct = link('#/account', 'nav.account');
  const adm = user?.role === 'admin' ? link('#/admin', 'nav.admin') : null;

  const out = document.createElement('button');
  out.type = 'button';
  out.setAttribute('role', 'menuitem');
  out.textContent = t('common.signout');
  out.addEventListener('click', async () => {
    try { await api('/api/logout', { method: 'POST' }); } catch { /* ignore */ }
    setState({ user: null, projectId: null });
    closeMenus();
    navigate('#/login');
  });

  menu.append(...[who, acct, adm, out].filter(Boolean));
  btn.insertAdjacentElement('afterend', menu);
  btn.setAttribute('aria-expanded', 'true');
}

function initChrome() {
  /* theme */
  byId('btn-theme')?.addEventListener('click', () => {
    const cur = document.documentElement.dataset.theme || '';
    const next = THEMES[(THEMES.indexOf(cur) + 1) % THEMES.length];
    applyTheme(next);
  });

  /* language */
  byId('btn-lang')?.addEventListener('click', () => {
    const cur = document.documentElement.lang || 'en';
    const next = cur === 'en' ? 'ja' : 'en';
    applyLang(next);
  });

  /* hamburger */
  byId('hamburger')?.addEventListener('click', (ev) => {
    const nav = byId('nav-links');
    if (!nav) return;
    const open = nav.classList.toggle('open');
    ev.currentTarget.setAttribute('aria-expanded', String(open));
  });

  /* more dropdown */
  document.querySelectorAll('.dropdown > button').forEach((btn) => {
    btn.addEventListener('click', (ev) => {
      ev.stopPropagation();
      const dd = btn.parentElement;
      const open = dd.classList.toggle('open');
      btn.setAttribute('aria-expanded', String(open));
    });
  });

  /* user menu */
  byId('btn-user')?.addEventListener('click', (ev) => {
    ev.stopPropagation();
    toggleUserMenu();
  });

  /* close nav links on link click */
  $('.topbar')?.addEventListener('click', (ev) => {
    if (ev.target.closest('a[href]')) closeMenus();
  });

  /* close dropdowns on outside click */
  document.addEventListener('click', (ev) => {
    if (!ev.target.closest('.dropdown') && !ev.target.closest('.user-menu') && !ev.target.closest('#btn-user')) {
      document.querySelectorAll('.dropdown.open').forEach((d) => {
        d.classList.remove('open');
        $('button', d)?.setAttribute('aria-expanded', 'false');
      });
      $('.user-menu')?.remove();
      byId('btn-user')?.setAttribute('aria-expanded', 'false');
    }
  });

  /* escape closes menus */
  document.addEventListener('keydown', (ev) => { if (ev.key === 'Escape') closeMenus(); });

  updateUserButton();
}

/* ---------- boot ---------- */

async function ensureProject() {
  try {
    const res = await api('/api/projects');
    let list = res?.projects ?? res ?? [];
    if (!Array.isArray(list)) list = [];
    let quick = list.find((p) => p?.name === QUICK_PROJECT);
    if (!quick) {
      const created = await api('/api/projects', { json: { name: QUICK_PROJECT } });
      quick = created?.project ?? created;
    }
    setState({ projectId: quick?.id ?? quick?._id ?? null });
  } catch (e) {
    if (e?.status !== 401) showToast(`Projects: ${errText(e)}`);
  }
}

async function boot() {
  setAuthHandler(() => {
    // only a session that was signed in has ended; a first visit that is not signed in gets no message
    if (getState().user) setState({ user: null, sessionEnded: true });
    navigate('#/login');
  });

  /* restore saved preferences */
  applyTheme(localStorage.getItem('nf.theme') || '');
  const savedLang = initialLang(localStorage.getItem('nf.lang'));
  applyLang(savedLang, false);

  /* health check (non-blocking) */
  try { const health = await api('/api/health'); setState({ health, setupRequired: !!health?.setup_required }); } catch { /* offline ok */ }

  /* authenticate */
  let authed = false;
  try {
    const me = await api('/api/me');
    setState({ user: me?.user ?? me ?? null });
    authed = true;
  } catch (e) {
    setState({ user: null });
    if (e?.status === 401) { navigate('#/login'); }
  }

  /* project */
  if (authed) await ensureProject();

  /* show app */
  byId('view')?.querySelector('.loading')?.remove();
  document.body.classList.add('loaded');
  initChrome();
  startRouter();
}

if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', boot);
} else {
  boot();
}
