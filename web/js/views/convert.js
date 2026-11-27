import { api, uploadFile } from '../api.js';
import { getState, setState, patchConvert } from '../store.js';
import { pollConversion } from '../lib/poll.js';
import { t } from '../i18n.js';

// the converter's upload refusals, shown as text instead of the internal wording
const MEMBRANE = {
  'file rejected at the membrane: unsupported_format': 'cv2.rej.format',
  'file rejected at the membrane: empty': 'cv2.rej.empty',
  'file rejected at the membrane: too_large': 'cv2.rej.large',
};

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
function overlay(show) {
  if (!root) return;
  const o = root.querySelector('.drop-overlay'); if (o) o.hidden = !show;
  const z = root.querySelector('.dropzone'); if (z) z.classList.toggle('drag', show);   // dashed outline only while a file is over the page
}
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
  const fxMs = getState().convert.phase === 'idle' ? splitFx() : 0;
  if (fxMs) await wait(fxMs);
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
    } else { patchConvert({ phase: 'error', error: MEMBRANE[err.message] ? t(MEMBRANE[err.message]) : err.message }); }
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
  if (!root) return;
  dropSteel();
  root.textContent = '';
  const p = getState().convert.phase;
  if (p === 'idle') renderIdle();
  else if (p === 'uploading' || p === 'converting' || p === 'polling') renderProgress();
  else if (p === 'done') renderDone();
  else renderFail();
}

let scene = null, sceneSeq = 0, cardSeq = 0;

function dropSteel() {
  sceneSeq++; cardSeq++;
  if (scene) { scene.dispose(); scene = null; }
}

const wait = (ms) => new Promise((r) => setTimeout(r, ms));
/* the short RGB-split on the headline (static tier and reduced-motion users get none) */
function splitFx() {
  const home = root && root.querySelector('.home');
  if (!home || (typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches)) return 0;
  if (scene) { scene.pulse(); return 330; }
  home.classList.remove('split'); void home.offsetWidth; home.classList.add('split');
  return 330;
}

function fmtDate(sec) {
  try { return new Date(sec * 1000).toLocaleDateString(document.documentElement.lang === 'ja' ? 'ja-JP' : 'en-GB', { day: 'numeric', month: 'short' }); }
  catch { return ''; }
}
function stateKey(s, sup) {
  if (sup) return 'badge.state.superseded';
  return { REVIEW_REQUIRED: 'badge.state.review', REFUSED: 'badge.state.refused', SUPERSEDED: 'badge.state.superseded' }[s] || 'badge.state.unknown';
}

function panel(cls, titleKey, n) {
  const p = el('section', 'fpanel ' + cls);
  p.style.setProperty('--d', String(n));
  const h = el('header', 'fp-h');
  h.appendChild(el('h2', null, t(titleKey)));
  p.appendChild(h);
  return p;
}

/* Recent jobs, AMOEBA status and quick links: the same API the other screens use. Nothing is invented: a failed
   request shows "not available", an empty list shows "none yet". */
function buildPanels() {
  const wrap = el('div', 'home-panels');
  const rp = panel('fp-recent', 'home.recent', 1);
  const rl = el('ul', 'fp-list'); rl.appendChild(el('li', 'fp-muted', t('home.loading'))); rp.appendChild(rl);
  const ap = panel('fp-amoeba', 'home.amoeba', 2);
  const ad = el('dl', 'fp-stats'); ap.appendChild(ad);
  const qp = panel('fp-quick', 'home.quick', 3);
  const ql = el('ul', 'fp-links');
  const link = (href, key, onClick) => {
    const li = document.createElement('li'); const a = el('a', null, t(key)); a.href = href;
    if (onClick) a.addEventListener('click', (e) => { e.preventDefault(); onClick(); });
    li.appendChild(a); li.appendChild(el('span', 'fp-arrow', '→')); ql.appendChild(li);
  };
  link('#/math', 'home.q.calc');
  link('#/help', 'home.q.help');
  link('#', 'home.q.lang', () => { const b = document.getElementById('btn-lang'); if (b) b.click(); });
  qp.appendChild(ql);
  wrap.append(rp, ap, qp);

  const seq = ++cardSeq;
  const stat = (k, v, warn) => {
    const d = document.createElement('div'); d.className = warn ? 'warn' : '';
    d.appendChild(el('dt', null, t(k))); d.appendChild(el('dd', null, String(v))); ad.appendChild(d);
  };
  const recent = getRecent();
  (async () => {
    let jobs = null, am = null;
    try { jobs = await api('/api/calculations?limit=40'); } catch { /* shown as not available */ }
    try { am = await api('/api/amoeba/summary'); } catch { /* idem */ }
    if (seq !== cardSeq || !root) return;
    rl.textContent = '';
    const rows = [];
    // conversions made in this browser first (at most two), then the newest jobs, three rows in all
    for (const r of recent.slice(0, 2)) rows.push({ href: '#/r/' + r.id, a: t('cv2.conv', { id: r.id }), b: t('home.conv'), c: r.ts ? fmtDate(r.ts / 1000) : '' });
    for (const c of (jobs || []).slice(0, 3 - rows.length)) rows.push({ href: '#/job/' + c.id, a: t('home.job', { id: c.id }), b: t(stateKey(c.state, c.superseded)), c: fmtDate(c.created_at) });
    if (!rows.length) rl.appendChild(el('li', 'fp-muted', jobs ? t('home.none') : t('home.na')));
    for (const r of rows) {
      const li = document.createElement('li'); const a = el('a', null, r.a); a.href = r.href;
      li.appendChild(a); li.appendChild(el('span', 'fp-meta', r.b + (r.c ? ' · ' + r.c : ''))); rl.appendChild(li);
    }
    ad.textContent = '';
    if (am) {
      const records = am.records_by_kind.reduce((n, r) => n + r.count, 0);
      const held = (jobs || []).reduce((n, c) => n + ((c.export && c.export.amoeba_holds) || []).length + ((c.export && c.export.withheld) || []).length, 0);
      stat('home.a.records', records); stat('home.a.open', am.obligations.open, am.obligations.open > 0); stat('home.a.held', held, held > 0);
    } else ad.appendChild(el('div', 'fp-muted', t('home.na')));
    // the engine id comes from the newest job (the health answer carries none)
    const eng = root.querySelector('[data-engine]');
    if (eng && jobs && jobs[0]) {
      try { const d = await api('/api/calculations/' + jobs[0].id); if (seq === cardSeq && root && d.engine) eng.textContent = d.engine; } catch { /* stays a dash */ }
    }
  })();
  return wrap;
}

function statusStrip() {
  const s = el('div', 'home-strip');
  const h = getState().health, mode = h && h.mode === 'desktop' ? t('home.mode.desktop') : t('home.mode.server');
  const item = (k, v, attrs) => { const i = el('span', 'hs-i'); i.appendChild(el('b', null, k)); const vv = el('span', null, v); if (attrs) Object.assign(vv.dataset, attrs); i.appendChild(vv); return i; };
  s.appendChild(item(t('home.s.mode'), mode));
  s.appendChild(item(t('home.s.engine'), '—', { engine: '' }));
  const pil = el('span', 'hs-pilot', t('home.pilot')); s.appendChild(pil);
  return s;
}

function renderIdle() {
  dropSteel();
  const seq = ++sceneSeq;
  const notice = getState().convert.notice;
  const home = el('div', 'home');
  const hero = el('section', 'home-hero');
  const top = el('div', 'hero-top');
  top.appendChild(el('p', 'eyebrow', t('cv2.title')));
  const h1 = el('h1', 'hero-title');
  h1.appendChild(el('span', 'ht-a', t('home.h1a'))); h1.appendChild(el('span', 'ht-b', t('home.h1b')));
  top.appendChild(h1);
  top.appendChild(el('p', 'hero-sub', t('cv2.sub')));
  if (notice) { const noticeEl = el('div', 'alert warn'); noticeEl.appendChild(el('p', null, notice)); top.appendChild(noticeEl); }
  const zone = el('div', 'dropzone');
  const main = el('div', 'dz-main');
  const ic = el('span', 'dz-icon'); ic.setAttribute('aria-hidden', 'true');
  ic.innerHTML = '<svg class="icon"><use href="#i-upload"/></svg>';
  main.appendChild(ic);
  main.appendChild(el('p', 'dz-hint', t('cv2.drop')));
  const inp = document.createElement('input');
  inp.type = 'file'; inp.accept = '.pdf,application/pdf'; inp.className = 'vh'; inp.id = 'conv-file';
  inp.addEventListener('change', () => {
    const f = inp.files[0]; if (!f) return;
    if (!isPdf(f)) { patchConvert({ phase: 'error', error: t('cv2.pdfonly') }); render(); return; }
    handleFile(f);
  });
  main.appendChild(inp);
  const btn = el('button', 'btn pri btn-lg', t('cv2.choose'));
  btn.type = 'button'; btn.addEventListener('click', () => inp.click());
  main.appendChild(btn);
  zone.appendChild(main);
  // the required scale and the options sit inside the surface, as one row
  const row = el('div', 'dz-row');
  row.appendChild(scaleField());
  const det = document.createElement('details'); det.className = 'conv-options';
  det.appendChild(el('summary', null, t('cv2.options')));
  const fld = el('div', 'field');
  const lbl = el('label', null, t('cv2.page')); lbl.htmlFor = 'conv-page'; fld.appendChild(lbl);
  const pg = document.createElement('input');
  pg.type = 'number'; pg.id = 'conv-page'; pg.value = '1'; pg.min = '1';
  fld.appendChild(pg); det.appendChild(fld); row.appendChild(det);
  zone.appendChild(row);
  zone.addEventListener('click', (e) => {
    const e2 = e.target.closest('.dz-icon, .dz-hint, .dz-main') || e.target;
    if (e2 === zone || e2.classList.contains('dz-icon') || e2.classList.contains('dz-hint') || e2.classList.contains('dz-main')) inp.click();
  });
  top.appendChild(zone);

  // the picture: the mark sits in this box. Static art first (shown at once, and kept on weak machines)
  const anchor = el('div', 'mark-anchor'); anchor.setAttribute('aria-hidden', 'true');
  anchor.appendChild(el('p', 'mark-cap', t('st.caption')));
  const art = el('div', 'home-art-wrap'); art.setAttribute('aria-hidden', 'true');
  hero.dataset.tier = 'static';
  hero.append(art, top, anchor, buildPanels());
  home.append(hero, statusStrip());
  root.appendChild(home);
  import('../lib/steel.js').then((m) => {
    if (seq !== sceneSeq || !root) return;
    art.appendChild(m.staticArt());
    // the 3D room starts after the first paint, so the drop zone is usable at once
    const start = () => {
      if (seq !== sceneSeq || !root) return;
      const sc = m.createHomeScene(hero, anchor);
      if (!sc) return;
      scene = sc;
      sc.onTier = (tr) => { if (tr === 'static' && scene === sc) { sc.dispose(); scene = null; hero.dataset.tier = 'static'; } };
    };
    requestAnimationFrame(() => setTimeout(start, 0));
  }).catch(() => { /* the picture is optional */ });
  splitFx();

  // Overlay
  const ov = el('div', 'drop-overlay'); ov.hidden = true;
  ov.appendChild(el('p', null, t('cv2.release')));
  root.appendChild(ov);
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
  runId++; teardown(); dropSteel();
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
