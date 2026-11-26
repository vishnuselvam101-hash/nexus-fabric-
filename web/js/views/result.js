import { api, esc } from '../api.js';
import { pollConversion } from '../lib/poll.js';
import { t } from '../i18n.js';

const STAGES = ['cv2.stage.upload', 'cv2.stage.read', 'cv2.stage.write'];
const STAGE_IDS = ['upload', 'read', 'write'];

let root = null;
let stopPolling = null;
let listeners = [];

export async function mount(container, { id }) {
  const tpl = document.getElementById('t-result');
  root = tpl.content.cloneNode(true).firstElementChild;
  container.appendChild(root);

  try {
    const data = await api('/api/pdf-conversions/' + id);
    renderConversion(unwrap(data), id);
  } catch (err) {
    showAlert(err.message || t('cv2.failed'));
  }
}

/* The server wraps the record: { conversion, report }. */
function unwrap(data) {
  return (data && data.conversion) || data || {};
}

function stageIdx(s) {
  if (!s || s === 'queued') return 0;
  if (s === 'extracting' || s === 'processing') return 1;
  if (s === 'computing' || s === 'flattening' || s === 'verifying') return 2;
  return 3;
}

function isDone(s) {
  return s === 'converted' || s === 'refused' || s === 'limit_exceeded' ||
    s === 'error' || s === 'timeout';
}

/* ---- scale (shown exactly as on the convert screen) ---- */
function scaleNumber(scale) {
  if (!scale || scale.value == null || scale.value === '') return null;
  const n = Number(scale.value);
  return Number.isFinite(n) && n > 0 ? n : null;
}

function scaleText(scale) {
  const n = scaleNumber(scale);
  if (n == null) return null;
  return scale.kind === 'mm_per_point' ? t('cv2.unit.mmpt', { n }) : t('cv2.unit.ratio', { n });
}

function conversionScale(c) {
  return c.scale || { kind: c.scale_kind, value: c.scale_value };
}

/* ---- small dom helpers ---- */
function setBadge(cls, text, href, title) {
  const badge = root.querySelector('.badge');
  const label = root.querySelector('.badge-label');
  const icon = root.querySelector('.badge-icon use');
  if (badge) {
    badge.classList.remove('warn', 'badge--refused');
    if (cls) badge.classList.add(cls);
  }
  if (label) {
    label.textContent = text;
    if (title) label.title = title;
  }
  if (icon) icon.setAttribute('href', href);
}

function setLabel(node, text) {
  const mark = node.querySelector('svg, .btn-icon');
  node.textContent = '';
  if (mark) node.appendChild(mark);
  node.appendChild(document.createTextNode(text));
}

function toLink(btn, href, text) {
  const a = document.createElement('a');
  a.href = href;
  a.download = '';
  a.className = btn.className;
  const mark = btn.querySelector('svg, .btn-icon');
  if (mark) a.appendChild(mark.cloneNode(true));
  a.appendChild(document.createTextNode(text));
  btn.replaceWith(a);
  return a;
}

function showAlert(message) {
  if (!root) return;
  setBadge('badge--refused', t('cv2.failed'), '#i-cross');
  const box = document.createElement('div');
  box.className = 'alert bad';
  box.setAttribute('role', 'alert');
  const p = document.createElement('p');
  p.textContent = message;
  box.appendChild(p);
  const grid = root.querySelector('#checks-grid');
  if (grid) { grid.textContent = ''; grid.appendChild(box); }
  else root.appendChild(box);
}

/* ---- render ---- */
function renderConversion(c, id) {
  if (!root) return;

  if (c.status === 'converted') {
    renderSuccess(c, id);
  } else if (isDone(c.status)) {
    renderRefused(c);
  } else {
    renderProcessing(c, id);
  }

  renderMeta(c);
  bindAnother();
}

function renderSuccess(c, id) {
  // A PDF conversion is read and written, never verified or released; never a green badge.
  setBadge('warn', t('cv2.label'), '#i-alert', c.label);

  const grid = root.querySelector('#checks-grid');
  if (grid) grid.textContent = '';

  const btnDxf = root.querySelector('#btn-dxf');
  const btnReport = root.querySelector('#btn-report');
  if (btnDxf) toLink(btnDxf, `/api/pdf-conversions/${id}/dxf`, t('cv2.dxf'));
  if (btnReport) toLink(btnReport, `/api/pdf-conversions/${id}/report`, t('cv2.report'));

  if (c.dxf_data || c.has_dxf) loadViewer(id);
}

function renderRefused(c) {
  setBadge('badge--refused', t('cv2.failed'), '#i-cross');

  const grid = root.querySelector('#checks-grid');
  if (grid) {
    grid.textContent = '';
    const msg = document.createElement('div');
    msg.className = 'alert bad';
    msg.setAttribute('role', 'alert');

    const code = c.refusal_code || c.code;
    if (code) {
      const chip = document.createElement('span');
      chip.className = 'chip';
      chip.textContent = String(code);
      msg.append(' ', chip);
    }

    // the badge already says "Conversion failed": the fallback text must not repeat it
    const text = c.message || c.error || c.refusal_reason ||
      (c.status === 'timeout' ? t('cv2.err.timeout') : '');
    if (text) {
      const p = document.createElement('p');
      p.textContent = text;
      msg.appendChild(p);
    }
    grid.appendChild(msg);
  }

  const actions = root.querySelector('.result-actions');
  if (actions) {
    actions.textContent = '';
    actions.style.display = '';
    const retryBtn = document.createElement('button');
    retryBtn.type = 'button';
    retryBtn.className = 'btn-secondary';
    retryBtn.textContent = t('cv2.retry');
    const handler = () => { location.hash = '#/'; };
    retryBtn.addEventListener('click', handler);
    listeners.push([retryBtn, 'click', handler]);
    actions.appendChild(retryBtn);
  }
}

function renderProcessing(c, id) {
  setBadge(null, t('cv2.processing'), '#i-spinner');

  const grid = root.querySelector('#checks-grid');
  if (grid) {
    grid.textContent = '';

    const bar = document.createElement('div');
    bar.className = 'progress-bar' + (c.progress == null ? ' indeterminate' : '');
    bar.setAttribute('role', 'progressbar');
    const fill = document.createElement('div');
    fill.className = 'progress-fill';
    bar.appendChild(fill);
    grid.appendChild(bar);

    const stages = document.createElement('ol');
    stages.className = 'stage-list';
    STAGES.forEach((key, i) => {
      const li = document.createElement('li');
      li.dataset.stage = STAGE_IDS[i];
      li.textContent = t(key);
      stages.appendChild(li);
    });
    grid.appendChild(stages);
    updateProgress(c);
  }

  const actions = root.querySelector('.result-actions');
  if (actions) actions.style.display = 'none';

  // pollConversion already hands over the unwrapped conversion record.
  stopPolling = pollConversion(id, (updated) => {
    if (!root) return;
    if (isDone(updated.status)) {
      if (stopPolling) { stopPolling(); stopPolling = null; }
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
  if (!root) return;
  const fill = root.querySelector('.progress-fill');
  if (fill && c.progress != null) {
    fill.style.width = Math.round(c.progress * 100) + '%';
  }
  const idx = stageIdx(c.status);
  root.querySelectorAll('.stage-list li').forEach((li, i) => {
    li.classList.toggle('stage-done', i < idx);
    li.classList.toggle('stage-active', i === idx && idx < STAGES.length);
  });
}

function renderMeta(c) {
  const meta = document.createElement('div');
  meta.className = 'card result-meta';

  const scale = scaleText(conversionScale(c));
  if (scale) {
    const p = document.createElement('p');
    p.className = 'sub';
    p.textContent = t('cv2.scale.meta', { scale });
    meta.appendChild(p);
  }

  const rows = [];
  const file = c.filename || c.document_name;
  if (file) rows.push([null, String(file)]);
  if (c.page != null) rows.push([t('cv2.page'), String(c.page)]);
  if (c.converter_version) rows.push([null, String(c.converter_version)]);

  for (const [label, value] of rows) {
    const row = document.createElement('div');
    row.className = 'kv';
    row.innerHTML = (label ? `<span class="kv-name">${esc(label)}</span>` : '') +
      `<span>${esc(value)}</span>`;
    meta.appendChild(row);
  }

  if (meta.childNodes.length) root.appendChild(meta);
}

function bindAnother() {
  const btn = root.querySelector('#btn-another');
  if (!btn) return;
  setLabel(btn, t('cv2.another'));
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

export function unmount() {
  if (stopPolling) { stopPolling(); stopPolling = null; }
  for (const [el, evt, fn] of listeners) el.removeEventListener(evt, fn);
  listeners = [];
  if (root) { root.remove(); root = null; }
}
