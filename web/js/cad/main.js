// NF CAD viewer. Read-only: it opens a stored DXF (a document, or a job part) through the nf-cad routes, shows the
// layers and entities, measures and picks on the canvas, and re-shows NF's quality checks. It never edits.
import { createView } from './view.js';
import { createMeasure, q } from './measure.js';
import { t, getLang, setLang } from './i18n.js';

const $ = (id) => document.getElementById(id);
const ACI = { 1: '#c0392b', 2: '#c79a00', 3: '#2e8b3c', 4: '#128f9e', 5: '#2b5fbf', 6: '#9b3fb0', 7: null };

const view = createView($('view'));
const measure = createMeasure();
let base = null, model = null, tool = 'pan';
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

function describeError(e) {
  if (e.status === 401) return t('signIn');
  return `${t('error')}: ${e.message}`;
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

async function onClick(worldPt) {
  if (!model) return;
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
      view.draw();
      showInfo([[t('picked'), t('none')]]);
      return;
    }
    view.v.highlight = r.entity;
    view.draw();
    const rows = [[t('picked'), `${r.entity.id} · ${r.entity.kind} · ${r.entity.layer}`]];
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
  if (!base) {
    status(t('noDrawing'));
    return;
  }
  status(t('loading'));
  try {
    const { head, entities } = await loadAll(base);
    model = { ...head, entities: undefined };
    for (const l of head.layers) if (l.on && !l.frozen) visible.add(l.name);
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
