// Offline help (#/help), ported from the old single-file client (work plan 3.5).
// Nothing here is asked of the network or the server: the inputs come from the calculation form's own field
// table (F), the gate names from the labels the job view shows (ID.gate), the refusal texts from the messages a
// refused field shows (the catalogue's refusal.* keys). One source each, in English and Japanese.
// No control here approves, releases or signs off anything: the sample projects go through the normal API, so the
// engine computes them, the server records and audits them append-only, and every sample job ends REVIEW REQUIRED.
import { api, esc } from '../api.js';
import { getState } from '../store.js';
import { t, h, has, idLabel, lang, ID_TABLE as ID, CATALOGUE } from '../i18n.js';
import { el, frag, $, toast, chip, busy } from '../ui.js';

const ja = () => lang() === 'ja';
const pad2 = (n) => String(n).padStart(2, '0');
// the old client's full-precision display of a value that was typed (not a computed result)
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : String(n ?? ''));
const words = (s) => String(s ?? '').replaceAll('_', ' ');
const monoHtml = (s) => `<span class="mono">${esc(s)}</span>`;

// ---------------------------------------------------------------- the calculation form's field table
// [key, example value, unit]; `sref` the shared wall-surface select, `ext` the optional per-part surfaces and joint.
// These are the form's own example values: no other number exists on this screen.
const F = {
  cylinder_shell: { f: [['r_dev', 500, 'mm'], ['length', 1000, 'mm']] },
  cone_frustum: { f: [['r1', 200, 'mm'], ['r2', 100, 'mm'], ['h', 300, 'mm']] },
  eccentric_reducer: { f: [['r1', 200, 'mm'], ['r2', 100, 'mm'], ['h', 300, 'mm'], ['e', 100, 'mm']] },
  header_with_branch: { f: [['header_radius', 150, 'mm'], ['header_dev_radius', 150, 'mm'], ['header_length', 1200, 'mm'], ['branch_station', 600, 'mm'], ['branch_radius', 100, 'mm'], ['branch_dev_radius', 100, 'mm'], ['beta_deg', 60, 'deg'], ['delta', 0, 'mm'], ['s_end', 400, 'mm']], sref: true, ext: true },
  branch_stub: { f: [['header_radius', 150, 'mm'], ['branch_radius', 100, 'mm'], ['branch_dev_radius', 100, 'mm'], ['beta_deg', 60, 'deg'], ['delta', 0, 'mm'], ['s_end', 400, 'mm']], sref: true },
};
const EXT_KEYS = ['header_surface_reference', 'branch_surface_reference', 'joint'];
const idText = (kind, id) => idLabel(kind, id) || (ja() ? String(id) : words(id));
const famLabel = (k) => idLabel('familyLong', k) || idText('family', k);
const plabel = (fam, key) => idLabel('param', `${fam}.${key}`) || idLabel('param', key) || (ja() ? key : words(key));
const unitText = (u) => (u === 'deg' ? t('unit.deg') : u);

// Gates G00–G22, named exactly as the job view names them. The ones the engine lists but does not evaluate are
// marked; G17 is proposed (labels.rs: evaluated vs not evaluated agree with the sources).
const NOT_EVALUATED = ['G03', 'G04', 'G07', 'G09', 'G12', 'G13', 'G17'];
function gateIds() {
  const ids = Object.keys((ID && ID.gate) || {}).filter((k) => /^G\d{2}$/.test(k)).sort();
  return ids.length ? ids : Array.from({ length: 23 }, (_, i) => `G${pad2(i)}`);
}
// Every refusal code that has a message a refused field can show (refusal.unmapped is the form's own note).
function refusalCodes() {
  const set = new Set();
  for (const messages of Object.values(CATALOGUE || {})) {
    for (const key of Object.keys(messages || {})) {
      const m = /^refusal\.([a-z][a-z0-9_]*)$/.exec(key);
      if (m && m[1] !== 'unmapped') set.add(m[1]);
    }
  }
  return [...set].sort();
}

// ---------------------------------------------------------------- sample projects (work plan 3.5)
const SAMPLES = [{ n: 1, jobs: ['cylinder_shell'] }, { n: 2, jobs: ['cone_frustum', 'eccentric_reducer'] }, { n: 3, jobs: ['header_with_branch', 'branch_stub'] }];
const exampleRequest = (fam) => ({ family: fam, ...Object.fromEntries(F[fam].f.map(([k, v]) => [k, v])), ...(F[fam].sref ? { surface_reference: 'unknown' } : {}) });
function role() {
  const s = getState() || {};
  const u = s.user || s.me || s.session || s;
  return String((u && u.role) || s.role || '');
}
const canCreate = () => ['admin', 'engineer'].includes(role());

async function withBusy(btn, label, fn) {
  if (!btn) return fn();
  const old = btn.textContent;
  busy(btn, true);
  if (label) btn.textContent = label;
  try { return await fn(); } finally { if (btn.isConnected) { busy(btn, false); btn.textContent = old; } }
}
// A sample is opened when a project of that name already exists (in either language), otherwise created once.
async function openOrCreate(sample, projects) {
  const key = `smp.${sample.n}.name`;
  const names = Object.values(CATALOGUE || {}).map((m) => m && m[key]).filter(Boolean);
  const hit = projects.find((p) => names.includes(p.name));
  if (hit) return hit.id;
  const { id } = await api('/api/projects', { method: 'POST', json: { name: t(key), description: t('smp.desc') } });
  for (const fam of sample.jobs) await api(`/api/projects/${id}/calculations`, { method: 'POST', json: exampleRequest(fam) });
  toast(t('smp.done', { id, n: sample.jobs.length }), 'ok');
  return id;
}
// One action per sample (`n`), or all three when called without one. Admin and engineer only.
export async function openSamples(n, btn) {
  if (!canCreate()) { toast(t('hp.smp.viewer'), 'warn'); return null; }
  const wanted = n === undefined || n === null ? SAMPLES : SAMPLES.filter((s) => s.n === Number(n));
  if (!wanted.length) return null;
  try {
    const id = await withBusy(btn, t('smp.busy'), async () => {
      const list = await api('/api/projects');
      const projects = Array.isArray(list) ? list : [];
      let first = null;
      for (const s of wanted) { const got = await openOrCreate(s, projects); if (first === null) first = got; }
      return first;
    });
    if (id !== null && id !== undefined) location.hash = `#/project/${id}`;
    return id;
  } catch (err) {
    const box = root && $('#smpmsg', root);
    if (box) box.replaceChildren(errorAlert(err));
    else toast(err && err.message ? String(err.message) : t('api.failed', { status: (err && err.status) || '' }), 'err');
    return null;
  }
}
// A refusal is shown by its code, in the active language; other server text is shown as the server wrote it.
function errorAlert(err) {
  const d = (err && err.detail) || {};
  const code = typeof d.code === 'string' ? d.code : '';
  if (code && has(`refusal.${code}`)) {
    const inner = d.detail || {};
    return el('div', {
      class: 'alert bad', role: 'alert',
      html: h(`refusal.${code}`, {
        fields: esc((d.fields || []).join(', ')),
        limit: esc(inner.limit === undefined || inner.limit === null ? '' : inner.limit),
        measured: esc(inner.measured === undefined || inner.measured === null ? '' : inner.measured),
      }),
    });
  }
  const msg = err && err.message ? String(err.message) : '';
  return el('div', { class: 'alert bad', role: 'alert' },
    msg ? el('span', { lang: 'en' }, msg) : t('api.failed', { status: (err && err.status) || '' }));
}

// ---------------------------------------------------------------- fragments
const td = (c) => (c instanceof Node ? el('td', {}, c) : el('td', { html: String(c ?? '') }));
function table(heads, rows) {
  return el('div', { class: 'table' },
    el('table', {},
      el('thead', {}, el('tr', {}, ...heads.map((k) => el('th', { scope: 'col' }, t(k))))),
      el('tbody', {}, ...rows.map((cells) => el('tr', {}, ...cells.map(td))))));
}
function panel(id, title, ...body) {
  return el('section', { class: 'panel', id: `help-${id}`, 'aria-labelledby': `help-${id}-h` },
    el('header', { class: 'panel-h' }, el('h2', { id: `help-${id}-h`, tabindex: '-1' }, title)),
    el('div', { class: 'panel-b' }, ...body));
}
const para = (key) => el('p', { html: h(key) });
const list = (items) => el('ul', {}, ...items.map((x) => el('li', {}, x)));

function famTable(fam) {
  const rows = F[fam].f.map(([k, v, u]) => [esc(plabel(fam, k)), monoHtml(k), esc(unitText(u)), monoHtml(fmt(v))]);
  if (F[fam].sref) rows.push([esc(t('math.sref.label')), monoHtml('surface_reference'), '—', esc(t('sref.unknown'))]);
  if (F[fam].ext) for (const k of EXT_KEYS) rows.push([esc(plabel(fam, k)), monoHtml(k), '—', esc(t('hp.optional'))]);
  return table(['hp.th.input', 'hp.th.key', 'hp.th.unit', 'hp.th.example'], rows);
}
function familiesBody() {
  const out = [el('p', {}, t('hp.fam.p'))];
  for (const fam of Object.keys(F)) {
    out.push(el('h3', {}, famLabel(fam), ' ', el('span', { class: 'mono' }, fam)));
    if (has(`hp.fam.${fam}`)) out.push(para(`hp.fam.${fam}`));
    out.push(famTable(fam));
  }
  return out;
}
function gatesBody() {
  const rows = gateIds().map((id) => {
    const means = has(`hp.gate.${id}`) ? h(`hp.gate.${id}`) : esc(idLabel('gate', id) || id);
    const cell = frag(el('span', { html: means }),
      NOT_EVALUATED.includes(id) ? frag(' ', chip(t(id === 'G17' ? 'hp.gate.proposed' : 'hp.gate.na'), 'neutral')) : null);
    return [monoHtml(id), esc(idLabel('gate', id) || id), cell];
  });
  return [el('p', {}, t('hp.gate.p')), table(['hp.th.gate', 'hp.th.name', 'hp.th.means'], rows)];
}
function refusalsBody() {
  const ph3 = { fields: esc(t('hp.ph.fields')), limit: esc(t('hp.ph.limit')), measured: esc(t('hp.ph.measured')) };
  const rows = refusalCodes().map((code) => [monoHtml(code), h(`refusal.${code}`, ph3)]);
  return [el('p', {}, t('hp.ref.p')), table(['hp.th.code', 'hp.th.message'], rows)];
}
function notModelledBody() {
  return [el('p', {}, t('hp.nm.p')), list([1, 2, 3, 4, 5].map((i) => t(`hp.nm.${i}`)))];
}
function samplesBody(onAction) {
  const out = [para('hp.smp.p')];
  if (!canCreate()) out.push(el('div', { class: 'alert warn', role: 'note' }, t('hp.smp.viewer')));
  const items = SAMPLES.map((sm) => {
    const name = t(`smp.${sm.n}.name`);
    const kids = [
      el('div', { class: 'l1' }, el('b', {}, name)),
      el('p', { class: 'sub' }, t('hp.smp.jobs', { list: sm.jobs.map((f) => idText('family', f)).join(', ') })),
    ];
    if (canCreate()) {
      const btn = el('button', { class: 'btn', type: 'button', 'aria-label': t('smp.btn.aria', { name }) }, name);
      onAction(btn, 'click', () => { openSamples(sm.n, btn); });
      kids.push(el('div', { class: 'row' }, btn));
    }
    return el('li', {}, ...kids);
  });
  out.push(el('ul', { class: 'il', id: 'samples' }, ...items));
  out.push(el('div', { id: 'smpmsg' }));
  return out;
}

// ---------------------------------------------------------------- view
const SECS = [['build', 'hp.build.h'], ['families', 'hp.fam.h'], ['gates', 'hp.gate.h'], ['refusals', 'hp.ref.h'], ['notmodelled', 'hp.nm.h'], ['samples', 'hp.smp.h']];
let root = null;
let bound = [];
let timers = [];
function on(node, type, fn) { node.addEventListener(type, fn); bound.push([node, type, fn]); }

function sectionParam(params) {
  if (typeof params === 'string') return params;
  const p = params || {};
  const fromHash = (location.hash.split('/')[2] || '').trim();
  return String(p.section || p.id || p.tab || fromHash || '');
}

export async function mount(container, params = {}) {
  unmount();
  const bodies = {
    build: [para('hp.build.p'), el('p', {}, t('hp.build.ev'))],
    families: familiesBody(),
    gates: gatesBody(),
    refusals: refusalsBody(),
    notmodelled: notModelledBody(),
    samples: samplesBody(on),
  };
  const toc = el('nav', { 'aria-label': t('hp.toc') },
    el('ul', { class: 'row' }, ...SECS.map(([id, key]) => {
      const btn = el('button', { class: 'btn btn-text', type: 'button' }, t(key));
      on(btn, 'click', () => goto(id));
      return el('li', {}, btn);
    })));
  root = el('div', {},
    el('h1', {}, t('hp.title')),
    el('p', { class: 'sub' }, t('hp.sub')),
    toc,
    ...SECS.map(([id, key]) => panel(id, t(key), ...bodies[id])));
  container.append(root);
  const section = sectionParam(params);
  if (section && SECS.some(([id]) => id === section)) {
    timers.push(setTimeout(() => { if (root) goto(section); }, 0));
  }
}

// A section named in the address is brought into view and its heading takes the focus (keyboard and reader).
function goto(id) {
  const heading = root && $(`#help-${id}-h`, root);
  if (!heading) return;
  heading.scrollIntoView({ block: 'start' });
  heading.focus({ preventScroll: true });
}

export function unmount() {
  for (const [node, type, fn] of bound) node.removeEventListener(type, fn);
  bound = [];
  for (const id of timers) clearTimeout(id);
  timers = [];
  if (root) root.remove();
  root = null;
}
