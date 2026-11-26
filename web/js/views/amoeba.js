import { mountAmoebas, unmountAmoebas } from '../lib/amoeba-mount.js';
const esc = (v) => String(v ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
import { api } from '../api.js';
import { t, h, idLabel, lang } from '../i18n.js';
import { $, $$ } from '../ui.js';

const ja = () => lang() === 'ja';
const words = (s) => esc(String(s ?? '').replaceAll('_', ' '));
const mono = (s) => `<span class="mono">${esc(s)}</span>`;
const ud = (v) => `<bdi>${esc(v)}</bdi>`;
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : esc(n));
const pad2 = (n) => String(n).padStart(2, '0');
const when = (ts) => { if (!ts) return '—'; const d = new Date(ts * 1000); return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`; };
const byWhen = (who, ts) => `${ud(who)}<br><span class="sub">${when(ts)}</span>`;
const CHIP = { ok: 'ok', warn: 'warn', bad: 'bad' };
const tag = (kind, text) => `<span class="chip ${CHIP[kind] || ''}">${esc(text)}</span>`;
const panel = (title, body, o = {}) => `<section class="panel"><div class="panel-h"><h2>${title}</h2>${o.count !== undefined ? `<span class="cnt">${esc(o.count)}</span>` : ''}${o.tools || ''}</div>${body}${o.foot ? `<div class="panel-b sub">${o.foot}</div>` : ''}</section>`;
const tbl = (id, heads, rows) => `<div class="panel-b tbl-sticky"><table class="table"${id ? ` id="${id}"` : ''}><thead><tr>${heads.map((x) => `<th>${x}</th>`).join('')}</tr></thead><tbody>${rows.join('')}</tbody></table></div>`;
const empty = (title, text) => `<div class="panel-b"><div class="alert"><b>${esc(title)}</b><p class="sub">${text}</p></div></div>`;
const filterBox = (target, label) => `<div class="search"><label class="vh" for="flt-${target}">${esc(label)}</label><svg class="icon" aria-hidden="true"><use href="#i-search"/></svg><input id="flt-${target}" type="search" data-filter="${target}" placeholder="${esc(label)}" autocomplete="off"></div>`;
const errHtml = (e) => (e && e.key ? esc(t(e.key, e.params)) : e && e.message ? srv(e.message) : esc(String(e)));

// identifiers the engine or server owns: reviewed label in Japanese with the identifier beside it
const idText = (kind, id) => idLabel(kind, id) || (ja() ? String(id) : String(id).replaceAll('_', ' '));
const named = (kind, id) => { const l = idLabel(kind, id); if (!ja()) return l ? esc(l) : words(id); return l ? `${esc(l)}<span class="mono">${esc(id)}</span>` : mono(id); };
const labelled = (kind, id) => { const l = idLabel(kind, id); return l ? esc(l) : ja() ? mono(id) : words(id); };
const jaHtml = (s) => esc(s).replace(/`([^`]+)`/g, '<span class="mono">$1</span>');
// engine/server prose: translated only when the whole statement is a known template, never in part
const eng = (text, key = 'eng.tag') => {
  const s = String(text ?? ''); if (!s) return '';
  if (!ja()) return `<span class="eng" lang="en">${esc(s)}</span>`;
  const j = engineJaSafe(s);
  return j ? `<span title="${esc(s)}">${jaHtml(j)}</span>` : `${tag('na', t(key))} <span lang="en">${esc(s)}</span>`;
};
const srv = (text) => eng(text, 'srv.tag');
let engineJaSafe = () => null;
const nodeParts = (l) => { const i = l.indexOf(' · '); return i < 0 ? ['', l] : [l.slice(0, i), l.slice(i + 3)]; };
const nodeName = (kind, name) => (ja() ? (kind === 'parameter' || kind === 'convention' ? idLabel('param', name) : kind ? idLabel('part', name) : null) || idLabel('dlabel', name) || idLabel('rkind', name) : null);
const nodeLabel = (l) => { const [k, n] = nodeParts(l); if (!ja()) return mono(l); const jn = nodeName(k, n); return `${k ? `${labelled('rkind', k)} · ` : ''}${jn ? `${esc(jn)} ` : ''}${mono(n)}`; };
const oblSubject = (o) => `${labelled('rkind', o.kind)} · ${ja() && nodeName(o.kind, o.label) ? `${esc(nodeName(o.kind, o.label))} ` : ''}${mono(o.label)}`;
const oblNote = (o) => (o.resolved_by ? ud(o.note) : eng(o.note));
const OBL = ['open', 'discharged', 'discharged_by_decision', 'waived'];
const oblText = (s) => (OBL.includes(s) ? t(`obl.${s}`) : String(s).replaceAll('_', ' '));
const oblTag = (s) => tag(s === 'open' || s === 'waived' ? 'warn' : 'ok', oblText(s));
const dirs = (d) => (['downstream', 'lateral', 'common_cause'].includes(d) ? t(`dirs.${d}`) : String(d).replaceAll('_', ' '));
const famShort = (k) => esc(idText('family', k));
const jobLink = (id, lc) => `<a href="#/job/${id}">${esc(t(lc ? 'job.n.lc' : 'job.n', { id }))}</a>`;

let amHandles = [], amSeq = 0;
let root = null, body = null, ac = null, data = null, projects = [], tabDefs = [], activeTab = 'records', depId = null, graph = null;
const pnameHtml = (id) => { const p = projects.find((x) => x.id === id); return p ? ud(p.name) : esc(t('project.n', { id })); };
const plink = (id) => `<a href="#/project/${id}" title="${esc(t('project.tip', { id }))}">${pnameHtml(id)}</a>`;
const aborted = () => !ac || ac.signal.aborted;

const lessonsTable = (lessons) => (lessons.length ? tbl('', ['#', esc(t('th.what')), esc(t('th.fromjob')), esc(t('th.matched')), esc(t('th.recorded'))], lessons.map((l) => `<tr><td>${l.id}</td>
  <td>${l.kind === 'corrected_value' ? `${h('lesson.corrected', { fam: famShort(l.family), param: `<b class="mono">${esc(l.parameter)}</b>`, wrong: mono(fmt(l.wrong_value)), right: mono(fmt(l.right_value)) })}<br><span class="sub">${ud(l.detail)}</span>` : h('lesson.other', { kind: named('lesson', l.kind), fam: famShort(l.family) })}</td>
  <td>${jobLink(l.source_calc)}<br><span class="sub">${pnameHtml(l.project_id)}</span></td><td>${l.times_matched}</td><td>${byWhen(l.created_by, l.created_at)}</td></tr>`))
  : empty(t('lessons.none.t'), esc(t('lessons.none.p'))));

const edgeTable = (edges) => `<details class="panel"><summary>${esc(t('dg.edges', { n: edges.length }))}</summary>${tbl('', [esc(t('th.record')), esc(t('th.relation')), esc(t('th.dependson'))], edges.map((e) => `<tr><td>${nodeLabel(e.from)}</td><td>${ja() && idLabel('rel', e.rel) ? `${esc(idLabel('rel', e.rel))}<br>` : ''}${mono(e.rel)}</td><td>${nodeLabel(e.to)}</td></tr>`))}</details>`;

// layered dependency graph, drawn from the real edge list; presentation attributes only (no inline style)
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
  const levelName = (d) => (d === 0 ? t('dg.level0') : t('dg.level', { n: d }));
  const idle = `${h('dg.idle', { amber: esc(t('dg.amber')), blue: esc(t('dg.blue')) })} ${esc(t('dg.flow.note'))}`;
  const nodeAria = (n) => esc(t('dg.node.aria', { label: n.l, a: n.out.length, b: n.inn.length }));
  const kindText = (k) => (ja() ? idLabel('rkind', k) || k : k);
  const clip = (s, n) => (s.length > n ? s.slice(0, n - 1) + '…' : s);
  const NW = 176, NH = 40, CG = 52, RG = 10, P = 14, HD = 24;
  const rows = Math.max(...cols.map((c) => c.length)), W = P * 2 + cols.length * NW + (cols.length - 1) * CG, H = P * 2 + HD + rows * (NH + RG);
  N.forEach((n) => { n.x = P + n.d * (NW + CG); n.y = P + HD + n.row * (NH + RG) + (rows - cols[n.d].length) * (NH + RG) / 2; });
  const txt = (x, y, s, size, op) => `<text x="${x}" y="${y}" font-size="${size}" fill="currentColor"${op ? ` opacity="${op}"` : ''}>${esc(s)}</text>`;
  const html = `<div class="panel-b" role="group" aria-label="${esc(t('dg.graph.aria', { n: N.size, e: E.length }))}"><svg data-dg width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="list">
    ${cols.map((c, d) => txt(P + d * (NW + CG), P + 10, levelName(d), 11, 0.7)).join('')}
    ${E.map((e, i) => { const x1 = e.b.x + NW, y1 = e.b.y + NH / 2, x2 = e.a.x, y2 = e.a.y + NH / 2, dx = Math.max(18, (x2 - x1) / 2); return `<path data-e="${i}" d="M${x1} ${y1}C${x1 + dx} ${y1} ${x2 - dx} ${y2} ${x2} ${y2}" fill="none" stroke="currentColor" stroke-width="1" opacity="0.45"><title>${esc(e.a.l)} —${esc(e.rel)}→ ${esc(e.b.l)}</title></path>`; }).join('')}
    ${[...N.values()].map((n) => { const [kind, name] = nodeParts(n.l); const shown = clip(nodeName(kind, name) || name, ja() ? 13 : 22); return `<g data-n="${n.i}" tabindex="0" role="listitem" aria-label="${nodeAria(n)}"><title>${esc(n.l)}</title><rect x="${n.x}" y="${n.y}" width="${NW}" height="${NH}" rx="2" fill="none" stroke="currentColor" stroke-width="1"/>${kind ? txt(n.x + 9, n.y + 15, kindText(kind), 10, 0.7) + txt(n.x + 9, n.y + 31, shown, 12) : txt(n.x + 9, n.y + 25, shown, 12)}</g>`; }).join('')}
  </svg></div><div class="panel-b sub" data-dginfo aria-live="polite">${idle}</div>`;
  const wire = (host) => {
    const svg = $('[data-dg]', host), info = $('[data-dginfo]', host); if (!svg || !info) return;
    const nodes = [...N.values()];
    const closure = (n, key) => { const set = new Set(); const walk = (x) => x[key].forEach((y) => { if (!set.has(y)) { set.add(y); walk(y); } }); walk(n); return set; };
    const focus = (n) => {
      const up = n ? closure(n, 'out') : new Set(), down = n ? closure(n, 'inn') : new Set();
      $$('[data-n]', svg).forEach((g) => { const m = nodes[g.dataset.n]; g.setAttribute('opacity', !n || m === n || up.has(m) || down.has(m) ? '1' : '0.25'); $('rect', g).setAttribute('stroke-width', m === n ? '2.5' : '1'); });
      $$('[data-e]', svg).forEach((q) => { const e = E[q.dataset.e]; const hot = !!n && (((e.a === n || up.has(e.a)) && up.has(e.b)) || ((e.b === n || down.has(e.b)) && down.has(e.a))); q.setAttribute('opacity', n ? (hot ? '1' : '0.15') : '0.45'); q.setAttribute('stroke-width', hot ? '2' : '1'); });
      info.innerHTML = n ? h(down.size ? 'dg.reach' : 'dg.reach.none', { node: `<b>${nodeLabel(n.l)}</b>`, n: up.size, m: down.size, list: [...down].map((x) => nodeLabel(x.l)).join(', ') }) : idle;
    };
    let pinned = null;
    svg.addEventListener('click', (e) => { const g = e.target.closest('[data-n]'); pinned = g && nodes[g.dataset.n] !== pinned ? nodes[g.dataset.n] : null; focus(pinned); });
    svg.addEventListener('keydown', (e) => { const g = e.target.closest('[data-n]'); if (g && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); pinned = nodes[g.dataset.n]; focus(pinned); } if (e.key === 'Escape' && pinned) { e.stopPropagation(); pinned = null; focus(null); } });
    svg.addEventListener('mouseover', (e) => { const g = e.target.closest('[data-n]'); if (!pinned && g) focus(nodes[g.dataset.n]); });
    svg.addEventListener('mouseleave', () => { if (!pinned) focus(null); });
  };
  return { html, wire };
}

function recordsPanel(a) {
  const kinds = a.records_by_kind.map((r) => `<tr><td>${named('rfamily', r.family)}</td><td>${ja() && idLabel('rkind', r.kind) ? `${esc(idLabel('rkind', r.kind))}<br>${mono(r.kind)}` : mono(r.kind)}</td><td>${r.count}</td></tr>`);
  const snaps = a.recent_snapshots.map((s) => `<tr><td>${plink(s.project_id)}</td><td><span class="mono">${esc(s.snapshot)}</span></td></tr>`);
  return `<div class="row">${panel(esc(t('am.rec.h')), a.records_by_kind.length ? tbl('tbl-rec', [esc(t('th.rfamily')), esc(t('th.kind')), esc(t('th.count'))], kinds) : empty(t('am.rec.none.t'), esc(t('am.rec.none.p'))), { tools: a.records_by_kind.length ? filterBox('tbl-rec', t('am.rec.filter')) : '', foot: esc(t('am.rec.foot')) })}
    ${panel(esc(t('am.snap.h')), a.recent_snapshots.length ? tbl('', [esc(t('th.project')), esc(t('th.snapshot'))], snaps) : empty(t('am.snap.none.t'), esc(t('am.snap.none.p'))))}</div>`;
}

function activitiesPanel(a) {
  const rows = a.activities.map((r) => `<tr><td>${ja() && idLabel('rkind', r.type) ? `${esc(idLabel('rkind', r.type))}<br>${mono(r.type)}` : mono(r.type)}</td><td>${r.distinct_activities}</td><td>${r.declared_work_units}</td></tr>`);
  return panel(esc(t('am.act.h')), a.activities.length ? tbl('', [esc(t('th.acttype')), esc(t('th.distinct')), esc(t('th.wu'))], rows) : empty(t('am.act.none.t'), esc(t('am.act.none.p'))), { foot: esc(t('am.act.foot')) });
}

function obligationsPanel(a, obs) {
  const rows = obs.map((o) => `<tr><td>${o.id}</td><td>${jobLink(o.target_calc)}</td><td>${esc(dirs(o.direction))}<br><span class="sub mono">${esc(o.via)}</span></td><td>${oblSubject(o)}</td><td>${eng(o.required_check)}</td><td>${oblTag(o.state)}${o.target_superseded ? ` ${tag('na', t('obl.onsup'))}` : ''}</td><td>${oblNote(o)}${o.resolved_by ? `<br><span class="sub">${ud(o.resolved_by)} · ${when(o.resolved_at)}</span>` : ''}</td></tr>`);
  return panel(esc(t('am.obl.h')), obs.length ? tbl('tbl-obl', ['#', esc(t('th.job')), esc(t('th.direction')), esc(t('th.affected')), esc(t('th.required')), esc(t('th.state.short')), esc(t('th.evidence'))], rows) : empty(t('am.obl.none.t'), esc(t('am.obl.none.p'))),
    { count: t('am.obl.count', { a: a.obligations.open, b: a.obligations.total }), tools: obs.length ? filterBox('tbl-obl', t('am.obl.filter')) : '', foot: esc(t('am.obl.foot')) + (a.obligations.open_on_superseded ? ` ${esc(t('obl.onsup.note', { n: a.obligations.open_on_superseded }))}` : '') });
}

function dependenciesPanel(withEdges, dep, depErr) {
  const sel = withEdges.length ? `<div class="panel-b"><div class="field"><label for="depjob">${esc(t('th.job'))}</label><select id="depjob">${withEdges.map((c) => `<option value="${c.id}"${String(c.id) === String(depId) ? ' selected' : ''}>${esc(t(c.superseded ? 'am.dep.opt.sup' : 'am.dep.opt', { id: c.id, fam: idText('family', c.family) }))}</option>`).join('')}</select></div></div>` : '';
  const inner = graph ? `${graph.html}${edgeTable(dep.edges)}<div class="panel-b sub">${h('am.dep.rule', { rule: mono(dep.rule_table) })}</div>`
    : empty(t(depErr ? 'am.dep.err' : 'gr.none.t'), depErr ? errHtml(depErr) : esc(t('am.dep.none.p')));
  return panel(esc(t('am.dep.h')), sel + inner);
}

function mechanismsPanel(a) {
  const li = (xs) => `<div class="panel-b"><ul class="il">${xs.map((x) => `<li>${eng(x)}</li>`).join('')}</ul></div>`;
  return `<div class="row">${panel(esc(t('am.mech.impl')), li(a.implemented), { count: a.implemented.length })}
    <section class="panel"><div class="panel-h"><h2>${esc(t('am.mech.not'))}</h2>${tag('na', t('am.mech.tag'))}<span class="cnt">${a.not_implemented.length}</span></div>${li(a.not_implemented)}</section></div>`;
}

function panelFor(k) {
  const { a, obs, lessons, withEdges, dep, depErr } = data;
  if (k === 'activities') return activitiesPanel(a);
  if (k === 'obligations') return obligationsPanel(a, obs);
  if (k === 'lessons') return panel(esc(t('am.les.h')), lessonsTable(lessons), { count: lessons.length, foot: jaHtml(t('am.les.foot')) });
  if (k === 'dependencies') return dependenciesPanel(withEdges, dep, depErr);
  if (k === 'mechanisms') return mechanismsPanel(a);
  return recordsPanel(a);
}

function wireFilters(host) {
  $$('input[data-filter]', host).forEach((inp) => {
    const tbody = $(`#${inp.dataset.filter} tbody`, host); if (!tbody) return;
    const holder = inp.closest('.panel'), cnt = holder && $('.cnt', holder), total = cnt ? cnt.textContent : '';
    inp.addEventListener('input', () => {
      const q = inp.value.trim().toLowerCase();
      const old = $('[data-nomatch]', tbody); if (old) old.remove();
      const rows = $$('tr', tbody); let shown = 0;
      rows.forEach((tr) => { tr.hidden = !!q && !tr.textContent.toLowerCase().includes(q); if (!tr.hidden) shown++; });
      if (cnt) cnt.textContent = q ? t('flt.count', { a: shown, b: rows.length }) : total;
      if (q && !shown) {
        const tr = document.createElement('tr'); tr.dataset.nomatch = '1';
        tr.innerHTML = `<td colspan="20" class="sub">${h('flt.nomatch', { q: ud(inp.value.trim()) })} <button type="button" class="btn btn-text">${esc(t('flt.clear'))}</button></td>`;
        tbody.append(tr);
        $('button', tr).onclick = () => { inp.value = ''; inp.dispatchEvent(new Event('input')); inp.focus(); };
      }
    });
  });
}

function show(k) {
  activeTab = tabDefs.some((x) => x[0] === k) ? k : 'records';
  $$('[role=tab]', root).forEach((b) => { const on = b.dataset.tab === activeTab; b.setAttribute('aria-selected', String(on)); b.tabIndex = on ? 0 : -1; });
  $$('[data-goto]', root).forEach((b) => b.setAttribute('aria-current', String(b.dataset.goto === activeTab)));
  body.innerHTML = panelFor(activeTab);
  wireFilters(body);
  if (activeTab === 'dependencies' && graph) graph.wire(body);
  const s = $('#depjob', body);
  if (s) s.onchange = () => { depId = s.value; location.hash = `#/amoeba/dependencies/${s.value}`; };
}

function tabHash(k) { return `#/amoeba/${k}${k === 'dependencies' && depId ? `/${depId}` : ''}`; }

function renderShell() {
  // summary cards first (the same totals as the tab badges); each opens its tab
  const stats = tabDefs.filter((d) => d[2] !== undefined && d[2] !== null).map(([k, l, n, attn]) =>
    `<button type="button" class="stat${attn ? ' warn' : ''}" data-goto="${k}"><span class="stat-n count" data-count="${n}">${n}</span><span class="stat-l">${esc(l)}</span></button>`).join('');
  // the organism reflects the real totals: hold while any obligation is open, idle otherwise
  const stage = data && data.a && data.a.obligations && data.a.obligations.open > 0 ? 'hold' : 'idle';
  root.innerHTML = `<div class="workspace"><header class="page-h"><h1>${esc(t('nav.amoeba'))}</h1><p class="sub">${esc(t('am.sub'))}</p></header>
    <div class="amoeba-band panel"><div class="amoeba-mount am-overview" data-amoeba data-stage="${stage}" role="img" aria-label="${esc(t('am.stage.aria'))}"></div></div>
    <div class="stats">${stats}</div>
    <div class="tabs" role="tablist" aria-label="${esc(t('am.tabs.aria'))}">${tabDefs.map(([k, l, n, attn]) => `<button type="button" role="tab" data-tab="${k}" aria-selected="${k === activeTab}" tabindex="${k === activeTab ? 0 : -1}">${esc(l)}${n !== undefined && n !== null ? `<span class="cnt${attn ? ' warn' : ''}">${n}</span>` : ''}</button>`).join('')}</div>
    <div class="ws-content" id="am-body"></div></div>`;
  $$('[data-goto]', root).forEach((b) => b.addEventListener('click', () => { show(b.dataset.goto); if (location.hash !== tabHash(activeTab)) location.hash = tabHash(activeTab); }));
  body = $('#am-body', root);
  const list = $('[role=tablist]', root), tabs = $$('[role=tab]', list);
  list.addEventListener('click', (e) => { const b = e.target.closest('[role=tab]'); if (!b) return; show(b.dataset.tab); if (location.hash !== tabHash(activeTab)) location.hash = tabHash(activeTab); });
  list.addEventListener('keydown', (e) => {
    const i = tabs.indexOf(document.activeElement); if (i < 0) return;
    const j = e.key === 'ArrowRight' ? (i + 1) % tabs.length : e.key === 'ArrowLeft' ? (i - 1 + tabs.length) % tabs.length : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : -1;
    if (j < 0) return;
    e.preventDefault(); tabs[j].focus(); show(tabs[j].dataset.tab);
    if (location.hash !== tabHash(activeTab)) location.hash = tabHash(activeTab);
  });
  show(activeTab);
}

export async function mount(container, params = {}) {
  unmount();
  root = container; ac = new AbortController();
  const { tab, id } = params || {};
  activeTab = tab || 'records'; depId = id || null;
  root.innerHTML = `<div class="panel-b"><p class="sub">${esc(t('loading'))}</p></div>`;
  try {
    const mod = await import('../i18n.js');
    engineJaSafe = typeof mod.engineJa === 'function' ? mod.engineJa : () => null;
  } catch { engineJaSafe = () => null; }
  try {
    const [a, obs, lessons, cs, ps] = await Promise.all([api('/api/amoeba/summary'), api('/api/obligations'), api('/api/lessons'), api('/api/calculations'), api('/api/projects')]);
    if (aborted()) return;
    projects = ps || [];
    const withEdges = cs.filter((c) => c.state !== 'REJECTED' || c.superseded);
    if (!depId) depId = withEdges[0] && withEdges[0].id;
    let dep = null, depErr = null;
    // a job id that is not a whole number is 'not found', as an unknown id is; no request is sent for it
    if (depId && !/^\d+$/.test(String(depId))) depErr = { message: 'not found', status: 404 };
    else if (depId) { try { dep = await api(`/api/calculations/${depId}/dependencies`); } catch (err) { depErr = err; } }
    if (aborted()) return;
    data = { a, obs, lessons, withEdges, dep, depErr };
    graph = dep && dep.edges.length ? depGraph(dep.edges) : null;
    tabDefs = [['records', t('am.t.records'), a.records_by_kind.reduce((s, r) => s + r.count, 0)],
      ['activities', t('am.t.activities'), a.activities.reduce((s, r) => s + r.distinct_activities, 0)],
      ['obligations', t('am.t.obligations'), a.obligations.open, a.obligations.open > 0],
      ['lessons', t('am.t.lessons'), lessons.length], ['dependencies', t('am.t.dependencies')], ['mechanisms', t('am.t.mechanisms')]];
    renderShell();
    const my = ++amSeq;
    mountAmoebas(root).then((h) => { if (my === amSeq && root) amHandles = h; else unmountAmoebas(h); });
  } catch (err) {
    if (aborted()) return;
    root.innerHTML = `<div class="panel-b"><div class="alert bad"><b>${esc(t('err.t'))}</b><p class="sub">${errHtml(err)}</p>
      <button type="button" class="btn btn-primary" id="am-retry">${esc(t('retry'))}</button></div></div>`;
    const b = $('#am-retry', root); if (b) b.onclick = () => mount(container, params);
  }
}

export function unmount() {
  if (ac) { ac.abort(); ac = null; }
  amSeq++; unmountAmoebas(amHandles); amHandles = [];
  if (root) root.textContent = '';
  root = null; body = null; data = null; graph = null; projects = []; tabDefs = []; depId = null;
}
