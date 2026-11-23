import { api, esc } from '../api.js';
import { pollConversion } from '../lib/poll.js';
import { t } from '../i18n.js';

let root = null;
let stopPolling = null;
let listeners = [];

export async function mount(container, { id }) {
  const tpl = document.getElementById('t-result');
  root = tpl.content.cloneNode(true).firstElementChild;
  container.appendChild(root);

  try {
    const conversion = await api('/api/pdf-conversions/' + id);
    renderConversion(conversion, id);
  } catch (err) {
    showAlert(err.message || 'Failed to load conversion');
  }
}

function renderConversion(c, id) {
  if (!root) return;

  if (c.status === 'converted') {
    renderSuccess(c, id);
  } else if (c.status === 'refused' || c.status === 'limit_exceeded') {
    renderRefused(c);
  } else {
    renderProcessing(c, id);
  }

  renderMeta(c);
  bindAnother();
}

function renderSuccess(c, id) {
  const badge = root.querySelector('.badge');
  const label = root.querySelector('.badge-label');
  const icon = root.querySelector('.badge-icon use');

  // A PDF conversion is never verified or released; show that, never a default verdict.
  badge.classList.add('warn');
  label.textContent = t('cv2.label');
  if (c.label) label.title = c.label;
  icon.setAttribute('href', '#i-alert');

  renderChecks(c);
  renderPatternInfo(c);

  const btnDxf = root.querySelector('#btn-dxf');
  const btnReport = root.querySelector('#btn-report');

  const dxfLink = document.createElement('a');
  dxfLink.href = `/api/pdf-conversions/${id}/dxf`;
  dxfLink.download = '';
  dxfLink.className = btnDxf.className;
  dxfLink.innerHTML = btnDxf.innerHTML;
  btnDxf.replaceWith(dxfLink);

  const reportLink = document.createElement('a');
  reportLink.href = `/api/pdf-conversions/${id}/report`;
  reportLink.download = '';
  reportLink.className = btnReport.className;
  reportLink.innerHTML = btnReport.innerHTML;
  btnReport.replaceWith(reportLink);

  if (c.dxf_data || c.has_dxf) {
    loadViewer(id);
  }
}

function renderChecks(c) {
  const grid = root.querySelector('#checks-grid');
  if (!grid) return;
  grid.innerHTML = '';

  const checks = c.checks || c.verification?.checks || [];
  for (const ch of checks) {
    const row = document.createElement('div');
    row.className = 'kv';
    row.setAttribute('role', 'listitem');

    const verdict = (ch.verdict || '').toUpperCase();
    const verdictClass = verdict === 'PASS' ? 'chip--pass' :
                         verdict === 'FAIL' ? 'chip--fail' : 'chip--warn';

    row.innerHTML = `
      <span class="kv-name">${esc(ch.name || ch.label || '')}</span>
      <span class="kv-primary">${esc(formatVal(ch.primary))}</span>
      <span class="kv-ref">${esc(formatVal(ch.reference))}</span>
      <span class="kv-diff">${esc(formatVal(ch.difference))}</span>
      <span class="kv-tol">${esc(formatVal(ch.tolerance))}</span>
      <span class="chip ${verdictClass}">${esc(verdict || 'N/A')}</span>
    `;
    grid.appendChild(row);
  }
}

function renderPatternInfo(c) {
  const pattern = c.pattern || c.flat_pattern || {};
  if (!pattern.family) return;

  const info = document.createElement('div');
  info.className = 'card pattern-info';
  info.innerHTML = `
    <h3>Pattern</h3>
    <div class="kv"><span class="kv-name">Family</span><span>${esc(pattern.family)}</span></div>
    ${pattern.eps_num != null ? `<div class="kv"><span class="kv-name">eps_num</span><span>${esc(String(pattern.eps_num))}</span></div>` : ''}
    ${pattern.eps_rep != null ? `<div class="kv"><span class="kv-name">eps_rep</span><span>${esc(String(pattern.eps_rep))}</span></div>` : ''}
    ${pattern.quantities ? `<div class="kv"><span class="kv-name">Quantities</span><span>${esc(JSON.stringify(pattern.quantities))}</span></div>` : ''}
  `;
  const actions = root.querySelector('.result-actions');
  if (actions) actions.before(info);
}

function renderRefused(c) {
  const badge = root.querySelector('.badge');
  const label = root.querySelector('.badge-label');
  const icon = root.querySelector('.badge-icon use');

  badge.classList.add('badge--refused');
  label.textContent = c.status === 'limit_exceeded' ? 'LIMIT EXCEEDED' : 'REFUSED';
  icon.setAttribute('href', '#i-cross');

  const grid = root.querySelector('#checks-grid');
  if (grid) {
    grid.innerHTML = '';
    const msg = document.createElement('div');
    msg.className = 'alert';
    msg.setAttribute('role', 'alert');
    msg.textContent = c.error || c.refusal_reason || 'Conversion was refused';
    if (c.refusal_code) {
      const code = document.createElement('span');
      code.className = 'chip';
      code.textContent = c.refusal_code;
      msg.prepend(code, ' ');
    }
    grid.appendChild(msg);
  }

  const actions = root.querySelector('.result-actions');
  if (actions) {
    actions.innerHTML = '';
    const retryBtn = document.createElement('button');
    retryBtn.type = 'button';
    retryBtn.className = 'btn-secondary';
    retryBtn.textContent = 'Adjust options & retry';
    const handler = () => { location.hash = '#/'; };
    retryBtn.addEventListener('click', handler);
    listeners.push([retryBtn, 'click', handler]);
    actions.appendChild(retryBtn);
  }
}

function renderProcessing(c, id) {
  const badge = root.querySelector('.badge');
  const label = root.querySelector('.badge-label');
  const icon = root.querySelector('.badge-icon use');

  badge.classList.remove('badge--refused');
  label.textContent = 'PROCESSING';
  icon.setAttribute('href', '#i-spinner');

  const grid = root.querySelector('#checks-grid');
  if (grid) {
    grid.innerHTML = '';
    const bar = document.createElement('div');
    bar.className = 'progress-bar';
    bar.setAttribute('role', 'progressbar');
    bar.innerHTML = '<div class="progress-fill"></div>';
    grid.appendChild(bar);

    const stages = document.createElement('ol');
    stages.className = 'stage-list';
    for (const s of ['upload', 'extract', 'pattern', 'verify']) {
      const li = document.createElement('li');
      li.dataset.stage = s;
      li.textContent = s.charAt(0).toUpperCase() + s.slice(1);
      const done = c.completed_stages?.includes(s);
      if (done) li.classList.add('stage-done');
      stages.appendChild(li);
    }
    grid.appendChild(stages);
  }

  const actions = root.querySelector('.result-actions');
  if (actions) actions.style.display = 'none';

  stopPolling = pollConversion(id, (updated) => {
    if (!root) return;
    if (updated.status === 'converted' || updated.status === 'refused' || updated.status === 'limit_exceeded') {
      root.innerHTML = '';
      const tpl = document.getElementById('t-result');
      const fresh = tpl.content.cloneNode(true).firstElementChild;
      root.replaceWith(fresh);
      root = fresh;
      listeners = [];
      renderConversion(updated, id);
    } else {
      updateProgress(updated);
    }
  });
}

function updateProgress(c) {
  const fill = root?.querySelector('.progress-fill');
  if (fill && c.progress != null) {
    fill.style.width = Math.round(c.progress * 100) + '%';
  }
  const stages = root?.querySelectorAll('.stage-list li');
  if (stages && c.completed_stages) {
    for (const li of stages) {
      if (c.completed_stages.includes(li.dataset.stage)) {
        li.classList.add('stage-done');
      }
    }
  }
}

function renderMeta(c) {
  const meta = document.createElement('div');
  meta.className = 'card result-meta';
  const items = [];
  if (c.converter_version) items.push(`<div class="kv"><span class="kv-name">Engine</span><span>${esc(c.converter_version)}</span></div>`);
  if (c.created_at) items.push(`<div class="kv"><span class="kv-name">Created</span><span>${esc(fmtTime(c.created_at))}</span></div>`);
  if (c.completed_at) items.push(`<div class="kv"><span class="kv-name">Completed</span><span>${esc(fmtTime(c.completed_at))}</span></div>`);
  if (c.filename || c.document_name) items.push(`<div class="kv"><span class="kv-name">File</span><span>${esc(c.filename || c.document_name)}</span></div>`);
  if (c.page_number != null) items.push(`<div class="kv"><span class="kv-name">Page</span><span>${esc(String(c.page_number))}</span></div>`);
  if (items.length) {
    meta.innerHTML = items.join('');
    root.appendChild(meta);
  }
}

function bindAnother() {
  const btn = root.querySelector('#btn-another');
  if (!btn) return;
  const handler = () => { location.hash = '#/'; };
  btn.addEventListener('click', handler);
  listeners.push([btn, 'click', handler]);
}

async function loadViewer(id) {
  try {
    const viewer = await import('../../viewer.js');
    const container = document.createElement('div');
    container.className = 'viewer-container';
    const actions = root.querySelector('.result-actions');
    if (actions) actions.before(container);
    else root.appendChild(container);
    if (viewer.render) viewer.render(container, id);
    else if (viewer.default) viewer.default(container, id);
  } catch { /* viewer optional */ }
}

function formatVal(v) {
  if (v == null) return '-';
  if (typeof v === 'number') return Number.isInteger(v) ? String(v) : v.toFixed(6);
  return String(v);
}

function fmtTime(ts) {
  try { return new Date(ts).toLocaleString(); } catch { return String(ts); }
}

export function unmount() {
  if (stopPolling) { stopPolling(); stopPolling = null; }
  for (const [el, evt, fn] of listeners) el.removeEventListener(evt, fn);
  listeners = [];
  if (root) { root.remove(); root = null; }
}
