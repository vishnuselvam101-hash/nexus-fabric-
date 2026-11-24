import { api, uploadFile } from '../api.js';
import { getState, setState, patchConvert } from '../store.js';
import { pollConversion } from '../lib/poll.js';
import { t } from '../i18n.js';

const RECENT_KEY = 'nf_recent', MAX_RECENT = 5, SCALE_KEY = 'nf.scale';
const STAGES = ['cv2.stage.upload', 'cv2.stage.read', 'cv2.stage.write'];
let root, runId = 0, stopPoll, timerH, timerT0, dragN = 0;

function el(tag, cls, txt) {
  const e = document.createElement(tag); if (cls) e.className = cls;
  if (txt != null) e.textContent = txt; return e;
}
function fmtSize(n) {
  if (n < 1024) return t('cv2.size.b', { n });
  return n < 1048576 ? t('cv2.size.kb', { n: (n / 1024).toFixed(1) })
    : t('cv2.size.mb', { n: (n / 1048576).toFixed(1) });
}
function fmtTime(ms) {
  const s = Math.floor(ms / 1000), m = Math.floor(s / 60);
  return m ? t('cv2.time.m', { m, s: s % 60 }) : t('cv2.time.s', { s });
}
function isPdf(f) { return f.type === 'application/pdf' || /\.pdf$/i.test(f.name); }
/* Recent entries never carry drawing file names — only id, time, scale, status. */
function getRecent() {
  try {
    const l = JSON.parse(localStorage.getItem(RECENT_KEY)) || [];
    return l.filter(r => r && r.id != null)
      .map(r => ({ id: r.id, ts: r.ts, scale: r.scale, status: r.status }));
  } catch { return []; }
}
function saveRecent(item) {
  try {
    const all = getRecent(), prev = all.find(r => r.id === item.id);
    const l = all.filter(r => r.id !== item.id);
    l.unshift({ ...prev, ...item });
    localStorage.setItem(RECENT_KEY, JSON.stringify(l.slice(0, MAX_RECENT)));
  } catch { /* full */ }
}
function stageIdx(s) {
  if (!s || s === 'queued') return 0; if (s === 'extracting' || s === 'processing') return 1;
  if (s === 'computing' || s === 'flattening' || s === 'verifying') return 2; return 3;
}
/* A PDF conversion has no release state: never a green badge. */
function badgeCls(s) { return s === 'converted' ? 'badge warn' : 'badge bad'; }
function isTerminal(s) { return s === 'converted' || s === 'refused' || s === 'limit_exceeded'; }

/* ---- scale (never guessed, never silently reused) ---- */
function getScale() {
  try {
    const s = JSON.parse(localStorage.getItem(SCALE_KEY)) || {};
    return { kind: s.kind === 'mm_per_point' ? 'mm_per_point' : 'ratio',
      value: s.value == null ? '' : String(s.value) };
  } catch { return { kind: 'ratio', value: '' }; }
}
function saveScale(s) {
  try { localStorage.setItem(SCALE_KEY, JSON.stringify({ kind: s.kind, value: s.value })); }
  catch { /* full */ }
}
function readScale() {
  const k = root && root.querySelector('#conv-scale-kind');
  const v = root && root.querySelector('#conv-scale-value');
  if (k && v) return { kind: k.value === 'mm_per_point' ? 'mm_per_point' : 'ratio', value: v.value.trim() };
  return { kind: getScale().kind, value: '' };
}
function scaleNumber(s) {
  if (!s || s.value === '' || s.value == null) return null;
  const n = Number(s.value);
  return Number.isFinite(n) && n > 0 ? n : null;
}
function scaleText(s) {
  const n = scaleNumber(s); if (n == null) return null;
  return s.kind === 'mm_per_point' ? t('cv2.unit.mmpt', { n }) : t('cv2.unit.ratio', { n });
}
function clearScaleError() {
  const w = root && root.querySelector('.scale-field');
  const a = w && w.querySelector('.alert.bad'); if (a) a.remove();
}
function showScaleError() {
  const w = root && root.querySelector('.scale-field');
  if (!w) { patchConvert({ phase: 'error', error: t('cv2.scale.need') }); render(); return; }
  clearScaleError();
  const n = el('div', 'alert bad'); n.appendChild(el('p', null, t('cv2.scale.need'))); w.appendChild(n);
  const v = root.querySelector('#conv-scale-value'); if (v) v.focus();
}
function scaleMeta() {
  const txt = scaleText(getState().convert.scale);
  return txt ? t('cv2.scale.meta', { scale: txt }) : null;
}
function scaleField() {
  const stored = getScale();
  const wrap = el('div', 'scale-field');
  const kf = el('div', 'field');
  const kl = el('label', null, t('cv2.scale')); kl.htmlFor = 'conv-scale-kind'; kf.appendChild(kl);
  const kind = document.createElement('select'); kind.id = 'conv-scale-kind';
  const o1 = el('option', null, t('cv2.scale.ratio')); o1.value = 'ratio';
  const o2 = el('option', null, t('cv2.scale.mmpt')); o2.value = 'mm_per_point';
  kind.appendChild(o1); kind.appendChild(o2); kind.value = stored.kind;
  kf.appendChild(kind); wrap.appendChild(kf);
  const vf = el('div', 'field');
  const vl = el('label'); vl.htmlFor = 'conv-scale-value';
  const val = document.createElement('input');
  val.type = 'number'; val.id = 'conv-scale-value'; val.min = '0'; val.step = 'any';
  val.inputMode = 'decimal'; val.value = '';
  const sync = () => {
    const ratio = kind.value !== 'mm_per_point';
    vl.textContent = ratio ? t('cv2.scale.ratio.unit') : t('cv2.scale.mmpt.unit');
    val.placeholder = ratio ? t('cv2.scale.ratio.ph') : t('cv2.scale.mmpt.ph');
    val.setAttribute('aria-label', ratio ? t('cv2.scale.ratio.aria') : t('cv2.scale.mmpt.aria'));
    vf.textContent = '';
    if (ratio) { vf.appendChild(vl); vf.appendChild(val); }
    else { vf.appendChild(val); vf.appendChild(vl); }
  };
  sync();
  kind.addEventListener('change', () => {
    sync(); clearScaleError();
    saveScale({ kind: kind.value, value: val.value.trim() }); val.focus();
  });
  val.addEventListener('input', () => {
    clearScaleError(); saveScale({ kind: kind.value, value: val.value.trim() });
  });
  wrap.appendChild(vf);
  // The last scale is offered, never applied on its own.
  const last = scaleText(stored);
  if (last) {
    const b = el('button', 'btn small', t('cv2.uselast', { scale: last }));
    b.type = 'button';
    b.addEventListener('click', () => {
      kind.value = stored.kind; sync();
      val.value = String(scaleNumber(stored));
      clearScaleError(); saveScale({ kind: kind.value, value: val.value.trim() }); val.focus();
    });
    wrap.appendChild(b);
  }
  return wrap;
}

/* ---- project ---- */
async function resolveProject() {
  const s = getState(); if (s.projectId) return s.projectId;
  const ps = await api('/api/projects');
  let p = ps.find(x => x.name === 'Quick conversions');
  if (!p) p = await api('/api/projects', { method: 'POST',
    json: { name: 'Quick conversions', description: 'PDF to DXF from the drop zone' } });
  setState({ projectId: p.id }); return p.id;
}

/* ---- drag/drop ---- */
function overlay(show) { if (root) { const o = root.querySelector('.drop-overlay'); if (o) o.hidden = !show; } }
function onEnter(e) { e.preventDefault(); dragN++; if (getState().convert.phase === 'idle') overlay(true); }
function onOver(e) { e.preventDefault(); e.dataTransfer.dropEffect = 'copy'; }
function onLeave(e) { e.preventDefault(); if (--dragN <= 0) { dragN = 0; overlay(false); } }
function onDrop(e) {
  e.preventDefault(); dragN = 0; overlay(false);
  if (getState().convert.phase !== 'idle') return;
  const f = e.dataTransfer.files[0]; if (!f) return;
  if (!isPdf(f)) { patchConvert({ phase: 'error', error: t('cv2.pdfonly') }); render(); return; }
  handleFile(f);
}

/* ---- flow ---- */
async function handleFile(file) {
  if (!isPdf(file)) return;
  const scale = readScale(), scaleValue = scaleNumber(scale);
  if (scaleValue == null) { showScaleError(); return; }
  saveScale({ kind: scale.kind, value: scale.value });
  const run = ++runId; teardown();
  const pgEl = root && root.querySelector('#conv-page');
  const page = pgEl ? parseInt(pgEl.value, 10) || 1 : 1;
  patchConvert({ phase: 'uploading', file: { name: file.name, size: file.size },
    progress: 0, conversionId: null, result: null, error: null, notice: null,
    scale: { kind: scale.kind, value: scaleValue } });
  render();
  try {
    const pid = await resolveProject(); if (run !== runId) return;
    const up = await uploadFile('/api/projects/' + pid + '/documents', file, (p) => {
      if (run !== runId) return; patchConvert({ progress: p });
      const f = root && root.querySelector('.progress-fill');
      if (f) f.style.width = Math.round(p * 100) + '%';
    });
    if (run !== runId) return;
    patchConvert({ phase: 'converting', progress: 1 }); render();
    // Find doc id via sha256 from upload response
    const proj = await api('/api/projects/' + pid); if (run !== runId) return;
    const doc = proj.documents.findLast(d => d.sha256 === up.sha256);
    if (!doc) throw new Error(t('cv2.err.nodoc'));
    const res = await api('/api/documents/' + doc.id + '/pdf-to-dxf', { method: 'POST',
      json: { page, scale_kind: scale.kind, scale_value: Number(scaleValue) } });
    if (run !== runId) return;
    const conv = res.conversion;
    saveRecent({ id: conv.id, ts: Date.now(), status: conv.status,
      scale: { kind: scale.kind, value: scaleValue } });
    if (isTerminal(conv.status)) {
      patchConvert({ phase: 'done', conversionId: conv.id,
        result: { conversion: conv, report: res.report } });
    } else {
      patchConvert({ phase: 'polling', conversionId: conv.id });
      render(); startPoll(conv.id, run); return;
    }
    render();
  } catch (err) {
    if (run !== runId) return;
    if (err.detail && err.detail.conversion) {
      const c = err.detail.conversion;
      saveRecent({ id: c.id, ts: Date.now(), status: c.status,
        scale: { kind: scale.kind, value: scaleValue } });
      patchConvert({ phase: 'done', conversionId: c.id,
        result: { conversion: c, report: err.detail.report }, error: err.message });
    } else { patchConvert({ phase: 'error', error: err.message }); }
    render();
  }
}

function startPoll(convId, run) {
  timerT0 = Date.now();
  if (timerH) clearInterval(timerH);
  timerH = setInterval(() => {
    const tEl = root && root.querySelector('.elapsed');
    if (tEl && Date.now() - timerT0 > 10000) tEl.textContent = fmtTime(Date.now() - timerT0);
  }, 1000);
  stopPoll = pollConversion(convId, (r) => {
    if (run !== runId) return;
    const idx = stageIdx(r.status);
    if (root) root.querySelectorAll('.checklist li').forEach((li, i) => {
      li.classList.toggle('done', i < idx);
      li.classList.toggle('active', i === idx && idx < STAGES.length);
    });
    if (isTerminal(r.status) || r.status === 'error' || r.status === 'timeout') {
      teardown();
      saveRecent({ id: convId, ts: Date.now(), status: r.status });
      if (r.status === 'error' || r.status === 'timeout')
        patchConvert({ phase: 'error', error: r.error || t('cv2.err.timeout') });
      else patchConvert({ phase: 'done', result: { conversion: r } });
      render();
    }
  });
}

/* The server cannot abort a conversion — we only stop waiting for it. */
function cancel() {
  const p = getState().convert.phase;
  const waiting = p === 'uploading' || p === 'converting' || p === 'polling';
  runId++; teardown();
  patchConvert({ phase: 'idle', file: null, progress: 0, conversionId: null, result: null,
    error: null, notice: waiting ? t('cv2.stopped') : null });
  render();
}
function teardown() {
  if (stopPoll) { stopPoll(); stopPoll = null; }
  if (timerH) { clearInterval(timerH); timerH = null; }
}

/* ---- render ---- */
function render() {
  if (!root) return; root.textContent = '';
  const p = getState().convert.phase;
  if (p === 'idle') renderIdle();
  else if (p === 'uploading' || p === 'converting' || p === 'polling') renderProgress();
  else if (p === 'done') renderDone();
  else renderFail();
}

function renderIdle() {
  const notice = getState().convert.notice;
  if (notice) {
    const n = el('div', 'alert warn'); n.appendChild(el('p', null, notice)); root.appendChild(n);
  }
  const zone = el('div', 'dropzone');
  zone.appendChild(el('h1', null, t('cv2.title')));
  zone.appendChild(el('p', 'sub', t('cv2.sub')));
  const inp = document.createElement('input');
  inp.type = 'file'; inp.accept = '.pdf,application/pdf'; inp.className = 'vh'; inp.id = 'conv-file';
  inp.addEventListener('change', () => {
    const f = inp.files[0]; if (!f) return;
    if (!isPdf(f)) { patchConvert({ phase: 'error', error: t('cv2.pdfonly') }); render(); return; }
    handleFile(f);
  });
  zone.appendChild(inp);
  const btn = el('button', 'btn pri', t('cv2.choose'));
  btn.type = 'button'; btn.addEventListener('click', () => inp.click());
  zone.appendChild(btn);
  // Options
  const det = document.createElement('details'); det.className = 'conv-options';
  det.appendChild(el('summary', null, t('cv2.options')));
  const fld = el('div', 'field');
  const lbl = el('label', null, t('cv2.page')); lbl.htmlFor = 'conv-page'; fld.appendChild(lbl);
  const pg = document.createElement('input');
  pg.type = 'number'; pg.id = 'conv-page'; pg.value = '1'; pg.min = '1';
  fld.appendChild(pg); det.appendChild(fld); zone.appendChild(det);
  zone.addEventListener('click', (e) => {
    const e2 = e.target;
    if (e2 === zone || e2.tagName === 'H1' || (e2.tagName === 'P' && e2.classList.contains('sub'))) inp.click();
  });
  root.appendChild(zone);
  // Required scale, directly under the drop zone
  root.appendChild(scaleField());
  // Overlay
  const ov = el('div', 'drop-overlay'); ov.hidden = true;
  ov.appendChild(el('p', null, t('cv2.release')));
  root.appendChild(ov);
  // Recent
  const recent = getRecent(); if (!recent.length) return;
  const pan = el('div', 'panel');
  const hd = el('div', 'panel-h');
  hd.appendChild(el('h2', null, t('cv2.recent'))); hd.appendChild(el('span', 'cnt', String(recent.length)));
  pan.appendChild(hd);
  const bd = el('div', 'panel-b'); const ul = el('ul', 'il');
  for (const r of recent) {
    const li = document.createElement('li'); const row = el('div', 'l1');
    const a = el('a', null, t('cv2.conv', { id: r.id })); a.href = '#/r/' + r.id;
    row.appendChild(a); row.appendChild(el('span', 'id', '#' + r.id));
    li.appendChild(row);
    const bits = [], sc = scaleText(r.scale);
    if (sc) bits.push(sc);
    if (r.status) bits.push(String(r.status).toUpperCase().replace(/_/g, ' '));
    if (bits.length) li.appendChild(el('div', 'sub', bits.join(' · ')));
    ul.appendChild(li);
  }
  bd.appendChild(ul); pan.appendChild(bd); root.appendChild(pan);
}

function renderProgress() {
  const { phase, file, progress } = getState().convert;
  const w = el('div', 'convert-processing');
  w.appendChild(el('h2', null, file ? file.name : t('cv2.processing')));
  if (file && file.size) w.appendChild(el('p', 'sub', fmtSize(file.size)));
  const meta = scaleMeta(); if (meta) w.appendChild(el('p', 'sub', meta));
  const bar = el('div', 'progress-bar' + (phase !== 'uploading' ? ' indeterminate' : ''));
  const fill = el('div', 'progress-fill');
  fill.style.width = phase === 'uploading' ? Math.round(progress * 100) + '%' : '100%';
  bar.appendChild(fill); w.appendChild(bar);
  const ol = el('ol', 'checklist');
  STAGES.forEach((key, i) => {
    const li = el('li', null, t(key));
    if (phase === 'uploading') { if (i === 0) li.classList.add('active'); }
    else { if (i === 0) li.classList.add('done'); else if (i === 1) li.classList.add('active'); }
    ol.appendChild(li);
  });
  w.appendChild(ol);
  w.appendChild(el('p', 'elapsed sub'));
  if (!timerH && phase !== 'uploading') {
    timerT0 = Date.now();
    timerH = setInterval(() => { const tEl = root && root.querySelector('.elapsed');
      if (tEl && Date.now() - timerT0 > 10000) tEl.textContent = fmtTime(Date.now() - timerT0); }, 1000);
  }
  const cb = el('button', 'btn', t('cv2.stop')); cb.type = 'button';
  cb.addEventListener('click', cancel); w.appendChild(cb);
  root.appendChild(w);
}

function renderDone() {
  const { result, error } = getState().convert;
  const conv = result && result.conversion;
  if (!conv) { renderFail(); return; }
  const w = el('div', 'convert-result');
  { const b = el('div', badgeCls(conv.status), t('cv2.label')); if (conv.label) b.title = conv.label; w.appendChild(b); }
  const meta = scaleMeta(); if (meta) w.appendChild(el('p', 'sub', meta));
  if (error || conv.status !== 'converted') {
    const n = el('div', 'alert bad');
    n.appendChild(el('p', null, error || conv.message || conv.code || t('cv2.failed')));
    w.appendChild(n);
  }
  const rpt = result.report;
  if (rpt && rpt.warnings && rpt.warnings.length) {
    const p = el('div', 'panel'), hd = el('div', 'panel-h');
    hd.appendChild(el('h2', null, t('cv2.warnings')));
    hd.appendChild(el('span', 'cnt', String(rpt.warnings.length))); p.appendChild(hd);
    const bd = el('div', 'panel-b'), ul = el('ul', 'il');
    for (const wn of rpt.warnings) {
      const li = document.createElement('li'), row = el('div', 'l1');
      row.appendChild(el('span', 'id mono', wn.code));
      row.appendChild(el('span', null, wn.message));
      if (wn.count > 1) row.appendChild(el('span', 'cnt', t('cv2.times', { n: wn.count })));
      li.appendChild(row); ul.appendChild(li);
    }
    bd.appendChild(ul); p.appendChild(bd); w.appendChild(p);
  }
  if (conv.status === 'converted') {
    const dl = el('div', 'row');
    const dxf = el('a', 'btn pri', t('cv2.dxf'));
    dxf.href = '/api/pdf-conversions/' + conv.id + '/dxf'; dxf.download = ''; dl.appendChild(dxf);
    const rep = el('a', 'btn', t('cv2.report'));
    rep.href = '/api/pdf-conversions/' + conv.id + '/report'; rep.download = ''; dl.appendChild(rep);
    w.appendChild(dl);
  }
  const ag = el('button', 'btn', t('cv2.another'));
  ag.type = 'button'; ag.addEventListener('click', cancel); w.appendChild(ag);
  root.appendChild(w);
}

function renderFail() {
  const { error } = getState().convert;
  const w = el('div', 'convert-error');
  const n = el('div', 'alert bad');
  n.appendChild(el('b', null, t('cv2.failed')));
  if (error) n.appendChild(el('p', null, error));
  w.appendChild(n);
  const b = el('button', 'btn', t('cv2.retry'));
  b.type = 'button'; b.addEventListener('click', cancel); w.appendChild(b);
  root.appendChild(w);
}

/* ---- mount / unmount ---- */
export function mount(container) {
  root = container;
  document.addEventListener('dragenter', onEnter);
  document.addEventListener('dragover', onOver);
  document.addEventListener('dragleave', onLeave);
  document.addEventListener('drop', onDrop);
  const s = getState().convert;
  if (s.conversionId && !s.result) {
    if (s.phase !== 'polling') patchConvert({ phase: 'converting' });
    const run = ++runId;
    api('/api/pdf-conversions/' + s.conversionId).then(r => {
      if (run !== runId) return;
      if (isTerminal(r.status)) patchConvert({ phase: 'done', result: { conversion: r } });
      else { patchConvert({ phase: 'polling' }); startPoll(s.conversionId, run); }
      render();
    }).catch(() => { if (run !== runId) return; patchConvert({ phase: 'idle' }); render(); });
  }
  render();
}

export function unmount() {
  runId++; teardown();
  // Reset to idle if mid-upload/convert with no server-side conversion yet
  const p = getState().convert.phase;
  if ((p === 'uploading' || p === 'converting') && !getState().convert.conversionId)
    patchConvert({ phase: 'idle', file: null, progress: 0, error: null });
  document.removeEventListener('dragenter', onEnter);
  document.removeEventListener('dragover', onOver);
  document.removeEventListener('dragleave', onLeave);
  document.removeEventListener('drop', onDrop);
  dragN = 0; root = null;
}
