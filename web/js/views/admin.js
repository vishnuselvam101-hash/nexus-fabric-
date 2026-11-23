import { api, esc } from '../api.js';
import { getState } from '../store.js';
import { t, h, idLabel, engineJa, lang } from '../i18n.js';
import { el, $, $$, toast } from '../ui.js';

// advisories belong to a job and are acknowledged on the job screen; the server has no list of them
const TABS = ['users', 'security', 'audit', 'lessons'];
const TAB_KEY = { users: 'adm.t.users', security: 'adm.t.security', audit: 'adm.t.audit', advisories: 's.known.h', lessons: 'am.t.lessons' };

let root = null, tab = 'users', d = null, seq = 0, filter = '', timers = [];

const ja = () => lang() === 'ja';
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : String(n ?? ''));
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
// every computed number: 3 decimals for display, the server's full precision on hover
const num = (v, unit = '') => (typeof v === 'number' ? `<span class="mono" title="${esc(fmt(v))}${unit ? ` ${esc(unit)}` : ''}">${n3(v)}</span>${unit ? ` <span class="sub">${esc(unit)}</span>` : ''}` : esc(v ?? '—'));
const mono = (s) => `<span class="mono">${esc(s)}</span>`;
const tg = (kind, text) => `<span class="chip${kind ? ` ${kind}` : ''}">${esc(text)}</span>`;
const pad2 = (n) => String(n).padStart(2, '0');
const when = (ts) => { if (!ts) return '—'; const x = new Date(typeof ts === 'number' && ts < 1e12 ? ts * 1000 : ts); return Number.isNaN(x.getTime()) ? esc(ts) : `${x.getFullYear()}-${pad2(x.getMonth() + 1)}-${pad2(x.getDate())} ${pad2(x.getHours())}:${pad2(x.getMinutes())}`; };
const byWhen = (who, ts) => `${esc(who ?? '—')}<br><span class="sub">${when(ts)}</span>`;
const words = (s) => esc(String(s ?? '').replaceAll('_', ' '));
const named = (kind, id) => { const l = idLabel(kind, id); if (!ja()) return l ? esc(l) : words(id); return l ? `${esc(l)} ${mono(id)}` : mono(id); };
const roleText = (r) => (['admin', 'engineer', 'viewer'].includes(r) ? t(`role.${r}`) : String(r ?? ''));
const advText = (s) => (['open', 'acknowledged'].includes(s) ? t(`adv.${s}`) : String(s ?? '').replaceAll('_', ' '));
const srvText = (v) => (ja() ? `<span lang="en">${esc(v ?? '')}</span>` : esc(v ?? ''));
const srvHead = (key) => `${esc(t(key))}${ja() ? ` ${tg('', t('srv.tag'))}` : ''}`;
const eng = (s) => { const v = String(s ?? ''); if (!v) return ''; if (!ja()) return `<span class="eng" lang="en">${esc(v)}</span>`; const j = engineJa(v); return j ? `${esc(j)} <span class="sub" lang="en">${esc(v)}</span>` : `${tg('', t('srv.tag'))} <span lang="en">${esc(v)}</span>`; };
const errText = (e) => String((e && e.message) || e || '');
const rowsOf = (v) => (Array.isArray(v) ? v : (v && (v.entries || v.advisories || v.lessons || v.items)) || []);
const jobLink = (id) => (id === undefined || id === null ? '—' : `<a href="#/job/${esc(id)}">${esc(t('job.n', { id }))}</a>`);

const table = (id, heads, rows) => `<table class="table"${id ? ` id="${id}"` : ''}><thead><tr>${heads.map((x) => `<th>${x}</th>`).join('')}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</tbody></table>`;
const panel = (title, body, o = {}) => `<section class="panel"><div class="panel-h"><h2>${title}</h2>${o.count !== undefined ? `<span class="cnt"${o.cid ? ` id="${o.cid}"` : ''}>${esc(o.count)}</span>` : ''}${o.tools || ''}</div><div class="panel-b">${body}${o.foot ? `<p class="sub">${o.foot}</p>` : ''}</div></section>`;
const fld = (id, label, input) => `<div class="field"><label for="${id}">${esc(label)}</label>${input}</div>`;
const emptyP = (key) => `<p class="sub">${esc(t(key))}</p>`;

function chainHtml(ch) {
  if (!ch) return '';
  const state = `${ch.intact ? tg('ok', t('adm.chain.ok')) : tg('bad', t('adm.chain.bad', { n: ch.first_broken_entry }))} ${esc(t('adm.chain.over', { n: ch.entries }))}`;
  return `${h('adm.chain', { state })} ${eng(ch.limitation)}`;
}

function usersHtml() {
  const me = getState().user || {};
  const rows = d.users.map((u) => [
    esc(u.id),
    `${esc(u.username)}${u.id === me.id ? ` <span class="sub">${esc(t('adm.you'))}</span>` : ''}`,
    esc(roleText(u.role)),
    tg('', t(u.active ? 'adm.active' : 'adm.inactive')),
    when(u.created_at),
    u.id === me.id ? '' : `<button class="btn btn-secondary" type="button" data-act="active" data-user="${esc(u.id)}" data-name="${esc(u.username)}" data-to="${u.active ? 0 : 1}">${esc(t(u.active ? 'adm.deactivate' : 'adm.reactivate'))}</button>`,
  ]);
  const form = `<form id="nu" class="calc-form" novalidate>
    ${fld('nuu', t('th.username'), '<input id="nuu" name="username" autocomplete="off" autocapitalize="none" spellcheck="false">')}
    ${fld('nup', t('adm.add.pass'), '<input id="nup" name="password" type="password" autocomplete="new-password">')}
    ${fld('nur', t('th.role'), `<select id="nur" name="role">${['engineer', 'viewer', 'admin'].map((r) => `<option value="${r}">${esc(roleText(r))}</option>`).join('')}</select>`)}
    <div id="umsg"></div><button class="btn pri" type="submit">${esc(t('adm.add.go'))}</button></form>`;
  return panel(esc(t('adm.t.users')), table('adm-users', ['#', esc(t('th.username')), esc(t('th.role')), esc(t('th.status')), esc(t('th.created')), `<span class="sub">${esc(t('th.actions'))}</span>`], rows),
    { count: d.users.length, foot: esc(t('adm.users.foot')) })
    + panel(esc(t('adm.add.h')), form);
}

function securityHtml() {
  const val = (v) => (v === true ? tg('', t('yes')) : v === false ? tg('', t('no')) : typeof v === 'number' ? mono(v) : srvText(Array.isArray(v) ? v.join(', ') : v));
  const rows = Object.entries(d.sec || {}).map(([k, v]) => [
    ja() ? (idLabel('security', k) ? `${esc(idLabel('security', k))}<br>${mono(k)}` : mono(k)) : words(k),
    val(v),
  ]);
  return panel(esc(t('adm.sec.h')), table('adm-sec', [esc(t('th.setting')), srvHead('th.value')], rows), { foot: esc(t('adm.sec.foot')) });
}

function auditHtml() {
  const rows = d.audit.map((x) => [
    when(x.at ?? x.timestamp ?? x.created_at),
    esc(x.username ?? x.user ?? ''),
    `${ja() && idLabel('audit', x.action) ? `${esc(idLabel('audit', x.action))}<br>` : ''}${mono(x.action)}`,
    srvText(x.detail ?? x.message),
  ]);
  const tools = `<div class="row"><label class="sub" for="adm-flt">${esc(t('adm.audit.filter'))}</label><input id="adm-flt" type="search" value="${esc(filter)}" autocomplete="off"><button class="btn" type="button" data-act="verify">${esc(t('adm.verify'))}</button></div>`;
  return panel(esc(t('adm.t.audit')), table('adm-audit', [esc(t('th.when')), esc(t('th.user')), esc(t('th.action')), srvHead('th.detail')], rows),
    { count: d.audit.length, cid: 'adm-audit-cnt', tools, foot: `<span id="adm-chain">${chainHtml(d.chain)}</span>` });
}

function advisoriesHtml() {
  if (!d.advs.length) return panel(esc(t('s.known.h')), `${emptyP('s.none.t')}${emptyP('s.none.p')}`);
  const rows = d.advs.map((a) => {
    const open = a.state === 'open';
    const act = open ? `<div class="field"><label for="advr-${esc(a.id)}">${esc(t('s.adv.aria', { id: a.id }))}</label><input id="advr-${esc(a.id)}" data-adv="${esc(a.id)}" placeholder="${esc(t('s.adv.ph'))}" autocomplete="off"></div><button class="btn" type="button" data-act="ack" data-adv="${esc(a.id)}">${esc(t('s.adv.ack'))}</button>` : '';
    return [
      `A${esc(a.id)}`,
      jobLink(a.target_calc),
      eng(a.message),
      tg(open ? 'warn' : '', advText(a.state)),
      `${a.note ? esc(a.note) : '—'}${a.resolved_by ? `<br><span class="sub">${byWhen(a.resolved_by, a.resolved_at)}</span>` : ''}`,
      act,
    ];
  });
  return panel(esc(t('s.known.h')), table('adm-adv', ['#', esc(t('th.job')), esc(t('th.what')), esc(t('th.state.short')), esc(t('th.evidence')), `<span class="sub">${esc(t('th.actions'))}</span>`], rows),
    { count: d.advs.filter((a) => a.state === 'open').length, foot: esc(t('s.known.p')) });
}

function lessonsHtml() {
  if (!d.lessons.length) return panel(esc(t('am.les.h')), `${emptyP('lessons.none.t')}${emptyP('lessons.none.p')}`);
  const rows = d.lessons.map((l) => [
    esc(l.id),
    l.kind === 'corrected_value'
      ? `${h('lesson.corrected', { fam: named('family', l.family), param: mono(l.parameter), wrong: num(l.wrong_value), right: num(l.right_value) })}<br><span class="sub">${esc(l.detail ?? '')}</span>`
      : h('lesson.other', { kind: named('lesson', l.kind), fam: named('family', l.family) }),
    `${jobLink(l.source_calc)}${l.project_id !== undefined && l.project_id !== null ? `<br><a class="sub" href="#/project/${esc(l.project_id)}">#${esc(l.project_id)}</a>` : ''}`,
    esc(l.times_matched),
    byWhen(l.created_by, l.created_at),
  ]);
  return panel(esc(t('am.les.h')), table('adm-les', ['#', esc(t('th.what')), esc(t('th.fromjob')), esc(t('th.matched')), esc(t('th.recorded'))], rows),
    { count: d.lessons.length, foot: esc(t('ver.lessons.foot')) });
}

const BODY = { users: usersHtml, security: securityHtml, audit: auditHtml, advisories: advisoriesHtml, lessons: lessonsHtml };

function render() {
  if (!root || !d) return;
  const counts = { users: d.users.length, audit: d.audit.length, advisories: d.advs.filter((a) => a.state === 'open').length, lessons: d.lessons.length };
  root.innerHTML = `<div class="panel-h"><h2>${esc(t('nav.admin'))}</h2><span class="sub">${esc(t('adm.sub'))}</span></div>
    <div class="tabs" role="tablist" aria-label="${esc(t('adm.tabs.aria'))}">${TABS.map((k) => `<button class="btn" type="button" role="tab" id="adtab-${k}" data-tab="${k}" aria-selected="${k === tab}" aria-controls="adpanel" tabindex="${k === tab ? 0 : -1}">${esc(t(TAB_KEY[k]))}${counts[k] !== undefined ? ` <span class="cnt">${counts[k]}</span>` : ''}</button>`).join('')}</div>
    <div class="ws-content" id="adpanel" role="tabpanel" aria-labelledby="adtab-${tab}" tabindex="0">${BODY[tab]()}</div>`;
  if (tab === 'audit') applyFilter();
}

async function load() {
  const my = ++seq;
  root.innerHTML = `<p class="sub">${esc(t('loading'))}</p>`;
  try {
    const [users, sec, audit, chain, lessons] = await Promise.all([
      api('/api/users'), api('/api/security/status'), api('/api/audit'), api('/api/audit/verify'), api('/api/lessons'),
    ]);
    if (my !== seq || !root) return;
    d = { users: rowsOf(users), sec, audit: rowsOf(audit), chain, advs: [], lessons: rowsOf(lessons) };
    render();
  } catch (err) {
    if (my !== seq || !root) return;
    root.innerHTML = `<div class="alert bad"><p>${esc(errText(err))}</p><div class="row"><button class="btn" type="button" data-act="retry">${esc(t('retry'))}</button></div></div>`;
  }
}

function setErr(input, msg) {
  const id = `${input.id}-err`;
  input.setAttribute('aria-invalid', 'true');
  input.setAttribute('aria-describedby', id);
  let m = document.getElementById(id);
  if (!m) { m = el('p', { class: 'sub', id, role: 'alert' }); input.after(m); }
  m.textContent = msg;
  input.focus();
}
function clearErr(input) {
  input.removeAttribute('aria-invalid');
  input.removeAttribute('aria-describedby');
  const m = document.getElementById(`${input.id}-err`);
  if (m) m.remove();
}

function applyFilter() {
  const tb = $('#adm-audit tbody', root);
  if (!tb) return;
  const q = filter.trim().toLowerCase();
  let shown = 0;
  $$('tr', tb).forEach((tr) => {
    if (tr.dataset.nomatch) { tr.remove(); return; }
    tr.hidden = !!q && !tr.textContent.toLowerCase().includes(q);
    if (!tr.hidden) shown++;
  });
  const c = $('#adm-audit-cnt', root);
  if (c) c.textContent = q ? t('flt.count', { a: shown, b: d.audit.length }) : String(d.audit.length);
  if (q && !shown) tb.insertAdjacentHTML('beforeend', `<tr data-nomatch="1"><td colspan="4">${h('flt.nomatch', { q: esc(filter.trim()) })} <button class="btn btn-text" type="button" data-act="clearfilter">${esc(t('flt.clear'))}</button></td></tr>`);
}

function go(k) {
  if (!TABS.includes(k)) return;
  tab = k; filter = '';
  location.hash = `#/admin/${k}`;
  render();
  const b = $(`#adtab-${k}`, root);
  if (b) b.focus();
}

async function addUser(f) {
  const u = f.elements.username, p = f.elements.password, role = f.elements.role.value;
  clearErr(u); clearErr(p);
  if (!u.value.trim()) { setErr(u, t('adm.e.user')); return; }
  if (p.value.length < 10) { setErr(p, t('adm.e.pass')); return; }
  const name = u.value, btn = $('button[type=submit]', f), old = btn.textContent;
  btn.disabled = true; btn.textContent = t('adm.add.busy');
  try {
    await api('/api/users', { method: 'POST', json: { username: name, password: p.value, role } });
    toast(t('adm.add.done', { user: name, role: roleText(role) }), 'ok');
    await load();
  } catch (err) {
    const m = $('#umsg', root);
    if (m) m.innerHTML = `<div class="alert bad" role="alert"><p>${esc(errText(err))}</p></div>`;
    if (btn.isConnected) { btn.disabled = false; btn.textContent = old; }
  }
}

async function toggleUser(b) {
  const off = b.dataset.to === '0', name = b.dataset.name;
  if (off && !b.dataset.armed) {
    b.dataset.armed = '1';
    b.textContent = t('adm.confirm', { user: name });
    timers.push(setTimeout(() => { if (b.isConnected) { delete b.dataset.armed; b.textContent = t('adm.deactivate'); } }, 5000));
    return;
  }
  b.disabled = true;
  try {
    await api(`/api/users/${b.dataset.user}/active`, { method: 'POST', json: { active: !off } });
    toast(t(off ? 'adm.off.done' : 'adm.on.done', { user: name }), 'ok');
    await load();
  } catch (err) {
    toast(errText(err), 'err');
    if (b.isConnected) b.disabled = false;
  }
}

async function verify(b) {
  const old = b.textContent;
  b.disabled = true; b.textContent = t('adm.verify.busy');
  try {
    const ch = await api('/api/audit/verify');
    if (!root) return;
    d.chain = ch;
    const s = $('#adm-chain', root);
    if (s) s.innerHTML = chainHtml(ch);
    toast(ch.intact ? t('adm.chain.t.ok', { n: ch.entries }) : t('adm.chain.t.bad', { n: ch.first_broken_entry }), ch.intact ? 'ok' : 'err');
  } catch (err) { toast(errText(err), 'err'); }
  finally { if (b.isConnected) { b.disabled = false; b.textContent = old; } }
}

async function ack(b) {
  const id = b.dataset.adv, input = $(`#advr-${id}`, root);
  if (!input) return;
  clearErr(input);
  const reason = input.value.trim();
  if (reason.length < 5) { setErr(input, t('s.e.adv')); return; }
  b.disabled = true;
  try {
    await api(`/api/advisories/${id}/acknowledge`, { method: 'POST', json: { reason } });
    toast(t('s.t.adv', { id }), 'ok');
    await load();
  } catch (err) {
    toast(errText(err), 'err');
    if (b.isConnected) b.disabled = false;
  }
}

function onClick(e) {
  const b = e.target.closest && e.target.closest('[data-tab],[data-act]');
  if (!b || !root || !root.contains(b)) return;
  if (b.dataset.tab) { go(b.dataset.tab); return; }
  const act = b.dataset.act;
  if (act === 'retry') load();
  else if (act === 'active') toggleUser(b);
  else if (act === 'verify') verify(b);
  else if (act === 'ack') ack(b);
  else if (act === 'clearfilter') { filter = ''; const i = $('#adm-flt', root); if (i) { i.value = ''; applyFilter(); i.focus(); } }
}

function onSubmit(e) {
  const f = e.target;
  if (f.id !== 'nu') return;
  e.preventDefault();
  addUser(f);
}

function onInput(e) {
  const i = e.target;
  if (i.id === 'adm-flt') { filter = i.value; applyFilter(); return; }
  if (i.getAttribute && i.getAttribute('aria-invalid')) clearErr(i);
}

function onKey(e) {
  const b = e.target.closest && e.target.closest('[role=tab]');
  if (!b) return;
  const i = TABS.indexOf(b.dataset.tab);
  const j = e.key === 'ArrowRight' ? (i + 1) % TABS.length
    : e.key === 'ArrowLeft' ? (i - 1 + TABS.length) % TABS.length
      : e.key === 'Home' ? 0 : e.key === 'End' ? TABS.length - 1 : -1;
  if (j < 0) return;
  e.preventDefault();
  go(TABS[j]);
}

function attach(on) {
  if (!root) return;
  const m = on ? 'addEventListener' : 'removeEventListener';
  root[m]('click', onClick);
  root[m]('submit', onSubmit);
  root[m]('input', onInput);
  root[m]('keydown', onKey);
}

export async function mount(container, params) {
  attach(false);
  root = container;
  d = null; filter = '';
  const want = params && (params.tab || params.id);
  tab = TABS.includes(want) ? want : 'users';
  attach(true);
  const me = getState().user;
  if (!me || me.role !== 'admin') {
    root.innerHTML = `<div class="alert bad"><p>${esc(t('adm.denied.t'))}</p><p class="sub">${esc(t('adm.denied.p'))}</p><div class="row"><a class="btn" href="#/workspace/calculations">${esc(t('adm.denied.go'))}</a></div></div>`;
    return;
  }
  await load();
}

export function unmount() {
  seq++;
  timers.forEach(clearTimeout);
  timers = [];
  attach(false);
  root = null;
  d = null;
  filter = '';
}
