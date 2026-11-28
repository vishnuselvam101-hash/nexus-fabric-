// NEXUS FABRIC job, pro view (lane F2/F3, pick 2A: check only). Sidebar of pages and parts, a viewer with the tabs
// Drawing / Cut pattern / 3D / NF CAD, an inspector with value chips that light their box on the drawing, review rows,
// the factory rules with a what-if preview, and the limited person override where the server allows it.
// Everything shown comes from the orders API. There is no approve or release control; the only write besides
// answering a question is the person's hold override (its limits are the server's).
import { api, esc, ApiError } from '../api.js';
import { t, lang, idLabel } from '../i18n.js';
import { createCursor } from '../operator-cursor.js';
import { mountLiveControls } from '../lib/live-controls.js';
import { questionsOf, renderQuestions } from '../lib/question-cards.js';

const NEED = ['held_missing', 'refused', 'not_computed'];
const isReady = (p) => p.state === 'computed' && p.dxf_allowed === true && Array.isArray(p.dxf_blocked_by) && p.dxf_blocked_by.length === 0;
const msgOf = (e) => (e instanceof ApiError ? e.message : String(e?.message || e));
const ja = () => lang() === 'ja';
const what = (key) => idLabel('param', key) || String(key).replaceAll('_', ' ');
const stamp = (secs) => new Date(secs * 1000).toISOString().slice(0, 16).replace('T', ' ');
const NS = 'http://www.w3.org/2000/svg';

let root = null, oid = null, order = null, res = [], ov = { allow_list: [], overrides: [], min_reason_chars: 20, mark: '' };
let sel = { no: null, key: null }, tab = 'drawing', pmode = 'parts', rules = null, impact = null;
let cursor = null, stopFollow = null, controls = null, stop3d = null, lastSeq = -1, timer = null;

const $ = (s) => root.querySelector(s);

export async function mount(viewEl, params = {}) {
  oid = Number(params.oid);
  root = document.createElement('section');
  root.className = 'jf1 jpro';
  root.setAttribute('aria-labelledby', 'jp-title');
  viewEl.append(root);
  sel = { no: null, key: null }; tab = 'drawing'; pmode = 'parts'; rules = null; impact = null;
  try { await load(); } catch (e) { root.innerHTML = `<div class="error-box"><h2>${esc(t('jf1.err.load'))}</h2><p>${esc(msgOf(e))}</p></div>`; return; }
  frame();
  paint();
  live();
}

export function unmount() {
  clearTimeout(timer);
  if (stopFollow) stopFollow();
  if (cursor) cursor.destroy();
  if (controls) controls.destroy();
  if (stop3d) stop3d();
  stopFollow = cursor = controls = stop3d = null;
  root?.remove(); root = null;
}

async function load() {
  order = await api(`/api/orders/${oid}`);
  const r = await api(`/api/orders/${oid}/compute`).catch(() => null);
  res = r?.current?.result?.parts || [];
  ov = await api(`/api/orders/${oid}/overrides`).catch(() => ov);
  if (!sel.no && (order.parts || [])[0]) sel.no = order.parts[0].no;
}

const rp = (no) => res.find((p) => p.no === no);
const statusOf = (no) => { const p = rp(no); return !p ? 'na' : isReady(p) ? 'ready' : NEED.includes(p.state) ? 'need' : 'held'; };

function frame() {
  root.innerHTML = `
    <header class="jp-head">
      <div><p class="jf1-kicker mono">${esc(order.order?.name || '')}</p><h1 id="jp-title">${esc(t('jf2.pro.title'))}</h1></div>
      <a class="jf1-btn jf1-link" href="#/start/${oid}">${esc(t('jf2.pro.simple'))}</a>
    </header>
    <p class="jf1-note">${esc(t('jf2.pro.check'))}</p>
    <div class="jp-tool">
      <div class="lv-seg" role="group" aria-label="${esc(t('jf2.pro.title'))}" data-pm>
        <button type="button" data-pm="parts">${esc(t('jf2.pro.parts'))}</button>
        <button type="button" data-pm="rules">${esc(t('jf2.pro.rules'))}</button>
      </div>
      <div data-controls class="jp-controls"></div>
    </div>
    <div class="jp-grid">
      <nav class="jp-side" aria-label="${esc(t('jf2.pro.sidebar'))}" data-side></nav>
      <section class="jp-main" data-main></section>
      <aside class="jp-insp" aria-label="${esc(t('jf2.pro.inspector'))}" data-insp></aside>
    </div>`;
  root.addEventListener('click', onClick);
}

function onClick(ev) {
  const b = ev.target.closest('button, [data-part]');
  if (!b) return;
  if (b.dataset.pm) { pmode = b.dataset.pm; if (pmode === 'rules' && !rules) loadRules(); paint(); }
  else if (b.dataset.part) { sel = { no: b.dataset.part, key: null }; paint(); }
  else if (b.dataset.tab) { tab = b.dataset.tab; paint(); }
  else if (b.dataset.chip) { sel.key = b.dataset.chip; tab = 'drawing'; paint(); }
  else if (b.dataset.ovOpen) openOverride(b);
  else if (b.dataset.ovUndo) undoOverride(Number(b.dataset.ovUndo), b);
}

function paint() {
  if (!root) return;
  root.querySelectorAll('[data-pm] button').forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.pm === pmode)));
  sidebar(); inspector();
  const main = $('[data-main]');
  threeToken++;
  if (stop3d) { stop3d(); stop3d = null; }
  if (pmode === 'rules') { main.innerHTML = rulesHtml(); wireRules(); return; }
  const tabs = ['drawing', 'cut', '3d', 'cad'];
  main.innerHTML = `<div class="lv-seg jp-tabs" role="group" aria-label="${esc(t('jf2.pro.tabs'))}">${tabs.map((k) => `<button type="button" data-tab="${k}" aria-pressed="${k === tab}">${esc(t(`jf2.pro.tab.${k}`))}</button>`).join('')}</div><div class="jp-view" data-view></div>`;
  const v = $('[data-view]');
  const part = (order.parts || []).find((p) => p.no === sel.no);
  if (!part) { v.innerHTML = `<p class="jf1-note">${esc((order.parts || []).length ? t('jf2.pro.nopart') : t('jf2.pro.noparts'))}</p>`; return; }
  if (tab === 'drawing') drawing(v, part);
  else if (tab === 'cut') cut(v, part);
  else if (tab === '3d') view3d(v, part);
  else cad(v);
}

function sidebar() {
  const parts = order.parts || [], pages = order.pages || [];
  const groups = pages.map((pg) => ({ pg, list: parts.filter((p) => p.page === pg.page_no) }));
  const loose = parts.filter((p) => !pages.some((pg) => pg.page_no === p.page));
  const row = (p) => `<button type="button" class="jp-part" data-part="${esc(p.no)}" aria-pressed="${p.no === sel.no}"><i class="jp-dot is-${statusOf(p.no)}" aria-hidden="true"></i><span class="mono">${esc(p.no)}</span> ${esc(p.name)}<span class="jp-st">${esc(stText(p.no))}</span></button>`;
  $('[data-side]').innerHTML = `<h2>${esc(t('jf2.pro.pages'))}</h2>` + groups.map((g) => `<div class="jp-pg"><p class="jp-pgh mono">${esc(t('jf2.pro.pdf', { n: g.pg.page_no }))}</p>${g.list.map(row).join('')}</div>`).join('') + (loose.length ? `<div class="jp-pg">${loose.map(row).join('')}</div>` : '');
}
const stText = (no) => { const s = statusOf(no); return s === 'na' ? '' : t(s === 'ready' ? 'jf1.res.ready' : s === 'need' ? 'jf1.res.need' : 'jf1.res.held'); };

/* ---- viewer tabs ---- */
function drawing(v, part) {
  const pg = (order.pages || []).find((p) => p.page_no === part.page);
  if (!pg || !pg.has_image) { v.innerHTML = `<p class="jf1-note">${esc(t('jf2.pro.nodrawing'))}</p>`; return; }
  v.innerHTML = `<div class="jp-draw"><img class="jp-img" alt="${esc(t('jf2.pro.pdf', { n: pg.page_no }))}" data-op-page="${pg.page_no}" data-op-width="${pg.width}" data-op-height="${pg.height}" src="/api/orders/${oid}/pages/${pg.page_no}"></div>`;
  const wrap = v.querySelector('.jp-draw');
  for (const val of part.values || []) {
    if (!Array.isArray(val.bbox) || val.bbox.length !== 4) continue;
    const [x0, y0, x1, y1] = val.bbox;
    const b = document.createElement('i');
    b.className = `jp-box${val.key === sel.key ? ' is-lit' : ''}`;
    b.dataset.key = val.key;
    b.style.left = `${(x0 / pg.width) * 100}%`; b.style.top = `${(y0 / pg.height) * 100}%`;
    b.style.width = `${((x1 - x0) / pg.width) * 100}%`; b.style.height = `${((y1 - y0) / pg.height) * 100}%`;
    wrap.append(b);
  }
}

function cut(v, part) {
  const pieces = rp(part.no)?.pattern || [];
  const pts = pieces.flatMap((p) => p.outline || []);
  if (!res.length) { v.innerHTML = `<p class="jf1-note">${esc(t('jf2.pro.noresult'))}</p>`; return; }
  if (pts.length < 3) { v.innerHTML = `<p class="jf1-note">${esc(t('jf2.pro.nopattern'))}</p>`; return; }
  const xs = pts.map((p) => p[0]), ys = pts.map((p) => p[1]);
  const x0 = Math.min(...xs), x1 = Math.max(...xs), y0 = Math.min(...ys), y1 = Math.max(...ys);
  const pad = Math.max(x1 - x0, y1 - y0) * 0.05 || 1;
  const svg = document.createElementNS(NS, 'svg');
  svg.setAttribute('class', 'jp-svg');
  svg.setAttribute('role', 'img');
  svg.setAttribute('aria-label', t('jf2.pro.tab.cut'));
  svg.setAttribute('viewBox', `${x0 - pad} ${-(y1 + pad)} ${x1 - x0 + 2 * pad} ${y1 - y0 + 2 * pad}`);
  const g = document.createElementNS(NS, 'g');
  g.setAttribute('transform', 'scale(1,-1)');
  const poly = (list, cls) => { const e = document.createElementNS(NS, 'polygon'); e.setAttribute('points', list.map((p) => p.join(',')).join(' ')); e.setAttribute('class', cls); e.setAttribute('vector-effect', 'non-scaling-stroke'); g.append(e); };
  for (const p of pieces) {
    if ((p.outline || []).length >= 3) poly(p.outline, 'jp-out');
    for (const h of p.holes || []) if (h.length >= 3) poly(h, 'jp-hole');
    for (const c of p.circles || []) { const e = document.createElementNS(NS, 'circle'); e.setAttribute('cx', c[0]); e.setAttribute('cy', c[1]); e.setAttribute('r', c[2]); e.setAttribute('class', 'jp-hole'); e.setAttribute('vector-effect', 'non-scaling-stroke'); g.append(e); }
  }
  svg.append(g);
  v.append(svg);
  const cap = document.createElement('p');
  cap.className = 'jf1-note mono';
  cap.textContent = pieces.map((p) => `${p.label || p.kind}: ${p.w != null ? p.w.toFixed(1) : '?'} × ${p.h != null ? p.h.toFixed(1) : '?'} mm × ${p.copies}`).join('  |  ');
  v.append(cap);
}

async function view3d(v, part) {
  const piece = (rp(part.no)?.pattern || []).find((p) => (p.outline || []).length >= 3);
  const thick = rp(part.no)?.t_mm;
  if (!piece || !(thick > 0)) { v.innerHTML = `<p class="jf1-note">${esc(res.length ? t('jf2.pro.nopattern') : t('jf2.pro.noresult'))}</p>`; return; }
  let gl = false;
  try { const c = document.createElement('canvas'); gl = !!(c.getContext('webgl2') || c.getContext('webgl')); } catch { gl = false; }
  if (!gl) { v.innerHTML = `<p class="jf1-note" role="status">${esc(t('jf2.pro.nowebgl'))}</p>`; return; }
  v.innerHTML = `<div class="jp-3d" data-3d></div><p class="jf1-note">${esc(t('jf2.pro.3d.note', { t: thick }))}</p>`;
  const mine = ++threeToken;
  let T;
  try { T = await import('../../vendor/three/three.module.js'); } catch { v.innerHTML = `<p class="jf1-note" role="status">${esc(t('jf2.pro.nowebgl'))}</p>`; return; }
  const host = v.querySelector('[data-3d]');
  if (mine !== threeToken || !host) return;
  const shape = new T.Shape(piece.outline.map((p) => new T.Vector2(p[0], p[1])));
  for (const h of piece.holes || []) if (h.length >= 3) shape.holes.push(new T.Path(h.map((p) => new T.Vector2(p[0], p[1]))));
  for (const c of piece.circles || []) { const p = new T.Path(); p.absarc(c[0], c[1], c[2], 0, Math.PI * 2, true); shape.holes.push(p); }
  const geo = new T.ExtrudeGeometry(shape, { depth: thick, bevelEnabled: false });
  geo.computeBoundingBox(); geo.center();
  const size = geo.boundingBox.getSize(new T.Vector3());
  const mesh = new T.Mesh(geo, new T.MeshStandardMaterial({ color: 0x9fb0c0, metalness: 0.4, roughness: 0.5 }));
  const scene = new T.Scene();
  scene.add(mesh, new T.AmbientLight(0xffffff, 0.7));
  const sun = new T.DirectionalLight(0xffffff, 1.2); sun.position.set(1, 1.5, 2); scene.add(sun);
  const cam = new T.PerspectiveCamera(35, 2, 0.1, 1e7);
  const R = Math.max(size.x, size.y, size.z) * 2.2;
  const renderer = new T.WebGLRenderer({ antialias: true, alpha: true });
  host.append(renderer.domElement);
  const fit = () => { const w = host.clientWidth || 600, h = Math.max(260, Math.round(w * 0.55)); renderer.setSize(w, h, false); renderer.domElement.setAttribute('width', w); cam.aspect = w / h; cam.updateProjectionMatrix(); };
  const calm = matchMedia('(prefers-reduced-motion: reduce)').matches;
  let raf = 0, a = 0.6;
  const draw = () => { cam.position.set(Math.sin(a) * R, R * 0.55, Math.cos(a) * R); cam.lookAt(0, 0, 0); renderer.render(scene, cam); };
  const tick = () => { a += 0.008; draw(); raf = requestAnimationFrame(tick); };
  fit(); draw();
  if (!calm) tick();
  addEventListener('resize', fit);
  stop3d = () => { cancelAnimationFrame(raf); removeEventListener('resize', fit); geo.dispose(); renderer.dispose(); renderer.domElement.remove(); };
}
let threeToken = 0;

async function cad(v) {
  const first = (order.files || [])[0];
  let installed = false;
  try { const r = await fetch('/cad', { method: 'HEAD', credentials: 'same-origin', headers: { 'X-NF-Request': '1' } }); installed = r.ok && !/text\/html/i.test(r.headers.get('content-type') || ''); } catch { installed = false; }
  if (!v.isConnected) return;
  v.innerHTML = installed && first
    ? `<p>${esc(t('jf2.pro.cad.text'))}</p><a class="jf1-btn jf1-primary jf1-link" target="_blank" rel="noopener" href="/cad?doc=${encodeURIComponent(first.document_id)}">${esc(t('jf2.pro.cad.open'))}</a>`
    : `<p class="jf1-note" role="status">${esc(t('jf2.pro.cad.note'))}</p>`;
}

/* ---- inspector ---- */
function inspector() {
  const box = $('[data-insp]');
  const part = (order.parts || []).find((p) => p.no === sel.no);
  if (!part) { box.innerHTML = `<p class="jf1-note">${esc(t('jf2.pro.nopart'))}</p>${overridesList()}`; wireOv(box); return; }
  const r = rp(part.no);
  const chips = (part.values || []).map((v) => `<button type="button" class="jp-chip is-${esc(v.state.toLowerCase())}" data-chip="${esc(v.key)}" aria-pressed="${v.key === sel.key}"><span>${esc(what(v.key))}</span> <b class="mono">${v.value ?? '—'}</b> <small>${esc(has(`jf2.vs.${v.state}`) ? t(`jf2.vs.${v.state}`) : v.state)}</small></button>`).join('');
  box.innerHTML = `<h2><span class="mono">${esc(part.no)}</span> ${esc(part.name)}</h2>
    <h3>${esc(t('jf2.pro.values'))}</h3><div class="jp-chips">${chips || '—'}</div>
    <h3>${esc(t('jf2.pro.review'))}</h3>${reviewHtml(part, r)}
    <div class="jp-qs" data-qs></div>
    ${overridesList()}`;
  renderQuestions(box.querySelector('[data-qs]'), oid, questionsOf(order, part.no), { onAnswered: refresh });
  wireOv(box);
}
const has = (k) => t(k) !== k;

function reviewHtml(part, r) {
  if (!r) return `<p class="jf1-note">${esc(t('jf2.pro.noresult'))}</p>`;
  const rows = [];
  const mine = (ov.overrides || []).filter((o) => o.part === part.no && o.active);
  for (const h of r.holds || []) {
    if (h.level !== 'need') continue;
    const done = mine.some((o) => o.code === h.code);
    const can = (ov.allow_list || []).includes(h.code) && !done;
    rows.push({ kind: 'jf2.pro.hold', code: h.code, text: ja() ? h.message_ja : h.message_en, can, done });
  }
  for (const q of r.questions || []) rows.push({ kind: 'jf2.pro.question', code: q.code, text: q.message, can: false });
  if (r.refusal) rows.push({ kind: 'jf2.pro.refused', code: r.refusal.code, text: r.refusal.message, can: false });
  if (!rows.length) return `<p class="jf1-note">${esc(t('jf2.pro.review.none'))}</p>`;
  return `<ul class="jp-rows">${rows.map((x) => `<li class="jp-row"><p><b>${esc(t(x.kind))}</b> <span class="mono">${esc(x.code)}</span></p><p>${esc(x.text || '')}</p>${x.can ? `<button type="button" class="jf1-btn" data-ov-open="${esc(x.code)}" data-ov-part="${esc(part.no)}">${esc(t('jf2.ov.btn'))}</button>` : ''}${x.done ? `<p class="jf1-note">${esc(t('jf2.ov.row', { code: x.code, part: part.no, by: mine.find((o) => o.code === x.code).by, at: stamp(mine.find((o) => o.code === x.code).at), reason: mine.find((o) => o.code === x.code).reason }))}</p>` : ''}<div data-ovform="${esc(x.code)}"></div></li>`).join('')}</ul>`;
}

function openOverride(b) {
  const code = b.dataset.ovOpen, part = b.dataset.ovPart;
  const slot = b.parentElement.querySelector(`[data-ovform="${CSS.escape(code)}"]`);
  if (slot.firstChild) { slot.innerHTML = ''; return; }
  slot.innerHTML = `<form class="jp-ov"><p class="jf1-note">${esc(t('jf2.ov.explain'))}</p>
    <label>${esc(t('jf2.ov.reason', { n: ov.min_reason_chars || 20 }))}<textarea rows="3" class="jp-ta" required></textarea></label>
    <p class="jf1-err" role="alert" hidden></p>
    <div class="jf1-actions"><button type="submit" class="jf1-btn jf1-primary">${esc(t('jf2.ov.confirm'))}</button><button type="button" class="jf1-btn" data-cancel>${esc(t('jf2.ov.cancel'))}</button></div></form>`;
  const form = slot.querySelector('form'), err = form.querySelector('.jf1-err');
  form.querySelector('[data-cancel]').addEventListener('click', () => { slot.innerHTML = ''; });
  form.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const reason = form.querySelector('textarea').value.trim();
    if ([...reason].length < (ov.min_reason_chars || 20)) { err.hidden = false; err.textContent = t('jf2.ov.short', { n: ov.min_reason_chars || 20 }); return; }
    try { await api(`/api/orders/${oid}/overrides`, { json: { part, code, reason } }); await refresh(); }
    catch (e) { err.hidden = false; err.textContent = msgOf(e); }
  });
  form.querySelector('textarea').focus();
}

async function undoOverride(id, b) {
  b.disabled = true;
  try { await api(`/api/orders/${oid}/overrides/${id}/undo`, { json: {} }); await refresh(); } catch (e) { b.disabled = false; b.insertAdjacentText('afterend', ` ${msgOf(e)}`); }
}

function overridesList() {
  const list = ov.overrides || [];
  const rows = list.map((o) => `<li>${esc(t('jf2.ov.row', { code: o.code, part: o.part, by: o.by, at: stamp(o.at), reason: o.reason }))}
    ${o.undone_at ? `<small>${esc(t('jf2.ov.undone'))}</small>` : o.stale ? `<small>${esc(t('jf2.ov.stale'))}</small>` : `<button type="button" class="jf1-btn" data-ov-undo="${o.id}">${esc(t('jf2.ov.undo'))}</button>`}</li>`).join('');
  return `<h3>${esc(t('jf2.ov.list'))}</h3>${list.length ? `<ul class="jp-ovl">${rows}</ul><p class="jf1-note mono">${esc(ov.mark || '')}</p>` : `<p class="jf1-note">${esc(t('jf2.ov.none'))}</p>`}`;
}
function wireOv() { /* clicks are delegated from the root */ }

/* ---- rules (what-if only) ---- */
async function loadRules() {
  try { rules = await api(`/api/orders/${oid}/rules`); } catch (e) { rules = { error: msgOf(e) }; }
  paint();
}
const RULE_KEYS = ['basis', 'allow', 'seam_deg', 'stock_margin'];
function rulesHtml() {
  if (!rules) return `<p class="jf1-note">…</p>`;
  if (rules.error) return `<p class="jf1-err" role="alert">${esc(rules.error)}</p>`;
  const rows = (rules.effective || []).map((r) => `<tr><td class="mono">${esc(r.name)}</td><td class="mono">${esc(r.value)}</td><td>${esc((ja() ? r.source_ja : r.source) || '')}</td></tr>`).join('');
  return `<h2>${esc(t('jf2.rules.title'))}</h2>
    <table class="jp-tbl"><thead><tr><th>${esc(t('jf2.rules.rule'))}</th><th>${esc(t('jf2.rules.value'))}</th><th>${esc(t('jf2.rules.src'))}</th></tr></thead><tbody>${rows}</tbody></table>
    <h3>${esc(t('jf2.rules.try'))}</h3>
    <form class="jp-rform"><label>${esc(t('jf2.rules.rule'))}<select class="jp-sel">${RULE_KEYS.map((k) => `<option>${k}</option>`).join('')}</select></label>
    <label>${esc(t('jf2.rules.value'))}<input class="jp-in mono" type="text" autocomplete="off" required></label>
    <button type="submit" class="jf1-btn">${esc(t('jf2.rules.run'))}</button></form>
    <div data-impact aria-live="polite">${impact ? impactHtml() : ''}</div>`;
}
function impactHtml() {
  if (impact.error) return `<p class="jf1-err" role="alert">${esc(impact.error)}</p>`;
  const s = impact.impact?.summary;
  if (!s || !(s.affected || []).length) return `<p class="jf1-note">${esc(t('jf2.rules.none'))}</p>`;
  const parts = (impact.impact.parts || []).map((p) => `<li><span class="mono">${esc(p.no)}</span> ${esc(p.name)}: ${esc(p.state_before)} → ${esc(p.state_after)}${p.mass_changed ? ` · ${p.mass_before ?? '—'} → ${p.mass_after ?? '—'} kg` : ''}</li>`).join('');
  return `<p>${esc(ja() ? s.text_ja : s.text_en)}</p><ul class="jp-ovl">${parts}</ul>`;
}
function wireRules() {
  const f = $('.jp-rform');
  if (!f) return;
  f.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    const key = f.querySelector('select').value;
    const raw = f.querySelector('input').value.trim();
    const val = key === 'basis' ? raw : Number(raw);
    const body = { rules: { ...(rules.rules || {}), [key]: val }, parts_hash: rules.parts_hash, outlines: false };
    try { impact = await api(`/api/orders/${oid}/rules/impact`, { json: body }); } catch (e) { impact = { error: msgOf(e) }; }
    const out = $('[data-impact]');
    if (out) out.innerHTML = impactHtml();
  });
}

/* ---- live: AMOEBA's blob cursor, the controls, and a refresh when something changed ---- */
async function refresh() {
  if (!root) return;
  try { await load(); } catch { return; }
  sidebar(); inspector();
  if (pmode === 'parts' && (tab === 'cut' || tab === '3d')) paint();
}

function live() {
  cursor = createCursor({
    root,
    look: 'blob',
    onPoll(d) {
      controls?.update(d);
      if (lastSeq >= 0 && d.last_seq !== lastSeq) { clearTimeout(timer); timer = setTimeout(refresh, 400); }
      lastSeq = d.last_seq;
    },
  });
  controls = mountLiveControls($('[data-controls]'), { oid, cursor, onChange: (act) => { if (act === 'undo') refresh(); } });
  stopFollow = cursor.follow(oid);
}
