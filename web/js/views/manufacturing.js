import { api, esc } from '../api.js';
import { t, h, idLabel, lang } from '../i18n.js';
import { $, $$ } from '../ui.js';

let root = null;
let ac = null;
let projects = [];

const ja = () => lang() === 'ja';
const mono = (s) => `<span class="mono">${esc(s)}</span>`;
const ud = (v) => `<bdi>${esc(v)}</bdi>`;
const jaHtml = (s) => esc(s).replace(/`([^`]+)`/g, '<span class="mono">$1</span>');
const chipHtml = (kind, text) => `<span class="chip${kind ? ` ${kind}` : ''}">${esc(text)}</span>`;
const short = (hash) => `<span class="mono" title="${esc(hash)}">${esc(String(hash).slice(0, 12))}…</span>`;

// Display rule kept from the old client: 3 decimals with thousands separators, full precision in the title.
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : String(n ?? ''));

const idText = (kind, id) => idLabel(kind, id) || (ja() ? String(id) : String(id).replaceAll('_', ' '));
const famShort = (k) => esc(idText('family', k));
const partName = (p) => (ja() && idLabel('part', p) ? `${esc(idLabel('part', p))} ${mono(p)}` : esc(p));
const partText = (p) => (ja() && idLabel('part', p) ? `${idLabel('part', p)} ${p}` : String(p));

const STATES = ['APPROVED', 'REJECTED', 'SUPERSEDED', 'NO_RESULT', 'REVIEW_REQUIRED', 'REFUSED'];
const STATE_KIND = { APPROVED: 'ok', REJECTED: 'bad', SUPERSEDED: '', NO_RESULT: 'bad' };
const stateText = (s) => (STATES.includes(s) ? t(`state.${s}`) : String(s).replaceAll('_', ' '));
const stateChip = (s) => chipHtml(STATE_KIND[s] === undefined ? 'warn' : STATE_KIND[s], stateText(s));

const extent = (p) => {
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const k of [...p.curves, ...p.holes]) for (const [x, y] of k.points) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; }
  return { w: x1 - x0, h: y1 - y0 };
};
const summarise = (c) => ({
  noResult: !c.patterns.length,
  superseded_by: c.superseded_by,
  released: c.state === 'REVIEW_REQUIRED',
  parts: c.patterns.map((p) => ({ part: p.part, cls: p.class, sha: p.dxf_sha256, ...extent(p), curves: p.curves.length, holes: p.holes.length })),
});
// Same precedence as every other list: superseded > no result > the server state.
const effState = (row) => (row.superseded || (row.facts && row.facts.superseded_by) ? 'SUPERSEDED'
  : row.no_result || (row.facts && row.facts.noResult) ? 'NO_RESULT' : row.state);

const pname = (id) => { const p = projects.find((x) => String(x.id) === String(id)); return p ? p.name : null; };
const pnameHtml = (id) => { const n = pname(id); return n === null ? esc(t('project.n', { id })) : ud(n); };
const plink = (id) => `<a href="#/project/${esc(id)}" title="${esc(t('project.tip', { id }))}">${pnameHtml(id)}</a>`;
const jobLink = (id) => `<a href="#/job/${esc(id)}">${esc(t('job.n', { id }))}</a>`;

const panel = (title, body, o = {}) => `<section class="panel"${o.id ? ` id="${o.id}"` : ''}${o.aria ? ` aria-label="${esc(o.aria)}"` : ''}>
  <header class="panel-h"><h2>${title}</h2>${o.tag || ''}${o.count !== undefined ? `<span class="cnt">${esc(o.count)}</span>` : ''}${o.tools ? `<div class="row">${o.tools}</div>` : ''}</header>
  <div class="panel-b">${body}</div>${o.foot ? `<div class="panel-b sub">${o.foot}</div>` : ''}</section>`;

const emptyBox = (title, text) => `<div class="card"><b>${esc(title)}</b><p class="sub">${esc(text)}</p></div>`;

// Capabilities without a backend are labelled "Planned — not implemented"; nothing here is mocked.
const plannedPanel = () => {
  const title = t('mfg.planned.t');
  const items = Array.from({ length: 7 }, (_, i) => t(`mfg.planned.${i + 1}`));
  return panel(esc(title), `<p class="sub">${esc(t('planned.sub'))}</p><ul class="il">${items.map((i) => `<li>${jaHtml(i)}</li>`).join('')}</ul>`,
    { tag: chipHtml('', t('planned.tag')), count: items.length, aria: t('planned.aria', { title }) });
};

const filterBox = () => `<div class="field"><label class="sub" for="mfg-filter">${esc(t('mfg.filter'))}</label>
  <input id="mfg-filter" type="search" placeholder="${esc(t('mfg.filter'))}" autocomplete="off"></div>`;

// A file is offered only where the server hands it out; a superseded or rejected job states why, read-only.
const dxfCell = (c, p) => {
  const s = effState(c);
  if (s === 'SUPERSEDED') return `<span class="sub">${c.facts.superseded_by
    ? h('mfg.w.sup.by', { job: `<a href="#/job/${esc(c.facts.superseded_by)}">#${esc(c.facts.superseded_by)}</a>` })
    : esc(t('mfg.w.sup'))}</span>`;
  if (s === 'REJECTED') return `<span class="sub">${esc(t('mfg.w.rej'))}</span>`;
  if (!c.facts || !c.facts.released) return `<span class="sub">${esc(t('mfg.w.held'))}</span>`;
  return `<a class="btn" href="/api/calculations/${esc(c.id)}/dxf/${encodeURIComponent(p.part)}" download
    aria-label="${esc(t('mfg.dl.aria', { id: c.id, part: partText(p.part) }))}">${esc(t('mfg.dl'))}</a>`;
};

const jobsTable = (rows) => `<table class="table" id="tbl-dxf"><thead><tr>
  <th>${esc(t('th.job'))}</th><th>${esc(t('th.project'))}</th><th>${esc(t('th.part'))}</th>
  <th>${esc(t('th.extent'))}</th><th>${esc(t('th.curves'))}</th><th>${esc(t('th.dxfsha'))}</th>
  <th>${esc(t('th.jobstate'))}</th><th>${esc(t('th.file'))}</th></tr></thead><tbody>${rows.map(({ c, p }) => `<tr${effState(c) === 'SUPERSEDED' ? ' class="sub"' : ''}>
    <td>${jobLink(c.id)}</td><td>${plink(c.project_id)}</td>
    <td>${partName(p.part)} <span class="sub mono">${esc(p.cls)}</span><br><span class="sub">${famShort(c.family)}</span></td>
    <td class="mono" title="${esc(`${fmt(p.w)} × ${fmt(p.h)} mm`)}">${n3(p.w)} × ${n3(p.h)}</td>
    <td class="mono">${p.curves}${p.holes ? ` ${esc(t('mfg.opening', { n: p.holes }))}` : ''}</td>
    <td>${short(p.sha)}</td><td>${stateChip(effState(c))}</td><td>${dxfCell(c, p)}</td></tr>`).join('')}</tbody></table>`;

function render(all) {
  const rows = all.filter((c) => c.facts && !c.facts.noResult).flatMap((c) => c.facts.parts.map((p) => ({ c, p })));
  const refused = all.filter((c) => c.facts && c.facts.noResult).length;
  const foot = jaHtml(t('mfg.foot')) + (refused ? ` ${esc(t('mfg.foot.refused', { n: refused }))}` : '');
  root.innerHTML = `<h1>${esc(t('nav.manufacturing'))}</h1><p class="sub">${esc(t('mfg.sub'))}</p>
    <div class="alert">${h('mfg.note')}</div>
    ${panel(esc(t('mfg.panel')), rows.length ? jobsTable(rows) : emptyBox(t('mfg.none.t'), t('mfg.none.p')),
    { id: 'p-dxf', count: rows.length, tools: rows.length ? filterBox() : '', foot })}
    ${plannedPanel()}`;
  wireFilter();
}

function wireFilter() {
  const inp = $('#mfg-filter', root);
  const body = $('#tbl-dxf tbody', root);
  if (!inp || !body) return;
  const cnt = $('#p-dxf .cnt', root);
  const total = $$('tr', body).length;
  const run = () => {
    const q = inp.value.trim().toLowerCase();
    const old = $('tr[data-nomatch]', body); if (old) old.remove();
    let shown = 0;
    $$('tr', body).forEach((tr) => { tr.hidden = !!q && !tr.textContent.toLowerCase().includes(q); if (!tr.hidden) shown++; });
    if (cnt) cnt.textContent = q ? t('flt.count', { a: shown, b: total }) : String(total);
    if (q && !shown) {
      body.insertAdjacentHTML('beforeend', `<tr data-nomatch><td colspan="8">${h('flt.nomatch', { q: ud(inp.value.trim()) })}
        <button class="btn btn-text" type="button">${esc(t('flt.clear'))}</button></td></tr>`);
      $('tr[data-nomatch] button', body).onclick = () => { inp.value = ''; run(); inp.focus(); };
    }
  };
  inp.addEventListener('input', run);
}

function renderError(err) {
  const msg = err && err.message
    ? `${ja() ? `${chipHtml('warn', t('srv.tag'))} ` : ''}<span${ja() ? ' lang="en"' : ''}>${esc(err.message)}</span>`
    : esc(t('api.unreachable'));
  root.innerHTML = `<h1>${esc(t('nav.manufacturing'))}</h1>
    <div class="alert bad"><b>${esc(t('err.t'))}</b><p>${msg}</p>
    <div class="row"><button class="btn btn-primary" type="button" id="mfg-retry">${esc(t('retry'))}</button></div></div>`;
  $('#mfg-retry', root).onclick = () => { const c = root; unmount(); mount(c); };
}

async function load(signal) {
  const [all, ps] = await Promise.all([api('/api/calculations'), api('/api/projects')]);
  if (signal.aborted) return null;
  projects = Array.isArray(ps) ? ps : [];
  const list = Array.isArray(all) ? all : [];
  const facts = await Promise.all(list.map((c) => api(`/api/calculations/${c.id}`).then(summarise, () => null)));
  if (signal.aborted) return null;
  list.forEach((c, i) => { c.facts = facts[i]; });
  return list;
}

export async function mount(container, params = {}) {
  unmount();
  root = container;
  ac = new AbortController();
  const signal = ac.signal;
  root.innerHTML = `<h1>${esc(t('nav.manufacturing'))}</h1><p class="sub">${esc(t('mfg.sub'))}</p>
    <p class="sub" role="status">${esc(t('loading'))}</p>`;
  try {
    const all = await load(signal);
    if (signal.aborted || root !== container) return;
    render(all || []);
  } catch (err) {
    if (signal.aborted || root !== container) return;
    renderError(err);
  }
}

export function unmount() {
  if (ac) { ac.abort(); ac = null; }
  if (root) root.innerHTML = '';
  root = null;
  projects = [];
}
