import { api, esc } from '../api.js';
import { getState } from '../store.js';
import { t, projName } from '../i18n.js';

const TABS = ['calculations', 'projects', 'obligations', 'audit', 'users'];

const FAMILY_FIELDS = {
  cylinder_shell: ['r_dev', 'length', 'surface_reference'],
  cone_frustum: ['r1', 'r2', 'h', 'surface_reference'],
  eccentric_reducer: ['r1', 'r2', 'h', 'e', 'surface_reference'],
  branch_stub: ['header_radius', 'branch_radius', 'branch_dev_radius', 'beta_deg', 'delta', 's_end', 'surface_reference'],
  header_with_branch: ['header_radius', 'header_dev_radius', 'header_length', 'branch_station',
    'branch_radius', 'branch_dev_radius', 'beta_deg', 'delta', 's_end',
    'surface_reference', 'header_surface_reference', 'branch_surface_reference', 'joint'],
};

let root, content, ac;

function el(tag, cls, txt) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (txt != null) e.textContent = txt;
  return e;
}

function fmtDate(d) {
  if (!d) return '';
  // the server sends Unix seconds
  const dt = new Date(typeof d === 'number' && d < 1e12 ? d * 1000 : d);
  return isNaN(dt) ? String(d) : dt.toLocaleDateString() + ' ' + dt.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}

function stateCls(s) {
  if (s === 'REVIEW_REQUIRED') return 't-warn';
  return 't-bad';
}

function newAc() {
  if (ac) ac.abort();
  ac = new AbortController();
  return ac.signal;
}

/* ---- loading / error ---- */

function showLoading() {
  content.textContent = '';
  content.appendChild(el('p', 'sub', 'Loading…'));
}

function showError(msg) {
  content.textContent = '';
  const a = el('div', 'alert bad');
  a.appendChild(el('p', null, msg));
  content.appendChild(a);
}

/* ---- table helper ---- */

function table(cols, rows, onClick) {
  const t = document.createElement('table');
  t.className = 'table';
  const thead = document.createElement('thead');
  const tr = document.createElement('tr');
  for (const c of cols) tr.appendChild(el('th', null, c));
  thead.appendChild(tr);
  t.appendChild(thead);
  const tbody = document.createElement('tbody');
  for (const r of rows) {
    const row = document.createElement('tr');
    for (const v of r.cells) row.appendChild(el('td', null, v));
    if (onClick) {
      row.style.cursor = 'pointer';
      row.addEventListener('click', () => onClick(r.data));
    }
    tbody.appendChild(row);
  }
  t.appendChild(tbody);
  const scroll = document.createElement('div');
  scroll.className = 'tbl-scroll';
  scroll.appendChild(t);
  return scroll;
}

/* ---- calculations ---- */

async function loadCalc(signal) {
  showLoading();
  try {
    const [calcs, projects] = await Promise.all([api('/api/calculations'), api('/api/projects')]);
    if (signal.aborted) return;
    content.textContent = '';
    const pnames = new Map((Array.isArray(projects) ? projects : []).map(p => [String(p.id), projName(p.name)]));

    const hd = el('div', 'panel-h');
    hd.appendChild(el('h2', null, t('ws.calculations')));
    const btn = el('button', 'btn pri', t('ws.new.calc'));
    btn.type = 'button';
    btn.addEventListener('click', () => { window.location.hash = '#/math'; });
    hd.appendChild(btn);
    content.appendChild(hd);

    if (!calcs || !calcs.length) {
      content.appendChild(el('p', 'sub', t('ws.none.calc')));
      return;
    }
    const rows = calcs.map(c => ({
      data: c,
      cells: ['#' + c.id, c.family || '', pnames.get(String(c.project_id)) || '', c.state || '',
        (c.gates_passed != null ? c.gates_passed + '/' + c.gates_total : ''), fmtDate(c.created_at)],
    }));
    content.appendChild(table([t('th.job'), t('th.family'), t('th.project'), t('th.state'), t('insp.gates'), t('th.created')], rows, (c) => {
      window.location.hash = '#/job/' + c.id;
    }));
  } catch (e) { if (!signal.aborted) showError(e.message); }
}

async function showCalcForm(signal) {
  content.textContent = '';
  const wrap = el('div', 'panel');
  wrap.appendChild(el('h2', null, 'New Calculation'));

  const form = document.createElement('form');
  form.className = 'calc-form';

  // Family select
  const fg = el('div', 'field');
  fg.appendChild(el('label', null, 'Geometry family'));
  const sel = document.createElement('select');
  sel.name = 'family';
  for (const f of Object.keys(FAMILY_FIELDS)) {
    const o = document.createElement('option');
    o.value = f;
    o.textContent = f.replace(/_/g, ' ');
    sel.appendChild(o);
  }
  fg.appendChild(sel);
  form.appendChild(fg);

  // Dynamic params container
  const paramsDiv = el('div', 'calc-params');
  form.appendChild(paramsDiv);

  function renderParams() {
    paramsDiv.textContent = '';
    const fields = FAMILY_FIELDS[sel.value] || [];
    for (const name of fields) {
      const f = el('div', 'field');
      const lbl = el('label', null, name.replace(/_/g, ' '));
      lbl.htmlFor = 'p-' + name;
      f.appendChild(lbl);
      const inp = document.createElement('input');
      inp.id = 'p-' + name;
      inp.name = name;
      inp.type = name === 'surface_reference' || name === 'header_surface_reference'
        || name === 'branch_surface_reference' || name === 'joint' ? 'text' : 'number';
      inp.step = 'any';
      inp.required = true;
      f.appendChild(inp);
      paramsDiv.appendChild(f);
    }
  }
  sel.addEventListener('change', renderParams);
  renderParams();

  // Project select
  const pg = el('div', 'field');
  pg.appendChild(el('label', null, 'Project'));
  const psel = document.createElement('select');
  psel.name = 'project';
  try {
    const projects = await api('/api/projects');
    if (signal.aborted) return;
    for (const p of projects) {
      const o = document.createElement('option');
      o.value = p.id;
      o.textContent = projName(p.name);
      psel.appendChild(o);
    }
  } catch { /* empty select */ }
  pg.appendChild(psel);
  form.appendChild(pg);

  // Submit
  const sub = el('button', 'btn pri', t('common.create'));
  sub.type = 'submit';
  form.appendChild(sub);

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    sub.disabled = true;
    sub.textContent = 'Creating…';
    const family = sel.value;
    const params = {};
    for (const name of (FAMILY_FIELDS[family] || [])) {
      const inp = form.querySelector('[name="' + name + '"]');
      if (!inp) continue;
      params[name] = inp.type === 'number' ? parseFloat(inp.value) : inp.value;
    }
    try {
      const r = await api('/api/projects/' + psel.value + '/calculations', {
        json: { family, params },
      });
      if (signal.aborted) return;
      if (r && r.id) window.location.hash = '#/job/' + r.id;
      else loadCalc(signal);
    } catch (err) {
      sub.disabled = false;
      sub.textContent = 'Create';
      const a = el('div', 'alert bad');
      a.appendChild(el('p', null, err.message));
      form.appendChild(a);
    }
  });

  // Back button
  const back = el('button', 'btn', t('common.back'));
  back.type = 'button';
  back.addEventListener('click', () => loadCalc(signal));
  form.appendChild(back);

  wrap.appendChild(form);
  content.appendChild(wrap);
}

/* ---- projects ---- */

async function loadProjects(signal) {
  showLoading();
  try {
    const projects = await api('/api/projects');
    if (signal.aborted) return;
    content.textContent = '';

    const hd = el('div', 'panel-h');
    hd.appendChild(el('h2', null, t('ws.projects')));
    const btn = el('button', 'btn pri', t('ws.new.project'));
    btn.type = 'button';
    btn.addEventListener('click', () => showProjectForm(signal));
    hd.appendChild(btn);
    content.appendChild(hd);

    if (!projects || !projects.length) {
      content.appendChild(el('p', 'sub', t('ws.none.projects')));
      return;
    }
    const rows = projects.map(p => ({
      data: p,
      cells: [String(p.id), esc(projName(p.name)), esc(p.description || ''),
        String(p.documents_count ?? p.documents?.length ?? 0),
        String(p.calculations_count ?? p.calculations?.length ?? 0),
        fmtDate(p.created_at)],
    }));
    content.appendChild(table([t('th.id'), t('th.name'), t('th.desc'), t('th.docs'), t('th.calcs'), t('th.created')], rows, (p) => {
      window.location.hash = '#/project/' + p.id;
    }));
  } catch (e) { if (!signal.aborted) showError(e.message); }
}

function showProjectForm(signal) {
  content.textContent = '';
  const wrap = el('div', 'panel');
  wrap.appendChild(el('h2', null, 'New Project'));
  const form = document.createElement('form');

  const fn = el('div', 'field');
  fn.appendChild(el('label', null, t('th.name')));
  const ni = document.createElement('input');
  ni.name = 'name'; ni.required = true;
  fn.appendChild(ni);
  form.appendChild(fn);

  const fd = el('div', 'field');
  fd.appendChild(el('label', null, 'Description'));
  const di = document.createElement('input');
  di.name = 'description';
  fd.appendChild(di);
  form.appendChild(fd);

  const sub = el('button', 'btn pri', t('common.create'));
  sub.type = 'submit';
  form.appendChild(sub);

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    sub.disabled = true;
    try {
      await api('/api/projects', { json: { name: ni.value, description: di.value } });
      if (!signal.aborted) loadProjects(signal);
    } catch (err) {
      sub.disabled = false;
      const a = el('div', 'alert bad');
      a.appendChild(el('p', null, err.message));
      form.appendChild(a);
    }
  });

  const back = el('button', 'btn', t('common.back'));
  back.type = 'button';
  back.addEventListener('click', () => loadProjects(signal));
  form.appendChild(back);

  wrap.appendChild(form);
  content.appendChild(wrap);
}

async function showProjectDetail(proj, signal) {
  content.textContent = '';
  try {
    const p = await api('/api/projects/' + proj.id);
    if (signal.aborted) return;
    content.appendChild(el('h2', null, esc(projName(p.name))));
    if (p.description) content.appendChild(el('p', 'sub', esc(p.description)));

    if (p.documents && p.documents.length) {
      content.appendChild(el('h3', null, 'Documents (' + p.documents.length + ')'));
      const ul = el('ul', 'il');
      for (const d of p.documents) {
        const li = document.createElement('li');
        li.appendChild(el('span', null, d.filename || d.sha256 || '#' + d.id));
        ul.appendChild(li);
      }
      content.appendChild(ul);
    }
    const back = el('button', 'btn', t('common.back'));
    back.type = 'button';
    back.addEventListener('click', () => loadProjects(signal));
    content.appendChild(back);
  } catch (e) { if (!signal.aborted) showError(e.message); }
}

/* ---- obligations ---- */

async function loadObligations(signal) {
  showLoading();
  try {
    const obs = await api('/api/obligations');
    if (signal.aborted) return;
    content.textContent = '';
    content.appendChild(el('h2', null, 'Obligations'));
    if (!obs || !obs.length) {
      content.appendChild(el('p', 'sub', t('ws.none.obl')));
      return;
    }
    const tbl = document.createElement('table');
    tbl.className = 'table';
    const thead = document.createElement('thead');
    const hr = document.createElement('tr');
    for (const h of [t('th.id'), t('th.desc'), t('th.status'), t('th.created'), '']) hr.appendChild(el('th', null, h));
    thead.appendChild(hr);
    tbl.appendChild(thead);
    const tbody = document.createElement('tbody');
    for (const o of obs) {
      const tr = document.createElement('tr');
      tr.appendChild(el('td', 'mono', '#' + o.id));
      tr.appendChild(el('td', null, o.label || o.note || ''));
      tr.appendChild(el('td', null, o.state || 'open'));
      tr.appendChild(el('td', null, fmtDate(o.created_at)));
      const td = document.createElement('td');
      if (o.state === 'open') {
        // a decision needs a reason, so it is taken on the job's Open items, not from this list
        const link = el('a', 'btn', t('ws.obl.resolve'));
        link.href = '#/job/' + o.target_calc;
        td.appendChild(link);
      }
      tr.appendChild(td);
      tbody.appendChild(tr);
    }
    tbl.appendChild(tbody);
    content.appendChild(tbl);
  } catch (e) { if (!signal.aborted) showError(e.message); }
}

/* ---- audit ---- */

async function loadAudit(signal) {
  showLoading();
  try {
    const log = await api('/api/audit');
    if (signal.aborted) return;
    content.textContent = '';
    content.appendChild(el('h2', null, 'Audit Log'));
    const entries = Array.isArray(log) ? log : (log && log.entries ? log.entries : []);
    if (!entries.length) {
      content.appendChild(el('p', 'sub', 'No audit entries.'));
      return;
    }
    const rows = entries.map(e => ({
      data: e,
      cells: [fmtDate(e.timestamp || e.created_at), e.user || e.username || '', e.action || '', e.detail || e.message || ''],
    }));
    content.appendChild(table([t('th.when'), t('th.user'), t('th.action'), t('th.detail')], rows));
  } catch (e) { if (!signal.aborted) showError(e.message); }
}

/* ---- users (admin): read-only; the one user-management screen is Administration > Users ---- */

async function loadUsers(signal) {
  const st = getState();
  if (!st.user || st.user.role !== 'admin') {
    content.textContent = '';
    const a = el('div', 'alert bad');
    a.appendChild(el('p', null, 'Admin access required.'));
    content.appendChild(a);
    return;
  }
  showLoading();
  try {
    const users = await api('/api/users');
    if (signal.aborted) return;
    content.textContent = '';

    const hd = el('div', 'panel-h');
    hd.appendChild(el('h2', null, t('ws.users')));
    const link = el('a', 'btn pri', t('ws.users.admin'));
    link.href = '#/admin/users';
    hd.appendChild(link);
    content.appendChild(hd);

    if (!users || !users.length) {
      content.appendChild(el('p', 'sub', t('ws.none.users')));
      return;
    }
    const tbl = document.createElement('table');
    tbl.className = 'table';
    const thead = document.createElement('thead');
    const hr = document.createElement('tr');
    for (const h of [t('th.username'), t('th.role'), t('th.active'), t('th.created')]) hr.appendChild(el('th', null, h));
    thead.appendChild(hr);
    tbl.appendChild(thead);
    const tbody = document.createElement('tbody');
    for (const u of users) {
      const tr = document.createElement('tr');
      tr.appendChild(el('td', null, u.username));
      tr.appendChild(el('td', null, u.role || ''));
      tr.appendChild(el('td', null, u.active !== false ? 'Yes' : 'No'));
      tr.appendChild(el('td', null, fmtDate(u.created_at)));
      tbody.appendChild(tr);
    }
    tbl.appendChild(tbody);
    content.appendChild(tbl);
  } catch (e) { if (!signal.aborted) showError(e.message); }
}

/* ---- tab dispatch ---- */

const TAB_LOADERS = {
  calculations: loadCalc,
  projects: loadProjects,
  obligations: loadObligations,
  audit: loadAudit,
  users: loadUsers,
};

function activateTab(tab) {
  if (!root) return;
  const signal = newAc();
  const tabs = root.querySelectorAll('[role="tab"]');
  for (const t of tabs) t.setAttribute('aria-selected', t.dataset.tab === tab ? 'true' : 'false');
  const loader = TAB_LOADERS[tab];
  if (loader) loader(signal);
}

/* ---- mount / unmount ---- */

export function mount(container, { tab } = {}) {
  const tpl = document.getElementById('t-workspace');
  if (!tpl) return;
  const frag = tpl.content.cloneNode(true);
  container.textContent = '';
  container.appendChild(frag);
  root = container.querySelector('.workspace');
  content = root.querySelector('.ws-content');

  const active = TABS.includes(tab) ? tab : 'calculations';

  // Wire sidebar tabs
  const tabs = root.querySelectorAll('[role="tab"]');
  for (const t of tabs) {
    t.addEventListener('click', () => {
      const name = t.dataset.tab;
      window.location.hash = '#/workspace/' + name;
      activateTab(name);
    });
  }

  activateTab(active);
}

export function unmount() {
  if (ac) { ac.abort(); ac = null; }
  content = null;
  root = null;
}
