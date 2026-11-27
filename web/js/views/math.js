// New calculation — routes '#/math', '#/math/{projectId}' and '#/math/{projectId}/{fromId}'. Ported from the old single-file client (views.math).
// This screen only sends engineering inputs and shows what the server answered. There is no control here that
// approves, releases or signs off a result, and no engineering value is guessed: an empty or unreadable field is
// an error, never a default.
import { api, esc, ApiError } from '../api.js';
import { getState } from '../store.js';
import { t, h, has, idLabel, lang, projName } from '../i18n.js';
import { el, $, $$, toast, busy } from '../ui.js';

// ---------------------------------------------------------------- geometry families (inputs the calculation API accepts)
const F = {
  cylinder_shell: { f: [['r_dev', 500, 'mm'], ['length', 1000, 'mm']] },
  cone_frustum: { f: [['r1', 200, 'mm'], ['r2', 100, 'mm'], ['h', 300, 'mm']] },
  eccentric_reducer: { f: [['r1', 200, 'mm'], ['r2', 100, 'mm'], ['h', 300, 'mm'], ['e', 100, 'mm']] },
  header_with_branch: { f: [['header_radius', 150, 'mm'], ['header_dev_radius', 150, 'mm'], ['header_length', 1200, 'mm'], ['branch_station', 600, 'mm'], ['branch_radius', 100, 'mm'], ['branch_dev_radius', 100, 'mm'], ['beta_deg', 60, 'deg'], ['delta', 0, 'mm'], ['s_end', 400, 'mm']], sref: true, ext: true },
  branch_stub: { f: [['header_radius', 150, 'mm'], ['branch_radius', 100, 'mm'], ['branch_dev_radius', 100, 'mm'], ['beta_deg', 60, 'deg'], ['delta', 0, 'mm'], ['s_end', 400, 'mm']], sref: true },
};
const POSITIVE = ['r_dev', 'length', 'r1', 'r2', 'h', 'header_radius', 'header_dev_radius', 'header_length', 'branch_radius', 'branch_dev_radius', 's_end'];
const SURFACES = ['unknown', 'outside', 'inside', 'neutral'];

// ---------------------------------------------------------------- numbers
// A dimension is accepted only as plain digits with an optional sign, one decimal point and an optional exponent.
// Full-width characters a Japanese IME produces are read as ASCII; a comma stays refused (BH-001).
const DIM = /^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/;
const FW = { '．': '.', '－': '-', '−': '-', '＋': '+', 'ｅ': 'e', 'Ｅ': 'E', '，': ',' };
const toAscii = (s) => String(s ?? '').replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0)).replace(/[．－−＋ｅＥ，]/g, (c) => FW[c]);
// Text is trimmed exactly as the server trims it (Rust str::trim, Unicode White_Space): String.trim() also trims
// U+FEFF and keeps U+0085. This form has no free-text field, so no character length rule applies here; the
// dimension text is read with the server's own trimming.
const RS_WS = '\\u0009-\\u000D\\u0020\\u0085\\u00A0\\u1680\\u2000-\\u200A\\u2028\\u2029\\u202F\\u205F\\u3000';
const RS_TRIM = new RegExp(`^[${RS_WS}]+|[${RS_WS}]+$`, 'g');
const trimRs = (v) => String(v ?? '').replace(RS_TRIM, '');
const parseDim = (s) => { const v = trimRs(toAscii(s)); return DIM.test(v) ? Number(v) : NaN; };
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : String(n ?? ''));
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));

const ja = () => lang() === 'ja';
const famLabel = (k) => idLabel('familyLong', k) || idLabel('family', k) || String(k);
const plabel = (fam, key) => idLabel('param', `${fam}.${key}`) || idLabel('param', key) || (ja() ? key : String(key).replaceAll('_', ' '));
const unitText = (u) => (u === 'deg' ? t('unit.deg') : u);
const STATES = ['APPROVED', 'REJECTED', 'SUPERSEDED', 'NO_RESULT', 'REVIEW_REQUIRED', 'REFUSED'];
const stateText = (s) => (STATES.includes(s) ? t(`state.${s}`) : String(s ?? '').replaceAll('_', ' '));
const numText = (v) => (v === undefined || v === null ? '' : String(v));
const errText = (err) => (err && err.message ? String(err.message) : err && err.status ? t('api.failed', { status: err.status }) : t('api.unreachable'));

// ---------------------------------------------------------------- input checks before a request is sent
function validateRequest(fam, v) {
  const errors = [], warnings = [], lab = (k) => plabel(fam, k);
  for (const [k] of F[fam].f) {
    if (!Number.isFinite(v[k])) { errors.push([k, t('v.number', { label: lab(k) })]); continue; }
    if (POSITIVE.includes(k) && v[k] <= 0) errors.push([k, t('v.positive', { label: lab(k) })]);
    else if (POSITIVE.includes(k) && v[k] < 1e-6) errors.push([k, t('v.below', { label: lab(k), v: fmt(v[k]) })]);
    else if (k !== 'beta_deg' && Math.abs(v[k]) > 20000) warnings.push([k, t('v.big', { label: lab(k), v: fmt(v[k]) })]);
    else if (POSITIVE.includes(k) && v[k] < 1) warnings.push([k, t('v.small', { label: lab(k), v: fmt(v[k]) })]);
  }
  const have = (...ks) => ks.every((k) => k in v && Number.isFinite(v[k]));
  const bad = (k) => errors.some((e) => e[0] === k);
  if (have('r1', 'r2') && !bad('r1') && !bad('r2') && v.r1 <= v.r2) errors.push(['r1', t('v.r1r2')]);
  if (have('e') && v.e < 0) errors.push(['e', t('v.e')]);
  if (have('beta_deg') && (v.beta_deg < 10 || v.beta_deg > 170)) errors.push(['beta_deg', t('v.beta')]);
  if (have('header_radius', 'branch_radius', 'delta') && !bad('header_radius') && !bad('branch_radius') && Math.abs(v.delta) + v.branch_radius >= v.header_radius) errors.push(['branch_radius', t('v.tangent', { sum: fmt(Math.abs(v.delta) + v.branch_radius), R: fmt(v.header_radius) })]);
  if (have('branch_station', 'header_length') && !bad('header_length') && (v.branch_station <= 0 || v.branch_station >= v.header_length)) errors.push(['branch_station', t('v.station')]);
  const sref = (part) => v[`${part}_surface_reference`] || v.surface_reference;
  for (const [c, d, part] of [['header_radius', 'header_dev_radius', 'header'], ['branch_radius', 'branch_dev_radius', 'branch']]) {
    const ref = sref(part);
    if (!ref || !have(c, d) || bad(c) || bad(d)) continue;
    const same = Math.abs(v[d] - v[c]) <= 1e-12 * Math.max(Math.abs(v[d]), Math.abs(v[c]));
    const p = { d: lab(d), dv: fmt(v[d]), c: lab(c), cv: fmt(v[c]) };
    if (ref === 'outside' && !same && v[d] > v[c]) errors.push([d, t('v.outside', p)]);
    if (ref === 'inside' && !same && v[d] < v[c]) errors.push([d, t('v.inside', p)]);
    if (ref === 'neutral' && !same) errors.push([d, t('v.neutral', p)]);
  }
  if (v.joint === 'set_on') {
    const hs = sref('header'), bs = sref('branch');
    if (hs !== 'outside' || bs !== 'inside') errors.push(['joint', t('v.joint.pair', { hs: t(`sref.${hs || 'unknown'}`), bs: t(`sref.${bs || 'unknown'}`) })]);
  }
  return { errors, warnings };
}

// ---------------------------------------------------------------- kept values ("new from these values")
function takeDraft() {
  try {
    const raw = sessionStorage.getItem('nf-draft'); if (!raw) return null;
    const d = JSON.parse(raw); if (!d) return null;
    const here = location.hash, mine = /^#\/math(\/|$)/;
    if (d.hash !== here && !(mine.test(String(d.hash)) && mine.test(here))) return null;
    sessionStorage.removeItem('nf-draft');
    return d;
  } catch { return null; }
}
function draftValues(d) {
  const m = new Map(); if (!d) return m;
  if (Array.isArray(d.fields)) for (const pair of d.fields) { if (Array.isArray(pair)) m.set(String(pair[0]).replace(/^f_/, ''), pair[1]); }
  const req = d.request || d.values;
  if (req && typeof req === 'object') for (const [k, v] of Object.entries(req)) m.set(k, v);
  return m;
}

// ---------------------------------------------------------------- field marks (BH-014)
// A field's message is part of its accessible description: a screen reader reads it again whenever the field gets
// focus. The ids of any other description (unit, hint) are kept.
function markField(input, html, focus) {
  const wrap = input.closest('.field') || input.parentElement;
  const id = `${input.id || input.name}-err`;
  let m = wrap ? $(`[data-err="${input.name}"]`, wrap) : null;
  if (!m) { m = el('div', { class: 'alert bad', id, role: 'alert', 'data-err': input.name }); if (wrap) wrap.append(m); else input.after(m); }
  m.innerHTML = html;
  input.setAttribute('aria-invalid', 'true');
  const ids = (input.getAttribute('aria-describedby') || '').split(/\s+/).filter(Boolean);
  if (!ids.includes(id)) ids.push(id);
  input.setAttribute('aria-describedby', ids.join(' '));
  if (focus) input.focus();
}
function clearMark(input) {
  input.removeAttribute('aria-invalid');
  const wrap = input.closest('.field') || input.parentElement;
  if (wrap) $$('[data-err]', wrap).forEach((m) => m.remove());
  const rest = (input.getAttribute('aria-describedby') || '').split(/\s+/).filter((x) => x && !x.endsWith('-err'));
  if (rest.length) input.setAttribute('aria-describedby', rest.join(' ')); else input.removeAttribute('aria-describedby');
}
const clearMarks = (form) => $$('[aria-invalid]', form).forEach(clearMark);
// A refusal is shown by its code with the values it names; a code without a message shows the server's text.
function refusalHtml(r) {
  if (r && r.code && has(`refusal.${r.code}`)) {
    const d = r.detail || {};
    return h(`refusal.${r.code}`, {
      fields: esc((r.fields || []).join(', ')),
      limit: esc(numText(d.limit ?? r.limit)),
      measured: esc(numText(d.measured ?? r.measured)),
    });
  }
  const kind = r && r.kind ? idLabel('rkind', r.kind) : null;
  const text = r ? (r.message || r.error || r.code || '') : '';
  return `${kind ? `${esc(kind)}: ` : ''}${esc(text)}`;
}
// BH-014: mark every field a refusal names with the message of its code, in the active language. `map` turns a
// request field into the form's control name where they differ. The fields the form has no control for are
// returned, so the caller states them in the message area: no refusal is ever left unshown.
function markRefusal(form, r, map = {}) {
  const missing = []; let marked = 0;
  if (!form || !r) return { marked, missing };
  for (const k of r.fields || []) {
    const i = form.elements.namedItem(map[k] || k);
    if (i && i.nodeType === 1) { markField(i, refusalHtml(r), marked === 0); marked++; } else missing.push(String(k));
  }
  return { marked, missing };
}
// the coded refusal of a failed request (400 field_missing / field_wrong_type / field_unknown / family_unknown /
// surface_reference_required, or a 422 of the engine), or null
function coded(err) {
  const d = (err && err.detail) || {};
  if (d.refusal && typeof d.refusal === 'object') return d.refusal;
  return typeof d.code === 'string' || Array.isArray(d.fields) ? d : null;
}
// a refusal in the form's message area (a live region): `lead` (HTML) or the text by code, and the fields the
// form cannot show
const refusalNote = (r, missing, lead) => `${lead || refusalHtml(r)}${missing.length ? ` ${h('refusal.unmapped', { fields: missing.map((f) => `<span class="mono">${esc(f)}</span>`).join(', ') })}` : ''}`;

// ---------------------------------------------------------------- form fragments
function selectField(name, label, opts, selected, hint, withKey = true) {
  const id = `f_${name}`;
  return el('div', { class: 'field' },
    el('label', { for: id }, label, withKey ? el('span', { class: 'mono' }, ` ${name}`) : null),
    el('select', { id, name, 'aria-describedby': hint ? `${id}_h` : null }, ...opts.map(([v, text]) => el('option', { value: v, selected: String(v) === String(selected ?? '') }, text))),
    hint ? el('p', { class: 'sub', id: `${id}_h` }, hint) : null);
}
function paramField(fam, key, unit, value) {
  const id = `f_${key}`;
  return el('div', { class: 'field' },
    el('label', { for: id }, plabel(fam, key), el('span', { class: 'mono' }, ` ${key}`)),
    el('div', { class: 'row' },
      el('input', { id, name: key, type: 'text', inputmode: 'decimal', autocomplete: 'off', autocapitalize: 'off', spellcheck: 'false', 'data-dim': '1', 'aria-describedby': `u_${key}`, value: String(value) }),
      el('span', { class: 'sub', id: `u_${key}` }, unitText(unit))));
}
function familyFields(fam, dv) {
  const val = (k, example) => (dv.has(k) && dv.get(k) !== '' && dv.get(k) !== null ? dv.get(k) : example);
  const out = F[fam].f.map(([k, example, unit]) => paramField(fam, k, unit, val(k, example)));
  if (F[fam].sref) out.push(selectField('surface_reference', t('math.sref.label'), SURFACES.map((o) => [o, t(`sref.${o}`)]), val('surface_reference', 'unknown')));
  if (F[fam].ext) {
    const surf = [['', t('math.psref.same')], ...SURFACES.map((o) => [o, t(`sref.${o}`)])];
    out.push(selectField('header_surface_reference', plabel(fam, 'header_surface_reference'), surf, val('header_surface_reference', '')));
    out.push(selectField('branch_surface_reference', plabel(fam, 'branch_surface_reference'), surf, val('branch_surface_reference', ''), t('math.psref.hint')));
    out.push(selectField('joint', plabel(fam, 'joint'), [['', t('math.joint.none')], ['set_on', t('math.joint.set')]], val('joint', ''), t('math.joint.hint')));
  }
  return out;
}
const lis = (prefix, n) => el('ul', { class: 'il' }, ...Array.from({ length: n }, (_, i) => el('li', { html: h(`${prefix}.${i + 1}`) })));

// ---------------------------------------------------------------- module state
let root = null, form = null, ac = null, confirmed = '';

export function unmount() {
  if (ac) { ac.abort(); ac = null; }
  if (root) root.remove();
  root = null; form = null; confirmed = '';
}

export async function mount(container, params = {}) {
  unmount();
  ac = new AbortController(); const signal = ac.signal;
  root = el('div', { class: 'card' });
  container.textContent = ''; container.append(root);

  const st = getState() || {};
  const me = st.user || st.me || {};
  const notice = (kind, title, text, href, cta) => {
    root.append(el('div', { class: 'panel' },
      el('div', { class: 'panel-h' }, el('h2', {}, t('new.calc'))),
      el('div', { class: 'panel-b' },
        el('div', { class: `alert ${kind}`, role: 'alert' }, el('b', {}, title), el('p', {}, text)),
        href ? el('div', { class: 'row' }, el('a', { class: 'btn', href }, cta)) : null)));
  };
  if (me.role === 'viewer') { notice('bad', t('math.viewer.t'), t('math.viewer.p'), '#/workspace/calculations', t('math.viewer.go')); return; }

  let projects;
  try { projects = await api('/api/projects'); } catch (err) { if (!signal.aborted) notice('bad', t('err.t'), errText(err), '#/workspace/projects', t('math.noproj.go')); return; }
  if (signal.aborted) return;
  if (!projects || !projects.length) { notice('warn', t('math.noproj.t'), t('math.noproj.p'), '#/workspace/projects', t('math.noproj.go')); return; }

  const wanted = params.projectId ?? params.project ?? params.id ?? (Array.isArray(params) ? params[0] : null);
  const fromId = params.fromId ?? params.from ?? (Array.isArray(params) ? params[1] : null);

  // '#/math/{projectId}/{fromId}' starts from the request of an existing job; the kept draft is only for a blank new calculation.
  const draft = fromId ? null : takeDraft();
  let dv = draftValues(draft), from = null, refusal = null, gone = false;
  if (fromId && me.role !== 'viewer') {
    try {
      const job = await api(`/api/calculations/${fromId}`);
      if (signal.aborted) return;
      from = job && job.request && typeof job.request === 'object' ? job.request : null;
      refusal = job && job.state === 'REFUSED' ? job.refusal || null : null;
      dv = draftValues({ request: from });
      const pj = dv.get('project') ?? (job && (job.project_id ?? job.project));
      if (pj !== undefined && pj !== null) dv.set('project', typeof pj === 'object' ? pj.id : pj);
    } catch (err) {
      if (signal.aborted) return;
      if (err && err.status === 404) { gone = true; from = null; refusal = null; dv = new Map(); }
      else { notice('bad', t('err.t'), errText(err), '#/workspace/calculations', t('math.viewer.go')); return; }
    }
  }

  const sourceId = from ? fromId : draft && (draft.job ?? draft.calculation ?? dv.get('job'));
  const fam0 = F[dv.get('fam') || dv.get('family')] ? (dv.get('fam') || dv.get('family')) : 'cylinder_shell';
  const pid = [wanted, dv.get('project')].map(String).find((x) => projects.some((p) => String(p.id) === x)) || projects[0].id;
  const valuesFor = (k) => (k === fam0 ? dv : new Map());

  const paramsBox = el('div', { class: 'calc-params', id: 'fields' }, ...familyFields(fam0, valuesFor(fam0)));
  const devNote = el('p', { class: 'sub', id: 'devnote' });
  const msg = el('div', { id: 'cmsg', 'aria-live': 'polite' });
  const run = el('button', { class: 'btn pri', id: 'run', type: 'submit' }, t('math.run'));
  const projSel = selectField('project', t('math.f.project'), projects.map((p) => [p.id, `#${p.id} ${projName(p.name)}`]), pid, null, false);
  const famSel = selectField('fam', t('math.f.family'), Object.keys(F).map((k) => [k, famLabel(k)]), fam0, null, false);
  const fromTag = el('span', { class: 'tag' }, sourceId ? t('math.tag.from', { id: sourceId }) : t('math.tag.example'));
  const hint = el('p', { class: 'sub' }, t(from || draft ? 'math.hint.from' : 'math.hint.example'));
  const convSec = el('div', {}, el('h3', {}, t('math.conv.h'), el('span', { class: 'tag' }, t('math.conv.tag'))), el('p', { class: 'sub' }, t('math.conv.lead')), lis('math.conv', 4));

  form = el('form', { class: 'calc-form', id: 'calc', novalidate: true },
    el('h3', {}, t('math.sec.job')), projSel, famSel,
    el('h3', {}, t('math.sec.params'), ' ', fromTag), paramsBox, hint, devNote, msg,
    el('div', { class: 'row' }, run));

  if (gone) root.append(el('div', { class: 'alert warn', role: 'alert' }, el('b', {}, t('math.from.t')), el('p', {}, t('math.from.p', { id: fromId }))));

  root.append(
    el('div', { class: 'panel' },
      el('div', { class: 'panel-h' }, el('h2', {}, t('new.calc')), el('span', { class: 'sub' }, t('math.meta'))),
      el('div', { class: 'panel-b' }, el('div', { class: 'alert warn', html: h('math.banner') }), form)),
    el('div', { class: 'panel' },
      el('div', { class: 'panel-h' }, el('h2', {}, t('math.limits.h'))),
      el('div', { class: 'panel-b' },
        el('h3', {}, t('math.result.h')), lis('math.result', 5),
        convSec,
        el('h3', {}, t('math.planned.h'), el('span', { class: 'tag' }, t('math.planned.tag')), el('span', { class: 'sub' }, ` ${t('planned.sub')}`)),
        lis('math.planned', 6))));

  const famOf = () => form.elements.namedItem('fam').value;
  const read = () => {
    const fam = famOf(), req = { family: fam };
    for (const [k] of F[fam].f) { const i = form.elements.namedItem(k); req[k] = i ? parseDim(i.value) : NaN; }
    if (F[fam].sref) { const sr = form.elements.namedItem('surface_reference'); if (sr) req.surface_reference = sr.value; }
    if (F[fam].ext) for (const n of ['header_surface_reference', 'branch_surface_reference', 'joint']) { const i = form.elements.namedItem(n); if (i && i.value) req[n] = i.value; }
    return req;
  };
  const clearMsg = () => { confirmed = ''; msg.textContent = ''; run.textContent = t('math.run'); };
  const paintDev = () => {
    const fam = famOf(), v = read();
    const items = F[fam].f.filter(([k]) => /dev_radius/.test(k) && fam !== 'cylinder_shell' && Number.isFinite(v[k]) && v[k] > 0)
      .map(([k]) => esc(t('sk.dev', { label: plabel(fam, k), v: fmt(v[k]), w: n3(2 * Math.PI * v[k]) })));
    devNote.innerHTML = items.length ? h('sk.notdrawn', { items: items.join(' · ') }) : '';
    devNote.hidden = !items.length;
    convSec.hidden = !/branch/.test(fam);
  };
  // A refusal is shown on every field it names and said in the message area, with any field this form has no
  // control for (BH-014). `lead` opens the note (the refused job of "new from these values").
  const showRefusal = (r, lead) => {
    const { marked, missing } = markRefusal(form, r, { family: 'fam' });
    msg.append(el('div', { class: 'alert bad', role: 'alert', html: refusalNote(r, missing, lead) }));
    return marked;
  };

  form.elements.namedItem('fam').addEventListener('change', () => {
    paramsBox.textContent = '';
    paramsBox.append(...familyFields(famOf(), valuesFor(famOf())));
    clearMarks(form); clearMsg(); paintDev();
  });
  form.addEventListener('input', (e) => {
    const i = e.target;
    if (i && i.getAttribute && i.getAttribute('aria-invalid') === 'true') clearMark(i);
    clearMsg(); paintDev();
  });
  form.addEventListener('change', (e) => {
    const i = e.target;
    if (i && i.getAttribute && i.getAttribute('aria-invalid') === 'true') clearMark(i);
    clearMsg(); paintDev();
  });
  // On leaving a dimension field its full-width characters are written as ASCII: the value shown is the value computed.
  form.addEventListener('focusout', (e) => {
    const i = e.target;
    if (i instanceof HTMLInputElement && i.hasAttribute('data-dim')) { const a = toAscii(i.value); if (a !== i.value) { i.value = a; paintDev(); } }
  });
  form.addEventListener('keydown', (e) => {
    const i = e.target;
    if (!(i instanceof HTMLInputElement) || !i.hasAttribute('data-dim') || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
    const v = parseDim(i.value); if (!Number.isFinite(v)) return;
    const dec = (/\.(\d+)$/.exec(trimRs(i.value)) || [, ''])[1].length;
    e.preventDefault(); i.value = (v + (e.key === 'ArrowUp' ? 1 : -1)).toFixed(dec);
    i.dispatchEvent(new Event('input', { bubbles: true }));
  });

  const fail = (err) => {
    const d = (err && err.detail) || {};
    // A request the engine refused is kept by the server as a job in state REFUSED (no result): show it as a job.
    if (d.state === 'REFUSED' && Number.isInteger(d.id)) {
      toast(t('math.toast.none', { id: d.id, why: d.outcome || t('math.why.default') }), 'err');
      location.hash = `#/job/${d.id}`; return;
    }
    const r = coded(err);
    if (r) {
      clearMarks(form);
      if (!showRefusal(r)) { const i = $('input,select', form); if (i) i.focus(); }
      return;
    }
    msg.append(el('div', { class: 'alert bad', role: 'alert' }, err instanceof ApiError ? errText(err) : errText(err)));
  };

  form.addEventListener('submit', async (e) => {
    e.preventDefault();
    $$('input[data-dim]', form).forEach((i) => { const a = toAscii(i.value); if (a !== i.value) i.value = a; });
    const req = read(), { errors, warnings } = validateRequest(req.family, req);
    clearMarks(form); msg.textContent = ''; confirmed = '';
    if (errors.length) {
      errors.forEach(([k, m], n) => { const i = form.elements.namedItem(k); if (i) markField(i, esc(m), n === 0); });
      msg.append(el('div', { class: 'alert bad', role: 'alert', html: h('math.notsent', { n: errors.length }) }));
      run.textContent = t('math.run');
      return;
    }
    const key = JSON.stringify(req);
    if (warnings.length && confirmed !== key) {
      confirmed = key;
      msg.append(el('div', { class: 'alert warn', role: 'alert' },
        el('b', {}, t('math.plaus.t')),
        el('ul', { class: 'il' }, ...warnings.map(([, m]) => el('li', {}, m))),
        el('p', {}, t('math.plaus.p'))));
      run.textContent = t('math.run.unusual');
      return;
    }
    busy(run, true); run.textContent = t('math.busy'); form.setAttribute('aria-busy', 'true');
    try {
      const r = await api(`/api/projects/${form.elements.namedItem('project').value}/calculations`, { method: 'POST', json: req });
      if (signal.aborted) return;
      // the server answers {id, state, advisories}; a refusal is state REFUSED with the reason in outcome or error
      const noResult = r && r.state === 'REFUSED';
      if (noResult) toast(t('math.toast.none', { id: r.id, why: r.outcome || r.error || t('math.why.default') }), 'err');
      else toast(t('math.toast.ok', { id: r && r.id, state: r && r.state ? stateText(r.state) : t('math.state.default') }), 'ok');
      if (r && r.id !== undefined) location.hash = `#/job/${r.id}`;
    } catch (err) {
      if (!signal.aborted) fail(err);
    } finally {
      if (form) form.removeAttribute('aria-busy');
      if (run.isConnected) { busy(run, false); run.textContent = t('math.run'); }
    }
  });

  paintDev();

  // A job that was refused opens with every field the refusal names already marked, and says so in the message
  // area, naming any field this form has no control for (BH-014).
  if (refusal) showRefusal(refusal, h('math.refused.note', { id: esc(fromId), why: refusalHtml(refusal) }));
  if (!$('[aria-invalid]', form)) {
    const first = $('input[data-dim]', paramsBox);
    if (first) first.focus({ preventScroll: true });
  }
}
