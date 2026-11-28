// NF CAD viewer. Opens a stored DXF (a document, or a job part) through the nf-cad routes, shows the layers and
// entities, measures and picks, and re-shows NF's quality checks. Edits exist only for documents: each one is sent
// to the server, which writes a NEW version (the original is kept) and re-runs the quality gate. Undo and redo move
// between versions; nothing is edited in the page itself.
import { createView } from './view.js';
import { createMeasure, q } from './measure.js';
import { snapCandidates, pivotOf, ops } from './edit.js';
import { t, getLang, setLang } from './i18n.js';

const $ = (id) => document.getElementById(id);
const ACI = { 1: '#c0392b', 2: '#c79a00', 3: '#2e8b3c', 4: '#128f9e', 5: '#2b5fbf', 6: '#9b3fb0', 7: null };
const REDO_KEY = 'nfcad.redo';
const FLASH_KEY = 'nfcad.flash';

const view = createView($('view'));
const measure = createMeasure();
let base = null, model = null, tool = 'pan';
let selected = null;      // the picked entity (top-level, editable)
let lineStart = null;     // first point of an add-line, after the first click
const visible = new Set();

function status(msg, cls) {
  const el = $('status');
  el.textContent = msg || '';
  el.className = cls || '';
}

function srcFromUrl() {
  const m = new URLSearchParams(location.search).get('src') || '';
  const d = /^doc:(\d+)$/.exec(m);
  if (d) return `/api/cad/documents/${d[1]}`;
  const c = /^calc:(\d+):([a-z0-9_-]{1,20})$/i.exec(m);
  if (c) return `/api/cad/calculations/${c[1]}/${c[2]}`;
  return null;
}

function docId() {
  const m = /^\/api\/cad\/documents\/(\d+)$/.exec(base || '');
  return m ? Number(m[1]) : null;
}

const editable = () => docId() !== null;

async function get(path) {
  const res = await fetch(path, { credentials: 'same-origin', headers: { accept: 'application/json' } });
  let body = null;
  try { body = await res.json(); } catch (_) { body = null; }
  if (!res.ok) {
    const err = new Error((body && body.error) || `HTTP ${res.status}`);
    err.status = res.status;
    err.code = body && body.code;
    throw err;
  }
  return body;
}

// POST with the header the server requires on every write; the body is JSON.
async function send(path, body) {
  const res = await fetch(path, {
    method: 'POST',
    credentials: 'same-origin',
    headers: { accept: 'application/json', 'content-type': 'application/json', 'x-nf-request': '1' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  let out = null;
  try { out = await res.json(); } catch (_) { out = null; }
  if (!res.ok) {
    const err = new Error((out && out.error) || `HTTP ${res.status}`);
    err.status = res.status;
    err.code = out && out.code;
    throw err;
  }
  return out;
}

function describeError(e) {
  if (e.status === 401) return t('signIn');
  if (e.status === 403) return t('agentRefused');
  return e.message;
}

function go(id) {
  const u = new URL(location.href);
  u.searchParams.set('src', `doc:${id}`);
  location.assign(u.toString());
}

function readRedo() {
  try { return JSON.parse(sessionStorage.getItem(REDO_KEY) || '[]'); } catch (_) { return []; }
}
function writeRedo(list) {
  try { sessionStorage.setItem(REDO_KEY, JSON.stringify(list)); } catch (_) { /* private window: redo lives for this page only */ }
}

async function loadAll(b) {
  const page = 2000;
  const first = await get(`${b}?offset=0&limit=${page}`);
  const entities = [...first.entities];
  for (let off = page; off < first.entity_count; off += page) {
    const more = await get(`${b}?offset=${off}&limit=${page}`);
    entities.push(...more.entities);
  }
  return { head: first, entities };
}

function renderLayers() {
  const ul = $('layers');
  ul.textContent = '';
  for (const l of model.layers) {
    const li = document.createElement('li');
    const cb = document.createElement('input');
    cb.type = 'checkbox';
    cb.id = `layer-${l.name}`;
    cb.checked = visible.has(l.name);
    cb.addEventListener('change', () => {
      if (cb.checked) visible.add(l.name); else visible.delete(l.name);
      view.draw();
    });
    const sw = document.createElement('span');
    sw.className = 'swatch';
    const c = ACI[l.colour];
    if (c) sw.style.background = c;
    const label = document.createElement('label');
    label.htmlFor = cb.id;
    label.textContent = `${l.name} (${l.count})${l.on ? '' : ' · off'}${l.frozen ? ' · frozen' : ''}`;
    li.append(cb, sw, label);
    ul.append(li);
  }
}

function fmt(n) {
  if (typeof n !== 'number' || !Number.isFinite(n)) return '—';
  return Math.abs(n) >= 1e6 || (Math.abs(n) > 0 && Math.abs(n) < 1e-4) ? n.toExponential(6) : n.toFixed(6).replace(/\.?0+$/, '');
}

function showInfo(rows) {
  const dl = document.createElement('dl');
  for (const [k, v] of rows) {
    const dt = document.createElement('dt');
    dt.textContent = k;
    const dd = document.createElement('dd');
    dd.textContent = v;
    dl.append(dt, dd);
  }
  $('info').textContent = '';
  $('info').append(dl);
}

async function runMeasure(params) {
  const r = await get(`${base}/measure?${q(params)}`);
  return r.values;
}

function fieldNum(id) {
  const v = Number($(id).value);
  return Number.isFinite(v) ? v : NaN;
}
function fieldText(id) {
  return ($(id).value || '').trim();
}

// Sends edit operations; the server answers with the new version, and the page opens it.
async function applyOps(list, what) {
  try {
    const r = await send(`${base}/edit`, { ops: list });
    try {
      sessionStorage.setItem(FLASH_KEY, t('flashEdited', { id: r.document_id }));
      writeRedo([]);
    } catch (_) { /* no flash message in a private window */ }
    showInfo([[what, `#${r.document_id}`]]);
    go(r.document_id);
  } catch (e) {
    showInfo([[t('error'), describeError(e)]]);
  }
}

function needSelection() {
  showInfo([[t('error'), t('needSelection')]]);
  return null;
}

async function editAction(name) {
  if (!editable()) return showInfo([[t('error'), t('notEditable')]]);
  if (name === 'line') {
    tool = 'line';
    lineStart = null;
    view.v.marks = null;
    showInfo([[t('addLine'), t('needLine')]]);
    return;
  }
  if (name === 'undo') {
    try {
      const r = await send(`${base}/undo`);
      const stack = readRedo();
      stack.push(docId());
      writeRedo(stack);
      go(r.revert_to);
    } catch (e) {
      showInfo([[t('error'), describeError(e)]]);
    }
    return;
  }
  if (name === 'redo') {
    const stack = readRedo();
    const want = stack.pop();
    if (!want) return showInfo([[t('error'), t('nothingRedo')]]);
    try {
      const v = await get(`/api/cad/documents/${docId()}/versions`);
      if (!v.children.some((c) => c.document_id === want)) throw new Error(t('nothingRedo'));
      writeRedo(stack);
      go(want);
    } catch (e) {
      showInfo([[t('error'), e.message]]);
    }
    return;
  }
  if (name === 'layer') {
    if (!selected) return needSelection();
    return applyOps([ops.setLayer([selected.id], fieldText('f-layer'))], t('setLayer'));
  }
  if (!selected) return needSelection();
  const id = selected.id;
  const pivot = pivotOf(selected.geom);
  if (name === 'move') return applyOps([ops.move([id], fieldNum('f-dx'), fieldNum('f-dy'))], t('move'));
  if (name === 'rotate') {
    if (!pivot) return showInfo([[t('error'), t('noPivot')]]);
    return applyOps([ops.rotate([id], pivot, fieldNum('f-deg'))], t('rotate'));
  }
  if (name === 'mirror') {
    if (!pivot) return showInfo([[t('error'), t('noPivot')]]);
    return applyOps([ops.mirror([id], pivot, [pivot[0] + 1, pivot[1]])], t('mirror'));
  }
  if (name === 'scale') {
    if (!pivot) return showInfo([[t('error'), t('noPivot')]]);
    return applyOps([ops.scale([id], pivot, fieldNum('f-factor'))], t('scale'));
  }
  if (name === 'delete') {
    if (!window.confirm(t('confirmDelete'))) return;
    return applyOps([ops.delete([id])], t('del'));
  }
  return undefined;
}

async function onClick(worldPt) {
  if (!model) return;
  if (tool === 'line') {
    const s = snapCandidates(view.v.entities, visible, worldPt, 10 / view.v.scale);
    const pt = s ? s.point : worldPt;
    if (!lineStart) {
      lineStart = pt;
      view.v.marks = [pt];
      view.draw();
      showInfo([[t('addLine'), t('clickEnd')]]);
      return;
    }
    const layer = fieldText('f-layer') || '0';
    const from = lineStart;
    lineStart = null;
    view.v.marks = null;
    tool = 'pick';
    return applyOps([ops.addLine(from, pt, layer)], t('addLine'));
  }
  if (tool === 'measure') {
    const step = measure.click(worldPt);
    if (step.wait) {
      view.v.marks = [worldPt];
      view.draw();
      showInfo([[t('measure'), t('clickB')]]);
      return;
    }
    view.v.marks = step.both;
    view.draw();
    const [a, b] = step.both;
    const [d, delta] = await Promise.all([
      runMeasure({ kind: 'distance', from: a, to: b }),
      runMeasure({ kind: 'vector', from: a, to: b }),
    ]);
    showInfo([
      [t('distance'), `${fmt(d[0].value)} ${model.units}`],
      [t('dxdy'), `${fmt(delta[0].value)}, ${fmt(delta[1].value)}`],
      [t('angle'), `${fmt(delta[2].value)}°`],
    ]);
    return;
  }
  if (tool === 'pick') {
    const tol = 8 / view.v.scale;
    const r = await get(`${base}/pick?at=${worldPt[0]},${worldPt[1]}&tol=${tol}`);
    if (!r.entity) {
      view.v.highlight = null;
      selected = null;
      view.draw();
      showInfo([[t('picked'), t('none')]]);
      return;
    }
    view.v.highlight = r.entity;
    selected = r.entity.parent ? null : r.entity;
    view.draw();
    const rows = [[t('picked'), `${r.entity.id} · ${r.entity.kind} · ${r.entity.layer}`]];
    if (r.entity.parent) rows.push([t('note'), t('insideBlock')]);
    const g = r.entity.geom.type;
    try {
      if (g === 'line' || g === 'arc' || g === 'circle' || g === 'poly') {
        const len = await runMeasure({ kind: 'length', entity: r.entity.id });
        rows.push([t('length'), `${fmt(len[0].value)} ${model.units}`]);
      }
      if (g === 'circle' || (g === 'poly' && r.entity.geom.closed)) {
        const area = await runMeasure({ kind: 'area', entity: r.entity.id });
        rows.push([t('area'), `${fmt(area[0].value)} (${model.units})²`]);
      }
    } catch (e) {
      rows.push([t('error'), e.message]);
    }
    showInfo(rows);
  }
}

function setTool(name) {
  tool = name;
  measure.reset();
  lineStart = null;
  view.v.marks = null;
  for (const b of document.querySelectorAll('[data-tool]')) {
    b.setAttribute('aria-pressed', String(b.dataset.tool === name));
  }
}

async function showChecks() {
  const out = $('checks-out');
  out.hidden = false;
  out.textContent = '…';
  try {
    const r = await get(`${base}/checks`);
    out.textContent = '';
    const note = document.createElement('p');
    note.textContent = r.context;
    out.append(note);
    const ul = document.createElement('ul');
    for (const c of r.rules) {
      const li = document.createElement('li');
      const tag = document.createElement('span');
      tag.className = c.pass ? 'ok' : 'bad';
      tag.textContent = `${c.pass ? t('pass') : t('fail')} `;
      li.append(tag, document.createTextNode(c.rule));
      for (const f of c.findings.slice(0, 5)) {
        const d = document.createElement('div');
        d.textContent = f;
        li.append(d);
      }
      ul.append(li);
    }
    out.append(ul);
  } catch (e) {
    out.textContent = describeError(e);
  }
}

function applyStatic() {
  document.documentElement.lang = getLang();
  $('title').textContent = t('title');
  $('lang').textContent = t('lang');
  $('t-pan').textContent = t('pan');
  $('t-pick').textContent = t('pick');
  $('t-measure').textContent = t('measure');
  $('fit').textContent = t('fit');
  $('checks').textContent = t('checks');
  $('layers-h').textContent = t('layers');
  $('note').textContent = `${t('readOnly')} ${t('notTo')}`;
  $('e-move').textContent = t('move');
  $('e-rotate').textContent = t('rotate');
  $('e-mirror').textContent = t('mirror');
  $('e-scale').textContent = t('scale');
  $('e-delete').textContent = t('del');
  $('e-line').textContent = t('addLine');
  $('e-layer').textContent = t('setLayer');
  $('e-undo').textContent = t('undo');
  $('e-redo').textContent = t('redo');
  $('edit-h').textContent = t('editH');
  $('l-dx').textContent = t('dx');
  $('l-dy').textContent = t('dy');
  $('l-deg').textContent = t('deg');
  $('l-factor').textContent = t('factor');
  $('l-layer').textContent = t('layerName');
  $('edit-note').textContent = t('editNote');
}

function wireEdit() {
  const on = editable();
  for (const b of document.querySelectorAll('[data-edit]')) {
    b.disabled = !on;
    b.addEventListener('click', () => { editAction(b.dataset.edit).catch((e) => showInfo([[t('error'), describeError(e)]])); });
  }
  if (!on) $('edit-note').textContent = t('notEditable');
  const flash = (() => { try { const f = sessionStorage.getItem(FLASH_KEY); sessionStorage.removeItem(FLASH_KEY); return f; } catch (_) { return null; } })();
  if (flash) $('edit-note').textContent = flash;
}

async function start() {
  applyStatic();
  view.hooks.click = (w) => { onClick(w).catch((e) => showInfo([[t('error'), describeError(e)]])); };
  view.v.colour = (layer) => {
    const l = model && model.layers.find((x) => x.name === layer);
    return l ? ACI[l.colour] : null;
  };
  for (const b of document.querySelectorAll('[data-tool]')) {
    b.addEventListener('click', () => setTool(b.dataset.tool));
  }
  $('fit').addEventListener('click', () => view.fit());
  $('checks').addEventListener('click', showChecks);
  $('lang').addEventListener('click', () => {
    setLang(getLang() === 'ja' ? 'en' : 'ja');
    const u = new URL(location.href);
    u.searchParams.set('lang', getLang());
    location.replace(u.toString());
  });

  base = srcFromUrl();
  wireEdit();
  if (!base) {
    status(t('noDrawing'));
    return;
  }
  status(t('loading'));
  try {
    const { head, entities } = await loadAll(base);
    model = { ...head, entities: undefined };
    for (const l of head.layers) if (l.on && !l.frozen) visible.add(l.name);
    if (head.layers.length) $('f-layer').value = head.layers[0].name;
    view.v.entities = entities;
    view.set(entities, head.extents);
    renderLayers();
    $('title').textContent = `${t('title')} · ${head.version}`;
    status(`${t('units')}: ${head.units === 'unknown' ? t('unknown') : head.units} · ${t('entities')}: ${head.entity_count}`);
    if (head.warnings && head.warnings.length) {
      showInfo([[t('warnings'), head.warnings.join(' · ')]]);
    }
  } catch (e) {
    status(describeError(e), 'bad');
  }
}

start();
