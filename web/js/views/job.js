import { api, esc } from '../api.js';
import { getState } from '../store.js';
import { t, h, has, idLabel, quantityUnit, engineJa, engineSentences, lang } from '../i18n.js';
import { el, $, $$, toast, busy, fmtTime, modal } from '../ui.js';
import { mountPattern } from './job-pattern.js';
import { mountAmoebas, unmountAmoebas } from '../lib/amoeba-mount.js';

const ja = () => lang() === 'ja';
const words = (s) => esc(String(s ?? '').replaceAll('_', ' '));
const mono = (s) => `<span class="mono">${esc(s)}</span>`;
const ud = (v) => `<bdi>${esc(v)}</bdi>`;
const short = (x) => `<span class="mono" title="${esc(x)}">${esc(String(x ?? '').slice(0, 12))}…</span>`;
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : esc(n));
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
const num = (v, u = '') => (typeof v === 'number' ? `<span class="mono" title="${fmt(v)}${u ? ` ${esc(u)}` : ''}">${n3(v)}</span>${u ? ` <span class="sub">${esc(u)}</span>` : ''}` : esc(v ?? '—'));
const signed = (v, u = '') => `<span class="mono" title="${fmt(v)}">${v > 0 ? '+' : ''}${n3(v)}</span>${u ? ` <span class="sub">${esc(u)}</span>` : ''}`;
const tag = (kind, text) => `<span class="tag chip${kind && kind !== 'na' ? ` ${kind}` : ''}">${esc(text)}</span>`;
const panel = (title, body, count) => `<section class="panel"><div class="panel-h"><h2>${title}</h2>${count !== undefined ? `<span class="cnt">${esc(count)}</span>` : ''}</div><div class="panel-b">${body}</div></section>`;
const ICON = { bad: 'i-alert', warn: 'i-alert', ok: 'i-check' };
const alert = (html, kind = 'bad') => `<div class="alert banner ${kind}" role="alert"><svg class="icon" aria-hidden="true"><use href="#${ICON[kind] || 'i-alert'}"/></svg><div>${html}</div></div>`;
const sub = (s) => `<p class="sub">${esc(s)}</p>`;
const kvRows = (pairs) => `<dl class="kv">${pairs.map(([k, v]) => `<dt>${k}</dt><dd>${v}</dd>`).join('')}</dl>`;

const idText = (kind, id) => idLabel(kind, id) || (ja() ? String(id) : String(id).replaceAll('_', ' '));
const labelled = (kind, id) => { const l = idLabel(kind, id); return l ? esc(l) : ja() ? mono(id) : words(id); };
const named = (kind, id) => { const l = idLabel(kind, id); if (!ja()) return l ? esc(l) : words(id); return l ? `${esc(l)} ${mono(id)}` : mono(id); };
const famLabel = (k) => esc(idLabel('familyLong', k) || idText('family', k));
const partName = (p) => (ja() && idLabel('part', p) ? `${esc(idLabel('part', p))} ${mono(p)}` : esc(p));
const partText = (p) => (ja() && idLabel('part', p) ? `${idLabel('part', p)} ${p}` : String(p));
const checkName = (n) => (ja() ? `${labelled('check', n)} ${mono(n)}` : words(n));
const plabel = (fam, key) => idLabel('param', `${fam}.${key}`) || idLabel('param', key) || (ja() ? key : String(key).replaceAll('_', ' '));
const jaHtml = (s) => esc(s).replace(/`([^`]+)`/g, '<span class="mono">$1</span>');
function eng(text, key = 'eng.tag') {
  const s = String(text ?? ''); if (!s) return '';
  if (!ja()) return `<span class="eng" lang="en">${esc(s)}</span>`;
  const j = engineJa(s);
  if (j) return `<span lang="ja" title="${esc(s)}">${jaHtml(j)}</span>`;
  return `${tag('na', t(key))} <span lang="en">${esc(s)}</span>`;
}
const srv = (text) => eng(text, 'srv.tag');
const errHtml = (err) => (err && err.status ? srv(err.message) : esc(String((err && err.message) || err)));
const refusalHtml = (r) => (r && has(`refusal.${r.code}`)
  ? h(`refusal.${r.code}`, { fields: esc((r.fields || []).join(', ')), limit: esc(r.detail?.limit ?? ''), measured: esc(r.detail?.measured ?? '') })
  : eng(r ? r.message : ''));

const STATES = ['APPROVED', 'REJECTED', 'SUPERSEDED', 'NO_RESULT', 'REVIEW_REQUIRED', 'REFUSED'];
const STATE_KIND = { APPROVED: 'ok', REJECTED: 'bad', SUPERSEDED: 'na', NO_RESULT: 'bad' };
const stateText = (s) => (STATES.includes(s) ? t(`state.${s}`) : String(s).replaceAll('_', ' '));
const stateTag = (s) => tag(STATE_KIND[s] || 'warn', stateText(s));
const RESULT_KIND = { PASS: 'ok', FAIL: 'bad', INCONCLUSIVE: 'warn' };
const resultText = (r) => (['PASS', 'FAIL', 'INCONCLUSIVE', 'NOT_IMPLEMENTED', 'NOT_RUN'].includes(r) ? t(`result.${r}`) : String(r).replaceAll('_', ' '));
const resultTag = (r) => tag(RESULT_KIND[r] || 'na', resultText(r));
const oblText = (s) => (['open', 'discharged', 'discharged_by_decision', 'waived'].includes(s) ? t(`obl.${s}`) : String(s).replaceAll('_', ' '));
const oblTag = (s) => tag(s === 'open' || s === 'waived' ? 'warn' : 'ok', oblText(s));
const advText = (s) => (['open', 'acknowledged'].includes(s) ? t(`adv.${s}`) : String(s));
const dirText = (d) => (['downstream', 'lateral', 'common_cause'].includes(d) ? t(`dir.${d}`) : String(d));
const nodeParts = (l) => { const i = l.indexOf(' · '); return i < 0 ? ['', l] : [l.slice(0, i), l.slice(i + 3)]; };
const nodeName = (k, n) => (ja() ? (k === 'parameter' || k === 'convention' ? idLabel('param', n) : k ? idLabel('part', n) : null) || idLabel('dlabel', n) || idLabel('rkind', n) : null);
const nodeLabel = (l) => { const [k, n] = nodeParts(l); if (!ja()) return mono(l); const jn = nodeName(k, n); return `${k ? `${labelled('rkind', k)} · ` : ''}${jn ? `${esc(jn)} ` : ''}${mono(n)}`; };
const oblSubject = (o) => `${labelled('rkind', o.kind)} · ${ja() && nodeName(o.kind, o.label) ? `${esc(nodeName(o.kind, o.label))} ` : ''}${mono(o.label)}`;
const DIM = /^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/;
const toAscii = (s) => String(s ?? '').replace(/[０-９]/g, (ch) => String.fromCharCode(ch.charCodeAt(0) - 0xfee0)).replace(/[．－−＋ｅＥ]/g, (ch) => ({ '．': '.', '－': '-', '−': '-', '＋': '+', 'ｅ': 'e', 'Ｅ': 'E' }[ch]));
const parseDim = (s) => { const v = toAscii(s).trim(); return DIM.test(v) ? Number(v) : NaN; };
const extent = (p) => { let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity; for (const k of [...p.curves, ...p.holes]) for (const [x, y] of k.points) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; } return { w: x1 - x0, h: y1 - y0 }; };
function quantity(k, v) {
  const l = idLabel('quantity', k), unit = quantityUnit(k);
  const label = `${l ? esc(l) : ja() ? '' : words(k)} ${mono(k)}`;
  if (typeof v !== 'number') return [label, esc(v)];
  if (unit === 'rad') return [label, `${num(v * 180 / Math.PI, '°')} <span class="sub">(${n3(v)} rad)</span>`];
  return [label, unit ? num(v, unit) : `${num(v)} <span class="sub" title="${esc(t('q.nounit.tip'))}">${esc(t('q.nounit'))}</span>`];
}

const TABS = ['result', 'pattern', 'details'];
const TAB_KEY = { result: 'math.result.h', pattern: 'stage.aria', details: 'pane.props.h' };
let root = null, gen = 0, dead = true, timers = new Set(), dlg = null, disposePattern = null;
let c = null, prev = null, dep = null, meta = null, fam = '', state = '', tab = 'result', scope = 'all', gf = 'all';
const role = () => getState()?.user?.role || 'viewer';
const canEdit = () => role() !== 'viewer';
const blocked = () => state !== 'REVIEW_REQUIRED' && state !== 'APPROVED';
const canCorrect = () => canEdit() && !blocked() && c.correctable_parameters.length > 0;
const later = (fn, ms) => { const id = setTimeout(() => { timers.delete(id); if (!dead) fn(); }, ms); timers.add(id); };
const openCount = () => c.obligations.filter((o) => o.state === 'open').length + c.advisories.filter((a) => a.state === 'open').length;
// the export follows the server's answer (job detail `export`, the rule of get_dxf): available, held by one gate,
// or held part by part (owner rule `withheld`, AMOEBA snapshot `amoeba_holds`)
const exportInfo = () => c.export || { available: false, held_by: null, not_checked: [], amoeba_holds: [], withheld: [] };
const released = () => exportInfo().available === true;
const withheldOf = () => exportInfo().withheld || [];
const amoebaHoldsOf = () => exportInfo().amoeba_holds || [];
const WITHHELD_CODE = 'dxf_withheld_inconclusive';
const partHeld = (part) => withheldOf().some((w) => w.part === part) || amoebaHoldsOf().some((h) => h.part === part);
// a part is served when no gate holds the job and no hold names the part (the rule of get_dxf, per part)
const servable = (part) => !exportInfo().held_by && state === 'REVIEW_REQUIRED' && !c.superseded_by && !partHeld(part);
const amoebaStage = () => {
  if (state === 'SUPERSEDED') return 'idle';
  if (!c.patterns.length || state === 'REJECTED' || state === 'NO_RESULT') return 'hold';
  if (!c.gates.length) return 'idle';
  if (exportInfo().held_by || withheldOf().length || amoebaHoldsOf().length) return 'hold';
  return released() ? 'release' : 'digest';
};
// the failing check of a held export: gate, result, what the engine found, one fix (the fix text is per result)
const FIX_KEY = { FAIL: 'exp.fix.fail', NOT_RUN: 'exp.fix.notrun' };
const heldHtml = (hb) => `<b>${esc(t('exp.held.check', { id: hb.gate, name: String(hb.name).replaceAll('_', ' '), result: resultText(hb.result) }))}</b> ${eng(hb.detail)} <span class="fix">${esc(t(FIX_KEY[hb.result] || 'exp.fix.inc'))}</span>`;
// allow-listed gates the engine does not check: listed, never hidden
const notCheckedHtml = () => {
  const rows = exportInfo().not_checked || [];
  return rows.length ? `<ul class="il sub exp-nc">${rows.map((n) => `<li><span class="mono">${esc(n.gate)}</span> ${esc(t('exp.nc', { reason: t(`nc.${n.gate}`) }))}</li>`).join('')}</ul>` : '';
};
// the AMOEBA holds of the DXF files: the code the 409 carries, and the checks that are not PASS
const amoebaHoldsHtml = () => {
  const rows = exportInfo().amoeba_holds || [];
  return rows.length ? `<ul class="il sub exp-am">${rows.map((h) => `<li><span class="mono">${esc(h.code)}</span> ${esc(t('am.hold.part', { part: partText(h.part) }))}${(h.checks || []).length ? `<ul class="il">${h.checks.map((k) => `<li>${checkName(k.name)} · ${eng(k.detail)}</li>`).join('')}</ul>` : ''}</li>`).join('')}</ul>` : '';
};
// the parts the owner rule withholds: the code, and the checks on the cut geometry that are not decided
const withheldHtml = () => {
  const rows = withheldOf();
  return rows.length ? `<ul class="il sub exp-wh">${rows.map((w) => `<li><span class="mono">${esc(WITHHELD_CODE)}</span> ${esc(t('dxf.withheld.t', { part: partText(w.part) }))}${(w.checks || []).length ? `<ul class="il">${w.checks.map((k) => `<li>${checkName(k.name)}${k.reason ? ` · ${eng(k.reason)}` : ''}</li>`).join('')}</ul>` : ''}</li>`).join('')}</ul>` : '';
};
// everything that holds or qualifies the export, beside the button: withheld parts, AMOEBA holds, then the gates not checked by NF
const exportNotesHtml = () => withheldHtml() + amoebaHoldsHtml() + notCheckedHtml();
const lockReason = () => {
  if (!c.patterns.length) return c.refusal ? refusalHtml(c.refusal) : c.outcome ? eng(c.outcome) : esc(t('ban.none.default'));
  if (exportInfo().held_by) return heldHtml(exportInfo().held_by);
  // the job's own state comes before the parts: a superseded or rejected job is not withheld part by part
  if (state !== 'REVIEW_REQUIRED') return esc(t('exp.reason.state', { state: stateText(state) }));
  if (withheldOf().length) {
    const parts = withheldOf().map((w) => partText(w.part)).join(', ');
    return esc(t(withheldOf().length > 1 ? 'exp.reason.withheld.many' : 'exp.reason.withheld', { parts }));
  }
  if (amoebaHoldsOf().length) return esc(t('exp.reason.amoeba', { parts: amoebaHoldsOf().map((h) => partText(h.part)).join(', ') }));
  return esc(t('exp.reason.state', { state: stateText(state) }));
};
const AM_STEPS = [
  ['surround', 'am.s.surround', () => c.patterns.length > 0],
  ['engulf', 'am.s.engulf', () => c.checks.length > 0],
  ['digest', 'am.s.digest', () => c.gates.length > 0],
  ['release', 'am.s.release', () => released()],
];
function timelineHtml() {
  const held = amoebaStage() === 'hold';
  const open = AM_STEPS.findIndex(([, , done]) => !done());
  return AM_STEPS.map(([k, key, done], i) => {
    const s = done() ? 'done' : i === open ? (held ? 'held' : 'now') : 'todo';
    return `<li class="am-step" data-s="${s}" data-stage="${k}"><span class="am-dot" aria-hidden="true"></span><b>${esc(t(key))}</b></li>`;
  }).join('');
}
function amoebaPanel() {
  const stage = amoebaStage();
  const why = stage === 'hold' ? `<p class="am-why"><b>${esc(t('exp.hold'))}</b> ${lockReason()}</p>${withheldHtml()}${amoebaHoldsHtml()}` : '';
  return `<section class="panel am-stage"><div class="amoeba-mount am-job" data-amoeba data-stage="${stage}" role="img" aria-label="${esc(t('am.stage.aria'))}"></div><ol class="am-timeline" aria-label="${esc(t('am.stage.aria'))}">${timelineHtml()}</ol>${why}</section>`;
}
function exportHtml() {
  const free = c.patterns.filter((p) => servable(p.part));
  const body = free.length
    ? `<a class="btn btn-primary btn-block" href="/api/calculations/${c.id}/dxf/${encodeURIComponent(free[0].part)}" download>${esc(t('exp.dxf'))}</a><p class="sub">${esc(t('exp.checked'))}</p>${exportNotesHtml()}`
    : `<button type="button" class="btn btn-block" disabled aria-disabled="true">${esc(t('exp.locked'))}</button><p class="sub lock-why"><b>${esc(t('exp.because'))}</b> ${lockReason()}</p>${exportNotesHtml()}`;
  return panel(esc(t('exp.h')), `<div class="export-box">${body}</div>`);
}
function inspectorHtml() {
  const count = (r) => c.gates.filter((g) => g.result === r).length;
  const cells = [['PASS', 'insp.g.pass', 'ok'], ['FAIL', 'insp.g.fail', 'bad'], ['INCONCLUSIVE', 'insp.g.inc', 'warn'], ['NOT_IMPLEMENTED', 'insp.g.ni', 'na']]
    .map(([r, key, kind]) => `<div class="gc gc-${kind}"><b class="count" data-count="${count(r)}">${count(r)}</b><span>${esc(t(key))}</span></div>`).join('');
  const dims = c.patterns.length
    ? c.patterns.map((p) => `<div class="dimrow"><div class="dimname">${partName(p.part)}</div>${kvRows([[esc(t('insp.len')), num(p.length, 'mm')], [esc(t('insp.holes')), `<span class="count" data-count="${p.holes.length}">${p.holes.length}</span>`], [esc(t('insp.side')), words(p.marking_side)]])}</div>`).join('')
    : sub(t('insp.none'));
  return panel(esc(t('insp.dims')), dims) + panel(esc(t('insp.gates')), `<div class="gcount">${cells}</div>`);
}
let amHandles = [];
function mountJobAmoeba() {
  const my = gen;
  mountAmoebas(root).then((h) => { if (my === gen && !dead) amHandles = h; else unmountAmoebas(h); });
}

function headerHtml() {
  const primary = c.superseded_by ? 'successor' : canCorrect() ? 'corr' : canEdit() ? 'clone' : '';
  const actions = [
    c.superseded_by ? `<a class="btn${primary === 'successor' ? ' btn-primary' : ''}" href="#/job/${c.superseded_by}">${esc(t('job.successor', { id: c.superseded_by }))}</a>` : '',
    canCorrect() ? `<button type="button" class="btn${primary === 'corr' ? ' btn-primary' : ''}" id="corr-open">${esc(t('job.correct'))}</button>` : '',
    canEdit() && !blocked() ? `<button type="button" class="btn" id="replay" title="${esc(t('job.replay.tip'))}">${esc(t('job.replay'))}</button>` : '',
    canEdit() ? `<a class="btn${primary === 'clone' ? ' btn-primary' : ''}" href="#/math/${c.project_id}/${c.id}" title="${esc(t('job.clone.tip'))}">${esc(t('job.clone'))}</a>` : '',
  ].join('');
  return `<div class="panel-h"><h1>${esc(t('job.n', { id: c.id }))} ${stateTag(state)}${c.supersedes ? ` <a class="tag chip" href="#/job/${c.supersedes}">${esc(t('job.corrects', { id: c.supersedes }))}</a>` : ''}</h1>
    <div class="row sub"><span>${famLabel(fam)}${ja() ? ` ${mono(fam)}` : ''}</span><span><a href="#/project/${c.project_id}">${esc(t('project.n', { id: c.project_id }))}</a></span>
    <span>${h('job.enteredby', { by: ud(c.created_by) })}</span>${meta ? `<span>${esc(fmtTime(meta.created_at))}</span>` : ''}<span>${esc(t('r.engine'))} ${mono(c.engine)}</span></div>
    <div class="row result-actions">${actions}</div></div>`;
}
function bannerHtml() {
  if (state === 'SUPERSEDED') return alert(h('ban.sup', { job: `<a href="#/job/${c.superseded_by}">#${c.superseded_by}</a>` }), 'warn');
  if (state === 'REJECTED') return alert(h('ban.rej'));
  if (state === 'NO_RESULT') {
    const why = c.refusal ? refusalHtml(c.refusal) : c.outcome ? eng(c.outcome) : esc(t('ban.none.default'));
    const thr = c.outcome ? /s_end\s*>\s*([0-9][0-9.eE+-]*)/.exec(c.outcome) : null;
    return alert(`${h('ban.none', { why: why.replace(/[.。]\s*$/, '') })}${thr ? `<p class="sub">${h('ban.none.min', { n: esc(thr[1]) })}</p>` : ''}`);
  }
  const held = c.gates.filter((g) => g.result !== 'PASS');
  return alert(`${h('ban.review')}${held.length ? `<p class="sub">${h('ban.review.sum', { list: held.map((g) => esc(g.id)).join(', '), n: held.length, total: c.gates.length })}</p>` : ''}`, 'warn');
}
function gateItem(g) {
  const segs = engineSentences(g.detail), allJa = ja() && segs.length > 0 && segs.every((s) => s.ja);
  const sent = (s) => (allJa ? `<span lang="ja" title="${esc(s.en)}">${jaHtml(s.ja)}</span>` : ja() ? `<span lang="en">${esc(s.en)}</span>` : esc(s.en));
  const raw = ja() && !allJa ? `${tag('na', t('eng.tag'))} ` : '';
  const topics = [...new Set(segs.map((s) => s.topic).filter(Boolean))];
  const why = topics.length > 1 ? esc(t('g.points', { n: topics.length, topics: topics.map((k) => t(`g.topic.${k}`)).join(' · ') })) : segs.length ? `${raw}${sent(segs[0])}` : '';
  const list = (xs) => `<ul class="il">${xs.map((s) => `<li>${sent(s)}</li>`).join('')}</ul>`;
  const body = topics.length > 1
    ? [...topics, null].map((k) => [k, segs.filter((s) => (s.topic || null) === k)]).filter(([, xs]) => xs.length)
      .map(([k, xs]) => `<h3>${esc(t(`g.topic.${k || 'other'}`))}</h3>${list(xs)}`).join('')
    : list(segs);
  const hide = gf !== 'all' && gf !== g.result;
  return `<li class="card" data-result="${esc(g.result)}"${hide ? ' hidden' : ''}><details><summary><span class="l1"><span class="id">${esc(g.id)}</span><b>${ja() ? labelled('gate', g.id) : words(g.name)}</b>${resultTag(g.result)}</span>
    <span class="sub">${why}</span> <span class="cnt">${esc(t('g.more', { n: segs.length }))}</span></summary>
    <div class="panel-b">${raw}${body}<p class="sub">${esc(t('g.name'))} ${mono(g.name)}</p></div></details></li>`;
}
function gatesHtml() {
  const count = (r) => c.gates.filter((g) => g.result === r).length;
  const gchip = (r, kind, key) => (r === 'all' || count(r) ? `<button type="button" class="chip${kind ? ` ${kind}` : ''}" data-gf="${r}" aria-pressed="${gf === r}">${esc(r === 'all' ? t('g.all', { n: c.gates.length }) : t(key, { n: count(r) }))}</button>` : '');
  return panel(esc(t('tab.gates')), `<div class="row" role="group" aria-label="${esc(t('g.filter.aria'))}">${gchip('all')}${gchip('FAIL', 'bad', 'g.n.fail')}${gchip('INCONCLUSIVE', 'warn', 'g.n.inc')}${gchip('NOT_IMPLEMENTED', '', 'g.n.ni')}${gchip('NOT_RUN', '', 'g.n.nr')}${gchip('PASS', 'ok', 'g.n.pass')}
    <button type="button" class="btn btn-text" id="g-all" data-open="0">${esc(t('g.expand'))}</button></div>
    ${!c.patterns.length ? alert(esc(t('g.noresult'))) : ''}
    <ul class="il" id="gates">${c.gates.map(gateItem).join('')}</ul>${sub(t('g.foot'))}`, c.gates.length);
}
function checksHtml() {
  if (!c.checks.length) return panel(esc(t('tab.checks')), `${sub(t('c.none.t'))}${sub(t('c.none.p'))}`);
  const scopes = [...new Set(c.checks.map((k) => k.scope))], passN = c.checks.filter((k) => k.verdict === 'PASS').length;
  if (scope !== 'all' && !scopes.includes(scope)) scope = 'all';
  return panel(esc(t('tab.checks')), `<div class="row">${tag(passN === c.checks.length ? 'ok' : 'bad', t('c.n', { a: passN, b: c.checks.length }))}
    <span class="row" role="group" aria-label="${esc(t('c.scope.aria'))}">${['all', ...scopes].map((s) => `<button type="button" class="chip" data-scope="${esc(s)}" aria-pressed="${s === scope}">${s === 'all' ? esc(t('c.all')) : partName(s)}</button>`).join('')}</span></div>
    <ul class="il" id="checks">${c.checks.map((k) => `<li class="card" data-scope="${esc(k.scope)}"${scope !== 'all' && scope !== k.scope ? ' hidden' : ''}>
      <div class="l1"><span class="id">${partName(k.scope)}</span><b>${checkName(k.name)}</b>${resultTag(k.verdict)}</div>
      <p class="sub">${h('c.method', { method: eng(k.method), cls: mono(k.independence) })}</p>
      <dl class="kv checks-grid"><dt>${esc(t('c.primary'))}</dt><dd>${num(k.primary)}</dd><dt>${esc(t('c.reference'))}</dt><dd>${num(k.reference)}</dd>
      <dt>${esc(t('c.difference'))}</dt><dd>${num(k.difference)}</dd><dt>${esc(t('c.tolerance'))}</dt><dd>${num(k.tolerance)}</dd></dl></li>`).join('')}</ul>${sub(t('c.foot'))}`, c.checks.length);
}
function recordsHtml() {
  const rp = c.replay;
  const replayTag = rp ? tag(rp.matches ? 'ok' : 'bad', t(rp.matches ? 'replay.match' : 'replay.mismatch')) : `<span class="sub">${esc(t('replay.none'))}</span>`;
  const recKind = (r) => (ja() ? `${labelled('rkind', r.kind)} ${mono(r.kind)}` : mono(r.kind));
  const subject = (r) => {
    const p = r.payload || {};
    const s = p.name !== undefined ? `${p.name}${p.value !== undefined ? ` = ${p.value && typeof p.value === 'object' ? Object.entries(p.value).map(([k, x]) => `${k} ${x}`).join(' · ') : typeof p.value === 'number' ? fmt(p.value) : p.value}${p.unit ? ` ${p.unit}` : ''}` : ''}`
      : p.part !== undefined ? p.part : Array.isArray(p.between) ? p.between.join(' × ') : p.outcome !== undefined ? p.message || p.outcome : '';
    return s ? ` · <span class="mono"${ja() ? ' lang="en"' : ''}>${esc(s)}</span>` : '';
  };
  return panel(esc(t('r.h', { n: c.records.length })), `${kvRows([
    [esc(t('r.closure')), short(c.closure_hash)], [esc(t('r.n')), esc(c.closure_size)], [esc(t('r.wu')), esc(c.work_units)],
    [esc(t('r.engine')), mono(c.engine)], [esc(t('r.replay')), `${replayTag}${rp ? ` <span class="sub">${esc(fmtTime(rp.at))} · ${ud(rp.run_by)}</span>` : ''}`],
  ])}
  ${rp ? `<p class="sub">${esc(t('r.checked', { n: rp.stored_records_checked, f: rp.stored_records_failing_identity_check }))} ${eng(rp.limitation)}</p>` : sub(t('r.replay.hint'))}
  ${c.records.map((r) => `<details class="card"><summary><span class="l1">${recKind(r)}${subject(r)} ${short(r.id)}</span></summary><div class="panel-b">
    <p class="sub">${named('rfamily', r.family)} · ${named('rkind', r.kind)}</p><p class="mono">${esc(r.id)}</p>
    ${r.relations.length ? `<p class="sub">${r.relations.map((x) => `${mono(x.rel)} → ${short(x.to)}`).join(' · ')}</p>` : ''}
    <pre class="mono" lang="en">${esc(JSON.stringify(r.payload, null, 1))}</pre></div></details>`).join('')}`, c.records.length);
}
function dxfHtml() {
  const free = c.patterns.filter((p) => servable(p.part));
  if (!free.length) return panel(esc(t('dxf.btn')), `${sub(t('exp.note'))}<p class="sub lock-why"><b>${esc(t('exp.because'))}</b> ${lockReason()}</p>${exportNotesHtml()}`);
  const link = (p) => `<a class="btn" href="/api/calculations/${c.id}/dxf/${encodeURIComponent(p.part)}" download aria-label="${esc(t('dxf.aria', { part: partText(p.part) }))}" title="${esc(t('dxf.tip', { part: partText(p.part) }))}">${esc(t('dxf.btn'))} · ${partName(p.part)}</a>`;
  // one alert per withheld part: the parts that are served stay downloadable beside it
  const held = (w) => alert(`<b>${esc(t('dxf.withheld.t', { part: partText(w.part) }))}</b><p>${esc(t('dxf.withheld.p', { n: (w.checks || []).length }))}</p>
      <ul class="il">${(w.checks || []).map((k) => `<li>${checkName(k.name)}${k.reason ? ` · ${eng(k.reason)}` : ''}</li>`).join('')}</ul>`, 'warn');
  return panel(esc(t('dxf.btn')), `<div class="row">${free.map(link).join('')}
        ${free.length > 1 ? `<button type="button" class="btn" id="dxf-all" aria-label="${esc(t('dxf.all.aria', { n: free.length }))}">${esc(t('dxf.all', { n: free.length }))}</button>` : ''}</div>
        ${withheldOf().map(held).join('')}`);
}
function reqHtml() {
  const refKeys = !c.patterns.length && c.refusal ? c.refusal.fields || [] : [];
  const keys = Object.keys(c.request).filter((k) => k !== 'family');
  const value = (k, v) => (typeof v === 'number' ? num(v) : ['surface_reference', 'header_surface_reference', 'branch_surface_reference'].includes(k) ? named('sref', v) : k === 'joint' ? named('joint', v) : esc(v));
  return panel(esc(t('req.h')), `${kvRows(keys.map((k) => {
    const refused = refKeys.includes(k), ok = canCorrect() && c.correctable_parameters.some((x) => x[0] === k);
    return [`${esc(plabel(fam, k))} ${mono(k)}`,
      `${value(k, c.request[k])}${refused ? ` ${tag('bad', t('req.refused'))}<div class="alert bad" id="ref-${esc(k)}" role="alert">${refusalHtml(c.refusal)}</div>` : ''}
       ${ok ? ` <button type="button" class="btn btn-text" data-correct="${esc(k)}" aria-label="${esc(t('req.correct.aria', { label: plabel(fam, k) }))}">${esc(t('req.correct'))}</button>` : ''}`];
  }))}${sub(t('req.hint'))}`);
}
function diffHtml() {
  const rows = []; let same = 0;
  const differs = (a, b) => (typeof a === 'number' && typeof b === 'number' ? Math.abs(a - b) > 1e-9 * Math.max(1, Math.abs(a), Math.abs(b)) : a !== b);
  const row = (label, a, b, unit) => rows.push(`<tr><td>${label}</td><td>${typeof a === 'number' ? num(a) : esc(a ?? '—')}</td><td>${typeof b === 'number' ? num(b) : esc(b ?? '—')}</td><td>${typeof a === 'number' && typeof b === 'number' ? signed(b - a, unit) : ''}</td></tr>`);
  const head = (html) => rows.push(`<tr><th colspan="4">${html}</th></tr>`);
  head(esc(t('diff.entered')));
  for (const k of new Set([...Object.keys(prev.request), ...Object.keys(c.request)])) {
    if (k === 'family') continue;
    if (differs(prev.request[k], c.request[k])) row(`${esc(plabel(fam, k))} ${mono(k)}`, prev.request[k], c.request[k], ''); else same++;
  }
  for (const p of c.patterns) {
    const q = prev.patterns.find((x) => x.part === p.part); head(h('diff.pattern', { part: partName(p.part) }));
    if (!q) { rows.push(`<tr><td colspan="4">${esc(t('diff.newpart'))}</td></tr>`); continue; }
    const n0 = rows.length, e0 = extent(q), e1 = extent(p);
    if (differs(e0.w, e1.w)) row(esc(t('diff.ex')), e0.w, e1.w, 'mm'); else same++;
    if (differs(e0.h, e1.h)) row(esc(t('diff.ey')), e0.h, e1.h, 'mm'); else same++;
    for (const k of [...p.curves, ...p.holes]) {
      const o = [...q.curves, ...q.holes].find((x) => x.name === k.name), lab = h('diff.curve', { name: mono(k.name) });
      if (!o) row(lab, '—', k.length, 'mm'); else if (differs(o.length, k.length)) row(lab, o.length, k.length, 'mm'); else same++;
    }
    for (const [k, v] of Object.entries(p.quantities)) { if (differs(q.quantities[k], v)) row(quantity(k, v)[0], q.quantities[k], v, quantityUnit(k) || ''); else same++; }
    if (rows.length === n0) rows.push(`<tr><td colspan="4" class="sub">${esc(t('diff.nochange'))}</td></tr>`);
  }
  return panel(esc(t('tab.changes')), `<p class="sub">${h('diff.lead', { job: `<a href="#/job/${prev.id}">#${prev.id}</a>`, n: same })}</p>
    <table class="table"><thead><tr><th>${esc(t('diff.value'))}</th><th>${esc(t('job.n', { id: prev.id }))}</th><th>${esc(t('job.n', { id: c.id }))}</th><th>${esc(t('diff.change'))}</th></tr></thead><tbody>${rows.join('')}</tbody></table>`);
}
function scrutinyHtml() {
  const adv = c.advisories.length ? `<h3>${esc(t('s.known.h'))}</h3>${sub(t('s.known.p'))}<ul class="il">${c.advisories.map((a) => `<li class="card">
    <div class="l1"><span class="id">A${a.id}</span><b>${esc(t('s.adv'))}</b>${tag(a.state === 'open' ? 'warn' : 'na', advText(a.state))}</div><p>${eng(a.message)}</p>
    ${a.note ? `<p class="sub">${ud(a.resolved_by)} · ${esc(fmtTime(a.resolved_at))}: ${ud(a.note)}</p>` : ''}
    ${a.state === 'open' && canEdit() && !blocked() ? `<div class="field"><label for="adv-${a.id}">${esc(t('s.adv.aria', { id: a.id }))}</label><input id="adv-${a.id}" data-advreason="${a.id}" placeholder="${esc(t('s.adv.ph'))}"></div>
      <div class="row"><button type="button" class="btn" data-ack="${a.id}">${esc(t('s.adv.ack'))}</button></div>` : ''}</li>`).join('')}</ul>` : '';
  const obl = c.obligations.length ? `<h3>${esc(t('s.obl.h'))}</h3><p class="sub">${h('s.obl.p', { rule: mono(dep.rule_table) })}</p>
    <ul class="il">${c.obligations.slice().sort((a, b) => (a.state === 'open' ? 0 : 1) - (b.state === 'open' ? 0 : 1) || a.id - b.id).map((o) => `<li class="card">
      <div class="l1"><span class="id">#${o.id}</span><b>${oblSubject(o)}</b>${oblTag(o.state)}</div><p>${eng(o.required_check)}</p>
      <p class="sub">${h('s.via', { dir: esc(dirText(o.direction)), rel: mono(o.via) })}</p>
      ${o.state !== 'open' ? `<p class="sub">${o.resolved_by ? h('s.resolved', { note: ud(o.note), by: ud(o.resolved_by), when: esc(fmtTime(o.resolved_at)) }) : eng(o.note)}</p>` : ''}
      ${o.state === 'open' && canEdit() ? `<div class="field"><label for="obl-${o.id}">${esc(t('s.obl.aria', { id: o.id }))}</label><input id="obl-${o.id}" data-reason="${o.id}" placeholder="${esc(t('s.obl.ph'))}"></div>
        <div class="row"><button type="button" class="btn" data-resolve="${o.id}" data-decision="confirmed">${esc(t('s.confirm'))}</button><button type="button" class="btn btn-text" data-resolve="${o.id}" data-decision="waived">${esc(t('s.waive'))}</button></div>` : ''}</li>`).join('')}</ul>` : '';
  return panel(esc(t('tab.scrutiny')), adv + obl || `${sub(t('s.none.t'))}${sub(t('s.none.p'))}`, openCount());
}
function depsHtml() {
  const body = dep && dep.edges.length
    ? `<table class="table"><thead><tr><th>${esc(t('th.record'))}</th><th>${esc(t('th.relation'))}</th><th>${esc(t('th.dependson'))}</th></tr></thead>
      <tbody>${dep.edges.map((e) => `<tr><td>${nodeLabel(e.from)}</td><td>${ja() && idLabel('rel', e.rel) ? `${esc(idLabel('rel', e.rel))} ` : ''}${mono(e.rel)}</td><td>${nodeLabel(e.to)}</td></tr>`).join('')}</tbody></table>
      <p class="sub">${h('am.dep.rule', { rule: mono(dep.rule_table) })}</p><div class="row"><a class="btn" href="#/amoeba/dependencies/${c.id}">${esc(t('gr.open'))}</a></div>`
    : `${sub(t('gr.none.t'))}${sub(t('gr.none.p'))}`;
  return panel(esc(t('tab.graph')), body, dep ? dep.edges.length : 0);
}
function historyHtml() {
  const ev = [];
  if (meta) ev.push({ at: meta.created_at, k: !c.patterns.length ? 'bad' : 'ok', text: `${esc(t(!c.patterns.length ? 'ev.none' : 'ev.computed', { id: c.id }))} — ${h(c.supersedes ? 'ev.computed.corr' : 'ev.computed.d', { fam: famLabel(fam), by: ud(c.created_by), n: c.closure_size, job: c.supersedes ? `<a href="#/job/${c.supersedes}">#${c.supersedes}</a>` : '' })}` });
  if (c.obligations.length) ev.push({ at: Math.min(...c.obligations.map((o) => o.created_at)), k: 'warn', text: `${esc(t('ev.obl', { n: c.obligations.length }))} — ${esc(t('ev.obl.d'))}` });
  c.obligations.filter((o) => o.resolved_by).forEach((o) => ev.push({ at: o.resolved_at, k: o.state === 'waived' ? 'warn' : 'ok', text: `${esc(t('ev.obl.res', { id: o.id, state: oblText(o.state) }))} — ${mono(o.label)} · ${ud(o.resolved_by)}: ${ud(o.note)}` }));
  c.advisories.filter((a) => a.resolved_by).forEach((a) => ev.push({ at: a.resolved_at, k: 'ok', text: `${esc(t('ev.adv', { id: a.id }))} — ${ud(a.resolved_by)}: ${ud(a.note)}` }));
  if (c.replay) ev.push({ at: c.replay.at, k: c.replay.matches ? 'ok' : 'bad', text: `${esc(t(c.replay.matches ? 'replay.match' : 'replay.mismatch'))} — ${h(c.replay.matches ? 'ev.replay.d' : 'ev.replay.d.bad', { by: ud(c.replay.run_by), n: c.replay.stored_records_checked, f: c.replay.stored_records_failing_identity_check })}` });
  ev.sort((a, b) => a.at - b.at);
  if (c.superseded_by) ev.push({ k: 'bad', text: `${esc(t('ev.sup'))} — ${h('ev.sup.d', { job: `<a href="#/job/${c.superseded_by}">#${c.superseded_by}</a>` })}` });
  return panel(esc(t('tab.history')), `<ul class="il">${ev.map((e) => `<li class="card"><div class="l1">${tag(e.k, e.at ? fmtTime(e.at) : '—')}</div><p>${e.text}</p></li>`).join('')}</ul>${sub(t('history.foot'))}`);
}
const measHtml = () => panel(esc(t('meas.h')), `${sub(t('meas.hint'))}<div class="row"><button type="button" class="btn" id="go-pattern">${esc(t('tool.measure'))}</button></div>`);

function render() {
  const tabsRow = `<div class="tabs" role="tablist" aria-label="${esc(t('tabs.aria'))}">${TABS.map((k) => `<button type="button" role="tab" id="tab-${k}" data-tab="${k}" aria-controls="tp-${k}" aria-selected="${k === tab}" tabindex="${k === tab ? 0 : -1}">${esc(t(TAB_KEY[k]))}${k === 'details' && openCount() ? ` <span class="cnt">${openCount()}</span>` : ''}</button>`).join('')}</div>`;
  const tp = (k, html) => `<div role="tabpanel" id="tp-${k}" aria-labelledby="tab-${k}"${k === tab ? '' : ' hidden'}>${html}</div>`;
  root.innerHTML = `<div class="workspace job-ws"><aside class="ws-sidebar">${tabsRow}</aside><div class="ws-content"><section class="panel job-head">${headerHtml()}</section>${amoebaPanel()}${bannerHtml()}
    ${tp('result', gatesHtml() + checksHtml() + dxfHtml() + recordsHtml())}
    ${tp('pattern', c.patterns.length ? '<div class="pattern-host" id="pattern-host"></div>' : panel(esc(t('nopat.t')), alert(c.refusal ? refusalHtml(c.refusal) : c.outcome ? eng(c.outcome) : esc(t('nopat.default')))))}
    ${tp('details', reqHtml() + (prev ? diffHtml() : '') + measHtml() + depsHtml() + scrutinyHtml() + historyHtml())}
  </div><aside class="ws-inspector">${exportHtml()}${inspectorHtml()}</aside></div>`;
  wire();
  show(tab);
  mountJobAmoeba();
}
function show(k, focus) {
  tab = TABS.includes(k) ? k : 'result';
  $$('[role=tab]', root).forEach((b) => {
    const on = b.dataset.tab === tab;
    b.setAttribute('aria-selected', String(on)); b.tabIndex = on ? 0 : -1;
    const p = document.getElementById(b.getAttribute('aria-controls')); if (p) p.hidden = !on;
    if (on && focus) b.focus();
  });
  if (tab === 'pattern' && !disposePattern) { const host = $('#pattern-host', root); if (host) disposePattern = mountPattern(host, c); }
}
function goTab(k) { show(k, true); location.hash = `#/job/${c.id}/${k}`; }
function markField(input, msgId, html) {
  input.setAttribute('aria-invalid', 'true'); input.setAttribute('aria-describedby', msgId);
  const box = input.closest('.field') || input.parentElement;
  let m = box.querySelector(`#${msgId}`);
  if (!m) { m = el('div', { class: 'alert bad', id: msgId, role: 'alert' }); box.append(m); }
  m.innerHTML = html;
  input.addEventListener('input', () => { input.removeAttribute('aria-invalid'); input.removeAttribute('aria-describedby'); m.remove(); }, { once: true });
  input.focus();
}
function openCorr(key) {
  const body = el('div', { class: 'calc-form' });
  body.innerHTML = `${sub(t('corr.p'))}
    <div class="field"><label for="corr-p">${esc(t('corr.param'))}</label><select id="corr-p">${c.correctable_parameters.map(([k, v]) => `<option value="${esc(k)}"${k === key ? ' selected' : ''}>${esc(t('corr.opt', { label: plabel(fam, k), key: k, v: fmt(v) }))}</option>`).join('')}</select></div>
    <div class="field"><label for="corr-v">${esc(t('corr.value'))}</label><input id="corr-v" type="text" inputmode="decimal" autocomplete="off" autocapitalize="off" spellcheck="false"></div>
    <div class="field"><label for="corr-r">${esc(t('corr.reason'))}</label><input id="corr-r" placeholder="${esc(t('corr.reason.ph'))}"></div>
    <div id="corr-msg"></div>`;
  dlg = modal({
    title: t('corr.h', { id: c.id }), body,
    actions: [
      { label: t('cancel'), onclick: (e, m) => { m.close(); dlg = null; } },
      { label: t('corr.go'), type: 'primary', onclick: (e) => submitCorr(body, e.currentTarget) },
    ],
  });
  $('#corr-v', body).focus();
}
async function submitCorr(body, btn) {
  const p = $('#corr-p', body), v = $('#corr-v', body), r = $('#corr-r', body), msg = $('#corr-msg', body);
  msg.innerHTML = '';
  const k = p.value, nv = parseDim(v.value), now = c.request[k];
  if (!v.value.trim() || !Number.isFinite(nv)) return markField(v, 'corr-err', esc(t('corr.e.num')));
  if (nv === now) return markField(v, 'corr-err', esc(t('corr.e.same', { v: fmt(now) })));
  if (r.value.trim().length < 5) return markField(r, 'corr-err', esc(t('corr.e.reason')));
  busy(btn, true);
  try {
    const res = await api(`/api/calculations/${c.id}/corrections`, { method: 'POST', json: { parameter: k, new_value: nv, reason: r.value } });
    busy(btn, false);
    if (dead) return;
    toast(t('corr.toast', { label: plabel(fam, k), a: fmt(now), b: fmt(nv), id: res.new_calculation, n: res.obligations, open: res.open }), 'warn');
    if (dlg) { dlg.close(); dlg = null; }
    location.hash = `#/job/${res.new_calculation}`;
  } catch (err) {
    busy(btn, false);
    if (dead) return;
    const ref = err.detail && (err.detail.refusal || (err.detail.state === 'REFUSED' ? err.detail : null));
    if (ref && (ref.fields || []).includes(k)) markField(v, 'corr-err', refusalHtml(ref));
    else msg.innerHTML = alert(errHtml(err));
  }
}
async function reload() {
  const my = ++gen;
  try {
    const [nc, nd, list] = await Promise.all([api(`/api/calculations/${c.id}`), api(`/api/calculations/${c.id}/dependencies`), api('/api/calculations')]);
    if (dead || my !== gen) return;
    c = nc; dep = nd; meta = (list || []).find((x) => x.id === c.id) || null;
    prev = null;
    if (c.supersedes) { try { prev = await api(`/api/calculations/${c.supersedes}`); } catch { prev = null; } }
    if (dead || my !== gen) return;
    fam = c.request.family;
    state = c.superseded_by ? 'SUPERSEDED' : c.state === 'REFUSED' ? 'REFUSED' : !c.patterns.length ? 'NO_RESULT' : c.state;
    if (disposePattern) { disposePattern(); disposePattern = null; }
    render();
  } catch (err) { if (!dead) toast(String(err.message || err), 'err'); }
}
function wire() {
  const list = $('[role=tablist]', root);
  list.addEventListener('click', (e) => { const b = e.target.closest('[role=tab]'); if (b) goTab(b.dataset.tab); });
  list.addEventListener('keydown', (e) => {
    const tabs = $$('[role=tab]', list), i = tabs.indexOf(document.activeElement); if (i < 0) return;
    const j = e.key === 'ArrowRight' ? (i + 1) % tabs.length : e.key === 'ArrowLeft' ? (i - 1 + tabs.length) % tabs.length : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : -1;
    if (j >= 0) { e.preventDefault(); goTab(tabs[j].dataset.tab); }
  });
  $$('#tp-result [data-gf]', root).forEach((b) => { b.onclick = () => { gf = b.dataset.gf; $$('#tp-result [data-gf]', root).forEach((x) => x.setAttribute('aria-pressed', String(x === b))); $$('#gates > li', root).forEach((li) => { li.hidden = gf !== 'all' && li.dataset.result !== gf; }); }; });
  $$('#tp-result [data-scope]', root).forEach((b) => { b.onclick = () => { scope = b.dataset.scope; $$('#tp-result [data-scope]', root).forEach((x) => x.setAttribute('aria-pressed', String(x === b))); $$('#checks > li', root).forEach((li) => { li.hidden = scope !== 'all' && li.dataset.scope !== scope; }); }; });
  const gall = $('#g-all', root);
  if (gall) gall.onclick = () => { const open = gall.dataset.open !== '1'; $$('#gates details', root).forEach((d) => { d.open = open; }); gall.dataset.open = open ? '1' : '0'; gall.textContent = t(open ? 'g.collapse' : 'g.expand'); };
  const rb = $('#replay', root);
  if (rb) rb.onclick = async () => { busy(rb, true); try { const r = await api(`/api/calculations/${c.id}/replay`, { method: 'POST' }); if (dead) return; const bad = r && r.matches === false; toast(t(bad ? 'replay.t.bad' : 'replay.t.ok', { id: c.id }), bad ? 'err' : 'ok'); reload(); } catch (err) { if (!dead) { busy(rb, false); toast(String(err.message || err), 'err'); } } };
  const co = $('#corr-open', root); if (co) co.onclick = () => openCorr();
  $$('[data-correct]', root).forEach((b) => { b.onclick = () => openCorr(b.dataset.correct); });
  const gp = $('#go-pattern', root); if (gp) gp.onclick = () => goTab('pattern');
  const da = $('#dxf-all', root);
  if (da) da.onclick = async () => {
    const parts = c.patterns.filter((p) => servable(p.part));
    for (const p of parts) {
      if (dead) return;
      const a = el('a', { href: `/api/calculations/${c.id}/dxf/${encodeURIComponent(p.part)}`, download: '' });
      document.body.append(a); a.click(); a.remove();
      await new Promise((res) => later(res, 350));
    }
    if (!dead) toast(t('dxf.all.toast', { n: parts.length }), 'warn');
  };
  $$('[data-ack]', root).forEach((btn) => { btn.onclick = async () => {
    const input = $(`[data-advreason="${btn.dataset.ack}"]`, root);
    if (input.value.trim().length < 5) return markField(input, `adv-err-${btn.dataset.ack}`, esc(t('s.e.adv')));
    busy(btn, true);
    try { await api(`/api/advisories/${btn.dataset.ack}/acknowledge`, { method: 'POST', json: { reason: input.value.trim() } }); if (dead) return; toast(t('s.t.adv', { id: btn.dataset.ack }), 'ok'); reload(); }
    catch (err) { if (!dead) { busy(btn, false); toast(String(err.message || err), 'err'); } }
  }; });
  $$('[data-resolve]', root).forEach((btn) => { btn.onclick = async () => {
    const oid = btn.dataset.resolve, input = $(`[data-reason="${oid}"]`, root);
    if (input.value.trim().length < 5) return markField(input, `obl-err-${oid}`, esc(t('s.e.obl')));
    busy(btn, true);
    try { await api(`/api/obligations/${oid}/resolve`, { method: 'POST', json: { decision: btn.dataset.decision, reason: input.value.trim() } }); if (dead) return; toast(t('s.t.obl', { id: oid, decision: t(`dec.${btn.dataset.decision}`) }), 'ok'); reload(); }
    catch (err) { if (!dead) { busy(btn, false); toast(String(err.message || err), 'err'); } }
  }; });
}
function renderMissing(id) {
  root.innerHTML = `<div class="workspace"><div class="ws-content">${panel(esc(t('calc.notfound.title')), alert(`${esc(t('calc.notfound.t', { id }))}<p class="sub">${esc(t('calc.notfound.p'))}</p>
    <div class="row"><a class="btn btn-primary" href="#/workspace/calculations">${esc(t('jobs.all'))}</a></div>`))}</div></div>`;
}
function renderError(err, params) {
  root.innerHTML = `<div class="workspace"><div class="ws-content">${panel(esc(t('err.title')), alert(`${errHtml(err)}<div class="row"><button type="button" class="btn btn-primary" id="retry">${esc(t('retry'))}</button><a class="btn" href="#/workspace/calculations">${esc(t('jobs.all'))}</a></div>`))}</div></div>`;
  $('#retry', root).onclick = () => mount(root, params);
}

export async function mount(container, params = {}) {
  unmount();
  dead = false;
  const my = ++gen;
  root = container;
  const id = params.id ?? params[0];
  tab = TABS.includes(params.tab) ? params.tab : 'result';
  scope = 'all'; gf = 'all';
  container.innerHTML = `<div class="workspace"><div class="ws-content">${sub(t('loading'))}</div></div>`;
  if (!/^\d+$/.test(String(id))) return renderMissing(id);
  let list;
  try {
    [c, dep, list] = await Promise.all([api(`/api/calculations/${id}`), api(`/api/calculations/${id}/dependencies`), api('/api/calculations')]);
  } catch (err) {
    if (dead || my !== gen) return;
    if (err.status === 404 || /not found/i.test(err.message || '')) return renderMissing(id);
    return renderError(err, params);
  }
  if (dead || my !== gen) return;
  prev = null;
  if (c.supersedes) { try { prev = await api(`/api/calculations/${c.supersedes}`); } catch { prev = null; } }
  if (dead || my !== gen) return;
  meta = (list || []).find((x) => x.id === c.id) || null;
  fam = c.request.family;
  state = c.superseded_by ? 'SUPERSEDED' : c.state === 'REFUSED' ? 'REFUSED' : !c.patterns.length ? 'NO_RESULT' : c.state;
  render();
}

export function unmount() {
  dead = true; gen++;
  timers.forEach(clearTimeout); timers.clear();
  if (disposePattern) { disposePattern(); disposePattern = null; }
  unmountAmoebas(amHandles); amHandles = [];
  if (dlg) { dlg.close(); dlg = null; }
  if (root) root.innerHTML = '';
  root = null; c = null; prev = null; dep = null; meta = null;
}
