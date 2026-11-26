import { api, esc } from '../api.js';
import { getState } from '../store.js';
import { t, h, idLabel, engineJa, lang } from '../i18n.js';
import { el, $, $$ } from '../ui.js';

let host = null, root = null, ac = null, projects = [];

const ja = () => lang() === 'ja';
const words = (s) => esc(String(s ?? '').replaceAll('_', ' '));
const mono = (s) => `<span class="mono">${esc(s)}</span>`;
const ud = (v) => `<bdi>${esc(v ?? '')}</bdi>`;
const pad2 = (n) => String(n).padStart(2, '0');
const when = (ts) => { if (!ts) return '—'; const d = new Date(typeof ts === 'number' && ts < 1e12 ? ts * 1000 : ts); return Number.isNaN(d.getTime()) ? esc(ts) : `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`; };
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : esc(n));
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
// computed numbers: 3 decimals for reading, the value the API returned on hover
const num = (v) => (typeof v === 'number' ? `<span class="mono" title="${esc(String(v))}">${n3(v)}</span>` : esc(v));
const raw = (v) => (typeof v === 'number' ? `<span class="mono" title="${esc(String(v))}">${fmt(v)}</span>` : esc(v));
const jaText = (s) => esc(s).replace(/`([^`]+)`/g, '<span class="mono">$1</span>');
const tagHtml = (kind, text) => `<span class="${kind ? `chip ${kind}` : 'tag'}">${esc(text)}</span>`;

const idText = (kind, id) => idLabel(kind, id) || (ja() ? String(id) : String(id).replaceAll('_', ' '));
const labelled = (kind, id) => { const l = idLabel(kind, id); return l ? esc(l) : ja() ? mono(id) : words(id); };
const named = (kind, id) => { const l = idLabel(kind, id); if (!ja()) return l ? esc(l) : words(id); return l ? `${esc(l)} ${mono(id)}` : mono(id); };
const famShort = (k) => esc(idText('family', k));
const nodeName = (kind, name) => (ja() ? (kind === 'parameter' || kind === 'convention' ? idLabel('param', name) : kind ? idLabel('part', name) : null) || idLabel('dlabel', name) || idLabel('rkind', name) : null);
const oblSubject = (o) => `${labelled('rkind', o.kind)} · ${nodeName(o.kind, o.label) ? `${esc(nodeName(o.kind, o.label))} ` : ''}${mono(o.label)}`;

// prose the engine or the server wrote: reviewed translation when known, otherwise the original, marked as such
const eng = (text) => {
  const s = String(text ?? ''); if (!s) return '';
  if (!ja()) return `<span class="eng" lang="en">${esc(s)}</span>`;
  const j = engineJa(s);
  if (j) return `<span lang="ja" title="${esc(s)}">${jaText(j)}</span>`;
  return `${tagHtml('', t('eng.tag'))} <span lang="en">${esc(s)}</span>`;
};
const errText = (err) => {
  const m = String((err && err.message) || err || '');
  return ja() ? `${tagHtml('', t('srv.tag'))} <span lang="en">${esc(m)}</span>` : esc(m);
};

const STATES = ['APPROVED', 'REJECTED', 'SUPERSEDED', 'NO_RESULT', 'REVIEW_REQUIRED', 'REFUSED'];
const STATE_KIND = { APPROVED: 'ok', REJECTED: 'bad', SUPERSEDED: '', NO_RESULT: 'bad' };
const stateText = (s) => (STATES.includes(s) ? t(`state.${s}`) : String(s ?? '').replaceAll('_', ' '));
const stateTag = (s) => tagHtml(STATE_KIND[s] === undefined ? 'warn' : STATE_KIND[s], stateText(s));
const oblText = (s) => (['open', 'discharged', 'discharged_by_decision', 'waived'].includes(s) ? t(`obl.${s}`) : String(s ?? '').replaceAll('_', ' '));
const effState = (c) => (c.superseded || (c.facts && c.facts.superseded_by) ? 'SUPERSEDED' : c.no_result || (c.facts && c.facts.noResult) ? 'NO_RESULT' : c.state);
const replayTag = (m) => (m === true ? tagHtml('ok', t('replay.match')) : m === false ? tagHtml('bad', t('replay.mismatch')) : `<span class="sub">${esc(t('replay.none'))}</span>`);
const gateCounts = (g) => {
  const [p, i, f, n, r] = g;
  return `${tagHtml('ok', t('g.n.pass', { n: p }))} ${tagHtml('warn', t('g.n.inc', { n: i }))}${f ? ` ${tagHtml('bad', t('g.n.fail', { n: f }))}` : ''} ${tagHtml('', t('g.n.ni', { n }))}${r ? ` ${tagHtml('', t('g.n.nr', { n: r }))}` : ''}`;
};

const pname = (id) => { const p = projects.find((x) => String(x.id) === String(id)); return p ? p.name : null; };
const pnameHtml = (id) => { const n = pname(id); return n === null ? esc(t('project.n', { id })) : ud(n); };
const plink = (id) => `<a href="#/project/${esc(id)}" title="${esc(t('project.tip', { id }))}">${pnameHtml(id)}</a>`;
const jobLink = (id) => `<a href="#/job/${esc(id)}">${esc(t('job.n', { id }))}</a>`;
const byWhen = (who, ts) => `${ud(who)}<br><span class="sub">${when(ts)}</span>`;
const role = () => { const st = getState() || {}; return (st.user && st.user.role) || st.role || ''; };
const newCalcBtn = () => (role() === 'viewer' ? '' : `<a class="btn btn-primary" href="#/math">${esc(t('new.calc'))}</a>`);

const panel = (title, body, o = {}) => `<section class="panel"${o.id ? ` id="${esc(o.id)}"` : ''}><div class="panel-h"><h2>${title}</h2>${o.count !== undefined ? `<span class="cnt">${esc(o.count)}</span>` : ''}${o.tools || ''}</div><div class="panel-b">${body}</div>${o.foot ? `<div class="panel-b sub">${o.foot}</div>` : ''}</section>`;
const empty = (title, text, actions = '') => `<div class="card"><b>${esc(title)}</b><p class="sub">${text}</p>${actions ? `<div class="row">${actions}</div>` : ''}</div>`;
const filterBox = (target, label) => `<div class="field"><label class="sub" for="flt-${esc(target)}">${esc(label)}</label><input id="flt-${esc(target)}" type="search" data-filter="${esc(target)}" autocomplete="off"></div>`;
const noJobs = () => empty(t('jobs.none.t'), esc(t('jobs.none.p')), (role() === 'viewer' ? '' : `<a class="btn" href="#/workspace/projects">${esc(t('jobs.none.cta'))}</a>`) + newCalcBtn());
const planned = (prefix, n) => `<aside class="card" aria-label="${esc(t('planned.aria', { title: t(`${prefix}.t`) }))}"><div class="row">${tagHtml('', t('planned.tag'))}<b>${esc(t(`${prefix}.t`))}</b><span class="sub">${esc(t('planned.sub'))}</span></div><ul class="il">${Array.from({ length: n }, (_, i) => `<li>${jaText(t(`${prefix}.${i + 1}`))}</li>`).join('')}</ul></aside>`;

const summarise = (c) => ({
  noResult: !(c.patterns || []).length,
  superseded_by: c.superseded_by,
  gates: ['PASS', 'INCONCLUSIVE', 'FAIL', 'NOT_IMPLEMENTED', 'NOT_RUN'].map((r) => (c.gates || []).filter((g) => g.result === r).length),
  checks: [(c.checks || []).filter((k) => k.verdict === 'PASS').length, (c.checks || []).length],
  advOpen: (c.advisories || []).filter((a) => a.state === 'open').length,
  replay: c.replay ? c.replay.matches : null,
});

const openTable = (rows) => `<div class="table"><table><thead><tr><th>#</th><th>${esc(t('th.job'))}</th><th>${esc(t('th.affected'))}</th><th>${esc(t('th.required'))}</th><th>${esc(t('th.raised'))}</th></tr></thead><tbody>${rows.map((o) => `<tr><td class="mono">${esc(o.id)}</td><td>${jobLink(o.target_calc)}<br><span class="sub">${esc(t('ver.corrects', { id: o.origin_calc }))}</span>${o.target_superseded ? `<br>${tagHtml('', t('obl.onsup'))}` : ''}</td><td>${oblSubject(o)}</td><td>${eng(o.required_check)}</td><td>${when(o.created_at)}</td></tr>`).join('')}</tbody></table></div>`;

const statusTable = (live) => `<div class="table"><table id="jobs-ver"><thead><tr><th>${esc(t('th.job'))}</th><th>${esc(t('th.project'))}</th><th>${esc(t('th.state.short'))}</th><th>${esc(t('th.gates'))}</th><th>${esc(t('th.checks'))}</th><th>${esc(t('th.oblopen'))}</th><th>${esc(t('th.advopen'))}</th><th>${esc(t('th.replay'))}</th></tr></thead><tbody>${live.map((c) => `<tr><td>${jobLink(c.id)}<br><span class="sub">${famShort(c.family)}</span></td><td>${plink(c.project_id)}</td><td>${stateTag(effState(c))}${c.open_obligations && !c.superseded ? ` ${tagHtml('warn', t('obl.open.n', { n: c.open_obligations }))}` : ''}</td><td>${c.facts ? gateCounts(c.facts.gates) : '—'}</td><td>${c.facts ? (c.facts.checks[1] ? esc(t('ver.checks', { a: c.facts.checks[0], b: c.facts.checks[1] })) : esc(t('ver.checks.none'))) : '—'}</td><td>${esc(c.open_obligations ?? '—')}</td><td>${c.facts ? esc(c.facts.advOpen) : '—'}</td><td>${replayTag(c.facts ? c.facts.replay : null)}</td></tr>`).join('')}</tbody></table></div>`;

const lessonsTable = (lessons) => (lessons.length ? `<div class="table"><table><thead><tr><th>#</th><th>${esc(t('th.what'))}</th><th>${esc(t('th.fromjob'))}</th><th>${esc(t('th.matched'))}</th><th>${esc(t('th.recorded'))}</th></tr></thead><tbody>${lessons.map((l) => `<tr><td class="mono">${esc(l.id)}</td><td>${l.kind === 'corrected_value'
  ? `${h('lesson.corrected', { fam: famShort(l.family), param: mono(l.parameter), wrong: num(l.wrong_value), right: num(l.right_value) })}<br><span class="sub">${ud(l.detail)}</span>`
  : h('lesson.other', { kind: named('lesson', l.kind), fam: famShort(l.family) })}</td><td>${jobLink(l.source_calc)}<br><span class="sub">${pnameHtml(l.project_id)}</span></td><td>${raw(l.times_matched)}</td><td>${byWhen(l.created_by, l.created_at)}</td></tr>`).join('')}</tbody></table></div>`
  : empty(t('lessons.none.t'), esc(t('lessons.none.p'))));

function wireFilters(scope) {
  $$('input[data-filter]', scope).forEach((inp) => {
    const head = inp.closest('.panel-h'), cnt = head && $('.cnt', head), total = cnt ? cnt.textContent : '';
    const run = () => {
      const q = inp.value.trim().toLowerCase(), body = $(`#${inp.dataset.filter} tbody`, scope);
      if (!body) return;
      const old = $('.nomatch', body); if (old) old.remove();
      const rows = $$('tr', body); let shown = 0;
      rows.forEach((tr) => { tr.hidden = !!q && !tr.textContent.toLowerCase().includes(q); if (!tr.hidden) shown++; });
      if (cnt) cnt.textContent = q ? t('flt.count', { a: shown, b: rows.length }) : total;
      if (q && !shown) {
        const tr = el('tr', { class: 'nomatch' });
        tr.innerHTML = `<td colspan="20">${h('flt.nomatch', { q: ud(inp.value.trim()) })} <button class="btn btn-text" type="button">${esc(t('flt.clear'))}</button></td>`;
        body.append(tr);
        $('button', tr).addEventListener('click', () => { inp.value = ''; run(); inp.focus(); });
      }
    };
    inp.addEventListener('input', run);
  });
}

function render({ cs, obs, lessons }) {
  const open = obs.filter((o) => o.state === 'open');
  const openNow = open.filter((o) => !o.target_superseded), openOld = open.filter((o) => o.target_superseded);
  const live = cs.filter((c) => effState(c) !== 'SUPERSEDED');
  const ledger = `<a href="#/workspace/obligations">${esc(t('ver.ledger'))}</a>`;
  root.innerHTML = `<div class="panel-h"><h1>${esc(t('nav.verification'))}</h1>${newCalcBtn()}</div>
    <p class="sub">${esc(t('ver.sub'))}</p>
    ${panel(esc(t('ver.open')), open.length ? openTable([...openNow, ...openOld]) : empty(t('ver.open.none.t'), esc(t('ver.open.none.p'))), {
      id: 'p-open', count: openNow.length,
      foot: h('ver.open.foot', { n: obs.length, ledger }) + (openOld.length ? ` ${esc(t('obl.onsup.note', { n: openOld.length }))}` : ''),
    })}
    ${panel(esc(t('ver.status')), live.length ? statusTable(live) : noJobs(), {
      count: live.length, tools: live.length ? filterBox('jobs-ver', t('jobs.filter')) : '',
      foot: jaText(t('ver.status.foot', { n: cs.length - live.length })),
    })}
    ${panel(esc(t('ver.lessons')), lessonsTable(lessons), { id: 'p-lessons', count: lessons.length, foot: esc(t('ver.lessons.foot')) })}
    ${planned('ver.planned', 4)}`;
  wireFilters(root);
}

function showError(err) {
  root.innerHTML = `<div class="alert bad" role="alert"><p>${errText(err)}</p><div class="row"><button class="btn" type="button" id="ver-retry">${esc(t('retry'))}</button></div></div>`;
  const b = $('#ver-retry', root);
  if (b) b.addEventListener('click', () => { if (host) mount(host, {}); });
}

export async function mount(container, params = {}) {
  unmount();
  host = container;
  ac = new AbortController();
  const signal = ac.signal;
  root = el('div', { class: 'workspace', id: 'verification' });
  container.textContent = '';
  container.append(root);
  root.innerHTML = `<p class="sub" role="status">${esc(t('loading'))}</p>`;
  let cs, obs, lessons, ps;
  try {
    [cs, obs, lessons, ps] = await Promise.all([api('/api/calculations'), api('/api/obligations'), api('/api/lessons'), api('/api/projects')]);
  } catch (err) { if (!signal.aborted) showError(err); return; }
  if (signal.aborted) return;
  projects = Array.isArray(ps) ? ps : [];
  cs = Array.isArray(cs) ? cs : [];
  obs = Array.isArray(obs) ? obs : [];
  lessons = Array.isArray(lessons) ? lessons : [];
  // the list API carries the state only: each job is read once for its gates, checks, advisories and replay
  const facts = await Promise.all(cs.map((c) => api(`/api/calculations/${c.id}`).then(summarise, () => null)));
  if (signal.aborted) return;
  cs.forEach((c, i) => { c.facts = facts[i]; });
  render({ cs, obs, lessons });
}

export function unmount() {
  if (ac) { ac.abort(); ac = null; }
  if (root) root.remove();
  root = null;
  host = null;
  projects = [];
}
