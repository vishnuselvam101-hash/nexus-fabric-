// NEXUS FABRIC web client. No build step, no dependencies, no external resources (offline-capable, strict CSP).
// Every value shown comes from the API. Capabilities without a backend are labelled "Planned — not implemented";
// nothing is mocked, and there is no control anywhere in this client that approves or releases a result.
import { PatternViewer, f3 } from './viewer.js';

// ---------------------------------------------------------------- utilities
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (v) => String(v ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : esc(n));
const dtf = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' });
const when = (t) => (t ? dtf.format(new Date(t * 1000)) : '—');
const words = (s) => esc(String(s).replaceAll('_', ' '));
const short = (h) => `<span class="hash" title="${esc(h)}">${esc(String(h).slice(0, 12))}…</span>`;
const prefs = {
  get(k, d = null) { try { const v = localStorage.getItem(k); return v === null ? d : v; } catch { return d; } },
  set(k, v) { try { if (v === null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch { /* storage unavailable: preference lasts for this page only */ } },
};

// ---------------------------------------------------------------- icons (one hand-drawn set: 16 px grid, 1.5 px round stroke)
const ICONS = {
  dashboard: '<rect x="2" y="2" width="5" height="5" rx="1"/><rect x="9" y="2" width="5" height="3" rx="1"/><rect x="9" y="7" width="5" height="7" rx="1"/><rect x="2" y="9" width="5" height="5" rx="1"/>',
  folder: '<path d="M2 4.5a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1V12a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"/>',
  pattern: '<path d="M3 12.5 5.6 4.2a8 8 0 0 1 4.8 0L13 12.5a14 14 0 0 0-10 0z"/>',
  nodes: '<circle cx="4" cy="4" r="1.6"/><circle cx="12" cy="5" r="1.6"/><circle cx="8" cy="12" r="1.6"/><path d="M5.6 4.2 10.4 4.8M11.2 6.4 8.8 10.6M4.8 5.4 7.2 10.6"/>',
  shield: '<path d="M8 1.8 13 3.6v4.1c0 3-2 5.3-5 6.5-3-1.2-5-3.5-5-6.5V3.6z"/><path d="m5.8 8 1.6 1.6 2.9-3.1"/>',
  layers: '<path d="M8 2 14 5 8 8 2 5z"/><path d="m2 8 6 3 6-3M2 11l6 3 6-3"/>',
  sliders: '<path d="M2 4.5h7M12 4.5h2M2 11.5h2M7 11.5h7"/><circle cx="10.5" cy="4.5" r="1.5"/><circle cx="5.5" cy="11.5" r="1.5"/>',
  user: '<circle cx="8" cy="5.5" r="2.5"/><path d="M3 13.5c.6-2.4 2.6-3.5 5-3.5s4.4 1.1 5 3.5"/>',
  theme: '<circle cx="8" cy="8" r="5.5"/><path d="M8 2.5v11a5.5 5.5 0 0 0 0-11z" fill="currentColor"/>',
  help: '<circle cx="8" cy="8" r="6"/><path d="M6.3 6.2a1.8 1.8 0 1 1 2.6 1.7c-.6.3-.9.7-.9 1.3M8 11.4v.1"/>',
  bell: '<path d="M4 11V7.5a4 4 0 0 1 8 0V11l1 1.5H3z"/><path d="M6.8 14a1.3 1.3 0 0 0 2.4 0"/>',
  signout: '<path d="M6.5 2.5h-3a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h3M10.5 5l3 3-3 3M13.5 8h-7"/>',
  plus: '<path d="M8 3v10M3 8h10"/>', minus: '<path d="M3 8h10"/>',
  fit: '<path d="M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10"/>',
  ruler: '<path d="m2 11 9-9 3 3-9 9z"/><path d="m5 8 1.5 1.5M7 6l1.5 1.5M9 4l1.5 1.5"/>',
  grid: '<path d="M2.5 2.5h11v11h-11zM2.5 6.2h11M2.5 9.8h11M6.2 2.5v11M9.8 2.5v11"/>',
  dims: '<path d="M2 4v8M14 4v8M2 8h12M4.5 6l-2 2 2 2M11.5 6l2 2-2 2"/>',
  download: '<path d="M8 2v8M4.5 6.5 8 10l3.5-3.5M2.5 13.5h11"/>',
  upload: '<path d="M8 10.5v-8M4.5 6 8 2.5 11.5 6M2.5 13.5h11"/>',
  replay: '<path d="M13 8a5 5 0 1 1-1.6-3.7"/><path d="M13.5 2v3h-3"/>',
  close: '<path d="m3.5 3.5 9 9M12.5 3.5l-9 9"/>',
  check: '<path d="m3 8.5 3.2 3.2L13 4.5"/>',
  warn: '<path d="M8 2 14.5 13.5h-13z"/><path d="M8 6.5v3.2M8 11.6v.1"/>',
  info: '<circle cx="8" cy="8" r="6"/><path d="M8 7.2v4M8 4.8v.1"/>',
  rail: '<rect x="2" y="2.5" width="12" height="11" rx="1"/><path d="M6 2.5v11"/>',
  pointer: '<path d="M3.5 2.5 12 7 8.4 8.2 7 12z"/>',
  chevron: '<path d="m6 3.5 4.5 4.5L6 12.5"/>',
  search: '<circle cx="7" cy="7" r="4.2"/><path d="m10.2 10.2 3.3 3.3"/>',
  clock: '<circle cx="8" cy="8" r="6"/><path d="M8 4.5V8l2.4 1.6"/>',
  dash: '<path d="M4 8h8"/>',
  planned: '<circle cx="8" cy="8" r="5.5" stroke-dasharray="2.4 2.4"/>',
  mark: '<path d="M2 13 6 3h4l4 10"/><path d="M4.4 9.5a14 14 0 0 1 7.2 0"/>',
};
const ic = (name, size = 16) => `<svg class="ic" width="${size}" height="${size}" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name] || ''}</svg>`;

// ---------------------------------------------------------------- shared fragments
const tag = (kind, text, icon) => `<span class="tag t-${kind}">${icon ? ic(icon, 10) : ''}${esc(text)}</span>`;
const TAGS = { ok: 'check', warn: 'warn', bad: 'close', na: 'dash', planned: 'planned' };
const stateTag = (s) => (s === 'APPROVED' ? tag('ok', 'Approved', 'check') : s === 'REJECTED' ? tag('bad', 'Rejected', 'close') : s === 'SUPERSEDED' ? tag('na', 'Superseded', 'dash') : tag('warn', String(s).replaceAll('_', ' '), 'warn'));
const resultTag = (r) => { const k = r === 'PASS' ? 'ok' : r === 'FAIL' ? 'bad' : r === 'INCONCLUSIVE' ? 'warn' : 'na'; return tag(k, String(r).replaceAll('_', ' '), TAGS[k]); };
const oblTag = (s) => { const k = s === 'open' ? 'bad' : s === 'waived' ? 'warn' : 'ok'; return tag(k, s.replaceAll('_', ' '), s === 'open' ? 'warn' : TAGS[k]); };
const ph = (title, sub, actions = '') => `<div class="ph"><div><h1>${title}</h1>${sub ? `<p class="sub">${sub}</p>` : ''}</div><div class="ph-actions">${actions}</div></div>`;
const panel = (title, body, o = {}) => `<section class="panel"${o.id ? ` id="${o.id}"` : ''}><header class="panel-h"><h2>${title}</h2>${o.count !== undefined ? `<span class="cnt">${o.count}</span>` : ''}${o.tools ? `<div class="panel-tools">${o.tools}</div>` : ''}</header><div class="panel-b${o.flush ? ' flush' : ''}">${body}</div>${o.foot ? `<div class="panel-f">${o.foot}</div>` : ''}</section>`;
const empty = (icon, title, text, actions = '', cls = '') => `<div class="empty ${cls}">${ic(icon, 28)}<b>${esc(title)}</b><p>${text}</p>${actions ? `<div class="row">${actions}</div>` : ''}</div>`;
const planned = (title, items, lead = '') => `<aside class="planned" aria-label="Planned: ${esc(title)}"><header>${tag('planned', 'Planned', 'planned')}<b>${esc(title)}</b><span class="sub">not implemented in this build</span></header>${lead ? `<p>${lead}</p>` : ''}<ul>${items.map((i) => `<li>${esc(i)}</li>`).join('')}</ul></aside>`;
const note = (html, kind = '') => `<div class="note ${kind}">${ic(kind === 'info' ? 'info' : 'warn')}<div>${html}</div></div>`;
const filterBox = (target, label) => `<div class="filter">${ic('search', 14)}<label class="vh" for="flt-${target}">${esc(label)}</label><input id="flt-${target}" type="search" placeholder="${esc(label)}" data-filter="${target}" autocomplete="off"></div>`;
const tabsHtml = (id, label, items, active) => `<div class="tabs" role="tablist" aria-label="${esc(label)}" data-tabs="${id}">${items.map(([k, l, n, attn]) => `<button role="tab" id="tab-${id}-${k}" data-tab="${k}" aria-controls="tp-${id}-${k}" aria-selected="${k === active}" tabindex="${k === active ? 0 : -1}">${esc(l)}${n !== undefined && n !== null ? `<span class="cnt${attn ? ' attn' : ''}">${n}</span>` : ''}</button>`).join('')}</div>`;
const tabPanel = (id, k, active, html, cls = '') => `<div role="tabpanel" id="tp-${id}-${k}" aria-labelledby="tab-${id}-${k}" class="${cls}" tabindex="0"${k === active ? '' : ' hidden'}>${html}</div>`;
function wireTabs(root, id, onChange) {
  const list = $(`[data-tabs="${id}"]`, root); if (!list) return;
  const tabs = $$('[role=tab]', list);
  const show = (k, focus) => {
    tabs.forEach((t) => { const on = t.dataset.tab === k; t.setAttribute('aria-selected', on); t.tabIndex = on ? 0 : -1; const p = document.getElementById(t.getAttribute('aria-controls')); if (p) p.hidden = !on; if (on && focus) t.focus(); });
    if (onChange) onChange(k);
  };
  list.addEventListener('click', (e) => { const t = e.target.closest('[role=tab]'); if (t) show(t.dataset.tab); });
  list.addEventListener('keydown', (e) => {
    const i = tabs.indexOf(document.activeElement); if (i < 0) return;
    const j = e.key === 'ArrowRight' ? (i + 1) % tabs.length : e.key === 'ArrowLeft' ? (i - 1 + tabs.length) % tabs.length : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : -1;
    if (j >= 0) { e.preventDefault(); show(tabs[j].dataset.tab, true); }
  });
  return show;
}

// ---------------------------------------------------------------- geometry families (inputs the calculation API accepts)
const F = {
  cylinder_shell: { label: 'Cylindrical shell', f: [['r_dev', 'Development radius', 500, 'mm'], ['length', 'Length', 1000, 'mm']] },
  cone_frustum: { label: 'Concentric reducer (right cone frustum)', f: [['r1', 'Large-end radius', 200, 'mm'], ['r2', 'Small-end radius', 100, 'mm'], ['h', 'Axial height', 300, 'mm']] },
  eccentric_reducer: { label: 'Eccentric reducer (oblique cone frustum)', f: [['r1', 'Large-end radius', 200, 'mm'], ['r2', 'Small-end radius', 100, 'mm'], ['h', 'Axial height', 300, 'mm'], ['e', 'Offset of small end', 100, 'mm']] },
  header_with_branch: { label: 'Assembly: header shell with set-on branch (two parts, one joint)', f: [['header_radius', 'Header contact radius', 150, 'mm'], ['header_dev_radius', 'Header development radius', 150, 'mm'], ['header_length', 'Header shell length', 1200, 'mm'], ['branch_station', 'Branch position along header', 600, 'mm'], ['branch_radius', 'Branch contact radius', 100, 'mm'], ['branch_dev_radius', 'Branch development radius', 100, 'mm'], ['beta_deg', 'Angle between axes', 60, 'deg'], ['delta', 'Axis offset', 0, 'mm'], ['s_end', 'Branch square end, distance along branch axis', 400, 'mm']], sref: true },
  branch_stub: { label: 'Branch stub on cylindrical header', f: [['header_radius', 'Header contact radius', 150, 'mm'], ['branch_radius', 'Branch contact radius', 100, 'mm'], ['branch_dev_radius', 'Branch development radius', 100, 'mm'], ['beta_deg', 'Angle between axes', 60, 'deg'], ['delta', 'Axis offset', 0, 'mm'], ['s_end', 'Square end, distance along branch axis', 400, 'mm']] },
};
const FSHORT = { cylinder_shell: 'Cylindrical shell', cone_frustum: 'Concentric reducer', eccentric_reducer: 'Eccentric reducer', header_with_branch: 'Header with branch (assembly)', branch_stub: 'Branch stub' };
const famLabel = (k) => (F[k] ? F[k].label : words(k));
const famShort = (k) => FSHORT[k] || words(k);
const paramMeta = (fam, key) => (F[fam] ? F[fam].f.find((x) => x[0] === key) : null);

// ---------------------------------------------------------------- session state
let me = null;
let summary = null;          // last /api/dashboard result: status bar and navigation counts
let viewer = null;           // active PatternViewer, if the current view has one
let wsState = { id: null, part: 0, tab: 'gates' };
let ctxProject = null;       // project of the view on screen, used by "new calculation"
const notes = [];            // notifications of this browser session (there is no server notification service)
let unseen = 0, busy = 0, online = true;

async function api(path, opts = {}) {
  const init = { method: opts.method || 'GET', headers: { 'X-NF-Request': '1' }, credentials: 'same-origin' }; // anti-forgery header required by the server
  if (opts.json !== undefined) { init.headers['Content-Type'] = 'application/json'; init.body = JSON.stringify(opts.json); }
  if (opts.body !== undefined) { init.body = opts.body; Object.assign(init.headers, opts.headers || {}); }
  setBusy(1);
  let res;
  try { res = await fetch(path, init); } catch { setOnline(false); throw new Error('Cannot reach the server. Check that it is running, then retry.'); } finally { setBusy(-1); }
  setOnline(true);
  let data = null;
  try { data = await res.json(); } catch { /* non-JSON */ }
  if (res.status === 401 && path !== '/api/login') { const was = me; me = null; renderLogin(false, was ? 'Your session has ended. Sign in again.' : ''); throw new Error('Please sign in.'); }
  if (!res.ok) throw new Error((data && data.error) || `Request failed (${res.status})`);
  return data;
}
function setBusy(d) { busy = Math.max(0, busy + d); const p = $('#progress'); if (p) p.classList.toggle('on', busy > 0); }
function setOnline(v) { online = v; const l = $('#sb-conn'); if (l) l.innerHTML = `<span class="led${v ? '' : ' off'}"></span>${v ? 'Server connected' : 'Server unreachable'}`; }

// Notifications: transient toast plus a session log behind the bell. Never a blocking browser dialog.
function toast(msg, kind = 'info') {
  notes.unshift({ at: Date.now(), msg, kind }); if (notes.length > 50) notes.pop();
  unseen++; paintBell();
  const host = $('#toasts'); if (!host) return;
  const t = document.createElement('div'); t.className = `toast ${kind}`; t.setAttribute('role', kind === 'err' ? 'alert' : 'status');
  t.innerHTML = `<span>${esc(msg)}</span><button aria-label="Dismiss notification">${ic('close', 12)}</button>`;
  const drop = () => t.remove(); $('button', t).onclick = drop; setTimeout(drop, kind === 'err' ? 9000 : 5000);
  host.append(t); while (host.children.length > 4) host.firstChild.remove();
}
function paintBell() { const d = $('#belldot'); if (d) d.hidden = unseen === 0; const b = $('#bell'); if (b) b.setAttribute('aria-label', `Notifications${unseen ? `, ${unseen} new` : ''}`); }
function fieldError(input, msg) {
  input.setAttribute('aria-invalid', 'true'); let m = input.parentElement.querySelector('.field-err');
  if (!m) { m = document.createElement('div'); m.className = 'field-err'; m.setAttribute('role', 'alert'); input.after(m); }
  m.textContent = msg; input.focus();
  input.addEventListener('input', () => { input.removeAttribute('aria-invalid'); m.remove(); }, { once: true });
}
const formError = (sel, err) => { const m = $(sel); if (m) m.innerHTML = `<div class="note bad" role="alert">${ic('warn')}<div>${esc(err.message || err)}</div></div>`; };
async function withBusy(btn, label, fn) { const old = btn.innerHTML; btn.disabled = true; btn.classList.add('busy'); if (label) btn.textContent = label; try { return await fn(); } finally { if (btn.isConnected) { btn.disabled = false; btn.classList.remove('busy'); btn.innerHTML = old; } } }

function setTheme(t) { document.documentElement.dataset.theme = t === 'light' ? 'light' : 'dark'; prefs.set('nf-theme', document.documentElement.dataset.theme); }
setTheme(prefs.get('nf-theme', 'dark'));

// ---------------------------------------------------------------- shell
const NAV = [
  ['Overview', [['dashboard', 'Dashboard', 'dashboard', 'd'], ['projects', 'Projects & Documents', 'folder', 'p']]],
  ['Engineering', [['workspace', 'Engineering Workspace', 'pattern', 'w'], ['manufacturing', 'Manufacturing', 'layers', 'm']]],
  ['Assurance', [['amoeba', 'AMOEBA Control Centre', 'nodes', 'a'], ['verification', 'Verification & Quality', 'shield', 'v']]],
  ['System', [['admin', 'Administration', 'sliders', 's']]],
];
const NAV_KEYS = Object.fromEntries(NAV.flatMap(([, items]) => items.map((i) => [i[3], i[0]])));
const BUILD_NOTE = 'Development build — not validated — outputs must not be used for manufacturing';

function railMode() { if (window.innerWidth < 900) return 'closed'; return prefs.get('nf-rail') || (window.innerWidth < 1440 ? 'closed' : 'open'); }
function applyRail() { const a = $('.app'); if (a) { a.dataset.rail = railMode(); $('#railtoggle').setAttribute('aria-expanded', a.dataset.rail === 'open'); } }
function toggleRail() { prefs.set('nf-rail', railMode() === 'open' ? 'closed' : 'open'); applyRail(); }

function mountShell() {
  if ($('.app')) return;
  $('#app').innerHTML = `<a class="skip" href="#view">Skip to content</a>
  <div class="app" data-rail="open">
    <header class="topbar">
      <button class="ib" id="railtoggle" aria-label="Toggle navigation labels" aria-controls="rail" data-tip="Navigation  [" data-tip-pos="start">${ic('rail')}</button>
      <a class="brand" href="#/dashboard" aria-label="NEXUS FABRIC, dashboard">${ic('mark', 18)}NEXUS FABRIC</a>
      <nav class="crumbs" id="crumbs" aria-label="Breadcrumb"></nav>
      <div class="devnote" role="note">${ic('warn', 12)}<span>Development build — not validated<span class="long"> — outputs must not be used for manufacturing</span></span></div>
      <div class="tools">
        <span class="bellwrap"><button class="ib" id="bell" aria-label="Notifications" aria-expanded="false" data-tip="Notifications" data-tip-pos="end">${ic('bell')}</button><span class="dot" id="belldot" hidden></span></span>
        <button class="ib" id="helpbtn" aria-label="Keyboard shortcuts and help" data-tip="Shortcuts  ?" data-tip-pos="end">${ic('help')}</button>
        <button class="ib" id="theme" aria-label="Switch between dark and light theme" data-tip="Theme  T" data-tip-pos="end">${ic('theme')}</button>
        <a class="who" href="#/account" aria-label="Account: ${esc(me.username)}, role ${esc(me.role)}">${ic('user')}<span>${esc(me.username)}</span><small>${esc(me.role)}</small></a>
        <button class="ib" id="logout" aria-label="Sign out" data-tip="Sign out" data-tip-pos="end">${ic('signout')}</button>
      </div>
      <div id="progress" role="progressbar" aria-label="Request in progress"></div>
    </header>
    <nav class="rail" id="rail" aria-label="Main">${NAV.map(([group, items]) => {
      const vis = items.filter(([k]) => k !== 'admin' || me.role === 'admin');
      return vis.length ? `<h3>${group}</h3>${vis.map(([k, l, i]) => `<a href="#/${k}" data-nav="${k}" data-tip="${l}" data-tip-pos="right">${ic(i)}<span class="lbl">${l}</span>${k === 'verification' ? '<span class="cnt" id="nav-obl" hidden></span>' : ''}</a>`).join('')}` : '';
    }).join('')}</nav>
    <main id="view" tabindex="-1"></main>
    <footer class="statusbar">
      <span id="sb-conn"><span class="led"></span>Server connected</span>
      <span id="sb-engine" class="sb-opt"></span>
      <span class="sb-build">DEVELOPMENT BUILD · NOT VALIDATED · NOT FOR MANUFACTURING</span>
      <span class="sp"></span>
      <a id="sb-obl" href="#/verification" hidden></a>
      <button id="sb-help" class="sb-opt">Shortcuts <kbd>?</kbd></button>
    </footer>
  </div>
  <div id="toasts" aria-live="polite"></div>
  <div class="pop" id="notes" role="region" aria-label="Notifications" hidden></div>
  <dialog id="help" aria-labelledby="help-h"></dialog>`;
  applyRail();
  $('#railtoggle').onclick = toggleRail;
  $('#theme').onclick = () => setTheme(document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark');
  $('#logout').onclick = async () => { try { await api('/api/logout', { method: 'POST' }); } catch { /* signed out locally either way */ } me = null; renderLogin(false); };
  $('#helpbtn').onclick = $('#sb-help').onclick = openHelp;
  $('#bell').onclick = () => toggleNotes();
}

function toggleNotes(force) {
  const p = $('#notes'); if (!p) return; const open = force !== undefined ? force : p.hidden;
  if (open) {
    unseen = 0; paintBell();
    p.innerHTML = `<header><h2>Notifications</h2><button class="ib" id="notes-x" aria-label="Close notifications">${ic('close', 12)}</button></header>
      ${notes.length ? `<ul>${notes.map((n) => `<li>${tag(n.kind === 'err' ? 'bad' : n.kind === 'ok' ? 'ok' : n.kind === 'warn' ? 'warn' : 'info', n.kind === 'err' ? 'error' : n.kind === 'ok' ? 'done' : n.kind === 'warn' ? 'attention' : 'info')} ${esc(n.msg)}<time>${dtf.format(n.at)}</time></li>`).join('')}</ul>` : empty('bell', 'Nothing yet', 'Results of actions you take in this session appear here.')}
      <div class="panel-f">This browser session only. A server-side notification service and job queue are planned, not implemented: jobs run synchronously.</div>`;
    $('#notes-x').onclick = () => toggleNotes(false);
  }
  p.hidden = !open; $('#bell').setAttribute('aria-expanded', open);
}

function openHelp() {
  const d = $('#help'); if (!d || d.open) return;
  const k = (...keys) => keys.map((x) => `<kbd>${x}</kbd>`).join(' ');
  d.innerHTML = `<header><h2 id="help-h">Keyboard shortcuts</h2><button class="ib" id="help-x" aria-label="Close">${ic('close')}</button></header>
    <div class="dlg-b">
      <div><h3>Go to</h3><dl>${NAV.flatMap(([, items]) => items).filter(([key]) => key !== 'admin' || me.role === 'admin').map(([, l, , key]) => `<dt>${k('G', key.toUpperCase())}</dt><dd>${l}</dd>`).join('')}</dl></div>
      <div><h3>Anywhere</h3><dl><dt>${k('?')}</dt><dd>This overlay</dd><dt>${k('N')}</dt><dd>New calculation</dd><dt>${k('/')}</dt><dd>Focus the filter of the current list</dd><dt>${k('[')}</dt><dd>Show or hide navigation labels</dd><dt>${k('T')}</dt><dd>Dark or light theme</dd><dt>${k('Esc')}</dt><dd>Close overlay, leave a field</dd></dl></div>
      <div><h3>Pattern viewer</h3><dl><dt>${k('F')}</dt><dd>Fit pattern to view (also double-click)</dd><dt>${k('+')} ${k('−')}</dt><dd>Zoom in, out (wheel zooms at the cursor)</dd><dt>${k('←', '↑', '↓', '→')}</dt><dd>Pan when the viewer has focus (drag pans)</dd><dt>${k('M')}</dt><dd>Measure between two points; snaps to computed vertices</dd><dt>${k('1')}…${k('9')}</dt><dd>Switch part of an assembly</dd><dt>${k('Esc')}</dt><dd>Clear measurement, then selection</dd></dl></div>
      <div><h3>Panels</h3><dl><dt>${k('←', '→')}</dt><dd>On a tab row: previous, next tab</dd><dt>${k('←', '→')}</dt><dd>On a panel divider: resize the panel</dd><dt>${k('Tab')}</dt><dd>Curve table rows select the curve in the viewer</dd></dl></div>
    </div>
    <div class="dlg-f">${esc(BUILD_NOTE)}. Hover or focus any icon button to see its name and shortcut.</div>`;
  $('#help-x').onclick = () => d.close();
  d.showModal();
}

function paintStatus() {
  if (!summary) return;
  const e = $('#sb-engine'); if (e) e.textContent = `Engine ${summary.engine}`;
  const n = summary.open_obligations, s = $('#sb-obl'), c = $('#nav-obl');
  if (s) { s.hidden = !n; s.textContent = `${n} open scrutiny obligation${n === 1 ? '' : 's'}`; }
  if (c) { c.hidden = !n; c.textContent = n; c.setAttribute('aria-label', `${n} open obligations`); }
}
async function refreshSummary() { try { summary = await api('/api/dashboard'); paintStatus(); } catch { /* status bar keeps its last state */ } }

// ---------------------------------------------------------------- sign in
function renderLogin(setup, message, kind = 'bad') {
  disposeView(); document.title = 'Sign in — NEXUS FABRIC';
  $('#app').innerHTML = `<main class="login"><div class="login-card">
    <div class="brand">${ic('mark', 20)}NEXUS FABRIC</div>
    <h1>${setup ? 'Create the administrator' : 'Sign in'}</h1>
    <p class="sub">${setup ? 'First start of this installation. This account manages users and projects.' : 'Verified flat patterns for pipe and plate fabrication.'}</p>
    ${note('<b>Development build.</b> Not validated. No output may be used for manufacturing.')}
    ${message ? `<div class="note ${kind}" role="alert">${ic(kind === 'info' ? 'info' : 'warn')}<div>${esc(message)}</div></div>` : ''}
    <form id="f"><div class="field"><label for="lu">Username</label><input id="lu" name="u" autocomplete="username" required autofocus></div>
    <div class="field"><label for="lp">Password${setup ? ' (at least 10 characters)' : ''}</label><input id="lp" name="p" type="password" autocomplete="${setup ? 'new-password' : 'current-password'}" required${setup ? ' minlength="10"' : ''}></div>
    <button class="pri">${setup ? 'Create administrator' : 'Sign in'}</button></form>
    <p class="legal">Runs entirely on this server. No external network resources are loaded.</p></div></main>`;
  $('#f').onsubmit = async (e) => {
    e.preventDefault();
    const body = { username: e.target.u.value, password: e.target.p.value };
    try {
      await withBusy($('#f button'), setup ? 'Creating…' : 'Signing in…', async () => {
        if (setup) await api('/api/setup', { method: 'POST', json: body });
        await api('/api/login', { method: 'POST', json: body });
        me = await api('/api/me');
      });
      route();
    } catch (err) { renderLogin(setup && !/already/i.test(err.message), err.message); }
  };
}

// ---------------------------------------------------------------- tables shared by several views
const jobsTable = (rows, id, projects) => {
  const pname = (pid) => { const p = projects && projects.find((x) => x.id === pid); return p ? esc(p.name) : `Project ${pid}`; };
  return rows.length ? `<div class="tbl"><table id="${id}" class="jobs"><thead><tr><th>Job</th><th>Project</th><th>Geometry family</th><th>Release state</th><th>Replay audit</th><th>Entered</th></tr></thead><tbody>${rows.map((c) =>
    `<tr><td class="nw"><a href="#/calc/${c.id}">Job #${c.id}</a></td><td><a href="#/project/${c.project_id}">${pname(c.project_id)}</a></td><td>${famShort(c.family)}</td><td class="nw">${c.superseded ? stateTag('SUPERSEDED') : stateTag(c.state)}${c.open_obligations ? ` ${tag('bad', `${c.open_obligations} open obligation${c.open_obligations === 1 ? '' : 's'}`, 'warn')}` : ''}</td><td>${c.replayed ? tag('info', 'audited') : '<span class="sub">not run</span>'}</td><td class="nw">${esc(c.created_by)}<br><span class="sub">${when(c.created_at)}</span></td></tr>`).join('')}</tbody></table></div>`
    : empty('pattern', 'No engineering jobs yet', 'A job develops a flat pattern from entered parameters, verifies it independently and generates DXF.', me.role === 'viewer' ? '' : `<a class="btn pri" href="#/math">${ic('plus')}New calculation</a>`);
};
const lessonsTable = (lessons) => (lessons.length ? `<div class="tbl"><table><thead><tr><th>#</th><th>What went wrong</th><th>From job</th><th class="num">Matched since</th><th>Recorded</th></tr></thead><tbody>${lessons.map((l) => `<tr><td>${l.id}</td>
  <td>${l.kind === 'corrected_value' ? `${famShort(l.family)} · <b class="mono">${esc(l.parameter)}</b>: <span class="mono">${fmt(l.wrong_value)}</span> was wrong, <span class="mono">${fmt(l.right_value)}</span> is right<br><span class="sub">${esc(l.detail)}</span>` : `${words(l.kind)} · ${famShort(l.family)}`}</td>
  <td class="nw"><a href="#/calc/${l.source_calc}">Job #${l.source_calc}</a><br><span class="sub">Project ${l.project_id}</span></td><td class="num">${l.times_matched}</td><td class="nw">${esc(l.created_by)}<br><span class="sub">${when(l.created_at)}</span></td></tr>`).join('')}</tbody></table></div>`
  : empty('clock', 'No lessons recorded yet', 'A lesson is recorded automatically when an engineer corrects a value, when verification rejects a result, or when a stored result fails its integrity check.'));

// ---------------------------------------------------------------- dependency graph (layered, drawn from the real edge list)
function depGraph(edges) {
  const N = new Map();
  const node = (l) => { if (!N.has(l)) N.set(l, { l, out: [], inn: [], d: -1, i: N.size }); return N.get(l); };
  const E = edges.map((e) => { const a = node(e.from), b = node(e.to); a.out.push(b); b.inn.push(a); return { a, b, rel: e.rel }; });
  const depth = (n, seen) => { if (n.d >= 0) return n.d; if (seen.has(n)) return 0; seen.add(n); n.d = n.out.length ? 1 + Math.max(...n.out.map((o) => depth(o, seen))) : 0; return n.d; };
  N.forEach((n) => depth(n, new Set()));
  const cols = []; N.forEach((n) => { (cols[n.d] = cols[n.d] || []).push(n); });
  cols.forEach((col, d) => {
    if (d === 0) col.sort((a, b) => a.l.localeCompare(b.l));
    else col.sort((a, b) => { const m = (n) => n.out.reduce((s, o) => s + o.row, 0) / n.out.length; return m(a) - m(b) || a.l.localeCompare(b.l); });
    col.forEach((n, r) => { n.row = r; });
  });
  const NW = 150, NH = 34, CG = 46, RG = 8, P = 12, HD = 20;
  const rows = Math.max(...cols.map((c) => c.length)), W = P * 2 + cols.length * NW + (cols.length - 1) * CG, H = P * 2 + HD + rows * (NH + RG);
  N.forEach((n) => { n.x = P + n.d * (NW + CG); n.y = P + HD + n.row * (NH + RG) + (rows - cols[n.d].length) * (NH + RG) / 2; });
  const clip = (s, n) => (s.length > n ? s.slice(0, n - 1) + '…' : s);
  const html = `<div class="dg-wrap" tabindex="0" role="group" aria-label="Dependency graph, ${N.size} records and ${E.length} edges. Scrollable."><svg class="dg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="list">
    ${cols.map((c, d) => `<text class="colh" x="${P + d * (NW + CG)}" y="${P + 9}">${d === 0 ? 'Entered / declared' : `Derived · level ${d}`}</text>`).join('')}
    ${E.map((e, i) => { const x1 = e.b.x + NW, y1 = e.b.y + NH / 2, x2 = e.a.x, y2 = e.a.y + NH / 2, dx = Math.max(18, (x2 - x1) / 2); return `<path class="e" data-e="${i}" d="M${x1} ${y1}C${x1 + dx} ${y1} ${x2 - dx} ${y2} ${x2} ${y2}"><title>${esc(e.a.l)} —${esc(e.rel)}→ ${esc(e.b.l)}</title></path>`; }).join('')}
    ${[...N.values()].map((n) => { const [kind, name] = n.l.includes(' · ') ? n.l.split(' · ') : ['', n.l]; return `<g class="n" data-n="${n.i}" tabindex="0" role="listitem" aria-label="${esc(n.l)}: depends on ${n.out.length}, used by ${n.inn.length}"><title>${esc(n.l)}</title><rect x="${n.x}" y="${n.y}" width="${NW}" height="${NH}" rx="2"/>${kind ? `<text class="k" x="${n.x + 8}" y="${n.y + 13}">${esc(kind)}</text><text class="t" x="${n.x + 8}" y="${n.y + 27}">${esc(clip(name, 23))}</text>` : `<text class="t" x="${n.x + 8}" y="${n.y + 21}">${esc(clip(name, 23))}</text>`}</g>`; }).join('')}
  </svg></div><p class="dg-info" aria-live="polite">Select a record to trace what it depends on <span style="color:var(--warn)">(amber)</span> and what a change to it reaches <span style="color:var(--accent-text)">(blue)</span>. The flow reads left to right: entered values, then what is derived from them.</p>`;
  const wire = (root) => {
    const svg = $('.dg', root), info = $('.dg-info', root); if (!svg) return;
    const nodes = [...N.values()], idle = info.innerHTML;
    const closure = (n, key) => { const s = new Set(); const walk = (x) => x[key].forEach((y) => { if (!s.has(y)) { s.add(y); walk(y); } }); walk(n); return s; };
    const focus = (n) => {
      $$('.on,.up,.down,.self', svg).forEach((x) => x.classList.remove('on', 'up', 'down', 'self'));
      if (!n) { svg.classList.remove('focus'); info.innerHTML = idle; return; }
      const up = closure(n, 'out'), down = closure(n, 'inn');
      svg.classList.add('focus');
      $$('.n', svg).forEach((g) => { const m = nodes[g.dataset.n]; if (m === n) g.classList.add('on', 'self'); else if (up.has(m)) g.classList.add('on', 'up'); else if (down.has(m)) g.classList.add('on', 'down'); });
      $$('.e', svg).forEach((p) => { const e = E[p.dataset.e]; if ((e.a === n || up.has(e.a)) && up.has(e.b)) p.classList.add('on', 'up'); else if ((e.b === n || down.has(e.b)) && down.has(e.a)) p.classList.add('on', 'down'); });
      info.innerHTML = `<b>${esc(n.l)}</b> depends on ${up.size} record${up.size === 1 ? '' : 's'}. A change to it reaches ${down.size}: ${down.size ? [...down].map((x) => esc(x.l)).join(', ') : 'nothing downstream'}.`;
    };
    let pinned = null;
    svg.addEventListener('click', (e) => { const g = e.target.closest('.n'); pinned = g && nodes[g.dataset.n] !== pinned ? nodes[g.dataset.n] : null; focus(pinned); });
    svg.addEventListener('keydown', (e) => { const g = e.target.closest('.n'); if (g && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); pinned = nodes[g.dataset.n]; focus(pinned); } if (e.key === 'Escape' && pinned) { e.stopPropagation(); pinned = null; focus(null); } });
    svg.addEventListener('mouseover', (e) => { const g = e.target.closest('.n'); if (!pinned && g) focus(nodes[g.dataset.n]); });
    svg.addEventListener('mouseleave', () => { if (!pinned) focus(null); });
  };
  return { html, wire };
}
const edgeTable = (edges) => `<details class="sec" style="border:0"><summary>Edge list (${edges.length})</summary><div class="tbl"><table><thead><tr><th>Record</th><th>Relation</th><th>Depends on</th></tr></thead><tbody>${edges.map((e) => `<tr><td>${esc(e.from)}</td><td class="mono">${esc(e.rel)}</td><td>${esc(e.to)}</td></tr>`).join('')}</tbody></table></div></details>`;

// ---------------------------------------------------------------- views
// A view returns { title, crumbs, html, wire(root), ws } — ws = full-height workspace layout.
const views = {
  async dashboard() {
    const [d, obs, ps] = await Promise.all([api('/api/dashboard'), api('/api/obligations'), api('/api/projects')]);
    summary = d; paintStatus();
    const open = obs.filter((o) => o.state === 'open');
    const total = d.review_required + d.rejected + d.approved || 1;
    const stat = (n, label, extra = '', attn = false) => `<div class="stat${attn && n ? ' attn' : ''}"><span>${label}</span><b>${n}</b>${extra}</div>`;
    return {
      title: 'Dashboard', crumbs: [['Dashboard']],
      html: `<div class="page">${ph('Dashboard', `Engine ${esc(d.engine)} · running for ${Math.round(d.uptime_seconds / 60)} min`, me.role === 'viewer' ? '' : `<a class="btn pri" href="#/math">${ic('plus')}New calculation</a>`)}
        <div class="stats" role="group" aria-label="Totals">${stat(d.projects, 'Projects')}${stat(d.documents, 'Source documents')}${stat(d.calculations, 'Engineering jobs')}${stat(d.review_required, 'Review required', '', true)}${stat(d.rejected, 'Rejected')}${stat(d.open_obligations, 'Open obligations', '', true)}${stat(d.approved, 'Approved', '<small>approval not implemented</small>')}${stat(d.records, 'Immutable records')}</div>
        <div class="cols"><div>
          ${panel('Recent engineering jobs', jobsTable(d.recent, 'jobs-recent', ps), { flush: true, count: d.recent.length, tools: `<a href="#/workspace">All jobs</a>` })}
        </div><div>
          ${panel('Needs an engineer', open.length ? `<ul class="il">${open.map((o) => `<li><div class="l1"><span class="id">#${o.id}</span><b>${esc(o.kind)} · ${esc(o.label)}</b>${oblTag(o.state)}</div><p>${esc(o.required_check)}</p><p class="sub"><a href="#/calc/${o.target_calc}">Open job #${o.target_calc}</a> · raised ${when(o.created_at)}</p></li>`).join('')}</ul>` : empty('shield', 'No open scrutiny obligations', 'Obligations are raised when a correction reaches other records.'), { count: open.length })}
          ${panel('Release state of all jobs', `<div class="bar" role="img" aria-label="${d.review_required} review required, ${d.rejected} rejected, ${d.approved} approved"><i class="b-warn" style="flex:${d.review_required}"></i><i class="b-bad" style="flex:${d.rejected}"></i><i class="b-ok" style="flex:${d.approved}"></i></div>
            <div class="legend"><span><i style="background:var(--warn)"></i>${d.review_required} review required</span><span><i style="background:var(--bad)"></i>${d.rejected} rejected or superseded</span><span><i style="background:var(--ok)"></i>${d.approved} approved</span></div>`,
            { foot: `No job can reach APPROVED in this build: the engineer approval gate (G16) is not implemented. ${Math.round(100 * d.review_required / total)}% of jobs are held for review.` })}
        </div></div>
        ${planned('Processing queue and server notifications', ['Jobs run synchronously in the request; there is no queue to monitor.', 'No notification service: the bell lists only what happened in this browser session.'])}</div>`,
    };
  },

  async projects() {
    const ps = await api('/api/projects');
    return {
      title: 'Projects & Documents', crumbs: [['Projects']],
      html: `<div class="page">${ph('Projects & Documents', 'Projects, their source documents and immutable snapshots.')}
        ${panel('Projects', ps.length ? `<div class="tbl"><table id="tbl-projects"><thead><tr><th>#</th><th>Name</th><th class="num">Documents</th><th class="num">Jobs</th><th>Created</th></tr></thead><tbody>${ps.map((p) => `<tr><td>${p.id}</td><td><a href="#/project/${p.id}">${esc(p.name)}</a><br><span class="sub">${esc(p.description)}</span></td><td class="num">${p.documents}</td><td class="num">${p.calculations}</td><td class="nw">${esc(p.created_by)}<br><span class="sub">${when(p.created_at)}</span></td></tr>`).join('')}</tbody></table></div>`
          : empty('folder', 'No projects yet', me.role === 'viewer' ? 'An administrator has not added you to a project yet.' : 'Create the first project below. Jobs and documents belong to a project.'), { flush: true, count: ps.length, tools: ps.length ? filterBox('tbl-projects', 'Filter projects') : '' })}
        ${me.role === 'viewer' ? '' : panel('New project', `<form id="np"><div class="fgrid"><div class="field"><label for="npn">Name</label><input id="npn" name="n" required maxlength="200"></div><div class="field"><label for="npd">Description</label><input id="npd" name="d"></div></div><div id="npmsg"></div><button class="pri">Create project</button></form>`)}
        ${planned('Revisions and comparison', ['Drawing revision chains and differences between revisions', 'Comparison of two project snapshots'])}</div>`,
      wire() {
        const f = $('#np'); if (f) f.onsubmit = async (e) => { e.preventDefault(); try { const r = await withBusy($('button', f), 'Creating…', () => api('/api/projects', { method: 'POST', json: { name: f.n.value, description: f.d.value } })); toast(`Project #${r.id} created.`, 'ok'); location.hash = `#/project/${r.id}`; } catch (err) { formError('#npmsg', err); } };
      },
    };
  },

  async project(id) {
    const p = await api(`/api/projects/${id}`);
    ctxProject = p.id;
    const fname = (d) => { try { return decodeURIComponent(d.filename); } catch { return d.filename; } };
    return {
      title: p.name, nav: 'projects', crumbs: [['Projects', '#/projects'], [p.name]],
      html: `<div class="page">${ph(esc(p.name), `${esc(p.description)}${p.description ? ' · ' : ''}created by ${esc(p.created_by)} · ${when(p.created_at)}`, me.role === 'viewer' ? '' : `<a class="btn pri" href="#/math/${p.id}">${ic('plus')}New calculation</a>`)}
        ${panel('Engineering jobs', jobsTable(p.calculations, 'jobs-project', [p]), { flush: true, count: p.calculations.length })}
        <div class="cols"><div>
        ${panel('Source documents', `${p.documents.length ? `<div class="tbl"><table><thead><tr><th>File</th><th>Detected type</th><th class="num">Size (bytes)</th><th>SHA-256</th><th>Uploaded</th></tr></thead><tbody>${p.documents.map((d) => `<tr><td>${esc(fname(d))}${d.extension_mismatch ? ` ${tag('warn', 'extension ≠ content', 'warn')}` : ''}</td><td>${esc(d.detected_type)}</td><td class="num">${d.size}</td><td>${short(d.sha256)}</td><td class="nw">${esc(d.created_by)}<br><span class="sub">${when(d.created_at)}</span></td></tr>`).join('')}</tbody></table></div>` : empty('upload', 'No documents stored', 'Stored files are hashed and encrypted. They are kept as evidence; their content is not read.')}`,
          { flush: true, count: p.documents.length, foot: `${me.role === 'viewer' ? '' : `<form id="up" class="row"><label class="vh" for="upf">File to store</label><input id="upf" type="file" name="file" required style="max-width:340px"><button>${ic('upload')}Store file</button></form><div id="upmsg"></div>`}
            <p style="margin-top:6px">Accepted by content: PDF, PNG, JPEG, TIFF, DXF, STEP (max 50 MB). Files are hashed and stored encrypted. <b>No drawing content is read</b> — reading drawings is planned, not implemented.</p>
            <p>Current project snapshot: <span class="hash">${esc(p.snapshot || 'none')}</span></p>` })}
        </div><div>
        ${panel('Access', `<p>${p.members.length ? p.members.map((m) => tag('na', m)).join(' ') : '<span class="sub">No members.</span>'}</p><p class="hint" style="margin:6px 0 10px">Administrators see every project; other users only the projects they are members of.</p>
          ${me.role === 'admin' ? `<form id="mem"><div class="field"><label for="memu">Username</label><input id="memu" name="u" required></div><div id="memmsg"></div><div class="row"><button>Add member</button><button type="button" id="memrm" class="danger">Remove member</button></div></form>` : ''}`, { count: p.members.length })}
        </div></div></div>`,
      wire() {
        const mem = $('#mem');
        if (mem) { const send = async (remove) => { if (!mem.reportValidity()) return; try { await api(`/api/projects/${id}/members`, { method: 'POST', json: { username: mem.u.value, remove } }); toast(`${mem.u.value} ${remove ? 'removed from' : 'added to'} the project.`, 'ok'); refresh(); } catch (err) { formError('#memmsg', err); } }; mem.onsubmit = (e) => { e.preventDefault(); send(false); }; $('#memrm').onclick = () => send(true); }
        const up = $('#up'); if (up) up.onsubmit = async (e) => {
          e.preventDefault(); const file = up.file.files[0]; if (!file) return;
          try { const r = await withBusy($('button', up), 'Storing…', () => api(`/api/projects/${id}/documents`, { method: 'POST', body: file, headers: { 'X-Filename': encodeURIComponent(file.name).slice(0, 180) } })); toast(`Stored (${r.detected_type}). ${r.interpretation}`, 'ok'); refresh(); }
          catch (err) { formError('#upmsg', err); }
        };
      },
    };
  },

  // Job browser. The workspace itself is the job view (#/calc/:id) and the new-calculation view (#/math).
  async workspace() {
    const [cs, ps] = await Promise.all([api('/api/calculations'), api('/api/projects')]);
    return {
      title: 'Engineering Workspace', crumbs: [['Engineering Workspace']],
      html: `<div class="page">${ph('Engineering Workspace', 'Open a job to inspect its flat pattern, verification, gates and provenance — or compute a new one.', me.role === 'viewer' ? '' : `<a class="btn pri" href="#/math">${ic('plus')}New calculation</a>`)}
        ${panel('Engineering jobs', jobsTable(cs, 'jobs-all', ps), { flush: true, count: cs.length, tools: cs.length ? filterBox('jobs-all', 'Filter jobs') : '' })}
        ${planned('Drawing intelligence', ['PDF and image viewer with measurement against the source drawing', 'Page classification, OCR (Japanese / English), dimension and symbol recognition', 'View and projection identification, scale and unit interpretation', 'Geometry reconstruction from drawings; 2D/3D comparison with the source', 'Human confirmation workflow for readings'],
          'Until this exists, geometry parameters are entered manually and every result is marked as not linked to a source drawing (gate G01).')}</div>`,
    };
  },

  // New calculation, in the workspace layout: inputs left, (empty) canvas centre, modelling limits right.
  async math(projectId, fromId) {
    const ps = await api('/api/projects');
    const crumbs = [['Engineering Workspace', '#/workspace'], ['New calculation']];
    if (!ps.length) return { title: 'New calculation', nav: 'workspace', crumbs, html: `<div class="page">${ph('New calculation', '')}${empty('folder', 'Create a project first', 'A calculation is recorded in a project.', '<a class="btn pri" href="#/projects">Go to projects</a>')}</div>` };
    if (me.role === 'viewer') return { title: 'New calculation', nav: 'workspace', crumbs, html: `<div class="page">${ph('New calculation', '')}${empty('shield', 'Your role can view results but not run calculations', 'Ask an administrator for the engineer role.', '<a class="btn" href="#/workspace">Browse jobs</a>')}</div>` };
    let from = null;
    if (fromId) { try { from = (await api(`/api/calculations/${fromId}`)).request; } catch { from = null; } }
    const pid = projectId || ctxProject || ps[0].id;
    const fam0 = from && F[from.family] ? from.family : 'cylinder_shell';
    const fields = (k) => F[k].f.map(([n, l, v, u]) => `<div class="field"><label for="f_${n}">${l} <span class="sub">(${u})</span></label><input id="f_${n}" name="${n}" type="number" step="any" value="${from && from.family === k && typeof from[n] === 'number' ? from[n] : v}" required aria-describedby="f_${n}_k"><div class="hint mono" id="f_${n}_k">${n}</div></div>`).join('')
      + (F[k].sref ? `<div class="field"><label for="f_sref">Entered radii refer to which wall surface?</label><select id="f_sref" name="surface_reference">${['unknown', 'outside', 'inside', 'neutral'].map((o) => `<option value="${o}"${from && from.surface_reference === o ? ' selected' : ''}>${o === 'unknown' ? 'unknown — to be established' : o}</option>`).join('')}</select></div>` : '');
    return {
      title: 'New calculation', nav: 'workspace', ws: true, crumbs,
      html: `<div class="ws"><div class="ws-head"><h1>New calculation</h1><div class="ws-meta">Surface development with independent verification. Parameters are entered manually.</div></div>
        <div class="ws-banner">${ic('warn', 14)}<span><b>PROVISIONAL MODELS</b> — radii are used as reference-surface radii exactly as entered. Thickness, allowances, weld gap and kerf are not modelled.</span></div>
        <div class="ws-body">
          <aside class="pane" aria-label="Inputs"><div class="pane-scroll"><form id="calc" novalidate>
            <details class="sec" open><summary>Job</summary><div class="sec-b">
              <div class="field"><label for="f_project">Project</label><select id="f_project" name="project">${ps.map((p) => `<option value="${p.id}" ${String(p.id) === String(pid) ? 'selected' : ''}>#${p.id} ${esc(p.name)}</option>`).join('')}</select></div>
              <div class="field"><label for="f_fam">Geometry family</label><select id="f_fam" name="fam">${Object.entries(F).map(([k, v]) => `<option value="${k}"${k === fam0 ? ' selected' : ''}>${v.label}</option>`).join('')}</select></div></div></details>
            <details class="sec" open><summary>Parameters ${from ? tag('info', `from job #${esc(fromId)}`) : tag('warn', 'example values')}</summary><div class="sec-b"><div id="fields">${fields(fam0)}</div>
              <p class="hint">${from ? 'Copied from the earlier job. Check every value.' : 'Pre-filled numbers are example values, not recommendations.'}</p></div></details>
            <div class="sec-b" style="padding-top:12px"><div id="cmsg"></div><button class="pri" id="run" style="width:100%">Develop, verify and generate DXF</button></div>
          </form></div></aside>
          <div class="split" role="separator" aria-orientation="vertical" aria-label="Resize inputs panel" tabindex="0" data-split="l"></div>
          <section class="stage" aria-label="Pattern"><div class="pv-host">${empty('pattern', 'No pattern computed yet', 'Run the calculation to see the flat pattern here. The server develops the surface, checks it with an independent formulation, checks the profile topology, writes DXF and re-imports it, then evaluates the release gates. The job runs synchronously.')}</div></section>
          <div class="split" role="separator" aria-orientation="vertical" aria-label="Resize notes panel" tabindex="0" data-split="r"></div>
          <aside class="pane" aria-label="Modelling limits"><div class="pane-scroll">
            <details class="sec" open><summary>What the result will be</summary><div class="sec-b"><ul><li>A flat pattern per part with its curves, lengths and estimated numerical error.</li><li>Independent verification checks with primary and reference values.</li><li>Release gates. Every job stays in <b>REVIEW REQUIRED</b> or <b>REJECTED</b>: engineer approval is not implemented.</li><li>DXF files named <span class="mono">NOT-RELEASED</span>.</li></ul></div></details>
            <details class="sec" open><summary>Planned ${tag('planned', 'not implemented', 'planned')}</summary><div class="sec-b"><ul><li>Constraint solving from drawing dimensions</li><li>Assemblies other than one header with one branch; several branches; flanges and supports</li><li>Rectangular-to-round and irregular transitions; equal-diameter and partial branch intersections; oblique cylinders; near-cylindrical cones</li><li>Thickness / neutral-surface treatment, bend and roll allowances from calibrated process data</li><li>Uncertainty and tolerance analysis; interval-certified error bounds</li><li>Plate division and seam optimisation</li></ul></div></details>
          </div></aside>
        </div></div>`,
      wire(root) {
        wireSplits(root);
        const form = $('#calc');
        form.fam.onchange = () => { $('#fields').innerHTML = fields(form.fam.value); };
        form.onsubmit = async (e) => {
          e.preventDefault(); const fam = form.fam.value; const req = { family: fam };
          for (const [n, l] of F[fam].f) { const inp = form[n]; if (inp.value.trim() === '' || !Number.isFinite(Number(inp.value))) { fieldError(inp, `${l}: enter a number.`); return; } req[n] = Number(inp.value); }
          if (F[fam].sref) req.surface_reference = form.surface_reference.value;
          try {
            const r = await withBusy($('#run'), 'Computing and verifying…', () => api(`/api/projects/${form.project.value}/calculations`, { method: 'POST', json: req }));
            toast(`Job #${r.id} computed: ${String(r.state || 'see gates').replaceAll('_', ' ')}.`, 'ok'); location.hash = `#/calc/${r.id}`;
          } catch (err) { formError('#cmsg', err); }
        };
      },
    };
  },

  // The engineering workspace for one job.
  async calc(id) {
    const [c, dep, list] = await Promise.all([api(`/api/calculations/${id}`), api(`/api/calculations/${id}/dependencies`), api('/api/calculations')]);
    const meta = list.find((x) => x.id === c.id) || null;
    const canEdit = me.role !== 'viewer';
    const fam = c.request.family;
    ctxProject = c.project_id;
    if (wsState.id !== c.id) wsState = { id: c.id, part: 0, tab: 'gates' };
    if (wsState.part >= c.patterns.length) wsState.part = 0;
    const blocked = !!c.superseded_by || c.state === 'REJECTED';
    const state = c.superseded_by ? 'SUPERSEDED' : c.state;
    const rp = c.replay;
    const openObl = c.obligations.filter((o) => o.state === 'open').length, openAdv = c.advisories.filter((a) => a.state === 'open').length;
    const gcount = (r) => c.gates.filter((g) => g.result === r).length;
    const dirLabel = { downstream: 'Downstream — derived from it', lateral: 'Lateral — must still fit', common_cause: 'Common cause — same convention' };
    const graph = dep.edges.length ? depGraph(dep.edges) : null;

    // history, built only from timestamps and names the server recorded
    const ev = [];
    if (meta) ev.push({ t: meta.created_at, h: `Job #${c.id} computed`, d: `${famLabel(fam)}, entered by ${esc(c.created_by)}. ${c.closure_size} records in the release closure.${c.supersedes ? ` Corrects <a href="#/calc/${c.supersedes}">job #${c.supersedes}</a>.` : ''}` });
    if (c.obligations.length) ev.push({ t: Math.min(...c.obligations.map((o) => o.created_at)), k: 'warn', h: `${c.obligations.length} scrutiny obligations raised`, d: 'Created by following the dependency edges of the corrected job.' });
    c.obligations.filter((o) => o.resolved_by).forEach((o) => ev.push({ t: o.resolved_at, k: o.state === 'waived' ? 'warn' : 'ok', h: `Obligation #${o.id} ${words(o.state)}`, d: `${esc(o.label)} — ${esc(o.resolved_by)}: ${esc(o.note)}` }));
    c.advisories.filter((a) => a.resolved_by).forEach((a) => ev.push({ t: a.resolved_at, k: 'ok', h: `Advisory #${a.id} acknowledged`, d: `${esc(a.resolved_by)}: ${esc(a.note)}` }));
    if (rp) ev.push({ t: rp.at, k: rp.matches ? 'ok' : 'bad', h: `Replay audit ${rp.matches ? 'reproduced the closure' : 'DID NOT reproduce the closure'}`, d: `Run by ${esc(rp.run_by)}; ${rp.stored_records_checked} stored records checked, ${rp.stored_records_failing_identity_check} failing.` });
    ev.sort((a, b) => a.t - b.t);
    if (c.superseded_by) ev.push({ k: 'bad', h: 'Superseded', d: `Replaced by <a href="#/calc/${c.superseded_by}">job #${c.superseded_by}</a> after a correction. Files withheld.` });

    const tabs = [['gates', 'Gates'], ['checks', 'Checks'], ['scrutiny', 'Scrutiny', openObl + openAdv || c.obligations.length + c.advisories.length, openObl + openAdv > 0], ['graph', 'Graph'], ['history', 'History'], ['records', 'Records', c.records.length]];
    if (!tabs.some((t) => t[0] === wsState.tab)) wsState.tab = 'gates';

    const gatesHtml = `<div class="summary-line"><span>${tag('ok', `${gcount('PASS')} pass`, 'check')}</span><span>${tag('warn', `${gcount('INCONCLUSIVE')} inconclusive`, 'warn')}</span>${gcount('FAIL') ? `<span>${tag('bad', `${gcount('FAIL')} fail`, 'close')}</span>` : ''}<span>${tag('na', `${gcount('NOT_IMPLEMENTED')} not implemented`, 'dash')}</span></div>
      <ul class="il">${c.gates.map((g) => `<li><div class="l1"><span class="id">${esc(g.id)}</span><b>${words(g.name)}</b>${resultTag(g.result)}</div><p>${esc(g.detail)}</p></li>`).join('')}</ul>
      <p class="hint">A job is released only when every critical gate passes. G16 (engineer approval) has no implementation, so no job can be released by this build.</p>`;
    const checksHtml = c.checks.length ? `<ul class="il">${c.checks.map((k) => `<li><div class="l1"><span class="id">${esc(k.scope)}</span><b>${words(k.name)}</b>${resultTag(k.verdict)}</div><p class="sub">${esc(k.method)} · independence class ${esc(k.independence)}</p>
        <div class="mini"><span><i>Primary</i>${fmt(k.primary)}</span><span><i>Reference</i>${fmt(k.reference)}</span><span><i>Difference</i>${fmt(k.difference)}</span><span><i>Tolerance</i>${fmt(k.tolerance)}</span></div></li>`).join('')}</ul>`
      : empty('shield', 'No verification checks', 'No pattern was produced, so there was nothing to verify.');
    const scrutinyHtml = (c.advisories.length ? `<h3>Known-mistake check</h3><p class="hint">This job was compared with the lessons recorded in this project. A match never changes a value; it asks an engineer to look.</p>
        <ul class="il">${c.advisories.map((a) => `<li><div class="l1"><span class="id">A${a.id}</span><b>Advisory</b>${tag(a.state === 'open' ? 'bad' : 'ok', a.state, a.state === 'open' ? 'warn' : 'check')}</div><p>${esc(a.message)}</p>${a.note ? `<p class="sub">${esc(a.resolved_by)} · ${when(a.resolved_at)}: ${esc(a.note)}</p>` : ''}
          ${a.state === 'open' && canEdit ? `<div class="act"><label class="vh" for="adv${a.id}">Reason for advisory ${a.id}</label><input id="adv${a.id}" data-advreason="${a.id}" placeholder="Why is it right this time?"><div class="row"><button data-ack="${a.id}">Acknowledge with reason</button></div></div>` : ''}</li>`).join('')}</ul>` : '')
      + (c.obligations.length ? `<h3 style="margin-top:${c.advisories.length ? 16 : 0}px">Scrutiny obligations from the correction</h3><p class="hint">Rule table: ${esc(dep.rule_table)}. Open obligations cannot be closed by time or budget — only by evidence or an engineer's recorded decision.</p>
        <ul class="il" id="obls">${c.obligations.slice().sort((a, b) => (a.state === 'open' ? 0 : 1) - (b.state === 'open' ? 0 : 1) || a.id - b.id).map((o) => `<li data-obl="${o.id}"><div class="l1"><span class="id">#${o.id}</span><b>${esc(o.kind)} · ${esc(o.label)}</b>${oblTag(o.state)}</div>
          <p>${esc(o.required_check)}</p><p class="sub">${esc(dirLabel[o.direction] || o.direction)} · via <span class="mono">${esc(o.via)}</span></p>
          ${o.state !== 'open' ? `<p class="sub">${esc(o.note)}${o.resolved_by ? ` — ${esc(o.resolved_by)}, ${when(o.resolved_at)}` : ''}</p>` : ''}
          ${o.state === 'open' && canEdit ? `<div class="act"><label class="vh" for="reason${o.id}">Reason for obligation ${o.id}</label><input id="reason${o.id}" data-reason="${o.id}" placeholder="What did you check?"><div class="row"><button data-resolve="${o.id}" data-decision="confirmed">Confirm checked</button><button class="ghost" data-resolve="${o.id}" data-decision="waived">Waive</button></div></div>` : ''}</li>`).join('')}</ul>` : '')
      || empty('shield', 'Nothing under scrutiny', 'No advisory matched a recorded lesson, and no correction has raised obligations for this job.');
    const graphHtml = graph ? `${graph.html}<p class="hint"><a href="#/amoeba/dependencies/${c.id}">Open at full width in AMOEBA</a></p>${edgeTable(dep.edges)}` : empty('nodes', 'No dependency edges', 'Edges exist when a pattern was produced.');
    const historyHtml = `<ol class="tl">${ev.map((e) => `<li class="k-${e.k || ''}"><b>${e.h}</b><time>${e.t ? when(e.t) : ''}</time>${e.d}</li>`).join('')}</ol><p class="hint">Built from the timestamps stored with this job. Administrators see the full hash-chained audit log under Administration.</p>`;
    const recordsHtml = `<dl class="kv"><dt>Release closure</dt><dd class="txt"><span class="hash">${esc(c.closure_hash)}</span></dd><dt>Records in closure</dt><dd>${c.closure_size}</dd><dt>Declared work units</dt><dd>${c.work_units}</dd><dt>Engine</dt><dd class="txt">${esc(c.engine)}</dd>
        <dt>Replay audit</dt><dd class="txt">${rp ? `${resultTag(rp.matches ? 'PASS' : 'FAIL')} ${when(rp.at)} · ${esc(rp.run_by)}` : 'not run'}</dd></dl>
      ${rp ? `<p class="hint">${rp.stored_records_checked} stored records checked, ${rp.stored_records_failing_identity_check} failing. ${esc(rp.limitation)}</p>` : '<p class="hint">A replay audit recomputes the job in a separate process and compares the closure hash.</p>'}
      <h3 style="margin:14px 0 4px">Records (${c.records.length})</h3>
      ${c.records.map((r, i) => `<details class="sec rec" data-rec="${i}"><summary style="text-transform:none;letter-spacing:0;font-weight:500"><span>${esc(r.family)} · <b>${esc(r.kind)}</b></span><span class="hash" style="margin-left:auto">${esc(r.id.slice(0, 10))}…</span></summary><div class="sec-b"></div></details>`).join('')}`;

    const html = `<div class="ws">
      <div class="ws-head"><h1>Job #${c.id} ${stateTag(state)}</h1>
        <div class="ws-meta">${famLabel(fam)} · <a href="#/project/${c.project_id}">Project ${c.project_id}</a> · entered by ${esc(c.created_by)}${meta ? ` · ${when(meta.created_at)}` : ''}${c.supersedes ? ` · corrects <a href="#/calc/${c.supersedes}">job #${c.supersedes}</a>` : ''}</div>
        <div class="ph-actions">${canEdit ? `<button id="replay" data-tip="Recompute in a separate process and compare the closure hash" data-tip-pos="end">${ic('replay')}Run replay audit</button><a class="btn" href="#/math/${c.project_id}/${c.id}" data-tip="New calculation starting from these values" data-tip-pos="end">${ic('plus')}New from these values</a>` : ''}</div></div>
      ${c.superseded_by ? `<div class="ws-banner bad" role="status">${ic('warn', 14)}<span><b>SUPERSEDED by <a href="#/calc/${c.superseded_by}">job #${c.superseded_by}</a></b> after a correction. This result must not be used; its files are withheld.</span></div>`
        : c.state === 'REJECTED' ? `<div class="ws-banner bad" role="status">${ic('warn', 14)}<span><b>REJECTED</b> — a critical gate failed. No output of this job may be used.</span></div>`
        : `<div class="ws-banner" role="status">${ic('warn', 14)}<span><b>REVIEW REQUIRED</b> — this result cannot be released for manufacturing. The Gates tab gives every reason.</span></div>`}
      <div class="ws-body">
        <aside class="pane" aria-label="Inputs and properties"><div class="pane-scroll">
          <details class="sec" open><summary>Request ${tag('warn', 'entered manually')}</summary><div class="sec-b"><dl class="kv">${Object.entries(c.request).filter(([k]) => k !== 'family').map(([k, v]) => { const m = paramMeta(fam, k); return `<dt>${m ? `${esc(m[1])}<small>${esc(k)}</small>` : words(k)}</dt><dd${typeof v === 'number' ? '' : ' class="txt"'}>${k === 'family' ? famLabel(v) : fmt(v)}${m && typeof v === 'number' ? ` <span class="sub">${m[3]}</span>` : ''}</dd>`; }).join('')}</dl>
            <p class="hint">Not linked to a source drawing: drawing interpretation is planned, not implemented.</p></div></details>
          ${c.patterns.length ? `<details class="sec" open><summary>Selection</summary><div class="sec-b" id="sel" aria-live="polite"></div></details>
          <details class="sec" open><summary>Curves <span class="sub" id="curves-part"></span></summary><div class="sec-b flush" id="curves"></div></details>
          <details class="sec" open><summary>Pattern quantities</summary><div class="sec-b" id="quant"></div></details>
          <details class="sec"><summary>Assumptions</summary><div class="sec-b" id="assume"></div></details>` : ''}
          ${c.correctable_parameters.length && !c.superseded_by && canEdit ? `<details class="sec" open><summary>Record a correction</summary><div class="sec-b">
            <p class="hint" style="margin-bottom:8px">A correction never edits this job. It creates a new job, marks this one superseded, and raises an obligation for every record that depends on the corrected value, mates with something affected, or was entered under the same convention.</p>
            <form id="corr" novalidate><div class="field"><label for="corrp">Parameter</label><select id="corrp" name="p">${c.correctable_parameters.map(([k, v]) => `<option value="${esc(k)}">${esc(k)} (now ${fmt(v)})</option>`).join('')}</select></div>
            <div class="field"><label for="corrv">Corrected value</label><input id="corrv" name="v" type="number" step="any" required></div>
            <div class="field"><label for="corrr">Reason (required, recorded with your name)</label><input id="corrr" name="r" required minlength="5" placeholder="e.g. drawing dimension is inside diameter"></div>
            <div id="corrmsg"></div><button>Record correction and propagate</button></form></div></details>` : ''}
        </div></aside>
        <div class="split" role="separator" aria-orientation="vertical" aria-label="Resize properties panel" tabindex="0" data-split="l"></div>
        <section class="stage" aria-label="Flat pattern">
          ${c.patterns.length ? `<div class="stage-bar">
            <div class="parts" role="tablist" aria-label="Parts" id="parts">${c.patterns.map((p, i) => `<button role="tab" data-part="${i}" aria-selected="${i === wsState.part}" tabindex="${i === wsState.part ? 0 : -1}">${esc(p.part)} <small>${esc(p.class)}</small></button>`).join('')}</div>
            <span class="grow"></span>
            <div class="seg" role="group" aria-label="Tool"><button class="ib" id="t-select" aria-pressed="true" aria-label="Select tool" data-tip="Select · inspect curves">${ic('pointer')}</button><button class="ib" id="t-measure" aria-pressed="false" aria-label="Measure tool" data-tip="Measure  M">${ic('ruler')}</button></div>
            <div class="seg" role="group" aria-label="View"><button class="ib" id="z-out" aria-label="Zoom out" data-tip="Zoom out  −">${ic('minus')}</button><span class="zoomv" id="zoomv" aria-live="off">100%</span><button class="ib" id="z-in" aria-label="Zoom in" data-tip="Zoom in  +">${ic('plus')}</button><button class="ib" id="z-fit" aria-label="Fit pattern to view" data-tip="Fit  F">${ic('fit')}</button></div>
            <div class="seg" role="group" aria-label="Display"><button class="ib" id="v-grid" aria-pressed="true" aria-label="Grid" data-tip="Grid">${ic('grid')}</button><button class="ib" id="v-dims" aria-pressed="true" aria-label="Extent dimensions" data-tip="Extents">${ic('dims')}</button></div>
            <div class="seg" id="dxfseg"></div></div>
          <div class="pv-host" id="pv"><span class="stage-wm">Not released · review required</span></div>
          <div class="readout" aria-hidden="true"><span><b>X</b><span id="ro-x">—</span></span><span><b>Y</b><span id="ro-y">—</span></span><span class="ro-info" id="ro-info">Hover a curve for its length and vertices. Click to select.</span><span class="ro-opt">mm · development plane</span></div>`
          : `<div class="pv-host">${empty('warn', 'No pattern produced', esc(c.outcome || 'The request did not produce a pattern.'), canEdit ? `<a class="btn" href="#/math/${c.project_id}/${c.id}">Adjust the values in a new calculation</a>` : '', 'err')}</div>`}
        </section>
        <div class="split" role="separator" aria-orientation="vertical" aria-label="Resize results panel" tabindex="0" data-split="r"></div>
        <aside class="pane" aria-label="Results, verification and gates">
          ${tabsHtml('ws', 'Results', tabs, wsState.tab)}
          <div class="pane-scroll">${tabPanel('ws', 'gates', wsState.tab, gatesHtml, 'tp')}${tabPanel('ws', 'checks', wsState.tab, checksHtml, 'tp')}${tabPanel('ws', 'scrutiny', wsState.tab, scrutinyHtml, 'tp')}${tabPanel('ws', 'graph', wsState.tab, graphHtml, 'tp')}${tabPanel('ws', 'history', wsState.tab, historyHtml, 'tp')}${tabPanel('ws', 'records', wsState.tab, recordsHtml, 'tp')}</div>
        </aside>
      </div></div>`;

    const wire = (root) => {
      wireSplits(root);
      wireTabs(root, 'ws', (k) => { wsState.tab = k; });
      if (graph) graph.wire(root);
      $$('.rec', root).forEach((d) => d.addEventListener('toggle', () => { const b = $('.sec-b', d); if (d.open && !b.firstChild) { const r = c.records[d.dataset.rec]; b.innerHTML = `<p class="hash">${esc(r.id)}</p>${r.relations.length ? `<p class="hint">${r.relations.map((x) => `${esc(x.rel)} → <span class="mono">${esc(x.to.slice(0, 10))}…</span>`).join(' · ')}</p>` : ''}<pre class="mono" style="white-space:pre-wrap;overflow-wrap:anywhere;margin:6px 0 0;color:var(--text-2)">${esc(JSON.stringify(r.payload, null, 1))}</pre>`; } }));

      const rb = $('#replay'); if (rb) rb.onclick = async () => { try { const r = await withBusy(rb, 'Replaying…', () => api(`/api/calculations/${id}/replay`, { method: 'POST' })); toast(r && r.matches === false ? `Replay audit of job #${id} did NOT reproduce the closure.` : `Replay audit of job #${id} reproduced the closure.`, r && r.matches === false ? 'err' : 'ok'); wsState.tab = 'records'; refresh(); } catch (err) { toast(err.message, 'err'); } };
      const f = $('#corr'); if (f) f.onsubmit = async (e) => {
        e.preventDefault();
        if (f.v.value.trim() === '' || !Number.isFinite(Number(f.v.value))) { fieldError(f.v, 'Enter the corrected value as a number.'); return; }
        if (f.r.value.trim().length < 5) { fieldError(f.r, 'Give the reason (at least 5 characters); it is recorded with your name.'); return; }
        try {
          const r = await withBusy($('button', f), 'Propagating…', () => api(`/api/calculations/${id}/corrections`, { method: 'POST', json: { parameter: f.p.value, new_value: Number(f.v.value), reason: f.r.value } }));
          toast(`Correction recorded: ${f.p.value} → ${f.v.value}. New job #${r.new_calculation}; ${r.obligations} obligations raised, ${r.open} need an engineer.`, 'warn');
          wsState = { id: r.new_calculation, part: 0, tab: 'scrutiny' }; location.hash = `#/calc/${r.new_calculation}`;
        } catch (err) { formError('#corrmsg', err); }
      };
      $$('[data-ack]', root).forEach((btn) => { btn.onclick = async () => {
        const input = $(`[data-advreason="${btn.dataset.ack}"]`, root);
        if (input.value.trim().length < 5) { fieldError(input, 'Enter why the value is right this time (recorded with your name).'); return; }
        try { await withBusy(btn, null, () => api(`/api/advisories/${btn.dataset.ack}/acknowledge`, { method: 'POST', json: { reason: input.value.trim() } })); toast(`Advisory ${btn.dataset.ack} acknowledged.`, 'ok'); refresh(); } catch (err) { toast(err.message, 'err'); }
      }; });
      $$('[data-resolve]', root).forEach((btn) => { btn.onclick = async () => {
        const input = $(`[data-reason="${btn.dataset.resolve}"]`, root);
        if (input.value.trim().length < 5) { fieldError(input, 'Enter what you checked (the reason is recorded with your name).'); return; }
        try { await withBusy(btn, null, () => api(`/api/obligations/${btn.dataset.resolve}/resolve`, { method: 'POST', json: { decision: btn.dataset.decision, reason: input.value.trim() } })); toast(`Obligation #${btn.dataset.resolve} ${btn.dataset.decision}.`, 'ok'); refresh(); } catch (err) { toast(err.message, 'err'); }
      }; });

      // ---- pattern viewer and the panels that follow the active part
      if (!c.patterns.length) return;
      const xy = (q) => `${f3(q[0])}, ${f3(q[1])}`;
      const selBox = $('#sel');
      const showSel = (i, k) => {
        $$('#curves tr[data-ci]').forEach((tr) => tr.setAttribute('aria-selected', Number(tr.dataset.ci) === i));
        if (i === null) { selBox.innerHTML = '<p class="hint">Nothing selected. Click a curve in the pattern, or a name in the curve table.</p>'; return; }
        const p = k.points, xs = p.map((q) => q[0]), ys = p.map((q) => q[1]), a = p[0], b = p[p.length - 1];
        selBox.innerHTML = `<dl class="kv"><dt>Curve</dt><dd>${esc(k.name)}</dd><dt>Entity</dt><dd class="txt">${esc(k.kind)}</dd><dt>Layer</dt><dd><span class="sw${k.layer === 'CUT_INNER' ? ' inner' : ''}"></span>${esc(k.layer)}</dd><dt>Length</dt><dd>${fmt(k.length)} <span class="sub">mm</span></dd>
          <dt>Vertices</dt><dd>${p.length}</dd><dt>First vertex (x, y)</dt><dd>${xy(a)}</dd><dt>Last vertex (x, y)</dt><dd>${xy(b)}</dd><dt>Extent x × y</dt><dd>${f3(Math.max(...xs) - Math.min(...xs))} × ${f3(Math.max(...ys) - Math.min(...ys))}</dd></dl>
          <p class="hint">Length is the value computed by the engine; coordinates are the vertices sent to the DXF writer.</p>`;
      };
      viewer = new PatternViewer($('#pv'), {
        onSelect: showSel,
        onTool: (t) => { $('#t-select').setAttribute('aria-pressed', t === 'select'); $('#t-measure').setAttribute('aria-pressed', t === 'measure'); },
        onView: (z) => { $('#zoomv').textContent = `${Math.round(z * 100)}%`; },
        onReadout: (r) => {
          $('#ro-x').textContent = r.cursor ? f3(r.cursor.x) : '—'; $('#ro-y').textContent = r.cursor ? f3(r.cursor.y) : '—';
          let t;
          if (r.measure) { const m = r.measure; t = m.b ? `Distance ${f3(m.distance)} mm · ΔX ${f3(m.dx)} · ΔY ${f3(m.dy)} · from ${m.a.on} to ${m.b.on}${m.done ? '' : ' (click to set)'}` : `Measure: first point at ${m.a.on} (${f3(m.a.x)}, ${f3(m.a.y)}) — click the second point`; }
          else if (r.hover) { const h = r.hover; t = `${h.curve.name} · ${h.curve.kind} · ${h.curve.layer} · length ${f3(h.curve.length)} mm${h.vertex ? ` · vertex ${h.vertex.vi + 1}/${h.curve.points.length} (${f3(h.vertex.x)}, ${f3(h.vertex.y)})` : ''}`; }
          else t = r.tool === 'measure' ? 'Measure: click the first point. Points snap to computed vertices within 10 px.' : 'Hover a curve for its length and vertices. Click to select.';
          $('#ro-info').textContent = t;
        },
      });
      const showPart = (i) => {
        wsState.part = i; const p = c.patterns[i]; const all = [...p.curves, ...p.holes];
        $$('#parts [role=tab]').forEach((b, j) => { b.setAttribute('aria-selected', j === i); b.tabIndex = j === i ? 0 : -1; });
        $('#curves-part').textContent = `· ${p.part}`;
        $('#curves').innerHTML = `<div class="tbl"><table class="curves"><thead><tr><th>Curve</th><th>Entity</th><th class="num">Length (mm)</th></tr></thead><tbody>${all.map((k, j) => `<tr data-ci="${j}" aria-selected="false"><td><span class="sw${k.layer === 'CUT_INNER' ? ' inner' : ''}" title="${esc(k.layer)}"></span><button data-ci="${j}" aria-label="Select curve ${esc(k.name)} in the viewer">${esc(k.name)}</button></td><td>${esc(k.kind)}</td><td class="num" title="${fmt(k.length)}">${f3(k.length)}</td></tr>`).join('')}</tbody></table></div>
          <p class="hint" style="padding:4px 12px 0">Select a curve for its full-precision length. <span class="sw"></span>CUT_OUTER profile${p.holes.length ? ' · <span class="sw inner"></span>CUT_INNER opening' : ''}</p>`;
        $('#quant').innerHTML = `<dl class="kv"><dt>Part</dt><dd class="txt">${esc(p.part)} · ${words(p.family)}</dd><dt>Development class</dt><dd class="txt">${esc(p.class)} <span class="sub">(proposed naming)</span></dd>${Object.entries(p.quantities).map(([k, v]) => `<dt>${words(k)}</dt><dd>${fmt(v)}</dd>`).join('')}
          <dt>Numerical error <span class="sub">(estimated)</span></dt><dd>${fmt(p.eps_num)} <span class="sub">mm</span></dd><dt>Curve representation error <span class="sub">(estimated)</span></dt><dd>${fmt(p.eps_rep)} <span class="sub">mm</span></dd><dt>DXF SHA-256</dt><dd class="txt">${short(p.dxf_sha256)}</dd></dl>
          <p class="hint">Quantities are in the units the engine reports (lengths in mm, areas in mm², angles in radians). Errors are estimates, not certified bounds.</p>`;
        $('#assume').innerHTML = `<ul>${p.assumptions.map((a) => `<li>${esc(a)}</li>`).join('')}</ul>`;
        $('#dxfseg').innerHTML = blocked ? tag('bad', 'DXF withheld', 'close') : `<a class="btn" id="dxf" href="/api/calculations/${c.id}/dxf/${encodeURIComponent(p.part)}" download aria-label="Download DXF of ${esc(p.part)}, not released" data-tip="Download DXF · ${esc(p.part)} — file name carries NOT-RELEASED" data-tip-pos="end">${ic('download')}DXF · not released</a>`;
        viewer.setPattern(p); showSel(null);
      };
      $('#parts').onclick = (e) => { const b = e.target.closest('[data-part]'); if (b) showPart(Number(b.dataset.part)); };
      $('#parts').onkeydown = (e) => { const n = c.patterns.length, i = wsState.part; const j = e.key === 'ArrowRight' ? (i + 1) % n : e.key === 'ArrowLeft' ? (i - 1 + n) % n : -1; if (j >= 0) { e.preventDefault(); showPart(j); $$('#parts [role=tab]')[j].focus(); } };
      $('#curves').onclick = (e) => { const b = e.target.closest('button[data-ci]'); if (b) viewer.select(Number(b.dataset.ci) === viewer.sel ? null : Number(b.dataset.ci)); };
      $('#t-select').onclick = () => viewer.setTool('select'); $('#t-measure').onclick = () => viewer.setTool(viewer.tool === 'measure' ? 'select' : 'measure');
      $('#z-in').onclick = () => viewer.zoomBy(1.25); $('#z-out').onclick = () => viewer.zoomBy(0.8); $('#z-fit').onclick = () => viewer.fit();
      $('#v-grid').onclick = (e) => e.currentTarget.setAttribute('aria-pressed', viewer.toggle('grid')); $('#v-dims').onclick = (e) => e.currentTarget.setAttribute('aria-pressed', viewer.toggle('dims'));
      viewer.showPart = showPart; viewer.partCount = c.patterns.length;
      showPart(wsState.part);
    };
    return { title: `Job #${c.id}`, nav: 'workspace', ws: true, crumbs: [['Engineering Workspace', '#/workspace'], [`Project ${c.project_id}`, `#/project/${c.project_id}`], [`Job #${c.id}`]], html, wire };
  },

  async manufacturing() {
    const [all, ps] = await Promise.all([api('/api/calculations'), api('/api/projects')]);
    const cs = all.filter((c) => c.state !== 'REJECTED');
    return {
      title: 'Manufacturing', crumbs: [['Manufacturing']],
      html: `<div class="page">${ph('Manufacturing', 'DXF outputs produced by engineering jobs. None is released.')}
        ${note('<b>No file here is released for cutting.</b> DXF file names carry the prefix NOT-RELEASED and the job state. Files of rejected and superseded jobs are withheld by the server.')}
        ${panel('Jobs with DXF output', jobsTable(cs, 'jobs-mfg', ps), { flush: true, count: cs.length, tools: cs.length ? filterBox('jobs-mfg', 'Filter jobs') : '', foot: 'Open a job to inspect each part and download its DXF from the pattern viewer.' })}
        ${planned('Manufacturing planning', ['Plate division and stock fitting', 'Seam and weld planning', 'Allowance management (requires calibrated process data)', 'Manufacturing feasibility checks against machine limits', 'Bend-line, marking and inner-profile layers; kerf handling', 'CAM system acceptance testing'])}</div>`,
    };
  },

  async verification() {
    const [cs, obs, lessons, ps] = await Promise.all([api('/api/calculations'), api('/api/obligations'), api('/api/lessons'), api('/api/projects')]);
    const open = obs.filter((o) => o.state === 'open');
    return {
      title: 'Verification & Quality', crumbs: [['Verification & Quality']],
      html: `<div class="page">${ph('Verification & Quality', 'Release state of every engineering job, and what still needs an engineer.')}
        ${panel('Open scrutiny obligations', open.length ? `<div class="tbl"><table><thead><tr><th>#</th><th>Job</th><th>Affected</th><th>Required check</th><th>Raised</th></tr></thead><tbody>${open.map((o) => `<tr><td>${o.id}</td><td class="nw"><a href="#/calc/${o.target_calc}">Job #${o.target_calc}</a><br><span class="sub">corrects #${o.origin_calc}</span></td><td>${esc(o.kind)} · <span class="mono">${esc(o.label)}</span></td><td>${esc(o.required_check)}</td><td class="nw">${when(o.created_at)}</td></tr>`).join('')}</tbody></table></div>` : empty('shield', 'None open', 'Every obligation raised so far has been discharged by evidence or by a recorded decision.'),
          { flush: true, count: open.length, foot: `${obs.length} obligations recorded in total. Open a job to confirm or waive one; every decision is recorded with its reason. The rule table that creates them is provisional and unreviewed. <a href="#/amoeba/obligations">Full ledger</a>` })}
        ${panel('Lessons learned', lessonsTable(lessons), { flush: true, count: lessons.length, foot: 'Every later job in the same project is compared with these lessons.' })}
        ${panel('Release state of engineering jobs', jobsTable(cs, 'jobs-ver', ps), { flush: true, count: cs.length, tools: cs.length ? filterBox('jobs-ver', 'Filter jobs') : '' })}
        ${planned('Quality workflow', ['Engineer approval — no job can reach APPROVED in this build', 'Findings by severity (critical / major / minor) and investigations', 'Self-consistency audits A1–A4; replay on a separate machine or build', 'Obligations from drawing revisions and from verification failures (only parameter corrections raise them today)'])}</div>`,
    };
  },

  async amoeba(tab, arg) {
    const [a, obs, lessons, cs] = await Promise.all([api('/api/amoeba/summary'), api('/api/obligations'), api('/api/lessons'), api('/api/calculations')]);
    const withEdges = cs.filter((c) => c.state !== 'REJECTED' || c.superseded);
    const depId = arg || (withEdges[0] && withEdges[0].id);
    let dep = null, depErr = null;
    if (depId) { try { dep = await api(`/api/calculations/${depId}/dependencies`); } catch (err) { depErr = err.message; } }
    const graph = dep && dep.edges.length ? depGraph(dep.edges) : null;
    const li = (xs) => `<ul>${xs.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>`;
    const tabs = [['records', 'Records', a.records_by_kind.reduce((s, r) => s + r.count, 0)], ['activities', 'Activities', a.activities.length], ['obligations', 'Obligations', a.obligations.open, a.obligations.open > 0], ['lessons', 'Lessons', lessons.length], ['dependencies', 'Dependencies'], ['mechanisms', 'Mechanisms']];
    const active = tabs.some((t) => t[0] === tab) ? tab : 'records';
    return {
      title: 'AMOEBA Control Centre', crumbs: [['AMOEBA Control Centre']],
      html: `<div class="page">${ph('AMOEBA Control Centre', 'Engineering organism state — live contents of the append-only record store.')}
        ${tabsHtml('am', 'AMOEBA views', tabs, active)}
        ${tabPanel('am', 'records', active, `<div class="cols">${panel('Records by family and kind', a.records_by_kind.length ? `<div class="tbl"><table id="tbl-rec"><thead><tr><th>Family</th><th>Kind</th><th class="num">Count</th></tr></thead><tbody>${a.records_by_kind.map((r) => `<tr><td>${esc(r.family)}</td><td class="mono">${esc(r.kind)}</td><td class="num">${r.count}</td></tr>`).join('')}</tbody></table></div>` : empty('nodes', 'No records yet', 'Records appear when a project, document or job is created.'), { flush: true, tools: a.records_by_kind.length ? filterBox('tbl-rec', 'Filter records') : '', foot: 'Records are content-addressed and append-only. Open a job to read individual records and their payloads.' })}
          ${panel('Recent project snapshots', a.recent_snapshots.length ? `<div class="tbl"><table><thead><tr><th>Project</th><th>Snapshot id</th></tr></thead><tbody>${a.recent_snapshots.map((s) => `<tr><td class="nw"><a href="#/project/${s.project_id}">Project ${s.project_id}</a></td><td><span class="hash">${esc(s.snapshot)}</span></td></tr>`).join('')}</tbody></table></div>` : empty('folder', 'No snapshots yet', 'A snapshot is recorded when a project changes.'), { flush: true })}</div>`)}
        ${tabPanel('am', 'activities', active, panel('Activities and declared logical work units', a.activities.length ? `<div class="tbl"><table><thead><tr><th>Activity type</th><th class="num">Distinct activities</th><th class="num">Work units</th></tr></thead><tbody>${a.activities.map((r) => `<tr><td class="mono">${esc(r.type)}</td><td class="num">${r.distinct_activities}</td><td class="num">${r.declared_work_units}</td></tr>`).join('')}</tbody></table></div>` : empty('clock', 'No activities yet', 'Activities are recorded when a job runs.'), { flush: true, foot: 'Identical activities on identical inputs are one record (content addressing), so repeated identical jobs do not add rows.' }))}
        ${tabPanel('am', 'obligations', active, panel('Scrutiny obligation ledger', obs.length ? `<div class="tbl"><table id="tbl-obl"><thead><tr><th>#</th><th>Job</th><th>Direction</th><th>Affected</th><th>Required check</th><th>State</th><th>Evidence / decision</th></tr></thead><tbody>${obs.map((o) => `<tr><td>${o.id}</td><td class="nw"><a href="#/calc/${o.target_calc}">Job #${o.target_calc}</a></td><td>${words(o.direction)}<br><span class="sub mono">${esc(o.via)}</span></td><td>${esc(o.kind)} · <span class="mono">${esc(o.label)}</span></td><td>${esc(o.required_check)}</td><td>${oblTag(o.state)}</td><td>${esc(o.note)}${o.resolved_by ? `<br><span class="sub">${esc(o.resolved_by)} · ${when(o.resolved_at)}</span>` : ''}</td></tr>`).join('')}</tbody></table></div>` : empty('shield', 'No obligations recorded', 'Obligations are raised when a correction propagates along dependency edges.'),
          { flush: true, count: `${a.obligations.open} open of ${a.obligations.total}`, tools: obs.length ? filterBox('tbl-obl', 'Filter obligations') : '', foot: 'Open obligations are resolved on the job, in the Scrutiny tab, with a recorded reason.' }))}
        ${tabPanel('am', 'lessons', active, panel('Lessons and how often they matched', lessonsTable(lessons), { flush: true, count: lessons.length, foot: 'A matching lesson raises an advisory on the new job (gate G20). Advisories are acknowledged on the job with a reason; a match never changes a value.' }))}
        ${tabPanel('am', 'dependencies', active, panel('Dependency graph of a job', `${withEdges.length ? `<div class="field" style="max-width:420px"><label for="depjob">Job</label><select id="depjob">${withEdges.map((c) => `<option value="${c.id}"${String(c.id) === String(depId) ? ' selected' : ''}>Job #${c.id} · ${famShort(c.family)}${c.superseded ? ' (superseded)' : ''}</option>`).join('')}</select></div>` : ''}
          ${graph ? `${graph.html}${edgeTable(dep.edges)}<p class="hint">Rule table used for propagation: ${esc(dep.rule_table)}.</p>` : empty('nodes', depErr ? 'Could not load the graph' : 'No dependency edges', esc(depErr || 'Edges exist for jobs that produced a pattern.'))}`))}
        ${tabPanel('am', 'mechanisms', active, `<div class="cols2">${panel('Implemented in this build', li(a.implemented), { count: a.implemented.length })}<section class="panel"><header class="panel-h"><h2>Not implemented</h2>${tag('planned', 'planned', 'planned')}<span class="cnt">${a.not_implemented.length}</span></header><div class="panel-b">${li(a.not_implemented)}</div></section></div>`)}</div>`,
      wire(root) {
        wireTabs(root, 'am', (k) => history.replaceState(null, '', `#/amoeba/${k}${k === 'dependencies' && depId ? `/${depId}` : ''}`));
        if (graph) graph.wire(root);
        const s = $('#depjob'); if (s) s.onchange = () => { location.hash = `#/amoeba/dependencies/${s.value}`; };
      },
    };
  },

  async account() {
    return {
      title: 'Account', crumbs: [['Account']],
      html: `<div class="page" style="max-width:920px">${ph('Account', `${esc(me.username)} · role ${esc(me.role)}`)}
        ${panel('Change password', `<form id="pw"><div class="fgrid"><div class="field"><label for="pwc">Current password</label><input id="pwc" name="c" type="password" autocomplete="current-password" required></div>
          <div class="field"><label for="pwn">New password (at least 10 characters)</label><input id="pwn" name="n" type="password" autocomplete="new-password" minlength="10" required></div>
          <div class="field"><label for="pwn2">Repeat new password</label><input id="pwn2" name="n2" type="password" autocomplete="new-password" minlength="10" required></div></div>
          <div id="pwmsg"></div><button class="pri">Change password</button></form>`, { foot: 'Changing the password signs out every session of this account.' })}
        ${panel('Preferences', `<div class="fgrid"><div class="field"><label for="pf-theme">Theme</label><select id="pf-theme"><option value="dark">Dark workstation (default)</option><option value="light">Light</option></select></div>
          <div class="field"><label for="pf-rail">Navigation</label><select id="pf-rail"><option value="">Automatic (icons below 1440 px)</option><option value="open">Always show labels</option><option value="closed">Icons only</option></select></div></div>
          <button id="pf-reset">Reset panel sizes</button>`, { foot: 'Stored in this browser only, not on the server.' })}
        ${planned('Account security', ['Multi-factor authentication', 'Account recovery (an administrator must create a new account)', 'List and revoke individual sessions'])}</div>`,
      wire() {
        const f = $('#pw');
        f.onsubmit = async (e) => {
          e.preventDefault();
          if (f.n.value !== f.n2.value) { fieldError(f.n2, 'The two new passwords differ.'); return; }
          try { await withBusy($('button', f), 'Changing…', () => api('/api/password', { method: 'POST', json: { current: f.c.value, new: f.n.value } })); me = null; renderLogin(false, 'Password changed. All sessions were signed out; sign in again.', 'info'); }
          catch (err) { formError('#pwmsg', err); }
        };
        const t = $('#pf-theme'); t.value = document.documentElement.dataset.theme; t.onchange = () => setTheme(t.value);
        const r = $('#pf-rail'); r.value = prefs.get('nf-rail', ''); r.onchange = () => { prefs.set('nf-rail', r.value || null); applyRail(); };
        $('#pf-reset').onclick = () => { prefs.set('nf-lw', null); prefs.set('nf-rw', null); toast('Panel sizes reset.', 'ok'); };
      },
    };
  },

  async admin(tab) {
    if (me.role !== 'admin') return { title: 'Administration', crumbs: [['Administration']], html: `<div class="page">${ph('Administration', '')}${empty('shield', 'Administrator role required', 'Users, security configuration and the audit log are visible to administrators only.', '<a class="btn" href="#/dashboard">Back to the dashboard</a>')}</div>` };
    const [users, audit, sec, chain] = await Promise.all([api('/api/users'), api('/api/audit'), api('/api/security/status'), api('/api/audit/verify')]);
    const yn = (v) => (v === true ? tag('ok', 'yes', 'check') : v === false ? tag('bad', 'no', 'close') : esc(v));
    const chainHtml = (ch) => `${ch.intact ? tag('ok', 'INTACT', 'check') : tag('bad', `BROKEN at entry ${ch.first_broken_entry}`, 'close')} over ${ch.entries} entries.`;
    const tabs = [['users', 'Users', users.length], ['security', 'Security configuration'], ['audit', 'Audit log', audit.length], ['planned', 'Planned']];
    const active = tabs.some((t) => t[0] === tab) ? tab : 'users';
    return {
      title: 'Administration', crumbs: [['Administration']],
      html: `<div class="page">${ph('Administration', 'Users, the security configuration as running, and the hash-chained audit log.')}
        ${tabsHtml('ad', 'Administration views', tabs, active)}
        ${tabPanel('ad', 'users', active, `${panel('Users', `<div class="tbl"><table id="tbl-users"><thead><tr><th>#</th><th>Username</th><th>Role</th><th>Status</th><th>Created</th><th></th></tr></thead><tbody>${users.map((u) => `<tr><td>${u.id}</td><td>${esc(u.username)}${u.id === me.id ? ' <span class="sub">(you)</span>' : ''}</td><td>${esc(u.role)}</td><td>${u.active ? tag('ok', 'active', 'check') : tag('na', 'deactivated', 'dash')}</td><td class="nw">${when(u.created_at)}</td>
            <td class="nw">${u.id === me.id ? '' : `<button class="${u.active ? 'danger' : ''}" data-user="${u.id}" data-name="${esc(u.username)}" data-to="${u.active ? 0 : 1}">${u.active ? 'Deactivate' : 'Reactivate'}</button>`}</td></tr>`).join('')}</tbody></table></div>`, { flush: true, count: users.length, foot: 'Project access is granted per project, on the project page.' })}
          ${panel('Add user', `<form id="nu"><div class="fgrid"><div class="field"><label for="nuu">Username</label><input id="nuu" name="u" required autocomplete="off"></div><div class="field"><label for="nup">Password (at least 10 characters)</label><input id="nup" name="p" type="password" autocomplete="new-password" required></div><div class="field"><label for="nur">Role</label><select id="nur" name="r"><option>engineer</option><option>viewer</option><option>admin</option></select></div></div><div id="umsg"></div><button class="pri">Add user</button></form>`)}`)}
        ${tabPanel('ad', 'security', active, panel('Security configuration (as running)', `<div class="tbl"><table><thead><tr><th>Setting</th><th>Value</th></tr></thead><tbody>${Object.entries(sec).map(([k, v]) => `<tr><td class="nw">${words(k)}</td><td>${yn(v)}</td></tr>`).join('')}</tbody></table></div>`, { flush: true, foot: 'Read from the running server. Settings are changed through environment variables at start-up, not from this page.' }))}
        ${tabPanel('ad', 'audit', active, panel('Audit log', `<div class="tbl"><table id="tbl-audit"><thead><tr><th>When</th><th>User</th><th>Action</th><th>Detail</th></tr></thead><tbody>${audit.map((x) => `<tr><td class="nw">${when(x.at)}</td><td>${esc(x.username)}</td><td class="mono">${esc(x.action)}</td><td>${esc(x.detail)}</td></tr>`).join('')}</tbody></table></div>`,
          { flush: true, count: audit.length, tools: `${filterBox('tbl-audit', 'Filter audit log')}<button id="verify">${ic('shield')}Verify chain</button>`, foot: `Chain check: <span id="chain">${chainHtml(chain)}</span> ${esc(chain.limitation)}` }))}
        ${tabPanel('ad', 'planned', active, planned('Administration', ['Multi-factor authentication, single sign-on', 'Built-in TLS, database encryption at rest, key rotation', 'Backup and restore tooling, retention policies', 'AI/model configuration, engineering standards and factory rule management', 'Separate tenants (isolation is per project within one installation)']))}</div>`,
      wire(root) {
        wireTabs(root, 'ad', (k) => history.replaceState(null, '', `#/admin/${k}`));
        const f = $('#nu'); f.onsubmit = async (e) => { e.preventDefault(); try { await withBusy($('button', f), 'Adding…', () => api('/api/users', { method: 'POST', json: { username: f.u.value, password: f.p.value, role: f.r.value } })); toast(`User ${f.u.value} added as ${f.r.value}.`, 'ok'); refresh(); } catch (err) { formError('#umsg', err); } };
        // deactivation is a two-step inline confirmation, never a browser dialog
        $$('[data-user]', root).forEach((b) => { b.onclick = async () => {
          const off = b.dataset.to === '0';
          if (off && !b.dataset.armed) { b.dataset.armed = '1'; b.textContent = `Confirm: deactivate ${b.dataset.name}`; setTimeout(() => { if (b.isConnected) { delete b.dataset.armed; b.textContent = 'Deactivate'; } }, 5000); return; }
          try { await api(`/api/users/${b.dataset.user}/active`, { method: 'POST', json: { active: !off } }); toast(`${b.dataset.name} ${off ? 'deactivated; their sessions no longer work' : 'reactivated'}.`, 'ok'); refresh(); } catch (err) { toast(err.message, 'err'); }
        }; });
        $('#verify').onclick = async (e) => { try { const ch = await withBusy(e.currentTarget, 'Verifying…', () => api('/api/audit/verify')); $('#chain').innerHTML = chainHtml(ch); toast(ch.intact ? `Audit chain intact over ${ch.entries} entries.` : `Audit chain BROKEN at entry ${ch.first_broken_entry}.`, ch.intact ? 'ok' : 'err'); } catch (err) { toast(err.message, 'err'); } };
      },
    };
  },
};

// ---------------------------------------------------------------- resizable workspace panels
function wireSplits(root) {
  const body = $('.ws-body', root); if (!body) return;
  const lim = (side, v) => Math.round(Math.min(Math.max(v, 200), body.clientWidth * (side === 'l' ? 0.4 : 0.55)));
  const apply = (side, v) => { body.style.setProperty(side === 'l' ? '--lw' : '--rw', `${lim(side, v)}px`); };
  for (const side of ['l', 'r']) { const v = Number(prefs.get(`nf-${side}w`)); if (v) apply(side, v); }
  $$('.split', body).forEach((sp) => {
    const side = sp.dataset.split, pane = side === 'l' ? sp.previousElementSibling : sp.nextElementSibling;
    const save = () => prefs.set(`nf-${side}w`, String(pane.offsetWidth));
    sp.addEventListener('pointerdown', (e) => {
      e.preventDefault(); sp.setPointerCapture(e.pointerId); sp.classList.add('drag'); const x0 = e.clientX, w0 = pane.offsetWidth;
      const move = (m) => apply(side, w0 + (side === 'l' ? 1 : -1) * (m.clientX - x0));
      const up = () => { sp.classList.remove('drag'); sp.removeEventListener('pointermove', move); save(); };
      sp.addEventListener('pointermove', move); sp.addEventListener('pointerup', up, { once: true }); sp.addEventListener('pointercancel', up, { once: true });
    });
    sp.addEventListener('keydown', (e) => { const d = e.key === 'ArrowLeft' ? -24 : e.key === 'ArrowRight' ? 24 : 0; if (!d) return; e.preventDefault(); apply(side, pane.offsetWidth + (side === 'l' ? d : -d)); save(); });
    sp.addEventListener('dblclick', () => { body.style.removeProperty(side === 'l' ? '--lw' : '--rw'); prefs.set(`nf-${side}w`, null); });
  });
}

// ---------------------------------------------------------------- router
let renderSeq = 0;
function disposeView() { if (viewer) { viewer.destroy(); viewer = null; } }
function wireCommon(root) {
  $$('input[data-filter]', root).forEach((inp) => { inp.oninput = () => { const q = inp.value.trim().toLowerCase(); $$(`#${inp.dataset.filter} tbody tr`, root).forEach((tr) => { tr.hidden = !!q && !tr.textContent.toLowerCase().includes(q); }); }; });
}
const refresh = () => route(true);
async function route(keep = false) {
  if (!me) return;
  const seq = ++renderSeq; // a slower, older navigation must never overwrite a newer one
  const [, rawName = 'dashboard', arg, arg2] = location.hash.split('/');
  const name = views[rawName] ? rawName : 'dashboard';
  mountShell(); toggleNotes(false);
  const main = $('#view');
  const scroll = keep ? [main.scrollTop, ...$$('.pane-scroll', main).map((p) => p.scrollTop)] : null;
  const slow = keep ? 0 : setTimeout(() => { if (seq === renderSeq) { disposeView(); main.innerHTML = '<div class="loading" role="status"><span class="vh">Loading</span><div class="skel" style="width:28%"></div><div class="skel" style="width:62%"></div><div class="skel" style="width:48%"></div></div>'; } }, 200);
  let out;
  try { out = await views[name](arg, arg2); }
  catch (err) {
    if (!me) return;
    out = { title: 'Error', nav: name, crumbs: [['Error']], html: `<div class="page">${empty('warn', 'This view could not be loaded', esc(err.message), `<button class="pri" id="retry">${ic('replay')}Retry</button><a class="btn" href="#/dashboard">Dashboard</a>`, 'err')}</div>`, wire() { $('#retry').onclick = () => route(); } };
  }
  clearTimeout(slow);
  if (seq !== renderSeq || !me) return;
  disposeView();
  const nav = out.nav || name;
  document.title = `${out.title} — NEXUS FABRIC`;
  $$('.rail a').forEach((a) => { if (a.dataset.nav === nav) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current'); });
  $('#crumbs').innerHTML = (out.crumbs || []).map(([l, h], i, all) => (h ? `<a href="${h}">${esc(l)}</a>` : `<span${i === all.length - 1 ? ' aria-current="page"' : ''}>${esc(l)}</span>`)).join(ic('chevron', 10));
  main.style.overflow = out.ws ? '' : 'auto';
  main.innerHTML = out.html;
  wireCommon(main); if (out.wire) out.wire(main);
  if (scroll) { main.scrollTop = scroll[0]; $$('.pane-scroll', main).forEach((p, i) => { p.scrollTop = scroll[i + 1] || 0; }); }
  else { main.scrollTop = 0; main.focus({ preventScroll: true }); }
  if (name !== 'dashboard') refreshSummary();
}

// ---------------------------------------------------------------- keyboard
let gPending = 0;
document.addEventListener('keydown', (e) => {
  if (!me || e.ctrlKey || e.metaKey || e.altKey) return;
  const t = e.target, typing = t instanceof Element && t.matches('input,textarea,select,[contenteditable]');
  if (e.key === 'Escape') { if (!$('#notes')?.hidden) toggleNotes(false); else if (typing) t.blur(); return; }
  if (typing || $('dialog[open]')) return;
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (gPending && Date.now() - gPending < 1500) { gPending = 0; if (NAV_KEYS[k] && (NAV_KEYS[k] !== 'admin' || me.role === 'admin')) { e.preventDefault(); location.hash = `#/${NAV_KEYS[k]}`; } return; }
  gPending = 0;
  let used = true;
  if (e.key === '?') openHelp();
  else if (k === 'g') gPending = Date.now();
  else if (k === 'n' && me.role !== 'viewer') location.hash = `#/math${ctxProject ? `/${ctxProject}` : ''}`;
  else if (k === '[') toggleRail();
  else if (k === 't') $('#theme').click();
  else if (k === '/') { const f = $('#view input[data-filter]'); if (f && f.offsetParent) f.focus(); else used = false; }
  else if (viewer && viewer.pattern) {
    if (k === 'f') viewer.fit(); else if (k === 'm') viewer.setTool(viewer.tool === 'measure' ? 'select' : 'measure');
    else if (k === '+' || k === '=') viewer.zoomBy(1.25); else if (k === '-') viewer.zoomBy(0.8);
    else if (/^[1-9]$/.test(k) && Number(k) <= viewer.partCount) viewer.showPart(Number(k) - 1);
    else used = false;
  } else used = false;
  if (used) e.preventDefault();
});
document.addEventListener('click', (e) => { const p = $('#notes'); if (p && !p.hidden && !e.target.closest('#notes,#bell')) toggleNotes(false); });
window.addEventListener('resize', applyRail);
window.addEventListener('hashchange', () => route());

// ---------------------------------------------------------------- start
(async () => {
  try {
    const h = await api('/api/health');
    if (h.setup_required) return renderLogin(true);
    try { me = await api('/api/me'); route(); } catch { /* sign-in shown by api() */ }
  } catch (err) {
    $('#app').innerHTML = `<main class="login"><div class="login-card">${empty('warn', 'Cannot reach the server', esc(err.message), '<button class="pri" id="retry">Retry</button>', 'err')}</div></main>`;
    $('#retry').onclick = () => location.reload();
  }
})();
