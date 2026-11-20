// NEXUS FABRIC web client. No build step, no dependencies, no external resources (offline-capable, strict CSP).
// Every value shown comes from the API. Capabilities without a backend are labelled "Planned — not implemented";
// nothing is mocked, and there is no control anywhere in this client that approves or releases a result.
// Every string the client owns comes from the catalogue in i18n.js (English and Japanese); text the engine or
// the server writes is shown through eng() / srv(), which never translate a statement in part.
import { PatternViewer, isSeam } from './viewer.js';
import { t, h, has, lang, setLang, initialLang, LANGS, LANG_NAME, idLabel, quantityUnit, engineJa, engineSentences, missing } from './i18n.js';

// ---------------------------------------------------------------- utilities
const $ = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => [...r.querySelectorAll(s)];
const esc = (v) => String(v ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : esc(n));
// A dimension is accepted only as plain digits with an optional sign, one decimal point and an optional exponent.
// Anything else (decimal comma, thousands separator, full-width digits, hex, spaces inside the number) is refused
// instead of being interpreted: the value computed must be exactly the value typed. (A browser number field silently
// drops such characters: "1,5" became 15. These fields are therefore text fields read by this parser.)
const DIM = /^[+-]?(\d+(\.\d*)?|\.\d+)([eE][+-]?\d+)?$/;
const parseDim = (s) => { const v = String(s ?? '').trim(); return DIM.test(v) ? Number(v) : NaN; };
const dimAttrs = 'type="text" inputmode="decimal" autocomplete="off" autocapitalize="off" spellcheck="false" data-dim';
// ArrowUp / ArrowDown step a dimension field by one unit, as a number field would, keeping its decimals.
document.addEventListener('keydown', (e) => {
  const el = e.target; if (!(el instanceof HTMLInputElement) || !el.hasAttribute('data-dim') || (e.key !== 'ArrowUp' && e.key !== 'ArrowDown')) return;
  const v = parseDim(el.value); if (!Number.isFinite(v)) return;
  const dec = (/\.(\d+)$/.exec(el.value.trim()) || [, ''])[1].length;
  e.preventDefault(); el.value = (v + (e.key === 'ArrowUp' ? 1 : -1)).toFixed(dec); el.dispatchEvent(new Event('input', { bubbles: true }));
});
const pad2 = (n) => String(n).padStart(2, '0');
// ISO 8601 date and time in the browser's time zone, in every language
const when = (ts) => { if (!ts) return '—'; const d = new Date(ts * 1000); return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`; };
const words = (s) => esc(String(s).replaceAll('_', ' '));
const mono = (s) => `<span class="mono">${esc(s)}</span>`;
const short = (hash) => `<span class="hash" title="${esc(hash)}">${esc(String(hash).slice(0, 12))}…</span>`;
// text a person typed (project names, user names, reasons, file names): shown as written, in any language
const ud = (v) => `<bdi class="u">${esc(v)}</bdi>`;
const prefs = {
  get(k, d = null) { try { const v = localStorage.getItem(k); return v === null ? d : v; } catch { return d; } },
  set(k, v) { try { if (v === null) localStorage.removeItem(k); else localStorage.setItem(k, v); } catch { /* storage unavailable: preference lasts for this page only */ } },
};
setLang(initialLang(prefs.get('nf-lang')));
const ja = () => lang() === 'ja';
// test hook: keys that were asked for and are missing from the active language
window.__nfI18nMissing = () => [...missing];

// ---------------------------------------------------------------- icons (one hand-drawn set: 16 px grid, 1.5 px round stroke)
const ICONS = {
  dashboard: '<rect x="2" y="2" width="5" height="5" rx="1"/><rect x="9" y="2" width="5" height="3" rx="1"/><rect x="9" y="7" width="5" height="7" rx="1"/><rect x="2" y="9" width="5" height="5" rx="1"/>',
  folder: '<path d="M2 4.5a1 1 0 0 1 1-1h3l1.5 1.5H13a1 1 0 0 1 1 1V12a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z"/>',
  pattern: '<path d="M3 12.5 5.6 4.2a8 8 0 0 1 4.8 0L13 12.5a14 14 0 0 0-10 0z"/>',
  nodes: '<circle cx="4" cy="4" r="1.6"/><circle cx="12" cy="5" r="1.6"/><circle cx="8" cy="12" r="1.6"/><path d="M5.6 4.2 10.4 4.8M11.2 6.4 8.8 10.6M4.8 5.4 7.2 10.6"/>',
  shield: '<path d="M8 1.8 13 3.6v4.1c0 3-2 5.3-5 6.5-3-1.2-5-3.5-5-6.5V3.6z"/><path d="m5.8 8 1.6 1.6 2.9-3.1"/>',
  layers: '<path d="M8 2 14 5 8 8 2 5z"/><path d="m2 8 6 3 6-3M2 11l6 3 6-3"/>',
  sliders: '<path d="M2 4.5h7M12 4.5h2M2 11.5h2M7 11.5h7"/><circle cx="10.5" cy="4.5" r="1.5"/><circle cx="5.5" cy="11.5" r="1.5"/>',
  user: '<circle cx="8" cy="5.5" r="2.5"/><path d="M3 13.5c.6-2.4 2.6-3.5 5-3.5s4.4 1.1 5 3.5"/>',
  theme: '<circle cx="8" cy="8" r="5.5"/><path d="M8 2.5v11a5.5 5.5 0 0 0 0-11z" fill="currentColor"/>',
  help: '<circle cx="8" cy="8" r="6"/><path d="M6.3 6.2a1.8 1.8 0 1 1 2.6 1.7c-.6.3-.9.7-.9 1.3M8 11.4v.1"/>',
  bell: '<path d="M4 11V7.5a4 4 0 0 1 8 0V11l1 1.5H3z"/><path d="M6.8 14a1.3 1.3 0 0 0 2.4 0"/>',
  signout: '<path d="M6.5 2.5h-3a1 1 0 0 0-1 1v9a1 1 0 0 0 1 1h3M10.5 5l3 3-3 3M13.5 8h-7"/>',
  plus: '<path d="M8 3v10M3 8h10"/>', minus: '<path d="M3 8h10"/>',
  fit: '<path d="M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10"/>',
  ruler: '<path d="m2 11 9-9 3 3-9 9z"/><path d="m5 8 1.5 1.5M7 6l1.5 1.5M9 4l1.5 1.5"/>',
  grid: '<path d="M2.5 2.5h11v11h-11zM2.5 6.2h11M2.5 9.8h11M6.2 2.5v11M9.8 2.5v11"/>',
  dims: '<path d="M2 4v8M14 4v8M2 8h12M4.5 6l-2 2 2 2M11.5 6l2 2-2 2"/>',
  names: '<path d="M2.5 12.5 6 3.5l3.5 9M3.8 9.5h4.4"/><path d="M11 6.5h3M11 9.5h3M11 12.5h3"/>',
  download: '<path d="M8 2v8M4.5 6.5 8 10l3.5-3.5M2.5 13.5h11"/>',
  upload: '<path d="M8 10.5v-8M4.5 6 8 2.5 11.5 6M2.5 13.5h11"/>',
  replay: '<path d="M13 8a5 5 0 1 1-1.6-3.7"/><path d="M13.5 2v3h-3"/>',
  close: '<path d="m3.5 3.5 9 9M12.5 3.5l-9 9"/>',
  check: '<path d="m3 8.5 3.2 3.2L13 4.5"/>',
  warn: '<path d="M8 2 14.5 13.5h-13z"/><path d="M8 6.5v3.2M8 11.6v.1"/>',
  info: '<circle cx="8" cy="8" r="6"/><path d="M8 7.2v4M8 4.8v.1"/>',
  rail: '<rect x="2" y="2.5" width="12" height="11" rx="1"/><path d="M6 2.5v11"/>',
  pointer: '<path d="M3.5 2.5 12 7 8.4 8.2 7 12z"/>',
  chevron: '<path d="m6 3.5 4.5 4.5L6 12.5"/>',
  search: '<circle cx="7" cy="7" r="4.2"/><path d="m10.2 10.2 3.3 3.3"/>',
  clock: '<circle cx="8" cy="8" r="6"/><path d="M8 4.5V8l2.4 1.6"/>',
  dash: '<path d="M4 8h8"/>',
  planned: '<circle cx="8" cy="8" r="5.5" stroke-dasharray="2.4 2.4"/>',
  mark: '<path d="M2 13 6 3h4l4 10"/><path d="M4.4 9.5a14 14 0 0 1 7.2 0"/>',
  face: '<path d="M2.5 11.5 8 3.5l5.5 8z"/><path d="M8 3.5v8" stroke-dasharray="1.6 1.6"/>',
  none: '<circle cx="8" cy="8" r="5.5"/><path d="m4.2 11.8 7.6-7.6"/>',
  seam: '<path d="M2 12h12M3.5 12l2-4M6.5 12l2-4M9.5 12l2-4M12.5 12l2-4" />',
};
const ic = (name, size = 16) => `<svg class="ic" width="${size}" height="${size}" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICONS[name] || ''}</svg>`;

// ---------------------------------------------------------------- text that is not the client's
// An identifier the engine or the server sends (gate id, check name, curve name, record kind ...).
// English: the label the client has always shown, or the identifier in words. Japanese: the reviewed label
// with the identifier kept beside it in monospace; an identifier without a label is shown as it is.
const idText = (kind, id) => idLabel(kind, id) || (ja() ? String(id) : String(id).replaceAll('_', ' '));
const named = (kind, id) => { const l = idLabel(kind, id); if (!ja()) return l ? esc(l) : words(id); return l ? `${esc(l)}<span class="mono idk">${esc(id)}</span>` : mono(id); };
const labelled = (kind, id) => { const l = idLabel(kind, id); return l ? esc(l) : ja() ? mono(id) : words(id); }; // label alone; the identifier is shown elsewhere in the same row
// Prose the engine or the server wrote. English: as received. Japanese: the reviewed translation when the whole
// text is made of known templates (numbers and identifiers pass through unchanged), with the original kept in the
// page for the status-bar switch; otherwise the original, marked as engine output in English.
// Japanese text with identifiers marked by backquotes in the catalogue: the identifiers are shown in monospace.
const jaHtml = (s) => esc(s).replace(/`([^`]+)`/g, '<span class="mono">$1</span>');
const engTag = (key = 'eng.tag') => `<span class="tag t-eng">${esc(t(key))}</span>`;
function eng(text, o = {}) {
  const s = String(text ?? ''); if (!s) return '';
  if (!ja()) return esc(s);
  const j = engineJa(s);
  if (j) return `<span class="eng"><span class="e-ja">${jaHtml(j)}</span><span class="e-en" lang="en">${esc(s)}</span></span>`;
  return `<span class="eng raw">${o.tag === false ? '' : `${engTag(o.key)} `}<span lang="en">${esc(s)}</span></span>`;
}
const srv = (text) => eng(text, { key: 'srv.tag' });
// A node of the dependency graph is named "<record kind> · <name>" by the server.
const nodeParts = (l) => { const i = l.indexOf(' · '); return i < 0 ? ['', l] : [l.slice(0, i), l.slice(i + 3)]; };
const nodeName = (kind, name) => (ja() ? (kind === 'parameter' || kind === 'convention' ? idLabel('param', name) : kind ? idLabel('part', name) : null) || idLabel('dlabel', name) || idLabel('rkind', name) : null);
const nodeLabel = (l) => { const [k, n] = nodeParts(l); if (!ja()) return esc(l); const jn = nodeName(k, n); return `${k ? `${labelled('rkind', k)} · ` : ''}${jn ? `${esc(jn)} ` : ''}${mono(n)}`; };

// ---------------------------------------------------------------- shared fragments
const tag = (kind, text) => `<span class="tag t-${kind}">${esc(text)}</span>`;
const TAGS = { ok: 'check', warn: 'warn', bad: 'close', na: 'dash', planned: 'planned' };
const STATE_KIND = { APPROVED: 'ok', REJECTED: 'bad', SUPERSEDED: 'na', NO_RESULT: 'bad' };
const stateText = (s) => (['APPROVED', 'REJECTED', 'SUPERSEDED', 'NO_RESULT', 'REVIEW_REQUIRED', 'REFUSED'].includes(s) ? t(`state.${s}`) : String(s).replaceAll('_', ' '));
const stateTag = (s) => { const k = STATE_KIND[s] || 'warn'; return tag(k, stateText(s), TAGS[k]); };
const RESULT_KIND = { PASS: 'ok', FAIL: 'bad', INCONCLUSIVE: 'warn' };
const resultText = (r) => (['PASS', 'FAIL', 'INCONCLUSIVE', 'NOT_IMPLEMENTED', 'NOT_RUN'].includes(r) ? t(`result.${r}`) : String(r).replaceAll('_', ' '));
const resultTag = (r) => { const k = RESULT_KIND[r] || 'na'; return tag(k, resultText(r), TAGS[k]); };
const oblText = (s) => (['open', 'discharged', 'discharged_by_decision', 'waived'].includes(s) ? t(`obl.${s}`) : String(s).replaceAll('_', ' '));
const oblTag = (s) => tag(s === 'open' || s === 'waived' ? 'warn' : 'ok', oblText(s));
const roleText = (r) => (['admin', 'engineer', 'viewer'].includes(r) ? t(`role.${r}`) : String(r));
const ph = (title, sub, actions = '') => `<div class="ph"><div><h1>${title}</h1>${sub ? `<p class="sub">${sub}</p>` : ''}</div><div class="ph-actions">${actions}</div></div>`;
const panel = (title, body, o = {}) => `<section class="panel"${o.id ? ` id="${o.id}"` : ''}><header class="panel-h"><h2>${title}</h2>${o.count !== undefined ? `<span class="cnt">${o.count}</span>` : ''}${o.tools ? `<div class="panel-tools">${o.tools}</div>` : ''}</header><div class="panel-b${o.flush ? ' flush' : ''}">${body}</div>${o.foot ? `<div class="panel-f">${o.foot}</div>` : ''}</section>`;
const empty = (icon, title, text, actions = '', cls = '') => `<div class="empty ${cls}"><b>${esc(title)}</b><p>${text}</p>${actions ? `<div class="row">${actions}</div>` : ''}</div>`;
const planned = (title, items, lead = '') => `<aside class="planned" aria-label="${esc(t('planned.aria', { title }))}"><header>${tag('planned', t('planned.tag'), 'planned')}<b>${esc(title)}</b><span class="sub">${esc(t('planned.sub'))}</span></header>${lead ? `<p>${jaHtml(lead)}</p>` : ''}<ul>${items.map((i) => `<li>${jaHtml(i)}</li>`).join('')}</ul></aside>`;
const plannedKeys = (prefix, n, lead) => planned(t(`${prefix}.t`), Array.from({ length: n }, (_, i) => t(`${prefix}.${i + 1}`)), lead ? t(lead) : '');
const note = (html, kind = '') => `<div class="note ${kind}"><div>${html}</div></div>`;
const filterBox = (target, label) => `<div class="filter">${ic('search', 14)}<label class="vh" for="flt-${target}">${esc(label)}</label><input id="flt-${target}" type="search" placeholder="${esc(label)}" data-filter="${target}" autocomplete="off"></div>`;
const tabsHtml = (id, label, items, active) => `<div class="tabs" role="tablist" aria-label="${esc(label)}" data-tabs="${id}">${items.map(([k, l, n, attn]) => `<button role="tab" id="tab-${id}-${k}" data-tab="${k}" aria-controls="tp-${id}-${k}" aria-selected="${k === active}" tabindex="${k === active ? 0 : -1}">${esc(l)}${n !== undefined && n !== null ? `<span class="cnt${attn ? ' attn' : ''}">${n}</span>` : ''}</button>`).join('')}</div>`;
const tabPanel = (id, k, active, html, cls = '') => `<div role="tabpanel" id="tp-${id}-${k}" aria-labelledby="tab-${id}-${k}" class="${cls}" tabindex="0"${k === active ? '' : ' hidden'}>${html}</div>`;
function wireTabs(root, id, onChange) {
  const list = $(`[data-tabs="${id}"]`, root); if (!list) return;
  const tabs = $$('[role=tab]', list);
  const show = (k, focus) => {
    tabs.forEach((b) => { const on = b.dataset.tab === k; b.setAttribute('aria-selected', on); b.tabIndex = on ? 0 : -1; const p = document.getElementById(b.getAttribute('aria-controls')); if (p) p.hidden = !on; if (on && focus) b.focus(); });
    if (onChange) onChange(k);
  };
  list.addEventListener('click', (e) => { const b = e.target.closest('[role=tab]'); if (b) show(b.dataset.tab); });
  list.addEventListener('keydown', (e) => {
    const i = tabs.indexOf(document.activeElement); if (i < 0) return;
    const j = e.key === 'ArrowRight' ? (i + 1) % tabs.length : e.key === 'ArrowLeft' ? (i - 1 + tabs.length) % tabs.length : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : -1;
    if (j >= 0) { e.preventDefault(); show(tabs[j].dataset.tab, true); }
  });
  return show;
}

// ---------------------------------------------------------------- geometry families (inputs the calculation API accepts)
// [key, example value, unit]; `groups` only arranges the form. Labels come from the catalogue, keyed by the key.
const F = {
  cylinder_shell: { f: [['r_dev', 500, 'mm'], ['length', 1000, 'mm']] },
  cone_frustum: { f: [['r1', 200, 'mm'], ['r2', 100, 'mm'], ['h', 300, 'mm']] },
  eccentric_reducer: { f: [['r1', 200, 'mm'], ['r2', 100, 'mm'], ['h', 300, 'mm'], ['e', 100, 'mm']] },
  header_with_branch: { f: [['header_radius', 150, 'mm'], ['header_dev_radius', 150, 'mm'], ['header_length', 1200, 'mm'], ['branch_station', 600, 'mm'], ['branch_radius', 100, 'mm'], ['branch_dev_radius', 100, 'mm'], ['beta_deg', 60, 'deg'], ['delta', 0, 'mm'], ['s_end', 400, 'mm']], sref: true, ext: true },
  branch_stub: { f: [['header_radius', 150, 'mm'], ['branch_radius', 100, 'mm'], ['branch_dev_radius', 100, 'mm'], ['beta_deg', 60, 'deg'], ['delta', 0, 'mm'], ['s_end', 400, 'mm']], sref: true },
};
const famLabel = (k) => esc(idLabel('familyLong', k) || idText('family', k));
const famShort = (k) => esc(idText('family', k));
const plabel = (fam, key) => idLabel('param', `${fam}.${key}`) || idLabel('param', key) || (ja() ? key : String(key).replaceAll('_', ' '));
// [key, label, example, unit] of a parameter of a family, or null for a key the form does not know
const paramMeta = (fam, key) => { const m = F[fam] && F[fam].f.find((x) => x[0] === key); return m ? [m[0], plabel(fam, key), m[1], m[2]] : null; };
const unitText = (u) => (u === 'deg' ? t('unit.deg') : u);
const partName = (p) => (ja() && idLabel('part', p) ? `${esc(idLabel('part', p))}<span class="mono idk">${esc(p)}</span>` : esc(p));
const partText = (p) => (ja() && idLabel('part', p) ? `${idLabel('part', p)} ${p}` : String(p));
const sideText = (s) => idLabel('side', s) || String(s);

// ---------------------------------------------------------------- numbers, units, quantities
// One display rule for computed numbers, in every panel, on the canvas and in the read-out: 3 decimals with
// thousands separators; magnitudes below 0.001 in scientific notation. The full-precision value is the
// element's title (hover) and what the API returned. Digits and separators are the same in both languages.
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
const num = (v, unit = '') => (typeof v === 'number' ? (unit ? `<span class="q"><span class="qv" title="${fmt(v)} ${unit}">${n3(v)}</span> <span class="qu">${unit}</span></span>` : `<span class="qv" title="${fmt(v)}">${n3(v)}</span>`) : esc(v));
const signed = (v, unit = '') => `<span class="q"><span class="qv" title="${fmt(v)}">${v > 0 ? '+' : ''}${n3(v)}</span>${unit ? ` <span class="qu">${unit}</span>` : ''}</span>`;
// [label html, value html] for an engine quantity. Angles arrive in radians and are shown in degrees with the radian value beside.
const quantity = (k, v) => {
  const l = idLabel('quantity', k), unit = quantityUnit(k);
  const label = `${l ? esc(l) : ja() ? '' : words(k)}<small class="mono">${esc(k)}</small>`;
  if (typeof v !== 'number') return [label, esc(v)];
  if (unit === 'rad') return [label, `${num(v * 180 / Math.PI, '°')} <span class="sub">(${n3(v)} rad)</span>`];
  return [label, unit ? num(v, unit) : `${num(v)} <span class="sub" title="${esc(t('q.nounit.tip'))}">${esc(t('q.nounit'))}</span>`];
};
const extent = (p) => { let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity; for (const k of [...p.curves, ...p.holes]) for (const [x, y] of k.points) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; } return { w: x1 - x0, h: y1 - y0, x0, x1, y0, y1 }; };

// ---------------------------------------------------------------- job facts and effective state
// The job list API carries the state, but not the parts, gates or checks, so list pages read each job once
// (cached until the next state-changing request). A request the engine refused is stored by the server in
// state REFUSED with an outcome and no pattern; the client shows it as NO RESULT everywhere.
const factsCache = new Map();
const summarise = (c) => ({
  id: c.id, outcome: c.outcome, noResult: !c.patterns.length, superseded_by: c.superseded_by, supersedes: c.supersedes, state: c.state,
  parts: c.patterns.map((p) => ({ part: p.part, cls: p.class, sha: p.dxf_sha256, ...extent(p), curves: p.curves.length, holes: p.holes.length })),
  gates: ['PASS', 'INCONCLUSIVE', 'FAIL', 'NOT_IMPLEMENTED', 'NOT_RUN'].map((r) => c.gates.filter((g) => g.result === r).length),
  checks: [c.checks.filter((k) => k.verdict === 'PASS').length, c.checks.length],
  advOpen: c.advisories.filter((a) => a.state === 'open').length, replay: c.replay ? c.replay.matches : null,
});
const jobFacts = (id) => { if (!factsCache.has(id)) factsCache.set(id, api(`/api/calculations/${id}`).then(summarise, () => null)); return factsCache.get(id); };
const withFacts = async (rows) => { const fs = await Promise.all(rows.map((r) => jobFacts(r.id))); rows.forEach((r, i) => { r.facts = fs[i]; }); return rows; };
// Same precedence everywhere: superseded > no result > the server state.
const effState = (row) => (row.superseded || (row.facts && row.facts.superseded_by) ? 'SUPERSEDED' : row.no_result || (row.facts && row.facts.noResult) ? 'NO_RESULT' : row.state);
const replayTag = (m) => (m === true ? tag('ok', t('replay.match'), 'check') : m === false ? tag('bad', t('replay.mismatch'), 'close') : `<span class="sub">${esc(t('replay.none'))}</span>`);
let projects = [];
const loadProjects = async () => { projects = await api('/api/projects'); return projects; };
const pname = (id) => { const p = projects.find((x) => x.id === id); return p ? p.name : null; };
const pnameHtml = (id) => { const n = pname(id); return n === null ? esc(t('project.n', { id })) : ud(n); };
const plink = (id) => `<a href="#/project/${id}" title="${esc(t('project.tip', { id }))}">${pnameHtml(id)}</a>`;
const jobLink = (id, lc) => `<a href="#/calc/${id}">${esc(t(lc ? 'job.n.lc' : 'job.n', { id }))}</a>`;
const gateCounts = (g) => { const [p, i, f, n, r] = g; return `${tag('ok', t('g.n.pass', { n: p }), 'check')} ${tag('warn', t('g.n.inc', { n: i }), 'warn')}${f ? ` ${tag('bad', t('g.n.fail', { n: f }), 'close')}` : ''} ${tag('na', t('g.n.ni', { n }), 'dash')}${r ? ` ${tag('na', t('g.n.nr', { n: r }), 'dash')}` : ''}`; };

// ---------------------------------------------------------------- input checks before a request is sent
// These mirror limits the engine documents, so that an impossible request is stopped at the field instead of
// being stored as a job. The server remains the authority: whatever it answers is shown as it is.
const POSITIVE = ['r_dev', 'length', 'r1', 'r2', 'h', 'header_radius', 'header_dev_radius', 'header_length', 'branch_radius', 'branch_dev_radius', 's_end'];
function validateRequest(fam, v) {
  const errors = [], warnings = [], lab = (k) => plabel(fam, k);
  for (const [k] of F[fam].f) {
    if (!Number.isFinite(v[k])) { errors.push([k, t('v.number', { label: lab(k) })]); continue; }
    if (POSITIVE.includes(k) && v[k] <= 0) errors.push([k, t('v.positive', { label: lab(k) })]);
    else if (k !== 'beta_deg' && Math.abs(v[k]) > 20000) warnings.push([k, t('v.big', { label: lab(k), v: fmt(v[k]) })]);
    else if (POSITIVE.includes(k) && v[k] < 1) warnings.push([k, t('v.small', { label: lab(k), v: fmt(v[k]) })]);
  }
  const has = (...ks) => ks.every((k) => k in v && Number.isFinite(v[k]));
  const bad = (k) => errors.some((e) => e[0] === k);
  if (has('r1', 'r2') && !bad('r1') && !bad('r2') && v.r1 <= v.r2) errors.push(['r1', t('v.r1r2')]);
  if (has('e') && v.e < 0) errors.push(['e', t('v.e')]);
  if (has('beta_deg') && (v.beta_deg < 10 || v.beta_deg > 170)) errors.push(['beta_deg', t('v.beta')]);
  if (has('header_radius', 'branch_radius', 'delta') && !bad('header_radius') && !bad('branch_radius') && Math.abs(v.delta) + v.branch_radius >= v.header_radius) errors.push(['branch_radius', t('v.tangent', { sum: fmt(Math.abs(v.delta) + v.branch_radius), R: fmt(v.header_radius) })]);
  if (has('branch_station', 'header_length') && !bad('header_length') && (v.branch_station <= 0 || v.branch_station >= v.header_length)) errors.push(['branch_station', t('v.station')]);
  // Same rule as the engine (Spec 02 §6.1: the development surface lies inside the wall), so that a declared
  // surface the radii contradict is stopped at the field. The engine refuses it in any case.
  // Each part uses its own declared surface (header_surface_reference, branch_surface_reference), or the shared one.
  const sref = (part) => v[`${part}_surface_reference`] || v.surface_reference;
  for (const [c, d, part] of [['header_radius', 'header_dev_radius', 'header'], ['branch_radius', 'branch_dev_radius', 'branch']]) {
    const ref = sref(part);
    if (!ref || !has(c, d) || bad(c) || bad(d)) continue;
    const same = Math.abs(v[d] - v[c]) <= 1e-12 * Math.max(Math.abs(v[d]), Math.abs(v[c]));
    const p = { d: lab(d), dv: fmt(v[d]), c: lab(c), cv: fmt(v[c]) };
    if (ref === 'outside' && !same && v[d] > v[c]) errors.push([d, t('v.outside', p)]);
    if (ref === 'inside' && !same && v[d] < v[c]) errors.push([d, t('v.inside', p)]);
    if (ref === 'neutral' && !same) errors.push([d, t('v.neutral', p)]);
  }
  // RB-001 (engine 0.4.0): a set-on joint needs the branch INSIDE surface on the header OUTSIDE surface; the engine
  // refuses any other pairing and names the field "joint". The field is marked here, so nothing is sent.
  if (v.joint === 'set_on') {
    const hs = sref('header'), bs = sref('branch');
    if (hs !== 'outside' || bs !== 'inside') errors.push(['joint', t('v.joint.pair', { hs: t(`sref.${hs || 'unknown'}`), bs: t(`sref.${bs || 'unknown'}`) })]);
  }
  return { errors, warnings };
}

// The optional parts of a header-with-branch request (review RB-001, engine 0.4.0): the wall surface each contact
// radius refers to, and the joint type. Left at their first option they are not sent, so the request is as before.
const extFields = (from) => {
  const sel = (name, label, opts, hint) => `<div class="field"><label for="f_${name}">${esc(label)}<span class="mono key">${name}</span></label><select id="f_${name}" name="${name}">${opts.map(([v, text]) => `<option value="${v}"${(from ? from[name] || '' : '') === v ? ' selected' : ''}>${esc(text)}</option>`).join('')}</select>${hint ? `<p class="hint">${esc(hint)}</p>` : ''}</div>`;
  const surf = [['', t('math.psref.same')], ...['unknown', 'outside', 'inside', 'neutral'].map((o) => [o, t(`sref.${o}`)])];
  const fam = 'header_with_branch';
  return sel('header_surface_reference', plabel(fam, 'header_surface_reference'), surf)
    + sel('branch_surface_reference', plabel(fam, 'branch_surface_reference'), surf, t('math.psref.hint'))
    + sel('joint', plabel(fam, 'joint'), [['', t('math.joint.none')], ['set_on', t('math.joint.set')]], t('math.joint.hint'));
};

// ---------------------------------------------------------------- parameter schematic
// A to-scale drawing made only from the values in the form: what each dimension means. It is not the computed
// pattern and nothing in it is verified; the caption under it says so. For the branch families it also states,
// in drafting terms, the sign convention of the axis offset δ and from which end the end view is seen
// (engine review RB-002): the result data carries neither, so the client states what Spec 02 §4.4 defines.
function sketch(fam, v, W, H) {
  const need = F[fam].f.map((x) => x[0]).filter((k) => POSITIVE.includes(k) || k === 'beta_deg');
  if (!need.every((k) => Number.isFinite(v[k]) && v[k] > 0) || !F[fam].f.every(([k]) => Number.isFinite(v[k])) || (v.beta_deg !== undefined && v.beta_deg >= 180))
    return { svg: `<div class="sk-msg">${esc(t('sk.msg'))}</div>`, cap: '' };
  const views = []; let cur = null;
  const view = (title, axis) => { cur = { title, axis, b: [Infinity, -Infinity] }; views.push(cur); };
  const L = [], D = [], T = [];
  const line = (pts, cls = '') => { L.push({ pts, cls }); for (const [x] of pts) { if (x < cur.b[0]) cur.b[0] = x; if (x > cur.b[1]) cur.b[1] = x; } };
  // a dimension symbol is either a parameter identifier (monospace, ident) or a word from the catalogue; `plain` is the same text for the aria label
  const dim = (a, b, off, sym, key, ident = false) => D.push({ a, b, off, text: `${ident ? `<tspan class="mono">${esc(sym)}</tspan>` : esc(sym)} ${esc(fmt(v[key]))}`, plain: `${sym} ${fmt(v[key])}`, key });
  const circle = (cx, cy, r, cls) => line(Array.from({ length: 73 }, (_, i) => [cx + r * Math.cos(i * Math.PI / 36), cy + r * Math.sin(i * Math.PI / 36)]), cls);
  let delta = '';
  if (fam === 'cylinder_shell') {
    const r = v.r_dev, len = v.length, cx = len + 1.9 * r;
    view(t('sk.v.sidev')); line([[0, -r], [len, -r], [len, r], [0, r], [0, -r]]); line([[-0.08 * len, 0], [len * 1.08, 0]], 'axis');
    dim([0, -r], [len, -r], 26, t('sk.d.length'), 'length');
    view(t('sk.v.endv')); circle(cx, 0, r); line([[cx - 1.15 * r, 0], [cx + 1.15 * r, 0]], 'axis'); line([[cx, -1.15 * r], [cx, 1.15 * r]], 'axis');
    dim([cx, 0], [cx + r * Math.SQRT1_2, r * Math.SQRT1_2], 0, 'r_dev', 'r_dev', true);
  } else if (fam === 'cone_frustum' || fam === 'eccentric_reducer') {
    const e = fam === 'eccentric_reducer' ? v.e : 0;
    view(t('sk.v.cone')); line([[-v.r1, 0], [v.r1, 0], [e + v.r2, v.h], [e - v.r2, v.h], [-v.r1, 0]]); line([[0, -0.1 * v.h], [0, v.h * 1.1]], 'axis'); if (e) line([[e, v.h * 0.55], [e, v.h * 1.1]], 'axis');
    dim([0, 0], [v.r1, 0], 26, 'r1', 'r1', true); dim([e, v.h], [e + v.r2, v.h], -24, 'r2', 'r2', true); dim([Math.max(v.r1, e + v.r2), 0], [Math.max(v.r1, e + v.r2), v.h], 30, 'h', 'h', true);
    if (fam === 'eccentric_reducer') dim([0, v.h], [e, v.h], -52, 'e', 'e', true);
  } else {
    const hw = fam === 'header_with_branch', R = v.header_radius, r = v.branch_radius, b = v.beta_deg * Math.PI / 180, u = [Math.cos(b), Math.sin(b)], n = [-u[1], u[0]];
    const st = hw ? v.branch_station : 0, xa = hw ? 0 : -Math.max(2 * R, 2.5 * r), xb = hw ? v.header_length : Math.max(2 * R, 2.5 * r);
    const P = (s, k) => [st + s * u[0] + k * r * n[0], s * u[1] + k * r * n[1]];
    // side elevation: x (station) to the right, z up; +y, and with it a positive δ, points away from the viewer
    view(t(hw ? 'sk.v.side' : 'sk.v.side.part'), { away: true, x: t(hw ? 'sk.ax.station' : 'sk.ax.x'), at: [xb + 0.06 * (xb - xa), 0] });
    line([[xa, -R], [xb, -R]]); line([[xa, R], [xb, R]]); line([[xa, -R], [xa, R]], hw ? '' : 'axis'); line([[xb, -R], [xb, R]], hw ? '' : 'axis'); line([[xa - 0.06 * (xb - xa), 0], [xb + 0.06 * (xb - xa), 0]], 'axis');
    line([P(0, 1), P(v.s_end, 1), P(v.s_end, -1), P(0, -1)]); line([P(0, 0), P(v.s_end * 1.08, 0)], 'axis');
    const ar = Math.min(0.42 * v.s_end, 0.9 * R);
    T.push({ arc: Array.from({ length: 25 }, (_, i) => [st + ar * Math.cos(b * i / 24), ar * Math.sin(b * i / 24)]), at: [st + ar * 1.18 * Math.cos(b / 2), ar * 1.18 * Math.sin(b / 2)], text: `β ${fmt(v.beta_deg)}°`, key: 'beta_deg' });
    dim([xa, 0], [xa, R], -24, 'R', 'header_radius', true);
    if (hw) { dim([xa, -R], [xb, -R], 50, t('sk.d.hlen'), 'header_length'); dim([xa, -R], [st, -R], 24, t('sk.d.station'), 'branch_station'); }
    dim(P(0, -1), P(v.s_end, -1), b > Math.PI / 2 ? -22 : 22, 's_end', 's_end', true); dim(P(v.s_end, 0), P(v.s_end, 1), -16, 'r', 'branch_radius', true);
    // end view, seen from the +x end looking back: y to the right, z up, +x toward the viewer (third-angle placement)
    const cx = xb + 0.16 * (xb - xa) + 1.6 * R, top = Math.max(v.s_end * u[1], 1.25 * R);
    view(t(hw ? 'sk.v.end' : 'sk.v.end.part'), { away: false });
    circle(cx, 0, R); line([[cx - 1.2 * R, 0], [cx + 1.2 * R, 0]], 'axis'); line([[cx, -1.2 * R], [cx, 1.2 * R]], 'axis');
    line([[cx + v.delta - r, 0], [cx + v.delta - r, top], [cx + v.delta + r, top], [cx + v.delta + r, 0]]); line([[cx + v.delta, 0], [cx + v.delta, top * 1.08]], 'axis');
    if (v.delta) dim([cx, top], [cx + v.delta, top], -22, 'δ', 'delta'); else T.push({ at: [cx, top * 1.16], text: 'δ 0', key: 'delta' });
    delta = t(v.delta > 0 ? 'sk.delta.pos' : v.delta < 0 ? 'sk.delta.neg' : 'sk.delta.zero', { v: fmt(v.delta) });
  }
  // fit: uniform scale, room for dimensions at the sides and for the view titles below
  const M = { l: 76, r: 76, t: 40, b: 104 };
  let x0 = Infinity, x1 = -Infinity, y0 = Infinity, y1 = -Infinity;
  for (const l of L) for (const [x, y] of l.pts) { if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y; }
  const w = x1 - x0 || 1, hh = y1 - y0 || 1, s = Math.min((W - M.l - M.r) / w, (H - M.t - M.b) / hh), ox = M.l + (W - M.l - M.r - w * s) / 2, oy = M.b + (H - M.t - M.b - hh * s) / 2;
  const map = ([x, y]) => [ox + (x - x0) * s, H - oy - (y - y0) * s], f1 = (q) => `${q[0].toFixed(1)} ${q[1].toFixed(1)}`;
  const out = L.map((l) => `<path class="${l.cls}" d="M${l.pts.map((q) => f1(map(q))).join('L')}"/>`);
  for (const d of D) {
    const A = map(d.a), B = map(d.b), len = Math.hypot(B[0] - A[0], B[1] - A[1]) || 1, ux = (B[0] - A[0]) / len, uy = (B[1] - A[1]) / len, nx = -uy, ny = ux, sg = Math.sign(d.off) || 1;
    const A2 = [A[0] + nx * d.off, A[1] + ny * d.off], B2 = [B[0] + nx * d.off, B[1] + ny * d.off], tx = (A2[0] + B2[0]) / 2 + nx * sg * 14, ty = (A2[1] + B2[1]) / 2 + ny * sg * 14;
    const anchor = Math.abs(nx) < 0.5 ? 'middle' : nx * sg > 0 ? 'start' : 'end';
    const head = (q, dir) => `M${f1(q)}l${(dir * ux * 7 + nx * 2.4).toFixed(1)} ${(dir * uy * 7 + ny * 2.4).toFixed(1)}l${(-nx * 4.8).toFixed(1)} ${(-ny * 4.8).toFixed(1)}z`;
    const ext = d.off ? `M${f1([A[0] + nx * sg * 3, A[1] + ny * sg * 3])}L${f1([A2[0] + nx * sg * 5, A2[1] + ny * sg * 5])}M${f1([B[0] + nx * sg * 3, B[1] + ny * sg * 3])}L${f1([B2[0] + nx * sg * 5, B2[1] + ny * sg * 5])}` : '';
    out.push(`<g class="dimg" data-dim="${d.key}"><path d="${ext}M${f1(A2)}L${f1(B2)}"/><path class="head" d="${len > 22 ? head(A2, 1) + head(B2, -1) : ''}"/><text x="${tx.toFixed(1)}" y="${(ty + 4).toFixed(1)}" text-anchor="${anchor}">${d.text}</text></g>`);
  }
  for (const a of T) { const q = map(a.at); out.push(`<g class="dimg" data-dim="${a.key}">${a.arc ? `<path d="M${a.arc.map((p) => f1(map(p))).join('L')}"/>` : ''}<text x="${q[0].toFixed(1)}" y="${q[1].toFixed(1)}" text-anchor="middle">${esc(a.text)}</text></g>`); }
  // view titles, and for the branch families the direction of the axis that points at or away from the viewer
  for (const vw of views) {
    const cxs = Math.min(W - 120, Math.max(120, (map([vw.b[0], 0])[0] + map([vw.b[1], 0])[0]) / 2));
    out.push(`<text class="vt" x="${cxs.toFixed(1)}" y="${H - 34}" text-anchor="middle">${esc(vw.title)}</text>`);
    if (!vw.axis) continue;
    const label = t(vw.axis.away ? 'sk.ax.away' : 'sk.ax.toward'), gx = cxs - (label.length * (ja() ? 6.2 : 3.4) + 12), gy = H - 17;
    out.push(`<g class="axg" data-dim="delta"><circle cx="${gx.toFixed(1)}" cy="${gy}" r="5.5"/>${vw.axis.away ? `<path d="M${(gx - 3.6).toFixed(1)} ${gy - 3.6}l7.2 7.2M${(gx + 3.6).toFixed(1)} ${gy - 3.6}l-7.2 7.2"/>` : `<circle class="dot" cx="${gx.toFixed(1)}" cy="${gy}" r="1.6"/>`}<text x="${(gx + 11).toFixed(1)}" y="${gy + 4}">${esc(label)}</text></g>`);
    if (vw.axis.x) { const q = map(vw.axis.at); out.push(`<g class="axg"><path d="M${f1([q[0] + 4, q[1]])}h18m-5 -3.5l5 3.5l-5 3.5"/><text x="${(q[0] + 27).toFixed(1)}" y="${(q[1] + 4).toFixed(1)}">${esc(vw.axis.x)}</text></g>`); }
  }
  const dev = F[fam].f.filter(([k]) => /dev_radius/.test(k) && fam !== 'cylinder_shell').map(([k]) => `<span data-dim="${k}">${esc(t('sk.dev', { label: plabel(fam, k), v: fmt(v[k]), w: n3(2 * Math.PI * v[k]) }))}</span>`).join(' · ');
  return {
    svg: `<svg class="sk" viewBox="0 0 ${W} ${H}" role="img" aria-label="${esc(t('sk.aria', { fam: idLabel('familyLong', fam) || fam, dims: D.map((d) => d.plain).join(', ') }))}">${out.join('')}</svg>`,
    cap: `${esc(t('sk.cap'))}${delta ? `<br><span data-dim="delta">${esc(delta)}</span>` : ''}${dev ? `<br>${h('sk.notdrawn', { items: dev })}` : ''}`,
  };
}

// ---------------------------------------------------------------- what a correction changed
function diffHtml(c, prev) {
  const fam = c.request.family, rows = [], row = (label, a, b, unit, key) => rows.push(`<tr${key ? ` data-diff="${esc(key)}"` : ''}><td>${label}</td><td class="num">${typeof a === 'number' ? num(a) : esc(a)}</td><td class="num">${typeof b === 'number' ? num(b) : esc(b)}</td><td class="num">${typeof a === 'number' && typeof b === 'number' ? signed(b - a, unit) : ''}</td></tr>`);
  const head = (html) => rows.push(`<tr class="grp"><th colspan="4">${html}</th></tr>`);
  let same = 0; const differs = (a, b) => (typeof a === 'number' && typeof b === 'number' ? Math.abs(a - b) > 1e-9 * Math.max(1, Math.abs(a), Math.abs(b)) : a !== b);
  head(esc(t('diff.entered')));
  const keys = [...new Set([...(F[fam] ? F[fam].f.map((x) => x[0]) : []), ...Object.keys(c.request)])].filter((k) => k !== 'family');
  for (const k of keys) { const m = paramMeta(fam, k); if (differs(prev.request[k], c.request[k])) row(`${m ? esc(m[1]) : labelled('param', k)}<small class="mono">${esc(k)}</small>`, prev.request[k], c.request[k], m ? unitText(m[3]) : '', `param:${k}`); else same++; }
  for (const p of c.patterns) {
    const q = prev.patterns.find((x) => x.part === p.part); head(h('diff.pattern', { part: partName(p.part) }));
    if (!q) { rows.push(`<tr><td colspan="4">${esc(t('diff.newpart'))}</td></tr>`); continue; }
    const n0 = rows.length, e0 = extent(q), e1 = extent(p);
    if (differs(e0.w, e1.w)) row(esc(t('diff.ex')), e0.w, e1.w, 'mm'); else same++;
    if (differs(e0.h, e1.h)) row(esc(t('diff.ey')), e0.h, e1.h, 'mm'); else same++;
    for (const k of [...p.curves, ...p.holes]) { const o = [...q.curves, ...q.holes].find((x) => x.name === k.name); const lab = h('diff.curve', { name: mono(k.name) }); if (!o) row(lab, '—', k.length, 'mm'); else if (differs(o.length, k.length)) row(lab, o.length, k.length, 'mm', `curve:${p.part}:${k.name}`); else same++; }
    for (const [k, val] of Object.entries(p.quantities)) { if (differs(q.quantities[k], val)) row(quantity(k, val)[0], q.quantities[k], val, quantityUnit(k) || ''); else same++; }
    if (rows.length === n0) rows.push(`<tr><td colspan="4" class="sub">${esc(t('diff.nochange'))}</td></tr>`);
  }
  return `<p class="hint lead">${h('diff.lead', { job: jobLink(prev.id, true), n: same })}</p>
    <div class="tbl"><table class="diff"><thead><tr><th>${esc(t('diff.value'))}</th><th class="num">${esc(t('job.n', { id: prev.id }))}</th><th class="num">${esc(t('job.n', { id: c.id }))}</th><th class="num">${esc(t('diff.change'))}</th></tr></thead><tbody>${rows.join('')}</tbody></table></div>`;
}

// ---------------------------------------------------------------- session state
let me = null;
let summary = null;          // last /api/dashboard result: status bar and navigation counts
let viewer = null;           // active PatternViewer, if the current view has one
let wsState = { id: null, part: 0, tab: 'gates', scope: 'all', gf: 'all', ghost: false };
let ctxProject = null;       // project of the view on screen, used by "new calculation"
let current = null;          // { name, args, data } of the view on screen: a language switch re-renders it without a request
let loginState = null;       // what the sign-in screen shows, for the same reason
const notes = [];            // notifications of this browser session (there is no server notification service)
let unseen = 0, busy = 0, online = true;

// A failed request. `server` marks text the server wrote (English); `key` a message of this client.
class ApiError extends Error { constructor(message, o = {}) { super(message); Object.assign(this, o); } }
const errHtml = (err) => (err && err.key ? esc(t(err.key, err.params)) : err && err.server ? srv(err.message) : esc(String((err && err.message) || err)));

// ---------------------------------------------------------------- kept input (bug hunt round 7)
let calcKeep = null; // what the new-calculation form shows after an unsent request (BH-003)
// The values of the new-calculation form as a request: the submit, the checks and the redraw after a switch read them alike.
function readCalcForm(form) {
  const fam = form.fam.value, req = { family: fam };
  for (const [n] of F[fam].f) { const el = form.elements.namedItem(n); req[n] = el ? parseDim(el.value) : NaN; }
  if (F[fam].sref) req.surface_reference = form.surface_reference.value;
  if (F[fam].ext) for (const n of ['header_surface_reference', 'branch_surface_reference', 'joint']) { const el = form.elements.namedItem(n); if (el && el.value) req[n] = el.value; }
  return req;
}
// After a language switch the form shows its errors again. The checks run again on the values now on the screen, and an
// engine refusal is drawn from the answer kept for it. Nothing is sent.
function reapplyCalc(keep) {
  const form = $('#calc'); if (!form) return;
  const { errors } = validateRequest(form.fam.value, readCalcForm(form));
  clearFieldErrors(form);
  if (!errors.length) { calcKeep = null; return; }
  errors.slice().reverse().forEach(([k, msg]) => fieldError(form.elements.namedItem(k), msg, false));
  $('#cmsg').innerHTML = `<div class="note bad" role="alert"><div>${h('math.notsent', { n: errors.length })}</div></div>`;
  calcKeep = keep;
}
// When the session ends while a form is open, its values are kept for the next sign-in (BH-010). Only the values of
// the visible form are kept, never a password or a token, and only in this tab's sessionStorage. If storage is not
// available, nothing is kept and the sign-in works as before.
function keepDraft() {
  try {
    const hash = location.hash;
    if (!/^#\/[a-z0-9_\/-]*$/i.test(hash)) return;
    const fields = [];
    $$('#view input, #view select, #view textarea').forEach((el) => {
      if (!el.id || el.type === 'password' || el.type === 'file' || el.dataset.nosnap !== undefined || /pass|token|setup/i.test(`${el.id} ${el.name}`)) return;
      fields.push([el.id, el.value]);
    });
    if (fields.length) sessionStorage.setItem('nf-draft', JSON.stringify({ hash, fields }));
  } catch { /* storage unavailable: nothing is kept */ }
}
// The kept values for the page now shown, taken once: they are put back when that page is drawn.
function takeDraft() {
  try {
    const raw = sessionStorage.getItem('nf-draft'); if (!raw) return null;
    sessionStorage.removeItem('nf-draft');
    const d = JSON.parse(raw);
    return d && d.hash === location.hash && Array.isArray(d.fields) ? d : null;
  } catch { return null; }
}
// A change of AMOEBA or administration tab is a step in the history: Back and Forward step through the tabs (BH-012).
function pushTab(url) { try { if (new URL(url, location.href).href !== location.href) history.pushState(null, '', url); } catch { /* address untouched */ } }

async function api(path, opts = {}) {
  const init = { method: opts.method || 'GET', headers: { 'X-NF-Request': '1' }, credentials: 'same-origin' }; // anti-forgery header required by the server
  if (opts.json !== undefined) { init.headers['Content-Type'] = 'application/json'; init.body = JSON.stringify(opts.json); }
  if (opts.body !== undefined) { init.body = opts.body; Object.assign(init.headers, opts.headers || {}); }
  setBusy(1);
  let res;
  try { res = await fetch(path, init); } catch { setOnline(false); throw new ApiError('', { key: 'api.unreachable' }); } finally { setBusy(-1); }
  setOnline(true);
  let data = null;
  try { data = await res.json(); } catch { /* non-JSON */ }
  if (res.status === 401 && path !== '/api/login') { const was = me; me = null; keepDraft(); renderLogin({ setup: false, key: was ? 'login.ended' : null }); throw new ApiError('', { key: 'api.signin', status: 401 }); }
  // A request the engine refused is answered 422 and kept by the server as job `id` in state REFUSED (no result).
  // The caller gets it back like any other job answer and shows it as NO RESULT.
  if (res.status === 422 && data && data.state === 'REFUSED' && Number.isInteger(data.id)) { factsCache.clear(); return data; }
  if (!res.ok) throw data && data.error ? new ApiError(String(data.error), { server: true, status: res.status }) : new ApiError('', { key: 'api.failed', params: { status: res.status }, status: res.status });
  if (init.method !== 'GET') factsCache.clear();
  return data;
}
function setBusy(d) { busy = Math.max(0, busy + d); const p = $('#progress'); if (p) p.classList.toggle('on', busy > 0); }
function setOnline(v) { online = v; const l = $('#sb-conn'); if (l) l.innerHTML = `<span class="led${v ? '' : ' off'}"></span>${esc(t(v ? 'shell.conn.ok' : 'shell.conn.off'))}`; }

// Notifications: transient toast plus a session log behind the bell. Never a blocking browser dialog.
// `msg` builds the HTML of the message when it is shown, so that the log follows a language switch.
function toast(msg, kind = 'info') {
  const render = typeof msg === 'function' ? msg : () => msg;
  notes.unshift({ at: Date.now(), render, kind }); if (notes.length > 50) notes.pop();
  unseen++; paintBell();
  const host = $('#toasts'); if (!host) return;
  const el = document.createElement('div'); el.className = `toast ${kind}`; el.setAttribute('role', kind === 'err' ? 'alert' : 'status'); el.dataset.at = Date.now();
  el.innerHTML = `<span>${render()}</span><button aria-label="${esc(t('toast.dismiss'))}">${ic('close', 12)}</button>`;
  const drop = () => el.remove(); $('button', el).onclick = drop; setTimeout(drop, kind === 'err' ? 8000 : 4000);
  host.replaceChildren(el); // one at a time; everything stays in the session log behind the bell
}
const toastErr = (err) => toast(() => errHtml(err), 'err');
const clearFieldErrors = (form) => { $$('.field-err', form).forEach((m) => m.remove()); $$('[aria-invalid]', form).forEach((i) => i.removeAttribute('aria-invalid')); };
function paintBell() { const d = $('#belldot'); if (d) d.hidden = unseen === 0; const b = $('#bell'); if (b) b.setAttribute('aria-label', unseen ? t('shell.bell.new', { n: unseen }) : t('shell.bell')); }
function fieldError(input, msg, focus = true) {
  input.setAttribute('aria-invalid', 'true');
  const field = input.closest('.field'); let m = (field || input.parentElement).querySelector('.field-err');
  if (!m) { m = document.createElement('div'); m.className = 'field-err'; m.setAttribute('role', 'alert'); if (field) field.append(m); else input.after(m); }
  m.textContent = msg; if (focus) input.focus();
  input.addEventListener('input', () => { input.removeAttribute('aria-invalid'); m.remove(); }, { once: true });
}
// BH-014: a refusal is shown by its code, with the values it names; a code without a message shows the server's text
const numText = (v) => (v === undefined || v === null ? '' : String(v));
const refusalHtml = (r) => (r && has(`refusal.${r.code}`)
  ? h(`refusal.${r.code}`, { fields: esc((r.fields || []).join(', ')), limit: esc(numText(r.detail && r.detail.limit)), measured: esc(numText(r.detail && r.detail.measured)) })
  : eng(r ? r.message : ''));
// the same marking as fieldError, with a message that is already HTML
function refusedField(input, html) {
  input.setAttribute('aria-invalid', 'true');
  const field = input.closest('.field'); let m = (field || input.parentElement).querySelector('.field-err');
  if (!m) { m = document.createElement('div'); m.className = 'field-err'; m.setAttribute('role', 'alert'); if (field) field.append(m); else input.after(m); }
  m.innerHTML = html;
  input.addEventListener('input', () => { input.removeAttribute('aria-invalid'); m.remove(); }, { once: true });
}
const formError = (sel, err) => { const m = $(sel); if (m) m.innerHTML = `<div class="note bad" role="alert"><div>${errHtml(err)}</div></div>`; };
async function withBusy(btn, label, fn) { const old = btn.innerHTML; btn.disabled = true; btn.classList.add('busy'); if (label) btn.textContent = label; try { return await fn(); } finally { if (btn.isConnected) { btn.disabled = false; btn.classList.remove('busy'); btn.innerHTML = old; } } }

function setTheme(th) { document.documentElement.dataset.theme = th === 'light' ? 'light' : 'dark'; prefs.set('nf-theme', document.documentElement.dataset.theme); }
setTheme(prefs.get('nf-theme', 'dark'));
// Japanese mode only: show engine and server prose in the reviewed translation or in the original English.
function setEngSrc(v) { document.documentElement.dataset.engsrc = v === 'en' ? 'en' : 'ja'; prefs.set('nf-engsrc', document.documentElement.dataset.engsrc); $$('.engsrc button').forEach((b) => b.setAttribute('aria-pressed', b.dataset.src === document.documentElement.dataset.engsrc)); }
setEngSrc(prefs.get('nf-engsrc', 'ja'));

// ---------------------------------------------------------------- language switch
// Nothing is reloaded and nothing is asked of the server: the screen is rendered again from the data it already
// holds, and what the user had typed, opened, scrolled or arranged is put back.
const langSwitch = () => `<div class="lang" role="group" aria-label="${esc(t('shell.lang.aria'))}">${LANGS.map((l) => `<button type="button" id="lang-${l}" data-lang="${l}" lang="${l}" aria-pressed="${l === lang()}">${esc(LANG_NAME[l])}</button>`).join('')}</div>`;
function snapUI() {
  const s = { fields: [], files: [], details: $$('#view details').map((d) => d.open), scroll: [$('#view') ? $('#view').scrollTop : 0, ...$$('.pane-scroll').map((p) => p.scrollTop)], focus: document.activeElement && document.activeElement.id, dialogs: $$('dialog[open]').map((d) => d.id), notes: !!$('#notes') && !$('#notes').hidden, viewer: viewer ? viewer.getState() : null };
  $$('input, select, textarea').forEach((el) => { if (!el.id || el.dataset.nosnap !== undefined) return; if (el.type === 'file') { if (el.files.length) s.files.push(el); } else s.fields.push([el.id, el.value]); });
  return s;
}
function restoreUI(s) {
  // selects first: one of them decides which fields exist (the geometry family)
  for (const sel of [true, false]) for (const [id, val] of s.fields) { const el = document.getElementById(id); if (!el || (el.tagName === 'SELECT') !== sel || el.value === val) continue; el.value = val; el.dispatchEvent(new Event(sel ? 'change' : 'input', { bubbles: true })); }
  $$('#view details').forEach((d, i) => { if (s.details[i] !== undefined && d.open !== s.details[i]) d.open = s.details[i]; });
  if ($('#view')) $('#view').scrollTop = s.scroll[0]; $$('.pane-scroll').forEach((p, i) => { p.scrollTop = s.scroll[i + 1] || 0; });
  if (s.notes) toggleNotes(true);
  for (const id of s.dialogs) { if (id === 'help') openHelp(); else { const d = document.getElementById(id); if (d && !d.open) d.showModal(); } }
  // typed values inside a dialog were restored above; fields the dialog fills on opening must not overwrite them
  for (const [id, val] of s.fields) { const el = document.getElementById(id); if (el && el.closest('dialog') && el.value !== val) el.value = val; }
  if (viewer && s.viewer) { viewer.setState(s.viewer); if (viewer.syncToggles) viewer.syncToggles(); }
  const f = s.focus && document.getElementById(s.focus); if (f) f.focus({ preventScroll: true });
}
function switchLang(l) {
  if (!LANGS.includes(l) || l === lang()) return;
  const snap = snapUI(), keepCalcState = calcKeep;
  setLang(l); prefs.set('nf-lang', l);
  // a ?lang= in the address would win again on the next load: keep it in step with the choice
  try { const u = new URL(location.href); if (u.searchParams.has('lang')) { u.searchParams.set('lang', l); history.replaceState(null, '', u); } } catch { /* address untouched */ }
  if (!me) { if (bootError) renderBootError(bootError); else renderLogin(loginState || { setup: false }); restoreUI(snap); return; }
  $$('#toasts .toast').forEach((x) => x.remove());
  mountShell(true);
  if (current) paint(current, { keep: true, files: snap.files }); else route();
  restoreUI(snap);
  if (keepCalcState) reapplyCalc(keepCalcState);
}
document.addEventListener('click', (e) => { const b = e.target.closest && e.target.closest('[data-lang]'); if (b) switchLang(b.dataset.lang); });

// ---------------------------------------------------------------- shell
const NAV = [
  ['overview', [['dashboard', 'dashboard', 'd'], ['projects', 'folder', 'p']]],
  ['engineering', [['workspace', 'pattern', 'w'], ['manufacturing', 'layers', 'm']]],
  ['assurance', [['amoeba', 'nodes', 'a'], ['verification', 'shield', 'v']]],
  ['system', [['admin', 'sliders', 's']]],
];
const NAV_KEYS = Object.fromEntries(NAV.flatMap(([, items]) => items.map((i) => [i[2], i[0]])));

function railMode() { if (window.innerWidth < 900) return 'closed'; return prefs.get('nf-rail') || (window.innerWidth < 1440 ? 'closed' : 'open'); }
function applyRail() { const a = $('.app'); if (a) { a.dataset.rail = railMode(); $('#railtoggle').setAttribute('aria-expanded', a.dataset.rail === 'open'); } }
function toggleRail() { prefs.set('nf-rail', railMode() === 'open' ? 'closed' : 'open'); applyRail(); }

function mountShell(force) {
  if ($('.app') && !force) return;
  $('#app').innerHTML = `<a class="skip" href="#view">${esc(t('shell.skip'))}</a>
  <div class="app" data-rail="open">
    <header class="topbar">
      <button class="ib" id="railtoggle" aria-label="${esc(t('shell.rail.toggle'))}" aria-controls="rail" data-tip="${esc(t('shell.rail.tip'))}" data-tip-pos="start">${ic('rail')}</button>
      <a class="brand" href="#/dashboard" aria-label="${esc(t('shell.brand.aria'))}">${ic('mark', 18)}<span lang="en">NEXUS FABRIC</span></a>
      <nav class="crumbs" id="crumbs" aria-label="${esc(t('shell.crumbs'))}"></nav>
      <div class="devnote" role="note"><span>${esc(t('build.short'))}<span class="long">${esc(t('build.long'))}</span></span></div>
      <div class="tools">
        ${langSwitch()}
        <span class="bellwrap"><button class="ib" id="bell" aria-label="${esc(t('shell.bell'))}" aria-expanded="false" data-tip="${esc(t('shell.bell'))}" data-tip-pos="end">${ic('bell')}</button><span class="dot" id="belldot" hidden></span></span>
        <button class="ib" id="helpbtn" aria-label="${esc(t('shell.help.aria'))}" data-tip="${esc(t('shell.help.tip'))}" data-tip-pos="end">${ic('help')}</button>
        <button class="ib" id="theme" aria-label="${esc(t('shell.theme.aria'))}" data-tip="${esc(t('shell.theme.tip'))}" data-tip-pos="end">${ic('theme')}</button>
        <a class="who" href="#/account" aria-label="${esc(t('shell.account.aria', { user: me.username, role: roleText(me.role) }))}">${ic('user')}${ud(me.username)}<small>${esc(roleText(me.role))}</small></a>
        <button class="ib" id="logout" aria-label="${esc(t('shell.signout'))}" data-tip="${esc(t('shell.signout'))}" data-tip-pos="end">${ic('signout')}</button>
      </div>
      <div id="progress" role="progressbar" aria-label="${esc(t('shell.progress'))}"></div>
    </header>
    <nav class="rail" id="rail" aria-label="${esc(t('shell.nav.main'))}">${NAV.map(([group, items]) => {
      const vis = items.filter(([k]) => k !== 'admin' || me.role === 'admin');
      return vis.length ? `<h3>${esc(t(`nav.g.${group}`))}</h3>${vis.map(([k, i]) => `<a href="#/${k}" data-nav="${k}" data-tip="${esc(t(`nav.${k}`))}" data-tip-pos="right">${ic(i)}<span class="lbl">${esc(t(`nav.${k}`))}</span>${k === 'verification' ? '<span class="cnt" id="nav-obl" hidden></span>' : ''}</a>`).join('')}` : '';
    }).join('')}</nav>
    <main id="view" tabindex="-1"></main>
    <footer class="statusbar">
      <span id="sb-conn"></span>
      <span id="sb-engine" class="sb-opt"></span>
      <span class="sb-build">${esc(t('build.status'))}</span>
      <span class="sp"></span>
      ${ja() ? `<span class="engsrc sb-opt" role="group" aria-label="${esc(t('engsrc.aria'))}" data-tip="${esc(t('engsrc.tip'))}" data-tip-pos="top"><span>${esc(t('engsrc.label'))}</span><button data-src="ja" aria-pressed="false">${esc(t('engsrc.ja'))}</button><button data-src="en" aria-pressed="false">${esc(t('engsrc.en'))}</button></span>` : ''}
      <a id="sb-obl" href="#/verification" hidden></a>
      <button id="sb-help" class="sb-opt">${esc(t('shell.shortcuts'))} <kbd>?</kbd></button>
    </footer>
  </div>
  <div id="toasts" aria-live="polite"></div>
  <div class="pop" id="notes" role="region" aria-label="${esc(t('notes.title'))}" hidden></div>
  <dialog id="help" aria-labelledby="help-h"></dialog>`;
  applyRail(); setOnline(online); paintStatus(); paintBell(); setBusy(0);
  $('#railtoggle').onclick = toggleRail;
  $('#theme').onclick = () => setTheme(document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark');
  $('#logout').onclick = async () => { try { await api('/api/logout', { method: 'POST' }); } catch { /* signed out locally either way */ } me = null; renderLogin({ setup: false }); };
  $('#helpbtn').onclick = $('#sb-help').onclick = openHelp;
  $('#bell').onclick = () => toggleNotes();
  $$('.engsrc button').forEach((b) => { b.onclick = () => setEngSrc(b.dataset.src); });
  setEngSrc(document.documentElement.dataset.engsrc);
}

function toggleNotes(force) {
  const p = $('#notes'); if (!p) return; const open = force !== undefined ? force : p.hidden;
  if (open) {
    unseen = 0; paintBell();
    const kind = (k) => (k === 'err' ? ['bad', 'err'] : k === 'ok' ? ['na', 'ok'] : k === 'warn' ? ['warn', 'warn'] : ['info', 'info']);
    p.innerHTML = `<header><h2>${esc(t('notes.title'))}</h2><button class="ib" id="notes-x" aria-label="${esc(t('notes.close'))}">${ic('close', 12)}</button></header>
      ${notes.length ? `<ul>${notes.map((n) => `<li>${tag(kind(n.kind)[0], t(`notes.k.${kind(n.kind)[1]}`))} ${n.render()}<time>${when(n.at / 1000)}</time></li>`).join('')}</ul>` : empty('bell', t('notes.empty.t'), esc(t('notes.empty.p')))}
      <div class="panel-f">${esc(t('notes.foot'))}</div>`;
    $('#notes-x').onclick = () => toggleNotes(false);
  }
  p.hidden = !open; $('#bell').setAttribute('aria-expanded', open);
}

function openHelp() {
  const d = $('#help'); if (!d || d.open) return;
  const k = (...keys) => keys.map((x) => `<kbd>${x}</kbd>`).join(' ');
  const row = (keys, key) => `<dt>${keys}</dt><dd>${esc(t(key))}</dd>`;
  d.innerHTML = `<header><h2 id="help-h">${esc(t('help.title'))}</h2><button class="ib" id="help-x" aria-label="${esc(t('close'))}">${ic('close')}</button></header>
    <div class="dlg-b" tabindex="0" aria-label="${esc(t('help.list'))}">
      <div><h3>${esc(t('help.goto'))}</h3><dl>${NAV.flatMap(([, items]) => items).filter(([key]) => key !== 'admin' || me.role === 'admin').map(([key, , sc]) => `<dt>${k('G', sc.toUpperCase())}</dt><dd>${esc(t(`nav.${key}`))}</dd>`).join('')}</dl></div>
      <div><h3>${esc(t('help.anywhere'))}</h3><dl>${row(k('?'), 'help.k.overlay')}${row(k('N'), 'help.k.new')}${row(k('/'), 'help.k.filter')}${row(k('['), 'help.k.rail')}${row(k('T'), 'help.k.theme')}${row(k('L'), 'help.k.lang')}${row(k('Esc'), 'help.k.esc')}</dl></div>
      <div><h3>${esc(t('help.viewer'))}</h3><dl>${row(k('F'), 'help.v.fit')}${row(`${k('+')} ${k('−')}`, 'help.v.zoom')}${row(k('←', '↑', '↓', '→'), 'help.v.pan')}${row(k('M'), 'help.v.measure')}${row(`${k('1')}…${k('9')}`, 'help.v.part')}${row(k('Esc'), 'help.v.esc')}</dl></div>
      <div><h3>${esc(t('help.panels'))}</h3><dl>${row(k('←', '→'), 'help.p.tabs')}${row(k('←', '→'), 'help.p.split')}${row(k('Tab'), 'help.p.curves')}</dl></div>
    </div>
    <div class="dlg-f">${esc(t('help.foot', { build: t('build.full') }))}</div>`;
  $('#help-x').onclick = () => d.close();
  d.showModal();
}

function paintStatus() {
  if (!summary) return;
  const e = $('#sb-engine'); if (e) e.innerHTML = h('shell.engine', { id: mono(summary.engine) });
  const n = summary.open_obligations, s = $('#sb-obl'), c = $('#nav-obl');
  if (s) { s.hidden = !n; s.textContent = t('shell.obl', { n }); }
  if (c) { c.hidden = !n; c.textContent = n; c.setAttribute('aria-label', t('shell.obl.aria', { n })); }
}
async function refreshSummary() { try { summary = await api('/api/dashboard'); paintStatus(); } catch { /* status bar keeps its last state */ } }

// ---------------------------------------------------------------- sign in
// Desktop mode (nf-server --desktop) hands the one-time setup token over in the URL fragment, which the browser never sends to the server.
// It is read once, kept in memory, and removed from the address bar. Server mode never sets a fragment, so this is a no-op there.
let desktopSetupToken = null;
try {
  const m = /^#setup=([0-9a-f]{64})$/.exec(location.hash);
  if (m) { desktopSetupToken = m[1]; history.replaceState(null, '', location.pathname + location.search); }
} catch { /* no fragment support: the token can be typed in */ }
// `st`: { setup, key, params, err, kind, username } — kept so that the screen can be drawn again in another language.
function renderLogin(st) {
  loginState = st = { setup: false, key: null, params: null, err: null, kind: 'bad', username: '', ...st };
  current = null; disposeView(); document.title = `${t('login.doc')} — NEXUS FABRIC`;
  const { setup } = st;
  // a server message starts in lower case; in English it is shown as a sentence
  const message = st.err ? (st.err.server && !ja() ? esc(st.err.message.charAt(0).toUpperCase() + st.err.message.slice(1)) : errHtml(st.err)) : st.key ? esc(t(st.key, st.params)) : '';
  $('#app').innerHTML = `<main class="login"><div class="login-top">${langSwitch()}</div><div class="login-card">
    <div class="brand">${ic('mark', 20)}<span lang="en">NEXUS FABRIC</span></div>
    <h1>${esc(t(setup ? 'login.h.setup' : 'login.h'))}</h1>
    <p class="sub">${esc(t(setup ? 'login.sub.setup' : 'login.sub'))}</p>
    ${note(h('build.login'))}
    ${message ? `<div class="note ${st.kind}" role="alert" id="loginmsg">${ic(st.kind === 'info' ? 'info' : 'warn')}<div>${message}</div></div>` : ''}
    <form id="f" novalidate><div class="field"><label for="lu">${esc(t('login.user'))}</label><input id="lu" name="u" autocomplete="username" autocapitalize="none" spellcheck="false" value="${esc(st.username)}"></div>
    <div class="field"><label for="lp">${esc(t(setup ? 'login.pass.setup' : 'login.pass'))}</label><input id="lp" name="p" type="password" autocomplete="${setup ? 'new-password' : 'current-password'}"></div>
    ${setup && !desktopSetupToken ? `<div class="field"><label for="lt">${esc(t('login.token'))}</label><input id="lt" name="t" autocomplete="off" spellcheck="false"><p class="hint">${h('login.token.hint')}</p></div>` : ''}
    <button class="pri">${esc(t(setup ? 'login.go.setup' : 'login.go'))}</button></form>
    <p class="legal">${esc(t('login.legal'))}</p></div></main>`;
  const f = $('#f'); (st.username ? f.p : f.u).focus();
  f.onsubmit = async (e) => {
    e.preventDefault(); clearFieldErrors(f);
    if (!f.u.value.trim()) { fieldError(f.u, t('login.e.user')); return; }
    if (!f.p.value) { fieldError(f.p, t('login.e.pass')); return; }
    if (setup && f.p.value.length < 10) { fieldError(f.p, t('login.e.len')); return; }
    if (setup && !desktopSetupToken && !f.t.value.trim()) { fieldError(f.t, t('login.e.token')); return; }
    const body = { username: f.u.value, password: f.p.value };
    try {
      await withBusy($('button', f), t(setup ? 'login.busy.setup' : 'login.busy'), async () => {
        if (setup) {
          await api('/api/setup', { method: 'POST', json: { ...body, setup_token: (f.t ? f.t.value.trim() : '') || desktopSetupToken || '' } });
          desktopSetupToken = null;
        }
        await api('/api/login', { method: 'POST', json: body });
        me = await api('/api/me');
      });
      loginState = null; route();
    } catch (err) {
      const badLogin = /invalid username or password/i.test(err.message);
      renderLogin({ setup: setup && !/already/i.test(err.message), key: badLogin ? 'login.bad' : null, err: badLogin ? null : err, kind: 'bad', username: body.username });
    }
  };
}

// ---------------------------------------------------------------- tables shared by several views
const stateCell = (c) => `${stateTag(effState(c))}${c.open_obligations && !c.superseded ? ` ${tag('warn', t('obl.open.n', { n: c.open_obligations }))}` : ''}`;
const partsCell = (c) => (!c.facts ? '<span class="sub">—</span>' : c.facts.noResult ? `<span class="sub">${esc(t('parts.none'))}</span>` : c.facts.parts.map((p) => partName(p.part)).join(', '));
const newCalcBtn = (pid) => (me.role === 'viewer' ? '' : `<a class="btn pri" href="#/math${pid ? `/${pid}` : ''}">${ic('plus')}${esc(t('new.calc'))}</a>`);
const noJobs = () => empty('pattern', t('jobs.none.t'), esc(t('jobs.none.p')), newCalcBtn());
const byWhen = (who, ts) => `${ud(who)}<br><span class="sub">${when(ts)}</span>`;
// rows must have been through withFacts(); `project` adds the project column, `replay` the replay column
const jobsTable = (rows, id, o = {}) => (rows.length ? `<div class="tbl"><table id="${id}" class="jobs"><thead><tr><th>${esc(t('th.job'))}</th>${o.project ? `<th>${esc(t('th.project'))}</th>` : ''}<th>${esc(t('th.family'))}</th><th>${esc(t('th.parts'))}</th><th>${esc(t('th.state'))}</th>${o.replay ? `<th>${esc(t('th.replay'))}</th>` : ''}<th>${esc(t('th.entered'))}</th></tr></thead><tbody>${rows.map((c) =>
  `<tr><td class="nw">${jobLink(c.id)}</td>${o.project ? `<td>${plink(c.project_id)}</td>` : ''}<td><span title="${esc(c.family)}">${famShort(c.family)}</span></td><td>${partsCell(c)}</td><td class="nw">${stateCell(c)}</td>${o.replay ? `<td class="nw">${replayTag(c.facts ? c.facts.replay : null)}</td>` : ''}<td class="nw">${byWhen(c.created_by, c.created_at)}</td></tr>`).join('')}</tbody></table></div>` : noJobs());
const lessonsTable = (lessons) => (lessons.length ? `<div class="tbl"><table><thead><tr><th>#</th><th>${esc(t('th.what'))}</th><th>${esc(t('th.fromjob'))}</th><th class="num">${esc(t('th.matched'))}</th><th>${esc(t('th.recorded'))}</th></tr></thead><tbody>${lessons.map((l) => `<tr><td>${l.id}</td>
  <td>${l.kind === 'corrected_value' ? `${h('lesson.corrected', { fam: famShort(l.family), param: `<b class="mono">${esc(l.parameter)}</b>`, wrong: mono(fmt(l.wrong_value)), right: mono(fmt(l.right_value)) })}<br><span class="sub">${ud(l.detail)}</span>` : h('lesson.other', { kind: named('lesson', l.kind), fam: famShort(l.family) })}</td>
  <td class="nw">${jobLink(l.source_calc)}<br><span class="sub">${pnameHtml(l.project_id)}</span></td><td class="num">${l.times_matched}</td><td class="nw">${byWhen(l.created_by, l.created_at)}</td></tr>`).join('')}</tbody></table></div>`
  : empty('clock', t('lessons.none.t'), esc(t('lessons.none.p'))));
// "<kind> · <label>" of an obligation, as the server names what is affected
const oblSubject = (o) => `${labelled('rkind', o.kind)} · ${ja() && nodeName(o.kind, o.label) ? `${esc(nodeName(o.kind, o.label))} ` : ''}${mono(o.label)}`;
// the note of an obligation is the engine's until a person resolved it; then it is that person's reason
const oblNote = (o) => (o.resolved_by ? ud(o.note) : eng(o.note));

// ---------------------------------------------------------------- dependency graph (layered, drawn from the real edge list)
function depGraph(edges, compact = false) {
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
  const idleInfo = () => h('dg.idle', { amber: `<span class="c-warn">${esc(t('dg.amber'))}</span>`, blue: `<span class="c-acc">${esc(t('dg.blue'))}</span>` });
  const nodeAria = (n) => esc(t('dg.node.aria', { label: n.l, a: n.out.length, b: n.inn.length }));
  const wireTrace = (root) => {
    const host = $('.dg,.dgl', root), info = $('.dg-info', root); if (!host) return;
    const nodes = [...N.values()], idle = info.innerHTML;
    const closure = (n, key) => { const set = new Set(); const walk = (x) => x[key].forEach((y) => { if (!set.has(y)) { set.add(y); walk(y); } }); walk(n); return set; };
    const focus = (n) => {
      $$('.on,.up,.down,.self', host).forEach((x) => x.classList.remove('on', 'up', 'down', 'self'));
      if (!n) { host.classList.remove('focus'); info.innerHTML = idle; return; }
      const up = closure(n, 'out'), down = closure(n, 'inn');
      host.classList.add('focus');
      $$('[data-n]', host).forEach((g) => { const m = nodes[g.dataset.n]; if (m === n) g.classList.add('on', 'self'); else if (up.has(m)) g.classList.add('on', 'up'); else if (down.has(m)) g.classList.add('on', 'down'); });
      $$('.e', host).forEach((q) => { const e = E[q.dataset.e]; if ((e.a === n || up.has(e.a)) && up.has(e.b)) q.classList.add('on', 'up'); else if ((e.b === n || down.has(e.b)) && down.has(e.a)) q.classList.add('on', 'down'); });
      info.innerHTML = h(down.size ? 'dg.reach' : 'dg.reach.none', { node: `<b>${nodeLabel(n.l)}</b>`, n: up.size, m: down.size, list: [...down].map((x) => nodeLabel(x.l)).join(', ') });
    };
    let pinned = null;
    host.addEventListener('click', (e) => { const g = e.target.closest('[data-n]'); pinned = g && nodes[g.dataset.n] !== pinned ? nodes[g.dataset.n] : null; focus(pinned); });
    host.addEventListener('keydown', (e) => { const g = e.target.closest('[data-n]'); if (g && g.tagName !== 'BUTTON' && (e.key === 'Enter' || e.key === ' ')) { e.preventDefault(); pinned = nodes[g.dataset.n]; focus(pinned); } if (e.key === 'Escape' && pinned) { e.stopPropagation(); pinned = null; focus(null); } });
    host.addEventListener('mouseover', (e) => { const g = e.target.closest('[data-n]'); if (!pinned && g) focus(nodes[g.dataset.n]); });
    host.addEventListener('mouseleave', () => { if (!pinned) focus(null); });
  };
  const kindText = (k) => (ja() ? idLabel('rkind', k) || k : k);
  const nameText = (k, n) => (ja() ? nodeName(k, n) : null);
  // narrow panels: the same layers as lists of records, traced the same way, nothing clipped
  if (compact) return { wire: wireTrace, html: `<div class="dgl" role="group" aria-label="${esc(t('dg.layers.aria', { n: N.size, e: E.length }))}">${cols.map((col, d) => `<section><h3>${esc(levelName(d))} <span class="cnt">${col.length}</span></h3><div>${col.map((n) => { const [kind, name] = nodeParts(n.l); const jn = nameText(kind, name); return `<button class="chip node" data-n="${n.i}" aria-label="${nodeAria(n)}">${kind ? `<small>${esc(kindText(kind))}</small>` : ''}<span>${jn ? `${esc(jn)} ` : ''}${ja() ? mono(name) : esc(name)}</span></button>`; }).join('')}</div></section>`).join('')}</div><p class="dg-info" aria-live="polite">${idleInfo()} ${esc(t('dg.layers.note'))}</p>` };
  const NW = 176, NH = 40, CG = 52, RG = 10, P = 14, HD = 24;
  const rows = Math.max(...cols.map((c) => c.length)), W = P * 2 + cols.length * NW + (cols.length - 1) * CG, H = P * 2 + HD + rows * (NH + RG);
  N.forEach((n) => { n.x = P + n.d * (NW + CG); n.y = P + HD + n.row * (NH + RG) + (rows - cols[n.d].length) * (NH + RG) / 2; });
  const clip = (s, n) => (s.length > n ? s.slice(0, n - 1) + '…' : s);
  const html = `<div class="dg-wrap" tabindex="0" role="group" aria-label="${esc(t('dg.graph.aria', { n: N.size, e: E.length }))}"><svg class="dg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="list">
    ${cols.map((c, d) => `<text class="colh" x="${P + d * (NW + CG)}" y="${P + 10}">${esc(levelName(d))}</text>`).join('')}
    ${E.map((e, i) => { const x1 = e.b.x + NW, y1 = e.b.y + NH / 2, x2 = e.a.x, y2 = e.a.y + NH / 2, dx = Math.max(18, (x2 - x1) / 2); return `<path class="e" data-e="${i}" d="M${x1} ${y1}C${x1 + dx} ${y1} ${x2 - dx} ${y2} ${x2} ${y2}"><title>${esc(e.a.l)} —${esc(e.rel)}→ ${esc(e.b.l)}</title></path>`; }).join('')}
    ${[...N.values()].map((n) => { const [kind, name] = nodeParts(n.l); const shown = nameText(kind, name) || name; return `<g class="n" data-n="${n.i}" tabindex="0" role="listitem" aria-label="${nodeAria(n)}"><title>${esc(n.l)}</title><rect x="${n.x}" y="${n.y}" width="${NW}" height="${NH}" rx="2"/>${kind ? `<text class="k" x="${n.x + 9}" y="${n.y + 15}">${esc(kindText(kind))}</text><text class="t${shown === name ? ' id' : ''}" x="${n.x + 9}" y="${n.y + 31}">${esc(clip(shown, ja() ? 13 : 22))}</text>` : `<text class="t${shown === name ? ' id' : ''}" x="${n.x + 9}" y="${n.y + 25}">${esc(clip(shown, ja() ? 13 : 22))}</text>`}</g>`; }).join('')}
  </svg></div><p class="dg-info" aria-live="polite">${idleInfo()} ${esc(t('dg.flow.note'))}</p>`;
  return { html, wire: wireTrace };
}
const edgeTable = (edges) => `<details class="sec edges"><summary>${esc(t('dg.edges', { n: edges.length }))}</summary><div class="tbl"><table><thead><tr><th>${esc(t('th.record'))}</th><th>${esc(t('th.relation'))}</th><th>${esc(t('th.dependson'))}</th></tr></thead><tbody>${edges.map((e) => `<tr><td>${nodeLabel(e.from)}</td><td>${ja() && idLabel('rel', e.rel) ? `${esc(idLabel('rel', e.rel))}<br>` : ''}${mono(e.rel)}</td><td>${nodeLabel(e.to)}</td></tr>`).join('')}</tbody></table></div></details>`;

// ---------------------------------------------------------------- views
// A view has two halves. `load(args…)` asks the server and returns plain data; `render(data, args…)` turns that
// data into { title, crumbs, html, wire(root), ws, nav } and asks nothing. A language switch calls render again
// on the data already loaded, so it needs no request and cannot fail half-way. ws = full-height workspace layout.
const views = {
  dashboard: {
    async load() {
      const [d, obs, cs] = await Promise.all([api('/api/dashboard'), api('/api/obligations'), api('/api/calculations'), loadProjects()]);
      summary = d; await withFacts(cs); return { d, obs, cs };
    },
    render({ d, obs, cs }) {
      paintStatus();
      const open = obs.filter((o) => o.state === 'open');
      // every count below comes from one function, the same one that labels the rows
      const count = (s) => cs.filter((c) => effState(c) === s).length;
      const n = { review: count('REVIEW_REQUIRED'), sup: count('SUPERSEDED'), none: count('NO_RESULT'), rej: count('REJECTED'), ok: count('APPROVED') };
      const stat = (v, key, o = {}) => `<div class="stat${o.attn && v ? ' attn' : ''}${o.cls ? ` ${o.cls}` : ''}"${o.id ? ` id="${o.id}"` : ''}><span>${esc(t(key))}</span><b>${v}</b></div>`;
      return {
        title: t('nav.dashboard'), crumbs: [[t('nav.dashboard')]],
        html: `<div class="page">${ph(esc(t('nav.dashboard')), h('dash.sub', { engine: mono(d.engine), min: Math.round(d.uptime_seconds / 60) }), newCalcBtn())}
          <div class="stats" role="group" aria-label="${esc(t('dash.totals'))}">${stat(d.projects, 'dash.s.projects')}${stat(d.calculations, 'dash.s.jobs')}${stat(n.review, 'dash.s.review', { id: 'st-review', attn: true, cls: 'g2' })}${stat(d.open_obligations, 'dash.s.obl', { attn: true })}${stat(n.sup, 'dash.s.sup', { id: 'st-sup', cls: 'g3' })}${stat(n.none, 'dash.s.none', { id: 'st-none', cls: 'bad' })}${stat(n.rej, 'dash.s.rej')}${stat(d.records, 'dash.s.records', { cls: 'g4' })}</div>
          <div class="cols"><div>
            ${panel(esc(t('dash.recent')), jobsTable(cs.slice(0, 8), 'jobs-recent', { project: true }), { flush: true, count: Math.min(8, cs.length), tools: `<a href="#/workspace">${esc(t('jobs.all'))}</a>` })}
          </div><div>
            ${panel(esc(t('dash.needs')), open.length ? `<ul class="il">${open.map((o) => `<li><div class="l1"><span class="id">#${o.id}</span><b>${oblSubject(o)}</b>${oblTag(o.state)}</div><p>${eng(o.required_check)}</p><p class="sub"><a href="#/calc/${o.target_calc}">${esc(t('dash.needs.open', { id: o.target_calc }))}</a> · ${esc(t('dash.needs.raised', { when: when(o.created_at) }))}</p></li>`).join('')}</ul>` : empty('shield', t('dash.needs.none.t'), esc(t('dash.needs.none.p'))), { count: open.length })}
            ${panel(esc(t('dash.state')), `<div class="bar" role="img" aria-label="${esc(t('dash.bar.aria', n))}"><i class="b-warn" style="flex:${n.review}"></i><i class="b-na" style="flex:${n.sup}"></i><i class="b-bad" style="flex:${n.none + n.rej}"></i><i class="b-ok" style="flex:${n.ok}"></i></div>
              <div class="legend" id="legend"><span><i class="b-warn"></i>${esc(t('dash.l.review', { n: n.review }))}</span><span><i class="b-na"></i>${esc(t('dash.l.sup', { n: n.sup }))}</span><span><i class="b-bad"></i>${esc(t('dash.l.none', { n: n.none, m: n.rej }))}</span><span><i class="b-ok"></i>${esc(t('dash.l.ok', { n: n.ok }))}</span></div>`,
              { foot: `${jaHtml(t('dash.state.foot'))}${d.calculations > cs.length ? ` ${esc(t('dash.partial', { n: cs.length, total: d.calculations }))}` : ''}` })}
          </div></div>
          ${plannedKeys('dash.planned', 2)}</div>`,
      };
    },
  },

  projects: {
    async load() { return { ps: await loadProjects() }; },
    render({ ps }) {
      return {
        title: t('nav.projects'), crumbs: [[t('proj.crumb')]],
        html: `<div class="page">${ph(esc(t('nav.projects')), esc(t('proj.sub')))}
          ${panel(esc(t('proj.panel')), ps.length ? `<div class="tbl"><table id="tbl-projects"><thead><tr><th>#</th><th>${esc(t('th.name'))}</th><th class="num">${esc(t('th.docs'))}</th><th class="num">${esc(t('th.jobs'))}</th><th>${esc(t('th.created'))}</th></tr></thead><tbody>${ps.map((p) => `<tr><td>${p.id}</td><td><a href="#/project/${p.id}">${ud(p.name)}</a><br><span class="sub">${ud(p.description)}</span></td><td class="num">${p.documents}</td><td class="num">${p.calculations}</td><td class="nw">${byWhen(p.created_by, p.created_at)}</td></tr>`).join('')}</tbody></table></div>`
            : empty('folder', t('proj.none.t'), esc(t(me.role === 'viewer' ? 'proj.none.viewer' : 'proj.none.p'))), { flush: true, count: ps.length, tools: ps.length ? filterBox('tbl-projects', t('proj.filter')) : '' })}
          ${me.role === 'viewer' ? '' : panel(esc(t('proj.new')), `<form id="np" novalidate><div class="fgrid"><div class="field"><label for="npn">${esc(t('proj.f.name'))}</label><input id="npn" name="n" maxlength="200"></div><div class="field"><label for="npd">${esc(t('proj.f.desc'))}</label><input id="npd" name="d"></div></div><div id="npmsg"></div><button class="pri">${esc(t('proj.create'))}</button></form>`)}
          ${plannedKeys('proj.planned', 2)}</div>`,
        wire() {
          const f = $('#np'); if (f) f.onsubmit = async (e) => { e.preventDefault(); clearFieldErrors(f); if (!f.n.value.trim()) { fieldError(f.n, t('proj.e.name')); return; } try { const r = await withBusy($('button', f), t('creating'), () => api('/api/projects', { method: 'POST', json: { name: f.n.value, description: f.d.value } })); toast(() => esc(t('proj.created', { id: r.id })), 'ok'); location.hash = `#/project/${r.id}`; } catch (err) { formError('#npmsg', err); } };
        },
      };
    },
  },

  project: {
    async load(id) {
      const [p, users] = await Promise.all([api(`/api/projects/${id}`), me.role === 'admin' ? api('/api/users') : [], loadProjects()]);
      await withFacts(p.calculations); return { p, users };
    },
    render({ p, users }, id) {
      ctxProject = p.id;
      const fname = (d) => { try { return decodeURIComponent(d.filename); } catch { return d.filename; } };
      const candidates = users.filter((u) => u.active && !p.members.includes(u.username));
      return {
        title: p.name, nav: 'projects', crumbs: [[t('proj.crumb'), '#/projects'], [ud(p.name), null, true]],
        html: `<div class="page">${ph(ud(p.name), `${p.description ? `${ud(p.description)} · ` : ''}${h('project.meta', { id: p.id, by: ud(p.created_by), when: when(p.created_at) })}`, newCalcBtn(p.id))}
          ${panel(esc(t('project.jobs')), jobsTable(p.calculations, 'jobs-project', { replay: true }), { flush: true, count: p.calculations.length, tools: p.calculations.length ? filterBox('jobs-project', t('jobs.filter')) : '' })}
          <div class="cols"><div>
          ${panel(esc(t('project.docs')), `${p.documents.length ? `<div class="tbl"><table><thead><tr><th>${esc(t('th.file'))}</th><th>${esc(t('th.dtype'))}</th><th class="num">${esc(t('th.size'))}</th><th>${esc(t('th.sha'))}</th><th>${esc(t('th.uploaded'))}</th></tr></thead><tbody>${p.documents.map((d) => `<tr><td>${ud(fname(d))}${d.extension_mismatch ? ` ${tag('warn', t('doc.mismatch'), 'warn')}` : ''}</td><td class="mono">${esc(d.detected_type)}</td><td class="num">${d.size}</td><td>${short(d.sha256)}</td><td class="nw">${byWhen(d.created_by, d.created_at)}</td></tr>`).join('')}</tbody></table></div>` : empty('upload', t('docs.none.t'), esc(t('docs.none.p')))}`,
            { flush: true, count: p.documents.length, foot: `${me.role === 'viewer' ? '' : `<form id="up" class="row"><input id="upf" class="vh" type="file" name="file" aria-label="${esc(t('doc.file.aria'))}"><label for="upf" class="btn">${ic('folder')}${esc(t('doc.choose'))}</label><span id="upname" class="sub">${esc(t('doc.nofile'))}</span><button>${ic('upload')}${esc(t('doc.store'))}</button></form><div id="upmsg"></div>`}
              <p>${h('doc.accepted')}</p>
              <p>${h('doc.snapshot', { id: p.snapshot ? `<span class="hash">${esc(p.snapshot)}</span>` : esc(t('doc.snapshot.none')) })}</p>` })}
          </div><div>
          ${panel(esc(t('project.access')), `${p.members.length ? `<ul class="members">${p.members.map((m) => `<li><span class="mono">${ud(m)}</span>${me.role === 'admin' ? `<button class="ghost" data-rm="${esc(m)}" aria-label="${esc(t('member.remove.aria', { user: m }))}">${ic('close', 12)}<span>${esc(t('member.remove'))}</span></button>` : ''}</li>`).join('')}</ul>` : `<p class="sub">${esc(t('members.none'))}</p>`}<p class="hint gap">${esc(t('members.hint'))}</p>
            ${me.role === 'admin' ? `<form id="mem" novalidate><div class="field"><label for="memu">${esc(t('member.add.label'))}</label><select id="memu" name="u">${candidates.length ? candidates.map((u) => `<option value="${esc(u.username)}">${esc(t('member.opt', { user: u.username, role: roleText(u.role) }))}</option>`).join('') : `<option value="">${esc(t('member.all'))}</option>`}</select></div><div id="memmsg"></div><button${candidates.length ? '' : ' disabled'}>${esc(t('member.add'))}</button></form>` : ''}`, { count: p.members.length })}
          </div></div></div>`,
        wire(root) {
          const send = async (username, remove) => { try { await api(`/api/projects/${id}/members`, { method: 'POST', json: { username, remove } }); toast(() => h(remove ? 'member.removed' : 'member.added', { user: ud(username) }), 'ok'); refresh(); } catch (err) { formError('#memmsg', err); } };
          const mem = $('#mem'); if (mem) mem.onsubmit = (e) => { e.preventDefault(); if (mem.u.value) send(mem.u.value, false); };
          $$('[data-rm]', root).forEach((b) => { b.onclick = () => { if (!b.dataset.armed) { b.dataset.armed = '1'; b.classList.add('danger'); b.lastChild.textContent = t('member.confirm', { user: b.dataset.rm }); setTimeout(() => { if (b.isConnected) { delete b.dataset.armed; b.classList.remove('danger'); b.lastChild.textContent = t('member.remove'); } }, 5000); return; } send(b.dataset.rm, true); }; });
          const up = $('#up'); if (up) {
            const show = () => { $('#upname').innerHTML = up.file.files[0] ? ud(up.file.files[0].name) : esc(t('doc.nofile')); };
            show(); up.file.onchange = () => { show(); $('#upmsg').innerHTML = ''; };
            up.onsubmit = async (e) => {
              e.preventDefault(); const file = up.file.files[0]; if (!file) { formError('#upmsg', new ApiError('', { key: 'doc.e.nofile' })); return; }
              try { const r = await withBusy($('button', up), t('storing'), () => api(`/api/projects/${id}/documents`, { method: 'POST', body: file, headers: { 'X-Filename': encodeURIComponent(file.name).slice(0, 180) } })); toast(() => h('doc.stored', { type: mono(r.detected_type), interp: eng(r.interpretation, { tag: false }) }), 'ok'); refresh(); }
              catch (err) { formError('#upmsg', err); }
            };
          }
        },
      };
    },
  },

  // Job browser. The workspace itself is the job view (#/calc/:id) and the new-calculation view (#/math).
  workspace: {
    async load() { const [cs] = await Promise.all([api('/api/calculations'), loadProjects()]); await withFacts(cs); return { cs }; },
    render({ cs }) {
      return {
        title: t('nav.workspace'), crumbs: [[t('nav.workspace')]],
        html: `<div class="page">${ph(esc(t('nav.workspace')), esc(t('ws.sub')), newCalcBtn())}
          ${panel(esc(t('ws.all')), jobsTable(cs, 'jobs-all', { project: true, replay: true }), { flush: true, count: cs.length, tools: cs.length ? filterBox('jobs-all', t('jobs.filter')) : '' })}</div>`,
      };
    },
  },

  // New calculation, in the workspace layout: inputs left, parameter schematic centre, scope of the build right.
  math: {
    async load(projectId, fromId) {
      const ps = await loadProjects(); let from = null, refusal = null;
      if (fromId && me.role !== 'viewer') { try { const job = await api(`/api/calculations/${fromId}`); from = job.request; refusal = job.state === 'REFUSED' ? job.refusal || null : null; } catch { from = null; } }
      return { ps, from, refusal };
    },
    render({ ps, from, refusal }, projectId, fromId) {
      const crumbs = [[t('nav.workspace'), '#/workspace'], [t('new.calc')]], title = t('new.calc');
      if (!ps.length) return { title, nav: 'workspace', crumbs, html: `<div class="page">${ph(esc(title), '')}${empty('folder', t('math.noproj.t'), esc(t('math.noproj.p')), `<a class="btn pri" href="#/projects">${esc(t('math.noproj.go'))}</a>`)}</div>` };
      if (me.role === 'viewer') return { title, nav: 'workspace', crumbs, html: `<div class="page">${ph(esc(title), '')}${empty('shield', t('math.viewer.t'), esc(t('math.viewer.p')), `<a class="btn" href="#/workspace">${esc(t('math.viewer.go'))}</a>`)}</div>` };
      const pid = projectId || ctxProject || ps[0].id;
      const fam0 = from && F[from.family] ? from.family : 'cylinder_shell';
      const fields = (k) => F[k].f.map(([n, v, u]) => `<div class="field prop"><label for="f_${n}">${esc(plabel(k, n))}<span class="mono key">${n}</span></label><div class="inp"><input id="f_${n}" name="${n}" ${dimAttrs} value="${from && from.family === k && typeof from[n] === 'number' ? from[n] : v}"><span class="unit" aria-hidden="true">${esc(unitText(u))}</span></div></div>`).join('')
        + (F[k].sref ? `<div class="field"><label for="f_sref">${esc(t('math.sref.label'))}<span class="mono key">surface_reference</span></label><select id="f_sref" name="surface_reference">${['unknown', 'outside', 'inside', 'neutral'].map((o) => `<option value="${o}"${from && from.surface_reference === o ? ' selected' : ''}>${esc(t(`sref.${o}`))}</option>`).join('')}</select></div>` : '')
        + (F[k].ext ? extFields(from) : '');
      const lis = (prefix, n) => `<ul>${Array.from({ length: n }, (_, i) => `<li>${h(`${prefix}.${i + 1}`)}</li>`).join('')}</ul>`;
      return {
        title, nav: 'workspace', ws: true, crumbs,
        html: `<div class="ws"><div class="ws-head"><h1>${esc(title)}</h1><div class="ws-meta">${esc(t('math.meta'))}</div></div>
          <div class="ws-banner"><span>${h('math.banner')}</span></div>
          <div class="ws-body ws-new">
            <aside class="pane" aria-label="${esc(t('math.inputs.aria'))}"><form id="calc" class="pane-form" novalidate><div class="pane-h"><h2>${esc(t('math.pane.h'))}</h2></div><div class="pane-scroll">
              <details class="sec" open><summary>${esc(t('math.sec.job'))}</summary><div class="sec-b">
                <div class="field"><label for="f_project">${esc(t('math.f.project'))}</label><select id="f_project" name="project">${ps.map((p) => `<option value="${p.id}" ${String(p.id) === String(pid) ? 'selected' : ''}>#${p.id} ${esc(p.name)}</option>`).join('')}</select></div>
                <div class="field"><label for="f_fam">${esc(t('math.f.family'))}</label><select id="f_fam" name="fam">${Object.keys(F).map((k) => `<option value="${k}"${k === fam0 ? ' selected' : ''}>${famLabel(k)}</option>`).join('')}</select></div></div></details>
              <details class="sec" open><summary>${esc(t('math.sec.params'))} ${from ? tag('info', t('math.tag.from', { id: fromId })) : tag('na', t('math.tag.example'))}</summary><div class="sec-b"><div id="fields">${fields(fam0)}</div>
                <p class="hint">${esc(t(from ? 'math.hint.from' : 'math.hint.example'))}</p></div></details>
            </div><div class="pane-foot"><div id="cmsg"></div><button class="pri" id="run">${esc(t('math.run'))}</button></div></form></aside>
            <div class="split" role="separator" aria-orientation="vertical" aria-label="${esc(t('math.split.l'))}" tabindex="0" data-split="l"></div>
            <section class="stage sk-stage" aria-label="${esc(t('math.stage.h'))}"><div class="stage-bar"><h2>${esc(t('math.stage.h'))}</h2><span class="grow"></span>${tag('na', t('math.stage.tag'))}</div><div class="sk-host" id="sketch"><div class="sk-canvas" id="sk-canvas"></div><p class="sk-cap" id="sk-cap"></p></div></section>
            <div class="split" role="separator" aria-orientation="vertical" aria-label="${esc(t('math.split.r'))}" tabindex="0" data-split="r"></div>
            <aside class="pane" aria-label="${esc(t('math.limits.aria'))}"><div class="pane-h"><h2>${esc(t('math.limits.h'))}</h2></div><div class="pane-scroll">
              <details class="sec" open><summary>${esc(t('math.result.h'))}</summary><div class="sec-b">${lis('math.result', 5)}</div></details>
              <details class="sec" open id="conv"><summary>${esc(t('math.conv.h'))} ${tag('na', t('math.conv.tag'))}</summary><div class="sec-b"><p class="hint lead">${esc(t('math.conv.lead'))}</p>${lis('math.conv', 4)}</div></details>
              <details class="sec" open><summary>${esc(t('math.planned.h'))} ${tag('planned', t('math.planned.tag'), 'planned')}</summary><div class="sec-b">${lis('math.planned', 6)}</div></details>
            </div></aside>
          </div></div>`,
        wire(root) {
          wireSplits(root);
          const form = $('#calc'), canvas = $('#sk-canvas'), cap = $('#sk-cap'); let hot = null, confirmed = '';
          const read = () => readCalcForm(form);
          const draw = () => {
            const fam = form.fam.value, r = sketch(fam, read(), Math.max(320, canvas.clientWidth), Math.max(240, canvas.clientHeight));
            cap.innerHTML = r.cap; canvas.innerHTML = r.svg;
            if (hot) $$(`[data-dim="${hot}"]`, $('#sketch')).forEach((g) => g.classList.add('hot'));
            $('#conv').hidden = !/branch/.test(fam);
          };
          const clearMsg = () => { calcKeep = null; $('#cmsg').innerHTML = ''; $('#run').textContent = t('math.run'); };
          form.fam.onchange = () => { $('#fields').innerHTML = fields(form.fam.value); clearMsg(); hot = null; draw(); };
          $('#fields').addEventListener('input', () => { clearMsg(); draw(); });
          $('#fields').addEventListener('focusin', (e) => { hot = e.target.name || null; draw(); });
          $('#fields').addEventListener('focusout', () => { hot = null; draw(); });
          draw();
          // the drawing is laid out in real pixels: draw it again when its box changes (panel resize, caption wrap)
          let last = `${canvas.clientWidth}x${canvas.clientHeight}`;
          const ro = new ResizeObserver(() => { const now = `${canvas.clientWidth}x${canvas.clientHeight}`; if (now !== last && canvas.isConnected) { last = now; draw(); } }); ro.observe(canvas);
          (refusal && refusal.fields || []).forEach((k) => { const el = form.elements.namedItem(k); if (el) refusedField(el, refusalHtml(refusal)); });
          form.onsubmit = async (e) => {
            e.preventDefault(); calcKeep = null; const req = read(), fam = req.family, { errors, warnings } = validateRequest(fam, req);
            clearFieldErrors(form);
            if (errors.length) { calcKeep = { kind: 'client' }; errors.slice().reverse().forEach(([k, msg]) => fieldError(form.elements.namedItem(k), msg)); $('#cmsg').innerHTML = `<div class="note bad" role="alert"><div>${h('math.notsent', { n: errors.length })}</div></div>`; return; }
            const key = JSON.stringify(req);
            if (warnings.length && confirmed !== key) { confirmed = key; $('#cmsg').innerHTML = `<div class="note" role="alert" id="plaus"><div><b>${esc(t('math.plaus.t'))}</b><ul>${warnings.map(([, m]) => `<li>${esc(m)}</li>`).join('')}</ul>${esc(t('math.plaus.p'))}</div></div>`; $('#run').textContent = t('math.run.unusual'); return; }
            try {
              const r = await withBusy($('#run'), t('math.busy'), () => api(`/api/projects/${form.project.value}/calculations`, { method: 'POST', json: req }));
              const facts = await jobFacts(r.id);
              if (facts && facts.noResult) toast(() => h('math.toast.none', { id: r.id, why: facts.outcome ? eng(facts.outcome) : esc(t('math.why.default')) }), 'err');
              else toast(() => esc(t('math.toast.ok', { id: r.id, state: r.state ? stateText(r.state) : t('math.state.default') })), 'ok');
              location.hash = `#/calc/${r.id}`;
            } catch (err) { formError('#cmsg', err); }
          };
        },
      };
    },
  },

  // The engineering workspace for one job.
  calc: {
    async load(id) {
      let c, dep, list;
      try { [c, dep, list] = await Promise.all([api(`/api/calculations/${id}`), api(`/api/calculations/${id}/dependencies`), api('/api/calculations'), loadProjects()]); }
      catch (err) { if (err.status !== 404 && !/not found/i.test(err.message)) throw err; return { missing: true }; }
      let prev = null;
      if (c.supersedes) { try { prev = await api(`/api/calculations/${c.supersedes}`); } catch { prev = null; } }
      return { c, dep, list, prev };
    },
    render(data, id) {
      if (data.missing) return { title: t('calc.notfound.title'), nav: 'workspace', crumbs: [[t('nav.workspace'), '#/workspace'], [t('calc.notfound.crumb')]], html: `<div class="page">${empty('pattern', t('calc.notfound.t', { id }), esc(t('calc.notfound.p')), `<a class="btn pri" href="#/workspace">${esc(t('jobs.all'))}</a>`)}</div>` };
      const { c, dep, list, prev } = data;
      const meta = list.find((x) => x.id === c.id) || null;
      const canEdit = me.role !== 'viewer';
      const fam = c.request.family;
      ctxProject = c.project_id;
      const noResult = !c.patterns.length;
      // same precedence as every list: superseded > no result > server state
      const state = c.superseded_by ? 'SUPERSEDED' : noResult ? 'NO_RESULT' : c.state;
      const blocked = state !== 'REVIEW_REQUIRED' && state !== 'APPROVED'; // nothing of a blocked job may be used
      const canCorrect = canEdit && !blocked && c.correctable_parameters.length > 0;
      if (wsState.id !== c.id) wsState = { id: c.id, part: 0, tab: prev ? 'changes' : 'gates', scope: 'all', gf: 'all', ghost: false };
      if (wsState.part >= c.patterns.length) wsState.part = 0;
      const rp = c.replay;
      const openObl = c.obligations.filter((o) => o.state === 'open').length, openAdv = c.advisories.filter((a) => a.state === 'open').length;
      const graph = dep.edges.length ? depGraph(dep.edges, true) : null;
      const punit = (k) => { const m = paramMeta(fam, k); return m ? unitText(m[3]) : ''; };
      const dirText = (d) => (['downstream', 'lateral', 'common_cause'].includes(d) ? t(`dir.${d}`) : String(d));
      const advText = (s) => (['open', 'acknowledged'].includes(s) ? t(`adv.${s}`) : String(s));

      // history, built only from timestamps and names the server recorded
      const ev = [];
      if (meta) ev.push({ t: meta.created_at, k: noResult ? 'bad' : '', h: esc(t(noResult ? 'ev.none' : 'ev.computed', { id: c.id })), d: h(c.supersedes ? 'ev.computed.corr' : 'ev.computed.d', { fam: famLabel(fam), by: ud(c.created_by), n: c.closure_size, job: c.supersedes ? jobLink(c.supersedes, true) : '' }) });
      if (c.obligations.length) ev.push({ t: Math.min(...c.obligations.map((o) => o.created_at)), k: 'warn', h: esc(t('ev.obl', { n: c.obligations.length })), d: esc(t('ev.obl.d')) });
      c.obligations.filter((o) => o.resolved_by).forEach((o) => ev.push({ t: o.resolved_at, k: o.state === 'waived' ? 'warn' : 'ok', h: esc(t('ev.obl.res', { id: o.id, state: oblText(o.state) })), d: `${mono(o.label)} — ${ud(o.resolved_by)}: ${ud(o.note)}` }));
      c.advisories.filter((a) => a.resolved_by).forEach((a) => ev.push({ t: a.resolved_at, k: 'ok', h: esc(t('ev.adv', { id: a.id })), d: `${ud(a.resolved_by)}: ${ud(a.note)}` }));
      if (rp) ev.push({ t: rp.at, k: rp.matches ? 'ok' : 'bad', h: esc(t(rp.matches ? 'replay.match' : 'replay.mismatch')), d: h(rp.matches ? 'ev.replay.d' : 'ev.replay.d.bad', { by: ud(rp.run_by), n: rp.stored_records_checked, f: rp.stored_records_failing_identity_check }) });
      ev.sort((a, b) => a.t - b.t);
      if (c.superseded_by) ev.push({ k: 'bad', h: esc(t('ev.sup')), d: h('ev.sup.d', { job: jobLink(c.superseded_by, true) }) });

      const tabs = [...(prev ? [['changes', t('tab.changes')]] : []), ['gates', t('tab.gates')], ['checks', t('tab.checks')], ['scrutiny', t('tab.scrutiny'), openObl + openAdv || c.obligations.length + c.advisories.length, openObl + openAdv > 0], ['graph', t('tab.graph')], ['history', t('tab.history')], ['records', t('tab.records')]];
      if (!tabs.some((x) => x[0] === wsState.tab)) wsState.tab = 'gates';

      // ---- gates: the verdict and one line of reason at a glance; the engine's full statement one click away
      const gcount = (r) => c.gates.filter((g) => g.result === r).length;
      const gateItem = (g) => {
        const segs = engineSentences(g.detail), allJa = ja() && segs.length > 0 && segs.every((s) => s.ja);
        const sent = (s) => (allJa ? `<span class="eng"><span class="e-ja">${jaHtml(s.ja)}</span><span class="e-en" lang="en">${esc(s.en)}</span></span>` : ja() ? `<span lang="en">${esc(s.en)}</span>` : esc(s.en));
        const raw = ja() && !allJa ? `${engTag()} ` : '';
        const topics = [...new Set(segs.map((s) => s.topic).filter(Boolean))];
        const why = topics.length > 1 ? esc(t('g.points', { n: topics.length, topics: topics.map((k) => t(`g.topic.${k}`)).join(' · ') })) : segs.length ? `${raw}${sent(segs[0])}` : '';
        const list = (xs) => `<ul>${xs.map((s) => `<li>${sent(s)}</li>`).join('')}</ul>`;
        const body = topics.length > 1 ? [...topics, null].map((k) => [k, segs.filter((s) => (s.topic || null) === k)]).filter(([, xs]) => xs.length).map(([k, xs]) => `<h4>${esc(t(`g.topic.${k || 'other'}`))}</h4>${list(xs)}`).join('') : list(segs);
        const hide = wsState.gf !== 'all' && wsState.gf !== g.result;
        return `<li class="gate r-${esc(g.result)}" data-result="${esc(g.result)}" data-gate="${esc(g.id)}"${hide ? ' hidden' : ''}><details><summary><span class="l1"><span class="id">${esc(g.id)}</span><b>${ja() ? labelled('gate', g.id) : words(g.name)}</b>${resultTag(g.result)}</span><span class="g-why">${why}</span><span class="g-more">${esc(t('g.more', { n: segs.length }))}</span></summary>
          <div class="g-body">${raw}${body}<p class="g-meta">${esc(t('g.name'))} ${mono(g.name)}</p></div></details></li>`;
      };
      const gchip = (r, kind, icon, key) => (r === 'all' || gcount(r) ? `<button class="chip${kind ? ` c-${kind}` : ''}" data-gf="${r}" aria-pressed="${wsState.gf === r}">${icon ? ic(icon, 10) : ''}${esc(r === 'all' ? t('g.all', { n: c.gates.length }) : t(key, { n: gcount(r) }))}</button>` : '');
      const gatesHtml = `<div class="summary-line"><span class="chips" role="group" aria-label="${esc(t('g.filter.aria'))}">${gchip('all')}${gchip('FAIL', 'bad', 'close', 'g.n.fail')}${gchip('INCONCLUSIVE', 'warn', 'warn', 'g.n.inc')}${gchip('NOT_IMPLEMENTED', 'na', 'dash', 'g.n.ni')}${gchip('NOT_RUN', 'na', 'dash', 'g.n.nr')}${gchip('PASS', 'ok', 'check', 'g.n.pass')}</span><span class="grow"></span><button class="ghost mini" id="g-all" data-open="0">${esc(t('g.expand'))}</button></div>
        ${noResult ? note(esc(t('g.noresult')), 'bad') : ''}
        <ul class="il gates" id="gates">${c.gates.map(gateItem).join('')}</ul>
        <p class="hint">${jaHtml(t('g.foot'))}</p>`;

      // ---- checks
      const scopes = [...new Set(c.checks.map((k) => k.scope))], passN = c.checks.filter((k) => k.verdict === 'PASS').length;
      if (wsState.scope !== 'all' && !scopes.includes(wsState.scope)) wsState.scope = 'all';
      const checksHtml = c.checks.length ? `<div class="summary-line"><span>${tag(passN === c.checks.length ? 'ok' : 'bad', t('c.n', { a: passN, b: c.checks.length }), passN === c.checks.length ? 'check' : 'close')}</span>
          <span class="chips" role="group" aria-label="${esc(t('c.scope.aria'))}">${['all', ...scopes].map((s) => `<button class="chip" data-scope="${esc(s)}" aria-pressed="${s === wsState.scope}">${s === 'all' ? esc(t('c.all')) : partName(s)}</button>`).join('')}</span></div>
        <ul class="il checks" id="checks">${c.checks.map((k) => `<li data-scope="${esc(k.scope)}"${wsState.scope !== 'all' && wsState.scope !== k.scope ? ' hidden' : ''}><div class="l1"><span class="id">${partName(k.scope)}</span><b>${ja() ? `${labelled('check', k.name)}<span class="mono idk">${esc(k.name)}</span>` : words(k.name)}</b>${resultTag(k.verdict)}</div><p class="sub">${h('c.method', { method: eng(k.method), cls: mono(k.independence) })}</p>
          <dl class="quad"><div><dt>${esc(t('c.primary'))}</dt><dd>${num(k.primary)}</dd></div><div><dt>${esc(t('c.reference'))}</dt><dd>${num(k.reference)}</dd></div><div><dt>${esc(t('c.difference'))}</dt><dd>${num(k.difference)}</dd></div><div><dt>${esc(t('c.tolerance'))}</dt><dd>${num(k.tolerance)}</dd></div></dl></li>`).join('')}</ul>
        <p class="hint">${esc(t('c.foot'))}</p>`
        : empty('shield', t('c.none.t'), esc(t('c.none.p')));

      // ---- scrutiny: advisories from recorded lessons, obligations from a correction
      const scrutinyHtml = (c.advisories.length ? `<h3>${esc(t('s.known.h'))}</h3><p class="hint">${esc(t('s.known.p'))}</p>
          <ul class="il">${c.advisories.map((a) => `<li><div class="l1"><span class="id">A${a.id}</span><b>${esc(t('s.adv'))}</b>${tag(a.state === 'open' ? 'warn' : 'na', advText(a.state))}</div><p>${eng(a.message)}</p>${a.note ? `<p class="sub">${ud(a.resolved_by)} · ${when(a.resolved_at)}: ${ud(a.note)}</p>` : ''}
            ${a.state === 'open' && canEdit && !blocked ? `<div class="act"><label class="vh" for="adv${a.id}">${esc(t('s.adv.aria', { id: a.id }))}</label><input id="adv${a.id}" data-advreason="${a.id}" placeholder="${esc(t('s.adv.ph'))}"><div class="row"><button data-ack="${a.id}">${esc(t('s.adv.ack'))}</button></div></div>` : ''}</li>`).join('')}</ul>` : '')
        + (c.obligations.length ? `<h3${c.advisories.length ? ' class="gap"' : ''}>${esc(t('s.obl.h'))}</h3><p class="hint">${h('s.obl.p', { rule: mono(dep.rule_table) })}</p>
          <ul class="il" id="obls">${c.obligations.slice().sort((a, b) => (a.state === 'open' ? 0 : 1) - (b.state === 'open' ? 0 : 1) || a.id - b.id).map((o) => `<li data-obl="${o.id}"><div class="l1"><span class="id">#${o.id}</span><b>${oblSubject(o)}</b>${oblTag(o.state)}</div>
            <p>${eng(o.required_check)}</p><p class="sub">${h('s.via', { dir: esc(dirText(o.direction)), rel: mono(o.via) })}</p>
            ${o.state !== 'open' ? `<p class="sub">${o.resolved_by ? h('s.resolved', { note: ud(o.note), by: ud(o.resolved_by), when: when(o.resolved_at) }) : eng(o.note)}</p>` : ''}
            ${o.state === 'open' && canEdit ? `<div class="act"><label class="vh" for="reason${o.id}">${esc(t('s.obl.aria', { id: o.id }))}</label><input id="reason${o.id}" data-reason="${o.id}" placeholder="${esc(t('s.obl.ph'))}"><div class="row"><button data-resolve="${o.id}" data-decision="confirmed">${esc(t('s.confirm'))}</button><button class="ghost" data-resolve="${o.id}" data-decision="waived">${esc(t('s.waive'))}</button></div></div>` : ''}</li>`).join('')}</ul>` : '')
        || empty('shield', t('s.none.t'), esc(t('s.none.p')));
      const graphHtml = graph ? `${graph.html}<p class="hint"><a href="#/amoeba/dependencies/${c.id}">${esc(t('gr.open'))}</a></p>${edgeTable(dep.edges)}` : empty('nodes', t('gr.none.t'), esc(t('gr.none.p')));
      const historyHtml = `<ol class="tl">${ev.map((e) => `<li class="k-${e.k || ''}"><b>${e.h}</b><time>${e.t ? when(e.t) : ''}</time>${e.d}</li>`).join('')}</ol><p class="hint">${esc(t('history.foot'))}</p>`;
      const subject = (r) => { const p = r.payload || {}; const s = p.name !== undefined ? `${p.name}${p.value !== undefined ? ` = ${p.value && typeof p.value === 'object' ? Object.entries(p.value).map(([k, x]) => `${k} ${x}`).join(' · ') : typeof p.value === 'number' ? fmt(p.value) : p.value}${p.unit ? ` ${p.unit}` : ''}` : ''}` : p.part !== undefined ? p.part : Array.isArray(p.between) ? p.between.join(' × ') : p.outcome !== undefined ? p.message || p.outcome : ''; return s ? ` · <span class="mono"${ja() ? ' lang="en"' : ''}>${esc(s)}</span>` : ''; };
      const recKind = (r) => (ja() ? `${labelled('rkind', r.kind)} ${mono(r.kind)}` : mono(r.kind));
      const recordsHtml = `<dl class="kv"><dt>${esc(t('r.closure'))}</dt><dd class="txt"><span class="hash">${esc(c.closure_hash)}</span></dd><dt>${esc(t('r.n'))}</dt><dd>${c.closure_size}</dd><dt>${esc(t('r.wu'))}</dt><dd>${c.work_units}</dd><dt>${esc(t('r.engine'))}</dt><dd class="txt">${mono(c.engine)}</dd>
          <dt>${esc(t('r.replay'))}</dt><dd class="txt" id="replay-state">${rp ? `${replayTag(rp.matches)} ${when(rp.at)} · ${ud(rp.run_by)}` : esc(t('replay.none'))}</dd></dl>
        ${rp ? `<p class="hint">${esc(t('r.checked', { n: rp.stored_records_checked, f: rp.stored_records_failing_identity_check }))} ${eng(rp.limitation)}</p>` : `<p class="hint">${esc(t('r.replay.hint'))}</p>`}
        <h3 class="gap">${esc(t('r.h', { n: c.records.length }))}</h3>
        ${c.records.map((r, i) => `<details class="sec rec" data-rec="${i}"><summary><span class="rec-t">${recKind(r)}${subject(r)}</span><span class="hash">${esc(r.id.slice(0, 8))}…</span></summary><div class="sec-b"></div></details>`).join('')}`;

      const reqKeys = [...new Set([...(F[fam] ? F[fam].f.map((x) => x[0]) : []), ...Object.keys(c.request)])].filter((k) => k !== 'family' && k in c.request);
      const reqValue = (k, v, m) => (typeof v === 'number' ? (m ? `<span class="q"><span class="qv">${fmt(v)}</span> <span class="qu">${esc(unitText(m[3]))}</span></span>` : fmt(v)) : (k === 'surface_reference' || k === 'header_surface_reference' || k === 'branch_surface_reference') ? named('sref', v) : k === 'joint' ? `<span class="nw">${named('joint', v)}</span>` : esc(v));
      // the engine names the refused parameter in its outcome ("out of range (joint): ..."); that row is marked
      const refusalJob = noResult && c.refusal ? c.refusal : null; const refKeys = refusalJob ? refusalJob.fields || [] : [];
      // the interface record of a header with branch: what the result itself says about the joint
      const ifc = c.records.find((r) => r.kind === 'interface'), jp = ifc ? ifc.payload : null;
      const jointHtml = jp ? `<p class="txt">${eng(jp.joint)}</p><dl class="kv"><dt title="joint">${esc(plabel(fam, 'joint'))}</dt><dd class="txt">${jp.declared === 'set-on' ? named('joint', 'set_on') : esc(t('joint.none.v'))}</dd><dt title="header_surface_reference">${esc(plabel(fam, 'header_surface_reference'))}</dt><dd class="txt">${named('sref', jp.header_surface)}</dd><dt title="branch_surface_reference">${esc(plabel(fam, 'branch_surface_reference'))}</dt><dd class="txt">${named('sref', jp.branch_surface)}</dd></dl>` : '';
      const wm = t(state === 'SUPERSEDED' ? 'wm.sup' : state === 'REJECTED' ? 'wm.rej' : 'wm.review');
      const banner = state === 'SUPERSEDED' ? h('ban.sup', { job: jobLink(c.superseded_by, true) }) : state === 'NO_RESULT' ? h('ban.none', { why: c.refusal ? refusalHtml(c.refusal) : c.outcome ? eng(c.outcome) : esc(t('ban.none.default')) }) : state === 'REJECTED' ? h('ban.rej') : h('ban.review');
      // one primary action at most: the successor when superseded, else the correction, else the new calculation from this job
      const primary = c.superseded_by ? 'successor' : canCorrect ? 'corr' : canEdit ? 'clone' : '';
      const jobActions = [c.superseded_by ? `<a class="btn${primary === 'successor' ? ' pri' : ''}" id="successor" href="#/calc/${c.superseded_by}">${esc(t('job.successor', { id: c.superseded_by }))}</a>` : '', canCorrect ? `<button id="corr-open"${primary === 'corr' ? ' class="pri"' : ''}>${esc(t('job.correct'))}</button>` : '', canEdit && !blocked ? `<button id="replay" data-tip="${esc(t('job.replay.tip'))}" data-tip-pos="end">${esc(t('job.replay'))}</button>` : '', canEdit ? `<a class="btn${primary === 'clone' ? ' pri' : ''}" href="#/math/${c.project_id}/${c.id}" data-tip="${esc(t('job.clone.tip'))}" data-tip-pos="end">${esc(t('job.clone'))}</a>` : ''].join('');
      const html = `<div class="ws">
        <div class="ws-head"><div class="ws-title"><h1>${esc(t('job.n', { id: c.id }))} ${stateTag(state)}${c.supersedes ? ` <a class="tag t-info" href="#/calc/${c.supersedes}" id="corrects">${esc(t('job.corrects', { id: c.supersedes }))}</a>` : ''}</h1>
          <div class="ws-meta"><span>${famLabel(fam)}${ja() ? ` ${mono(fam)}` : ''}</span><span>${plink(c.project_id)}</span><span>${h('job.enteredby', { by: ud(c.created_by) })}</span>${meta ? `<span>${when(meta.created_at)}</span>` : ''}</div></div>
          <div class="ws-actions">${jobActions}</div></div>
        <div class="ws-banner${blocked ? ' bad' : ''}${state === 'SUPERSEDED' ? ' neutral' : ''}" role="status"><span>${banner}</span></div>
        <div class="ws-body">
          <aside class="pane" aria-label="${esc(t('pane.props.aria'))}"><div class="pane-h"><h2>${esc(t('pane.props.h'))}</h2></div><div class="pane-scroll">
            <details class="sec" open><summary>${esc(t('req.h'))} ${tag('na', t('req.tag'))}</summary><div class="sec-b"><dl class="kv req">${reqKeys.map((k) => { const v = c.request[k], m = paramMeta(fam, k), ok = canCorrect && c.correctable_parameters.some((x) => x[0] === k); return `<dt>${m ? esc(m[1]) : labelled('param', k)}<small class="mono">${esc(k)}</small></dt><dd${typeof v === 'number' ? '' : ' class="txt"'}>${reqValue(k, v, m)}${refKeys.includes(k) ? ` ${tag('bad', t('req.refused'))}<div class="field-err">${refusalHtml(refusalJob)}</div>` : ''}${ok ? `<button class="ghost mini" data-correct="${esc(k)}" aria-label="${esc(t('req.correct.aria', { label: m ? m[1] : k }))}">${esc(t('req.correct'))}</button>` : ''}</dd>`; }).join('')}</dl>
              <p class="hint">${esc(t('req.hint'))}</p></div></details>
            ${c.patterns.length ? `<details class="sec" open><summary>${esc(t('sel.h'))}</summary><div class="sec-b"><div id="sel" aria-live="polite"></div><div id="meas" aria-live="polite"></div></div></details>
            <details class="sec" open><summary>${esc(t('curves.h'))} <span class="sub" id="curves-part"></span></summary><div class="sec-b flush" id="curves"></div></details>
            <details class="sec" open><summary>${esc(t('quant.h'))}</summary><div class="sec-b" id="quant"></div></details>
            ${ifc ? `<details class="sec" open id="joint"><summary>${esc(t('joint.h'))}</summary><div class="sec-b">${jointHtml}</div></details>` : ''}
            <details class="sec"><summary>${esc(t('assume.h'))}</summary><div class="sec-b" id="assume"></div></details>` : ''}
          </div></aside>
          <div class="split" role="separator" aria-orientation="vertical" aria-label="${esc(t('split.props'))}" tabindex="0" data-split="l"></div>
          <section class="stage${blocked ? ' dead' : ''}" aria-label="${esc(t('stage.aria'))}">
            ${c.patterns.length ? `<div class="stage-bar">
              <div class="parts" role="tablist" aria-label="${esc(t('parts.aria'))}" id="parts">${c.patterns.map((p, i) => `<button role="tab" data-part="${i}" aria-selected="${i === wsState.part}" tabindex="${i === wsState.part ? 0 : -1}">${partName(p.part)} <small class="mono">${esc(p.class)}</small></button>`).join('')}</div>
              <span class="grow"></span>
              <div class="seg" role="group" aria-label="${esc(t('tool.aria'))}"><button class="ib" id="t-select" aria-pressed="true" aria-label="${esc(t('tool.select'))}" data-tip="${esc(t('tool.select.tip'))}">${ic('pointer')}</button><button class="ib" id="t-measure" aria-pressed="false" aria-label="${esc(t('tool.measure'))}" data-tip="${esc(t('tool.measure.tip'))}">${ic('ruler')}</button></div>
              <div class="seg" role="group" aria-label="${esc(t('view.aria'))}"><button class="ib" id="z-out" aria-label="${esc(t('zoom.out'))}" data-tip="${esc(t('zoom.out.tip'))}">${ic('minus')}</button><span class="zoomv" id="zoomv" aria-live="off">100%</span><button class="ib" id="z-in" aria-label="${esc(t('zoom.in'))}" data-tip="${esc(t('zoom.in.tip'))}">${ic('plus')}</button><button class="ib" id="z-fit" aria-label="${esc(t('zoom.fit'))}" data-tip="${esc(t('zoom.fit.tip'))}">${ic('fit')}</button></div>
              <div class="seg" role="group" aria-label="${esc(t('disp.aria'))}"><button class="ib" id="v-grid" aria-pressed="true" aria-label="${esc(t('disp.grid'))}" data-tip="${esc(t('disp.grid'))}">${ic('grid')}</button><button class="ib" id="v-dims" aria-pressed="true" aria-label="${esc(t('disp.dims'))}" data-tip="${esc(t('disp.dims.tip'))}">${ic('dims')}</button><button class="ib" id="v-names" aria-pressed="true" aria-label="${esc(t('disp.labels'))}" data-tip="${esc(t('disp.labels.tip'))}" data-tip-pos="end">${ic('names')}</button>${prev && prev.patterns.length ? `<button class="ib" id="v-prev" aria-pressed="false" aria-label="${esc(t('disp.prev', { id: prev.id }))}" data-tip="${esc(t('disp.prev.tip', { id: prev.id }))}" data-tip-pos="end">${ic('layers')}</button>` : ''}</div>
              <div class="seg" id="dxfseg"></div></div>
            <div class="pv-host" id="pv"><span class="stage-wm${state === 'SUPERSEDED' ? ' grey' : blocked ? ' bad' : ''}" id="wm">${esc(wm)}</span><div class="tblock" id="tblock"></div></div>
            <div class="stage-notes" id="cnotes" role="note" aria-label="${esc(t('cn.aria'))}"></div>
            <div class="readout" aria-hidden="true"><span><b>X</b><span id="ro-x">—</span></span><span><b>Y</b><span id="ro-y">—</span></span><span class="ro-info" id="ro-info"></span></div>`
            : `<div class="pv-host">${empty('warn', t('nopat.t'), c.refusal ? refusalHtml(c.refusal) : c.outcome ? eng(c.outcome) : esc(t('nopat.default')), canEdit ? `<a class="btn pri" href="#/math/${c.project_id}/${c.id}">${esc(t('nopat.go'))}</a>` : '', 'err')}</div>`}
          </section>
          <div class="split" role="separator" aria-orientation="vertical" aria-label="${esc(t('split.results'))}" tabindex="0" data-split="r"></div>
          <aside class="pane" aria-label="${esc(t('pane.results.aria'))}">
            ${tabsHtml('ws', t('tabs.aria'), tabs, wsState.tab)}
            <div class="pane-scroll">${prev ? tabPanel('ws', 'changes', wsState.tab, diffHtml(c, prev), 'tp') : ''}${tabPanel('ws', 'gates', wsState.tab, gatesHtml, 'tp')}${tabPanel('ws', 'checks', wsState.tab, checksHtml, 'tp')}${tabPanel('ws', 'scrutiny', wsState.tab, scrutinyHtml, 'tp')}${tabPanel('ws', 'graph', wsState.tab, graphHtml, 'tp')}${tabPanel('ws', 'history', wsState.tab, historyHtml, 'tp')}${tabPanel('ws', 'records', wsState.tab, recordsHtml, 'tp')}</div>
          </aside>
        </div>
        ${canCorrect ? `<dialog id="corrdlg" aria-labelledby="corr-h"><header><h2 id="corr-h">${esc(t('corr.h', { id: c.id }))}</h2><button class="ib" type="button" id="corr-x" aria-label="${esc(t('close'))}">${ic('close')}</button></header>
          <form id="corr" novalidate><div class="dlg-b one">
            <p class="hint lead">${esc(t('corr.p'))}</p>
            <div class="field"><label for="corrp">${esc(t('corr.param'))}</label><select id="corrp" name="p">${c.correctable_parameters.map(([k, v]) => `<option value="${esc(k)}">${esc(t('corr.opt', { label: plabel(fam, k), key: k, v: `${fmt(v)}${punit(k) ? ` ${punit(k)}` : ''}` }))}</option>`).join('')}</select></div>
            <div class="field"><label for="corrv">${esc(t('corr.value'))}</label><input id="corrv" name="v" ${dimAttrs}></div>
            <div class="field"><label for="corrr">${esc(t('corr.reason'))}</label><input id="corrr" name="r" placeholder="${esc(t('corr.reason.ph'))}"></div>
            <div id="corrmsg"></div></div>
            <div class="dlg-f row"><span class="grow"></span><button type="button" id="corr-cancel">${esc(t('cancel'))}</button><button class="pri" id="corr-go">${esc(t('corr.go'))}</button></div></form></dialog>` : ''}</div>`;

      const wire = (root) => {
        wireSplits(root);
        wireTabs(root, 'ws', (k) => { wsState.tab = k; });
        if (graph) graph.wire($('#tp-ws-graph', root));
        $$('.rec', root).forEach((d) => d.addEventListener('toggle', () => { const b = $('.sec-b', d); if (d.open && !b.firstChild) { const r = c.records[d.dataset.rec]; b.innerHTML = `<p class="sub">${named('rfamily', r.family)} · ${named('rkind', r.kind)}</p><p class="hash">${esc(r.id)}</p>${r.relations.length ? `<p class="hint">${r.relations.map((x) => `${mono(x.rel)} → <span class="mono">${esc(x.to.slice(0, 10))}…</span>`).join(' · ')}</p>` : ''}<pre class="mono payload" lang="en">${esc(JSON.stringify(r.payload, null, 1))}</pre>`; } }));
        $$('#tp-ws-checks .chip', root).forEach((b) => { b.onclick = () => { wsState.scope = b.dataset.scope; $$('#tp-ws-checks .chip', root).forEach((x) => x.setAttribute('aria-pressed', x === b)); $$('#checks > li', root).forEach((li) => { li.hidden = b.dataset.scope !== 'all' && li.dataset.scope !== b.dataset.scope; }); }; });
        $$('#tp-ws-gates .chip', root).forEach((b) => { b.onclick = () => { wsState.gf = b.dataset.gf; $$('#tp-ws-gates .chip', root).forEach((x) => x.setAttribute('aria-pressed', x === b)); $$('#gates > li', root).forEach((li) => { li.hidden = b.dataset.gf !== 'all' && li.dataset.result !== b.dataset.gf; }); }; });
        const gall = $('#g-all', root); if (gall) gall.onclick = () => { const open = gall.dataset.open !== '1'; $$('#gates details', root).forEach((d) => { d.open = open; }); gall.dataset.open = open ? '1' : '0'; gall.textContent = t(open ? 'g.collapse' : 'g.expand'); };

        const rb = $('#replay'); if (rb) rb.onclick = async () => { try { const r = await withBusy(rb, t('replay.busy'), () => api(`/api/calculations/${id}/replay`, { method: 'POST' })); const bad = r && r.matches === false; toast(() => esc(t(bad ? 'replay.t.bad' : 'replay.t.ok', { id })), bad ? 'err' : 'ok'); wsState.tab = 'records'; refresh(); } catch (err) { toastErr(err); } };
        const dlg = $('#corrdlg'), f = $('#corr');
        if (dlg) {
          const open = (k) => { if (k) f.p.value = k; clearFieldErrors(f); $('#corrmsg').innerHTML = ''; dlg.showModal(); f.v.focus(); };
          $('#corr-open').onclick = () => open(); $('#corr-x').onclick = $('#corr-cancel').onclick = () => dlg.close();
          $$('[data-correct]', root).forEach((b) => { b.onclick = () => open(b.dataset.correct); });
          f.onsubmit = async (e) => {
            e.preventDefault(); clearFieldErrors(f);
            const k = f.p.value, nv = parseDim(f.v.value), now = c.request[k];
            if (f.v.value.trim() === '' || !Number.isFinite(nv)) { fieldError(f.v, t('corr.e.num')); return; }
            if (nv === now) { fieldError(f.v, t('corr.e.same', { v: fmt(now) })); return; }
            const trial = validateRequest(fam, { ...c.request, [k]: nv });
            if (trial.errors.length) { fieldError(f.v, trial.errors[0][1]); return; }
            if (f.r.value.trim().length < 5) { fieldError(f.r, t('corr.e.reason')); return; }
            try {
              const r = await withBusy($('#corr-go'), t('corr.busy'), () => api(`/api/calculations/${id}/corrections`, { method: 'POST', json: { parameter: k, new_value: nv, reason: f.r.value } }));
              toast(() => esc(t('corr.toast', { label: plabel(fam, k), a: fmt(now), b: fmt(nv), id: r.new_calculation, n: r.obligations, open: r.open })), 'warn');
              dlg.close(); wsState = { id: r.new_calculation, part: 0, tab: 'changes', scope: 'all', gf: 'all', ghost: false }; location.hash = `#/calc/${r.new_calculation}`;
            } catch (err) { formError('#corrmsg', err); }
          };
        }
        $$('[data-ack]', root).forEach((btn) => { btn.onclick = async () => {
          const input = $(`[data-advreason="${btn.dataset.ack}"]`, root);
          if (input.value.trim().length < 5) { fieldError(input, t('s.e.adv')); return; }
          try { await withBusy(btn, null, () => api(`/api/advisories/${btn.dataset.ack}/acknowledge`, { method: 'POST', json: { reason: input.value.trim() } })); toast(() => esc(t('s.t.adv', { id: btn.dataset.ack })), 'ok'); refresh(); } catch (err) { toastErr(err); }
        }; });
        $$('[data-resolve]', root).forEach((btn) => { btn.onclick = async () => {
          const input = $(`[data-reason="${btn.dataset.resolve}"]`, root), oid = btn.dataset.resolve, dec = btn.dataset.decision;
          if (input.value.trim().length < 5) { fieldError(input, t('s.e.obl')); return; }
          try { await withBusy(btn, null, () => api(`/api/obligations/${oid}/resolve`, { method: 'POST', json: { decision: dec, reason: input.value.trim() } })); toast(() => esc(t('s.t.obl', { id: oid, decision: t(`dec.${dec}`) })), 'ok'); refresh(); } catch (err) { toastErr(err); }
        }; });

        // ---- pattern viewer and the panels that follow the active part
        if (!c.patterns.length) return;
        const xy = (q) => `${n3(q[0])}, ${n3(q[1])}`;
        const kindText = (k) => idText('entity', k);
        const point = (q) => (q.free ? esc(t('pv.free')) : h('pv.on', { curve: mono(q.curve), i: q.vi }));
        const selBox = $('#sel'), measBox = $('#meas');
        const showSel = (i, k) => {
          $$('#curves tr[data-ci]').forEach((tr) => tr.setAttribute('aria-selected', Number(tr.dataset.ci) === i));
          if (i === null) { selBox.innerHTML = `<p class="hint">${esc(t('sel.none'))}</p>`; return; }
          const p = k.points, xs = p.map((q) => q[0]), ys = p.map((q) => q[1]), a = p[0], b = p[p.length - 1], gloss = idLabel('curve', k.name);
          selBox.innerHTML = `<dl class="kv"><dt>${esc(t('sel.curve'))}</dt><dd class="txt">${mono(k.name)}${gloss ? `<small>${esc(gloss)}</small>` : ''}</dd><dt>${esc(t('sel.entity'))}</dt><dd class="txt">${esc(kindText(k.kind))}</dd><dt>${esc(t('sel.layer'))}</dt><dd class="txt"><span class="sw${k.layer === 'CUT_INNER' ? ' inner' : ''}"></span>${mono(k.layer)}</dd><dt>${esc(t('sel.length'))}</dt><dd>${num(k.length, 'mm')}</dd>
            <dt>${esc(t('sel.vertices'))}</dt><dd>${p.length}</dd><dt>${esc(t('sel.first'))}</dt><dd>${xy(a)} <span class="sub">mm</span></dd><dt>${esc(t('sel.last'))}</dt><dd>${xy(b)} <span class="sub">mm</span></dd><dt>${esc(t('sel.extent'))}</dt><dd>${n3(Math.max(...xs) - Math.min(...xs))} × ${n3(Math.max(...ys) - Math.min(...ys))} <span class="sub">mm</span></dd></dl>
            <p class="hint">${esc(t('sel.hint'))}${k.points_exact === false ? ` ${esc(t('sel.arc', { n: p.length - 1, d: n3(k.points_max_deviation || 0) }))}` : ''}</p>`;
        };
        viewer = new PatternViewer($('#pv'), {
          onSelect: showSel,
          onTool: (tool) => { $('#t-select').setAttribute('aria-pressed', tool === 'select'); $('#t-measure').setAttribute('aria-pressed', tool === 'measure'); },
          onView: (z) => { $('#zoomv').textContent = `${Math.round(z * 100)}%`; },
          onReadout: (r) => {
            $('#ro-x').textContent = r.cursor ? `${n3(r.cursor.x)} mm` : '—'; $('#ro-y').textContent = r.cursor ? `${n3(r.cursor.y)} mm` : '—';
            let text; const m = r.measure;
            if (m) text = m.b ? t(m.done ? 'ro.dist' : 'ro.dist.live', { d: n3(m.distance), dx: n3(m.dx), dy: n3(m.dy) }) : t('ro.measure2');
            else if (r.hover) { const hv = r.hover, base = { name: hv.curve.name, kind: kindText(hv.curve.kind), layer: hv.curve.layer, len: n3(hv.curve.length) }; text = hv.vertex ? t('ro.hover.v', { ...base, i: hv.vertex.vi + 1, n: hv.curve.points.length, x: n3(hv.vertex.x), y: n3(hv.vertex.y) }) : t('ro.hover', base); }
            else text = t(r.tool === 'measure' ? 'ro.measure1' : 'ro.idle');
            $('#ro-info').textContent = text;
            measBox.innerHTML = m && m.b && m.done ? `<h3 class="gap">${esc(t('meas.h'))}</h3><dl class="kv"><dt>${esc(t('meas.dist'))}</dt><dd>${n3(m.distance)} <span class="sub">mm</span></dd><dt>${esc(t('meas.dxy'))}</dt><dd>${n3(m.dx)} · ${n3(m.dy)} <span class="sub">mm</span></dd><dt>${esc(t('meas.from'))}</dt><dd class="txt">${point(m.a)} (${xy([m.a.x, m.a.y])})</dd><dt>${esc(t('meas.to'))}</dt><dd class="txt">${point(m.b)} (${xy([m.b.x, m.b.y])})</dd></dl><p class="hint">${esc(t('meas.hint'))}</p>` : '';
          },
        }, { t, fmt: n3 });
        const showPart = (i) => {
          wsState.part = i; const p = c.patterns[i]; const all = [...p.curves, ...p.holes];
          $$('#parts [role=tab]').forEach((b, j) => { b.setAttribute('aria-selected', j === i); b.tabIndex = j === i ? 0 : -1; });
          $('#curves-part').innerHTML = h('cv.part', { part: partName(p.part) });
          $('#curves').innerHTML = `<div class="tbl"><table class="curves"><thead><tr><th>${esc(t('cv.curve'))}</th><th>${esc(t('cv.entity'))}</th><th class="num">${esc(t('cv.length'))}</th></tr></thead><tbody>${all.map((k, j) => { const gloss = idLabel('curve', k.name); return `<tr data-ci="${j}" aria-selected="false"><td><span class="sw${k.layer === 'CUT_INNER' ? ' inner' : ''}"></span><button class="mono" data-ci="${j}" aria-label="${esc(t('cv.select', { name: k.name }))}">${esc(k.name)}</button>${gloss ? `<small>${esc(gloss)}</small>` : ''}</td><td>${esc(kindText(k.kind))}</td><td class="num">${num(k.length)}</td></tr>`; }).join('')}</tbody></table></div>
            <p class="hint legend"><span><span class="sw"></span>${mono('CUT_OUTER')} ${esc(t('cv.outer'))}</span>${p.holes.length ? `<span><span class="sw inner"></span>${mono('CUT_INNER')} ${esc(t('cv.inner'))}</span>` : ''}</p>`;
          const e = extent(p), cls = idLabel('devclass', p.class);
          // where the opening sits across the plate: the only place the sign of the axis offset shows in the pattern
          const hole = p.holes.length === 1 ? extent({ curves: [], holes: p.holes }) : null;
          $('#quant').innerHTML = `<dl class="kv"><dt>${esc(t('q.part'))}</dt><dd class="txt">${partName(p.part)}<small>${named('family', p.family)}</small></dd><dt>${esc(t('q.class'))}</dt><dd class="txt">${mono(p.class)} <span class="sub">${esc(t('q.class.note'))}</span>${cls ? `<small>${esc(cls)}</small>` : ''}</dd>
            <dt class="wide">${esc(t('q.mside'))}<small>${esc(t('q.mside.note'))}</small></dt><dd class="txt wide" id="mside">${p.marking_side ? h('q.mside.v', { side: esc(sideText(p.marking_side)) }) : esc(t('q.mside.none'))}</dd>
            <dt>${esc(t('q.extent'))}<small>${esc(t('q.extent.note'))}</small></dt><dd><span class="q"><span class="qv">${n3(e.w)} × ${n3(e.h)}</span> <span class="qu">mm</span></span></dd>${Object.entries(p.quantities).map(([k, v]) => { const [l, val] = quantity(k, v); return `<dt>${l}</dt><dd>${val}</dd>`; }).join('')}
            ${hole ? `<dt>${esc(t('q.opening'))}<small>${esc(t('q.opening.note'))}</small></dt><dd>${signed((hole.y0 + hole.y1) / 2 - (e.y0 + e.y1) / 2, 'mm')}</dd>` : ''}
            <dt>${esc(t('q.epsnum'))}<small>${esc(t('q.eps.note'))}</small></dt><dd>${num(p.eps_num, 'mm')}</dd><dt>${esc(t('q.epsrep'))}<small>${esc(t('q.eps.note'))}</small></dt><dd>${num(p.eps_rep, 'mm')}</dd><dt>${esc(t('q.sha'))}</dt><dd class="txt">${short(p.dxf_sha256)}</dd></dl>
            <p class="hint">${esc(t('q.hint'))}</p>`;
          $('#assume').innerHTML = `<ul>${p.assumptions.map((a) => `<li>${eng(a)}</li>`).join('')}</ul>`;
          const seams = all.filter(isSeam).map((k) => k.name);
          $('#cnotes').innerHTML = `<span class="cn" id="pv-side"><span>${p.marking_side ? h('cn.side', { side: esc(sideText(p.marking_side)) }) : esc(t('cn.side.none'))}</span></span><span class="cn"><span>${esc(t('cn.not'))}</span></span>${seams.length ? `<span class="cn" title="${esc(t('cn.seams.tip'))}"><span>${h('cn.seams', { names: seams.map(mono).join(', ') })}</span></span>` : ''}`;
          $('#dxfseg').innerHTML = blocked ? tag('na', t('dxf.withheld')) : `<a class="btn" id="dxf" href="/api/calculations/${c.id}/dxf/${encodeURIComponent(p.part)}" download aria-label="${esc(t('dxf.aria', { part: partText(p.part) }))}" data-tip="${esc(t('dxf.tip', { part: partText(p.part) }))}" data-tip-pos="end">${ic('download')}${esc(t('dxf.btn'))}</a>${c.patterns.length > 1 ? `<button id="dxf-all" aria-label="${esc(t('dxf.all.aria', { n: c.patterns.length }))}" data-tip="${esc(t('dxf.all.tip'))}" data-tip-pos="end">${esc(t('dxf.all', { n: c.patterns.length }))}</button>` : ''}`;
          const all2 = $('#dxf-all'); if (all2) all2.onclick = async () => { for (const q of c.patterns) { const a = document.createElement('a'); a.href = `/api/calculations/${c.id}/dxf/${encodeURIComponent(q.part)}`; a.download = ''; document.body.append(a); a.click(); a.remove(); await new Promise((res) => setTimeout(res, 350)); } toast(() => esc(t('dxf.all.toast', { n: c.patterns.length })), 'warn'); };
          $('#tblock').innerHTML = [[t('tb.part'), partName(p.part)], [t('tb.job'), `<span class="mono">#${c.id}</span>`], [t('tb.engine'), `<span class="mono">${esc(c.engine)}</span>`], [t('tb.side'), p.marking_side ? esc(sideText(p.marking_side)) : esc(t('tb.side.none'))], [t('tb.status'), `<span class="tag t-warn">${esc(t('tb.nr'))}</span>${stateTag(state)}`]].map(([k, v]) => `<span class="tb"><span class="k">${esc(k)}</span><span class="v">${v}</span></span>`).join('');
          viewer.setPattern(p); showSel(null);
          viewer.setGhost(wsState.ghost && prev ? prev.patterns.find((x) => x.part === p.part) || null : null);
        };
        $('#parts').onclick = (e) => { const b = e.target.closest('[data-part]'); if (b) showPart(Number(b.dataset.part)); };
        $('#parts').onkeydown = (e) => { const n = c.patterns.length, i = wsState.part; const j = e.key === 'ArrowRight' ? (i + 1) % n : e.key === 'ArrowLeft' ? (i - 1 + n) % n : -1; if (j >= 0) { e.preventDefault(); showPart(j); $$('#parts [role=tab]')[j].focus(); } };
        $('#curves').onclick = (e) => { const b = e.target.closest('button[data-ci]'); if (b) viewer.select(Number(b.dataset.ci) === viewer.sel ? null : Number(b.dataset.ci)); };
        $('#t-select').onclick = () => viewer.setTool('select'); $('#t-measure').onclick = () => viewer.setTool(viewer.tool === 'measure' ? 'select' : 'measure');
        $('#z-in').onclick = () => viewer.zoomBy(1.25); $('#z-out').onclick = () => viewer.zoomBy(0.8); $('#z-fit').onclick = () => viewer.fit();
        const toggle = (sel, what) => { $(sel).onclick = (e) => e.currentTarget.setAttribute('aria-pressed', viewer.toggle(what)); };
        toggle('#v-grid', 'grid'); toggle('#v-dims', 'dims'); toggle('#v-names', 'labels');
        const vp = $('#v-prev'); if (vp) { vp.setAttribute('aria-pressed', wsState.ghost); vp.onclick = () => { wsState.ghost = !wsState.ghost; vp.setAttribute('aria-pressed', wsState.ghost); viewer.setGhost(wsState.ghost ? prev.patterns.find((x) => x.part === c.patterns[wsState.part].part) || null : null); }; }
        viewer.showPart = showPart; viewer.partCount = c.patterns.length;
        // after a language switch the display toggles follow the restored viewer state
        viewer.syncToggles = () => { $('#v-grid').setAttribute('aria-pressed', viewer.showGrid); $('#v-dims').setAttribute('aria-pressed', viewer.showDims); $('#v-names').setAttribute('aria-pressed', viewer.showLabels); };
        showPart(wsState.part);
      };
      return { title: t('job.n', { id: c.id }), nav: 'workspace', ws: true, crumbs: [[t('nav.workspace'), '#/workspace'], [pnameHtml(c.project_id), `#/project/${c.project_id}`, true], [t('job.n', { id: c.id })]], html, wire };
    },
  },

  // One row per DXF file that exists: part, size of the plate, hash, and whether the server will hand it out.
  manufacturing: {
    async load() { const [all] = await Promise.all([api('/api/calculations'), loadProjects()]); await withFacts(all); return { all }; },
    render({ all }) {
      const rows = all.filter((c) => c.facts && !c.facts.noResult).flatMap((c) => c.facts.parts.map((p) => ({ c, p })));
      const refused = all.filter((c) => c.facts && c.facts.noResult).length;
      const dxfCell = ({ c, p }) => { const s = effState(c); return s === 'SUPERSEDED' ? `<span class="sub">${c.facts.superseded_by ? h('mfg.w.sup.by', { job: `<a href="#/calc/${c.facts.superseded_by}">#${c.facts.superseded_by}</a>` }) : esc(t('mfg.w.sup'))}</span>` : s === 'REJECTED' ? `<span class="sub">${esc(t('mfg.w.rej'))}</span>` : `<a class="btn" href="/api/calculations/${c.id}/dxf/${encodeURIComponent(p.part)}" download aria-label="${esc(t('mfg.dl.aria', { id: c.id, part: partText(p.part) }))}">${ic('download')}${esc(t('mfg.dl'))}</a>`; };
      return {
        title: t('nav.manufacturing'), crumbs: [[t('nav.manufacturing')]],
        html: `<div class="page">${ph(esc(t('nav.manufacturing')), esc(t('mfg.sub')))}
          ${note(h('mfg.note'))}
          ${panel(esc(t('mfg.panel')), rows.length ? `<div class="tbl"><table id="tbl-dxf"><thead><tr><th>${esc(t('th.job'))}</th><th>${esc(t('th.project'))}</th><th>${esc(t('th.part'))}</th><th class="num">${esc(t('th.extent'))}</th><th class="num">${esc(t('th.curves'))}</th><th>${esc(t('th.dxfsha'))}</th><th>${esc(t('th.jobstate'))}</th><th>${esc(t('th.file'))}</th></tr></thead><tbody>${rows.map((r) => `<tr${effState(r.c) === 'SUPERSEDED' ? ' class="dim"' : ''}><td class="nw">${jobLink(r.c.id)}</td><td>${plink(r.c.project_id)}</td><td>${partName(r.p.part)} <span class="sub mono">${esc(r.p.cls)}</span><br><span class="sub">${famShort(r.c.family)}</span></td><td class="num">${n3(r.p.w)} × ${n3(r.p.h)}</td><td class="num">${r.p.curves}${r.p.holes ? ` ${esc(t('mfg.opening', { n: r.p.holes }))}` : ''}</td><td>${short(r.p.sha)}</td><td class="nw">${stateTag(effState(r.c))}</td><td class="nw">${dxfCell(r)}</td></tr>`).join('')}</tbody></table></div>`
            : empty('layers', t('mfg.none.t'), esc(t('mfg.none.p'))),
            { flush: true, count: rows.length, tools: rows.length ? filterBox('tbl-dxf', t('mfg.filter')) : '', foot: `${jaHtml(t('mfg.foot'))}${refused ? ` ${esc(t('mfg.foot.refused', { n: refused }))}` : ''}` })}
          ${plannedKeys('mfg.planned', 7)}</div>`,
      };
    },
  },

  // What stands between each job and release: gates, checks, obligations, advisories, replay.
  verification: {
    async load() { const [cs, obs, lessons] = await Promise.all([api('/api/calculations'), api('/api/obligations'), api('/api/lessons'), loadProjects()]); await withFacts(cs); return { cs, obs, lessons }; },
    render({ cs, obs, lessons }) {
      const open = obs.filter((o) => o.state === 'open');
      const openNow = open.filter((o) => !o.target_superseded), openOld = open.filter((o) => o.target_superseded); // BH-002
      const live = cs.filter((c) => effState(c) !== 'SUPERSEDED');
      return {
        title: t('nav.verification'), crumbs: [[t('nav.verification')]],
        html: `<div class="page">${ph(esc(t('nav.verification')), esc(t('ver.sub')))}
          ${panel(esc(t('ver.open')), open.length ? `<div class="tbl"><table><thead><tr><th>#</th><th>${esc(t('th.job'))}</th><th>${esc(t('th.affected'))}</th><th>${esc(t('th.required'))}</th><th>${esc(t('th.raised'))}</th></tr></thead><tbody>${[...openNow, ...openOld].map((o) => `<tr><td>${o.id}</td><td class="nw">${jobLink(o.target_calc)}<br><span class="sub">${esc(t('ver.corrects', { id: o.origin_calc }))}</span>${o.target_superseded ? `<br>${tag('na', t('obl.onsup'))}` : ''}</td><td>${oblSubject(o)}</td><td>${eng(o.required_check)}</td><td class="nw">${when(o.created_at)}</td></tr>`).join('')}</tbody></table></div>` : empty('shield', t('ver.open.none.t'), esc(t('ver.open.none.p'))),
            { id: 'p-open', flush: true, count: openNow.length, foot: h('ver.open.foot', { n: obs.length, ledger: `<a href="#/amoeba/obligations">${esc(t('ver.ledger'))}</a>` }) + (openOld.length ? ` ${esc(t('obl.onsup.note', { n: openOld.length }))}` : '') })}
          ${panel(esc(t('ver.status')), live.length ? `<div class="tbl"><table id="jobs-ver"><thead><tr><th>${esc(t('th.job'))}</th><th>${esc(t('th.project'))}</th><th>${esc(t('th.state.short'))}</th><th>${esc(t('th.gates'))}</th><th class="num">${esc(t('th.checks'))}</th><th class="num">${esc(t('th.oblopen'))}</th><th class="num">${esc(t('th.advopen'))}</th><th>${esc(t('th.replay'))}</th></tr></thead><tbody>${live.map((c) => `<tr><td class="nw">${jobLink(c.id)}<br><span class="sub">${famShort(c.family)}</span></td><td>${plink(c.project_id)}</td><td class="nw">${stateTag(effState(c))}</td><td class="gcell">${c.facts ? gateCounts(c.facts.gates) : '—'}</td><td class="num">${c.facts ? (c.facts.checks[1] ? esc(t('ver.checks', { a: c.facts.checks[0], b: c.facts.checks[1] })) : esc(t('ver.checks.none'))) : '—'}</td><td class="num">${c.open_obligations}</td><td class="num">${c.facts ? c.facts.advOpen : '—'}</td><td class="nw">${replayTag(c.facts ? c.facts.replay : null)}</td></tr>`).join('')}</tbody></table></div>` : noJobs(),
            { flush: true, count: live.length, tools: live.length ? filterBox('jobs-ver', t('jobs.filter')) : '', foot: jaHtml(t('ver.status.foot', { n: cs.length - live.length })) })}
          ${panel(esc(t('ver.lessons')), lessonsTable(lessons), { id: 'p-lessons', flush: true, count: lessons.length, foot: esc(t('ver.lessons.foot')) })}
          ${plannedKeys('ver.planned', 4)}</div>`,
      };
    },
  },

  amoeba: {
    async load(tab, arg) {
      const [a, obs, lessons, cs] = await Promise.all([api('/api/amoeba/summary'), api('/api/obligations'), api('/api/lessons'), api('/api/calculations'), loadProjects()]);
      const withEdges = cs.filter((c) => c.state !== 'REJECTED' || c.superseded);
      const depId = arg || (withEdges[0] && withEdges[0].id);
      let dep = null, depErr = null;
      // BH-011: a job id that is not a whole number is 'not found', as an unknown id is; no request is sent for it
      if (depId && !/^\d+$/.test(String(depId))) depErr = new ApiError('not found', { server: true, status: 404 });
      else if (depId) { try { dep = await api(`/api/calculations/${depId}/dependencies`); } catch (err) { depErr = err; } }
      return { a, obs, lessons, withEdges, depId, dep, depErr };
    },
    render({ a, obs, lessons, withEdges, depId, dep, depErr }, tab) {
      const graph = dep && dep.edges.length ? depGraph(dep.edges) : null;
      const li = (xs) => `<ul>${xs.map((x) => `<li>${eng(x)}</li>`).join('')}</ul>`;
      const tabs = [['records', t('am.t.records'), a.records_by_kind.reduce((s, r) => s + r.count, 0)], ['activities', t('am.t.activities'), a.activities.reduce((s, r) => s + r.distinct_activities, 0)], ['obligations', t('am.t.obligations'), a.obligations.open, a.obligations.open > 0], ['lessons', t('am.t.lessons'), lessons.length], ['dependencies', t('am.t.dependencies')], ['mechanisms', t('am.t.mechanisms')]];
      const active = tabs.some((x) => x[0] === tab) ? tab : 'records';
      const kindCell = (k) => (ja() && idLabel('rkind', k) ? `${esc(idLabel('rkind', k))}<br>${mono(k)}` : mono(k));
      const dirs = (d) => (['downstream', 'lateral', 'common_cause'].includes(d) ? t(`dirs.${d}`) : String(d).replaceAll('_', ' '));
      return {
        title: t('nav.amoeba'), crumbs: [[t('nav.amoeba')]],
        html: `<div class="page">${ph(esc(t('nav.amoeba')), esc(t('am.sub')))}
          ${tabsHtml('am', t('am.tabs.aria'), tabs, active)}
          ${tabPanel('am', 'records', active, `<div class="cols">${panel(esc(t('am.rec.h')), a.records_by_kind.length ? `<div class="tbl"><table id="tbl-rec"><thead><tr><th>${esc(t('th.rfamily'))}</th><th>${esc(t('th.kind'))}</th><th class="num">${esc(t('th.count'))}</th></tr></thead><tbody>${a.records_by_kind.map((r) => `<tr><td>${named('rfamily', r.family)}</td><td>${kindCell(r.kind)}</td><td class="num">${r.count}</td></tr>`).join('')}</tbody></table></div>` : empty('nodes', t('am.rec.none.t'), esc(t('am.rec.none.p'))), { flush: true, tools: a.records_by_kind.length ? filterBox('tbl-rec', t('am.rec.filter')) : '', foot: esc(t('am.rec.foot')) })}
            ${panel(esc(t('am.snap.h')), a.recent_snapshots.length ? `<div class="tbl"><table><thead><tr><th>${esc(t('th.project'))}</th><th>${esc(t('th.snapshot'))}</th></tr></thead><tbody>${a.recent_snapshots.map((s) => `<tr><td class="nw">${plink(s.project_id)}</td><td><span class="hash">${esc(s.snapshot)}</span></td></tr>`).join('')}</tbody></table></div>` : empty('folder', t('am.snap.none.t'), esc(t('am.snap.none.p'))), { flush: true })}</div>`)}
          ${tabPanel('am', 'activities', active, panel(esc(t('am.act.h')), a.activities.length ? `<div class="tbl"><table><thead><tr><th>${esc(t('th.acttype'))}</th><th class="num">${esc(t('th.distinct'))}</th><th class="num">${esc(t('th.wu'))}</th></tr></thead><tbody>${a.activities.map((r) => `<tr><td>${kindCell(r.type)}</td><td class="num">${r.distinct_activities}</td><td class="num">${r.declared_work_units}</td></tr>`).join('')}</tbody></table></div>` : empty('clock', t('am.act.none.t'), esc(t('am.act.none.p'))), { flush: true, foot: esc(t('am.act.foot')) }))}
          ${tabPanel('am', 'obligations', active, panel(esc(t('am.obl.h')), obs.length ? `<div class="tbl"><table id="tbl-obl"><thead><tr><th>#</th><th>${esc(t('th.job'))}</th><th>${esc(t('th.direction'))}</th><th>${esc(t('th.affected'))}</th><th>${esc(t('th.required'))}</th><th>${esc(t('th.state.short'))}</th><th>${esc(t('th.evidence'))}</th></tr></thead><tbody>${obs.map((o) => `<tr><td>${o.id}</td><td class="nw">${jobLink(o.target_calc)}</td><td>${esc(dirs(o.direction))}<br><span class="sub mono">${esc(o.via)}</span></td><td>${oblSubject(o)}</td><td>${eng(o.required_check)}</td><td>${oblTag(o.state)}${o.target_superseded ? ` ${tag('na', t('obl.onsup'))}` : ''}</td><td>${oblNote(o)}${o.resolved_by ? `<br><span class="sub">${ud(o.resolved_by)} · ${when(o.resolved_at)}</span>` : ''}</td></tr>`).join('')}</tbody></table></div>` : empty('shield', t('am.obl.none.t'), esc(t('am.obl.none.p'))),
            { flush: true, count: esc(t('am.obl.count', { a: a.obligations.open, b: a.obligations.total })), tools: obs.length ? filterBox('tbl-obl', t('am.obl.filter')) : '', foot: esc(t('am.obl.foot')) + (a.obligations.open_on_superseded ? ` ${esc(t('obl.onsup.note', { n: a.obligations.open_on_superseded }))}` : '') }))}
          ${tabPanel('am', 'lessons', active, panel(esc(t('am.les.h')), lessonsTable(lessons), { flush: true, count: lessons.length, foot: jaHtml(t('am.les.foot')) }))}
          ${tabPanel('am', 'dependencies', active, panel(esc(t('am.dep.h')), `${withEdges.length ? `<div class="field narrow"><label for="depjob">${esc(t('th.job'))}</label><select id="depjob">${withEdges.map((c) => `<option value="${c.id}"${String(c.id) === String(depId) ? ' selected' : ''}>${esc(t(c.superseded ? 'am.dep.opt.sup' : 'am.dep.opt', { id: c.id, fam: idText('family', c.family) }))}</option>`).join('')}</select></div>` : ''}
            ${graph ? `${graph.html}${edgeTable(dep.edges)}<p class="hint">${h('am.dep.rule', { rule: mono(dep.rule_table) })}</p>` : empty('nodes', t(depErr ? 'am.dep.err' : 'gr.none.t'), depErr ? errHtml(depErr) : esc(t('am.dep.none.p')))}`))}
          ${tabPanel('am', 'mechanisms', active, `<div class="cols2">${panel(esc(t('am.mech.impl')), li(a.implemented), { count: a.implemented.length })}<section class="panel"><header class="panel-h"><h2>${esc(t('am.mech.not'))}</h2>${tag('planned', t('am.mech.tag'), 'planned')}<span class="cnt">${a.not_implemented.length}</span></header><div class="panel-b">${li(a.not_implemented)}</div></section></div>`)}</div>`,
        wire(root) {
          wireTabs(root, 'am', (k) => pushTab(`${location.pathname}${location.search}#/amoeba/${k}${k === 'dependencies' && depId ? `/${depId}` : ''}`));
          if (graph) graph.wire(root);
          const s = $('#depjob'); if (s) s.onchange = () => { location.hash = `#/amoeba/dependencies/${s.value}`; };
        },
      };
    },
  },

  notfound: {
    async load() { return {}; },
    render() {
      return { title: t('nf.title'), nav: '', crumbs: [[t('nf.crumb')]], html: `<div class="page">${empty('search', t('nf.t'), h('nf.p', { hash: mono(location.hash) }), `<a class="btn pri" href="#/dashboard">${esc(t('to.dashboard'))}</a><a class="btn" href="#/workspace">${esc(t('jobs.all'))}</a>`)}</div>` };
    },
  },

  account: {
    async load() { return {}; },
    render() {
      return {
        title: t('nav.account'), crumbs: [[t('nav.account')]],
        html: `<div class="page narrow">${ph(esc(t('nav.account')), h('acc.sub', { user: ud(me.username), role: esc(roleText(me.role)) }))}
          ${panel(esc(t('acc.pw.h')), `<form id="pw" novalidate><div class="fgrid"><div class="field"><label for="pwc">${esc(t('acc.pw.cur'))}</label><input id="pwc" name="c" type="password" autocomplete="current-password"></div>
            <div class="field"><label for="pwn">${esc(t('acc.pw.new'))}</label><input id="pwn" name="n" type="password" autocomplete="new-password"></div>
            <div class="field"><label for="pwn2">${esc(t('acc.pw.rep'))}</label><input id="pwn2" name="n2" type="password" autocomplete="new-password"></div></div>
            <div id="pwmsg"></div><button class="pri">${esc(t('acc.pw.go'))}</button></form>`, { foot: esc(t('acc.pw.foot')) })}
          ${panel(esc(t('acc.pref.h')), `<div class="fgrid"><div class="field"><label for="pf-lang">${esc(t('acc.pref.lang'))}</label><select id="pf-lang" data-nosnap>${LANGS.map((l) => `<option value="${l}" lang="${l}"${l === lang() ? ' selected' : ''}>${esc(LANG_NAME[l])}</option>`).join('')}</select></div>
            <div class="field"><label for="pf-theme">${esc(t('acc.pref.theme'))}</label><select id="pf-theme" data-nosnap><option value="dark">${esc(t('acc.pref.dark'))}</option><option value="light">${esc(t('acc.pref.light'))}</option></select></div>
            <div class="field"><label for="pf-rail">${esc(t('acc.pref.rail'))}</label><select id="pf-rail" data-nosnap><option value="">${esc(t('acc.pref.rail.auto'))}</option><option value="open">${esc(t('acc.pref.rail.open'))}</option><option value="closed">${esc(t('acc.pref.rail.closed'))}</option></select></div></div>
            <button id="pf-reset">${esc(t('acc.pref.reset'))}</button>`, { foot: esc(t('acc.pref.foot')) })}
          ${plannedKeys('acc.planned', 3)}</div>`,
        wire() {
          const f = $('#pw');
          f.onsubmit = async (e) => {
            e.preventDefault(); clearFieldErrors(f);
            if (!f.c.value) { fieldError(f.c, t('acc.e.cur')); return; }
            if (f.n.value.length < 10) { fieldError(f.n, t('acc.e.len')); return; }
            if (f.n.value !== f.n2.value) { fieldError(f.n2, t('acc.e.differ')); return; }
            try { await withBusy($('button', f), t('acc.pw.busy'), () => api('/api/password', { method: 'POST', json: { current: f.c.value, new: f.n.value } })); me = null; renderLogin({ setup: false, key: 'login.changed', kind: 'info' }); }
            catch (err) { formError('#pwmsg', err); }
          };
          const th = $('#pf-theme'); th.value = document.documentElement.dataset.theme; th.onchange = () => setTheme(th.value);
          const r = $('#pf-rail'); r.value = prefs.get('nf-rail', ''); r.onchange = () => { prefs.set('nf-rail', r.value || null); applyRail(); };
          $('#pf-lang').onchange = (e) => switchLang(e.target.value);
          $('#pf-reset').onclick = () => { prefs.set('nf-lw', null); prefs.set('nf-rw', null); toast(() => esc(t('acc.pref.reset.done')), 'ok'); };
        },
      };
    },
  },

  admin: {
    async load() {
      if (me.role !== 'admin') return { denied: true };
      const [users, audit, sec, chain] = await Promise.all([api('/api/users'), api('/api/audit'), api('/api/security/status'), api('/api/audit/verify')]);
      return { users, audit, sec, chain };
    },
    render(data, tab) {
      if (data.denied) return { title: t('nav.admin'), crumbs: [[t('nav.admin')]], html: `<div class="page">${ph(esc(t('nav.admin')), '')}${empty('shield', t('adm.denied.t'), esc(t('adm.denied.p')), `<a class="btn" href="#/dashboard">${esc(t('adm.denied.go'))}</a>`)}</div>` };
      const { users, audit, sec, chain } = data;
      // what the server states about itself is shown as it wrote it; in Japanese the column says so once
      const srvText = (v) => (ja() ? `<span lang="en">${esc(v)}</span>` : esc(v));
      const srvHead = (key) => `${esc(t(key))}${ja() ? ` ${engTag('srv.tag')}` : ''}`;
      const val = (v) => (v === true ? tag('na', t('yes')) : v === false ? tag('na', t('no')) : typeof v === 'number' ? `<span class="mono">${v}</span>` : srvText(Array.isArray(v) ? v.join(', ') : v));
      const chainHtml = (ch) => h('adm.chain', { state: `${ch.intact ? tag('ok', t('adm.chain.ok'), 'check') : tag('bad', t('adm.chain.bad', { n: ch.first_broken_entry }), 'close')} ${esc(t('adm.chain.over', { n: ch.entries }))}` });
      const tabs = [['users', t('adm.t.users'), users.length], ['security', t('adm.t.security')], ['audit', t('adm.t.audit'), audit.length], ['planned', t('adm.t.planned')]];
      const active = tabs.some((x) => x[0] === tab) ? tab : 'users';
      return {
        title: t('nav.admin'), crumbs: [[t('nav.admin')]],
        html: `<div class="page">${ph(esc(t('nav.admin')), esc(t('adm.sub')))}
          ${tabsHtml('ad', t('adm.tabs.aria'), tabs, active)}
          ${tabPanel('ad', 'users', active, `${panel(esc(t('adm.t.users')), `<div class="tbl"><table id="tbl-users"><thead><tr><th>#</th><th>${esc(t('th.username'))}</th><th>${esc(t('th.role'))}</th><th>${esc(t('th.status'))}</th><th>${esc(t('th.created'))}</th><th><span class="vh">${esc(t('th.actions'))}</span></th></tr></thead><tbody>${users.map((u) => `<tr><td>${u.id}</td><td class="u">${esc(u.username)}${u.id === me.id ? ` <span class="sub">${esc(t('adm.you'))}</span>` : ''}</td><td>${esc(roleText(u.role))}</td><td>${u.active ? tag('na', t('adm.active')) : tag('na', t('adm.inactive'))}</td><td class="nw">${when(u.created_at)}</td>
              <td class="nw">${u.id === me.id ? '' : `<button class="${u.active ? 'danger' : ''}" data-user="${u.id}" data-name="${esc(u.username)}" data-to="${u.active ? 0 : 1}">${esc(t(u.active ? 'adm.deactivate' : 'adm.reactivate'))}</button>`}</td></tr>`).join('')}</tbody></table></div>`, { flush: true, count: users.length, foot: esc(t('adm.users.foot')) })}
            ${panel(esc(t('adm.add.h')), `<form id="nu" novalidate><div class="fgrid"><div class="field"><label for="nuu">${esc(t('th.username'))}</label><input id="nuu" name="u" autocomplete="off" autocapitalize="none" spellcheck="false"></div><div class="field"><label for="nup">${esc(t('adm.add.pass'))}</label><input id="nup" name="p" type="password" autocomplete="new-password"></div><div class="field"><label for="nur">${esc(t('th.role'))}</label><select id="nur" name="r">${['engineer', 'viewer', 'admin'].map((r) => `<option value="${r}">${esc(roleText(r))}</option>`).join('')}</select></div></div><div id="umsg"></div><button class="pri">${esc(t('adm.add.go'))}</button></form>`)}`)}
          ${tabPanel('ad', 'security', active, panel(esc(t('adm.sec.h')), `<div class="tbl"><table><thead><tr><th>${esc(t('th.setting'))}</th><th>${srvHead('th.value')}</th></tr></thead><tbody>${Object.entries(sec).map(([k, v]) => `<tr><td>${ja() && idLabel('security', k) ? `${jaHtml(idLabel('security', k))}<br>${mono(k)}` : ja() ? mono(k) : words(k)}</td><td>${val(v)}</td></tr>`).join('')}</tbody></table></div>`, { flush: true, foot: esc(t('adm.sec.foot')) }))}
          ${tabPanel('ad', 'audit', active, panel(esc(t('adm.t.audit')), `<div class="tbl"><table id="tbl-audit"><thead><tr><th>${esc(t('th.when'))}</th><th>${esc(t('th.user'))}</th><th>${esc(t('th.action'))}</th><th>${srvHead('th.detail')}</th></tr></thead><tbody>${audit.map((x) => `<tr><td class="nw">${when(x.at)}</td><td>${ud(x.username)}</td><td>${ja() && idLabel('audit', x.action) ? `${esc(idLabel('audit', x.action))}<br>` : ''}${mono(x.action)}</td><td>${srvText(x.detail)}</td></tr>`).join('')}</tbody></table></div>`,
            { flush: true, count: audit.length, tools: `${filterBox('tbl-audit', t('adm.audit.filter'))}<button id="verify">${ic('shield')}${esc(t('adm.verify'))}</button>`, foot: `<span id="chain">${chainHtml(chain)}</span> ${eng(chain.limitation)}` }))}
          ${tabPanel('ad', 'planned', active, planned(t('nav.admin'), Array.from({ length: 5 }, (_, i) => t(`adm.planned.${i + 1}`))))}</div>`,
        wire(root) {
          wireTabs(root, 'ad', (k) => pushTab(`${location.pathname}${location.search}#/admin/${k}`));
          const f = $('#nu'); f.onsubmit = async (e) => {
            e.preventDefault(); clearFieldErrors(f);
            if (!f.u.value.trim()) { fieldError(f.u, t('adm.e.user')); return; }
            if (f.p.value.length < 10) { fieldError(f.p, t('adm.e.pass')); return; }
            const name = f.u.value, role = f.r.value;
            try { await withBusy($('button', f), t('adm.add.busy'), () => api('/api/users', { method: 'POST', json: { username: name, password: f.p.value, role } })); toast(() => h('adm.add.done', { user: ud(name), role: esc(roleText(role)) }), 'ok'); refresh(); } catch (err) { formError('#umsg', err); }
          };
          // deactivation is a two-step inline confirmation, never a browser dialog
          $$('[data-user]', root).forEach((b) => { b.onclick = async () => {
            const off = b.dataset.to === '0', name = b.dataset.name;
            if (off && !b.dataset.armed) { b.dataset.armed = '1'; b.textContent = t('adm.confirm', { user: name }); setTimeout(() => { if (b.isConnected) { delete b.dataset.armed; b.textContent = t('adm.deactivate'); } }, 5000); return; }
            try { await api(`/api/users/${b.dataset.user}/active`, { method: 'POST', json: { active: !off } }); toast(() => h(off ? 'adm.off.done' : 'adm.on.done', { user: ud(name) }), 'ok'); refresh(); } catch (err) { toastErr(err); }
          }; });
          $('#verify').onclick = async (e) => { try { const ch = await withBusy(e.currentTarget, t('adm.verify.busy'), () => api('/api/audit/verify')); current.data.chain = ch; $('#chain').innerHTML = chainHtml(ch); toast(() => esc(ch.intact ? t('adm.chain.t.ok', { n: ch.entries }) : t('adm.chain.t.bad', { n: ch.first_broken_entry })), ch.intact ? 'ok' : 'err'); } catch (err) { toastErr(err); } };
        },
      };
    },
  },
};

// ---------------------------------------------------------------- resizable workspace panels
function wireSplits(root) {
  const body = $('.ws-body', root); if (!body) return;
  const lim = (side, v) => Math.round(Math.min(Math.max(v, 240), body.clientWidth * (side === 'l' ? 0.4 : 0.55)));
  const apply = (side, v) => { body.style.setProperty(side === 'l' ? '--lw' : '--rw', `${lim(side, v)}px`); };
  for (const side of ['l', 'r']) { const v = Number(prefs.get(`nf-${side}w`)); if (v) apply(side, v); }
  $$('.split', body).forEach((sp) => {
    const side = sp.dataset.split, pane = side === 'l' ? sp.previousElementSibling : sp.nextElementSibling;
    const save = () => prefs.set(`nf-${side}w`, String(pane.offsetWidth));
    sp.addEventListener('pointerdown', (e) => {
      e.preventDefault(); sp.setPointerCapture(e.pointerId); sp.classList.add('drag'); const x0 = e.clientX, w0 = pane.offsetWidth;
      const move = (m) => apply(side, w0 + (side === 'l' ? 1 : -1) * (m.clientX - x0));
      const up = () => { sp.classList.remove('drag'); sp.removeEventListener('pointermove', move); save(); };
      sp.addEventListener('pointermove', move); sp.addEventListener('pointerup', up, { once: true }); sp.addEventListener('pointercancel', up, { once: true });
    });
    sp.addEventListener('keydown', (e) => { const d = e.key === 'ArrowLeft' ? -24 : e.key === 'ArrowRight' ? 24 : 0; if (!d) return; e.preventDefault(); apply(side, pane.offsetWidth + (side === 'l' ? d : -d)); save(); });
    sp.addEventListener('dblclick', () => { body.style.removeProperty(side === 'l' ? '--lw' : '--rw'); prefs.set(`nf-${side}w`, null); });
  });
}

// ---------------------------------------------------------------- router
let renderSeq = 0;
function disposeView() { if (viewer) { viewer.destroy(); viewer = null; } }
function wireCommon(root) {
  $$('input[data-filter]', root).forEach((inp) => {
    const cnt = inp.closest('.panel-h') && $('.cnt', inp.closest('.panel-h')), total = cnt ? cnt.textContent : '';
    inp.oninput = () => {
      const q = inp.value.trim().toLowerCase(), body = $(`#${inp.dataset.filter} tbody`, root); if (!body) return;
      const old = $('.nomatch', body); if (old) old.remove();
      const rows = $$('tr', body); let shown = 0;
      rows.forEach((tr) => { tr.hidden = !!q && !tr.textContent.toLowerCase().includes(q); if (!tr.hidden) shown++; });
      if (cnt) cnt.textContent = q ? t('flt.count', { a: shown, b: rows.length }) : total;
      if (q && !shown) { const tr = document.createElement('tr'); tr.className = 'nomatch'; tr.innerHTML = `<td colspan="20">${h('flt.nomatch', { q: ud(inp.value.trim()) })} <button class="ghost mini">${esc(t('flt.clear'))}</button></td>`; body.append(tr); $('button', tr).onclick = () => { inp.value = ''; inp.oninput(); inp.focus(); }; }
    };
  });
}
// Draw a loaded view: `cur` is { name, args, data }, or { name, err } when loading failed.
function paint(cur, o = {}) {
  const out = cur.err
    ? { title: t('err.title'), crumbs: [[t('err.title')]], html: `<div class="page">${empty('warn', t('err.t'), errHtml(cur.err), `<button class="pri" id="retry">${ic('replay')}${esc(t('retry'))}</button><a class="btn" href="#/dashboard">${esc(t('to.dashboard'))}</a>`, 'err')}</div>`, wire() { $('#retry').onclick = () => route(); } }
    : views[cur.name].render(cur.data, ...cur.args);
  disposeView();
  const main = $('#view'), nav = out.nav !== undefined ? out.nav : cur.name;
  document.title = `${out.title} — NEXUS FABRIC`;
  $$('.rail a').forEach((a) => { if (a.dataset.nav === nav) a.setAttribute('aria-current', 'page'); else a.removeAttribute('aria-current'); });
  // a crumb is [label, href]; a label that holds a name a person typed is already escaped and wrapped (raw = true)
  $('#crumbs').innerHTML = (out.crumbs || []).map(([l, href, raw], i, all) => { const s = raw ? l : esc(l); return href ? `<a href="${href}">${s}</a>` : `<span${i === all.length - 1 ? ' aria-current="page"' : ''}>${s}</span>`; }).join('<span class="sep" aria-hidden="true">›</span>');
  main.style.overflow = out.ws ? '' : 'auto';
  main.innerHTML = out.html; main.dataset.hash = location.hash;
  // a file the user had chosen cannot be set again by a script: the element itself is carried over
  for (const old of o.files || []) { const now = document.getElementById(old.id); if (now && now !== old) now.replaceWith(old); }
  wireCommon(main); if (out.wire) out.wire(main);
}
const refresh = () => route(true);
async function route(keep = false) {
  if (!me) return;
  const seq = ++renderSeq; // a slower, older navigation must never overwrite a newer one
  // "#/" and an empty hash are the dashboard; a default in a destructuring pattern does not apply to an empty segment
  const parts = location.hash.split('/'), arg = parts[2], arg2 = parts[3];
  const rawName = parts[1] || 'dashboard';
  const name = views[rawName] ? rawName : 'notfound';
  mountShell(); toggleNotes(false);
  if (!keep) $$('#toasts .toast').forEach((x) => { if (Date.now() - Number(x.dataset.at) > 1500) x.remove(); });
  const scroll = keep ? [$('#view').scrollTop, ...$$('.pane-scroll', $('#view')).map((p) => p.scrollTop)] : null;
  if (!keep) { delete $('#view').dataset.hash; current = null; }
  const slow = keep ? 0 : setTimeout(() => { if (seq === renderSeq && $('#view')) { disposeView(); $('#view').innerHTML = `<div class="loading" role="status"><span class="vh">${esc(t('loading'))}</span><div class="skel w1"></div><div class="skel w2"></div><div class="skel w3"></div></div>`; } }, 200);
  let cur;
  try { cur = { name, args: [arg, arg2], data: await views[name].load(arg, arg2) }; }
  catch (err) { if (!me) return; cur = { name, args: [arg, arg2], err }; }
  clearTimeout(slow);
  if (seq !== renderSeq || !me) return;
  current = cur; paint(cur);
  const draft = takeDraft();
  if (draft) restoreUI({ fields: draft.fields, details: [], scroll: [0], focus: null, dialogs: [], notes: false, viewer: null, files: [] });
  const main = $('#view');
  if (scroll) { main.scrollTop = scroll[0]; $$('.pane-scroll', main).forEach((p, i) => { p.scrollTop = scroll[i + 1] || 0; }); }
  else { main.scrollTop = 0; main.focus({ preventScroll: true }); }
  if (name !== 'dashboard') refreshSummary();
}

// ---------------------------------------------------------------- keyboard
let gPending = 0;
document.addEventListener('keydown', (e) => {
  if (!me || e.ctrlKey || e.metaKey || e.altKey) return;
  const el = e.target, typing = el instanceof Element && el.matches('input,textarea,select,[contenteditable]');
  if (e.key === 'Escape') { if ($('#notes') && !$('#notes').hidden) toggleNotes(false); else if (typing) el.blur(); return; }
  if (typing || $('dialog[open]')) return;
  const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
  if (gPending && Date.now() - gPending < 1500) { gPending = 0; if (NAV_KEYS[k] && (NAV_KEYS[k] !== 'admin' || me.role === 'admin')) { e.preventDefault(); location.hash = `#/${NAV_KEYS[k]}`; } return; }
  gPending = 0;
  let used = true;
  if (e.key === '?') openHelp();
  else if (k === 'g') gPending = Date.now();
  else if (k === 'n' && me.role !== 'viewer') location.hash = `#/math${ctxProject ? `/${ctxProject}` : ''}`;
  else if (k === '[') toggleRail();
  else if (k === 't') $('#theme').click();
  else if (k === 'l') switchLang(LANGS[(LANGS.indexOf(lang()) + 1) % LANGS.length]);
  else if (k === '/') { const f = $('#view input[data-filter]'); if (f && f.offsetParent) f.focus(); else used = false; }
  else if (viewer && viewer.pattern) {
    if (k === 'f') viewer.fit(); else if (k === 'm') viewer.setTool(viewer.tool === 'measure' ? 'select' : 'measure');
    else if (k === '+' || k === '=') viewer.zoomBy(1.25); else if (k === '-') viewer.zoomBy(0.8);
    else if (/^[1-9]$/.test(k) && Number(k) <= viewer.partCount) viewer.showPart(Number(k) - 1);
    else used = false;
  } else used = false;
  if (used) e.preventDefault();
});
document.addEventListener('click', (e) => { const b = e.target.closest && e.target.closest('[data-tip]'); if (b) { b.dataset.tipOff = '1'; b.addEventListener('mouseleave', () => { delete b.dataset.tipOff; }, { once: true }); } }, true);
document.addEventListener('click', (e) => { const p = $('#notes'); if (p && !p.hidden && !e.target.closest('#notes,#bell')) toggleNotes(false); });
window.addEventListener('resize', applyRail);
window.addEventListener('hashchange', () => route());

// ---------------------------------------------------------------- start
let bootError = null;
function renderBootError(err) {
  bootError = err;
  $('#app').innerHTML = `<main class="login"><div class="login-top">${langSwitch()}</div><div class="login-card">${empty('warn', t('start.unreachable'), errHtml(err), `<button class="pri" id="retry">${esc(t('retry'))}</button>`, 'err')}</div></main>`;
  $('#retry').onclick = () => location.reload();
}
(async () => {
  try {
    const health = await api('/api/health');
    if (health.setup_required) return renderLogin({ setup: true });
    // The start-up probe answers 200 with signed_in false for a visitor who is not signed in (BH-008: a 401 here was
    // logged by the browser as an error on every sign-in screen load).
    try {
      const session = await api('/api/session');
      if (!session.signed_in) { me = null; renderLogin({ setup: false, key: null }); return; }
      me = { id: session.id, username: session.username, role: session.role }; route();
    } catch { /* an error is shown by api() */ }
  } catch (err) { renderBootError(err); }
})();
