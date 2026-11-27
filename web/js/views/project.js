// Project detail screen — route '#/project/{id}'. Ported from views.project and views.convert of the old client.
// Release states are read-only; nothing here approves, releases or signs off a result. No engineering value is
// guessed: the conversion scale must be typed, and every computed number is shown as the server returned it.
import { api, uploadFile, esc } from '../api.js';
import { getState } from '../store.js';
import { t, h, idLabel, lang, projName } from '../i18n.js';
import { el, frag, $, $$, toast, chip, busy } from '../ui.js';

let root = null;
let ac = null;
let seq = 0;
let projectId = null;
let data = null;
const timers = new Set();

const on = (node, ev, fn) => node.addEventListener(ev, fn, { signal: ac.signal });
const role = () => (getState() && getState().user ? getState().user.role : 'viewer');
const ja = () => lang() === 'ja';
const words = (s) => String(s ?? '').replaceAll('_', ' ');
const pad2 = (n) => String(n).padStart(2, '0');
const when = (ts) => { if (!ts) return '—'; const d = new Date(ts * 1000); return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`; };
const fmt = (n) => (typeof n === 'number' ? (n === 0 ? '0' : Math.abs(n) >= 1e-3 && Math.abs(n) < 1e9 ? Number(n.toPrecision(12)).toString() : n.toExponential(3)) : String(n ?? ''));
const nf3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
const n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : nf3.format(Math.abs(v) < 5e-4 ? 0 : v));
const num = (v, unit = '') => (typeof v === 'number' ? el('span', { class: 'mono', title: unit ? `${fmt(v)} ${unit}` : fmt(v) }, unit ? `${n3(v)} ${unit}` : n3(v)) : String(v ?? '—'));
const ud = (v) => el('bdi', {}, String(v ?? ''));
const mono = (s) => el('span', { class: 'mono' }, String(s ?? ''));
const short = (hash) => el('span', { class: 'mono', title: String(hash ?? '') }, `${String(hash ?? '').slice(0, 12)}…`);
const decodeName = (n) => { try { return decodeURIComponent(n); } catch { return String(n ?? ''); } };
const famShort = (k) => idLabel('family', k) || words(k);
const partName = (p) => (ja() && idLabel('part', p) ? frag(idLabel('part', p), ' ', mono(p)) : String(p));
const roleText = (r) => (['admin', 'engineer', 'viewer'].includes(r) ? t(`role.${r}`) : String(r));
const byWhen = (who, ts) => frag(ud(who), el('br'), el('span', { class: 'sub' }, when(ts)));

const STATE_KIND = { APPROVED: 'ok', REJECTED: 'bad', SUPERSEDED: 'neutral', NO_RESULT: 'bad' };
const stateText = (s) => (['APPROVED', 'REJECTED', 'SUPERSEDED', 'NO_RESULT', 'REVIEW_REQUIRED', 'REFUSED'].includes(s) ? t(`state.${s}`) : words(s));
const stateChip = (s) => chip(stateText(s), STATE_KIND[s] || 'warn');
const replayCell = (m) => (m === true ? chip(t('replay.match'), 'ok') : m === false ? chip(t('replay.mismatch'), 'bad') : el('span', { class: 'sub' }, t('replay.none')));
// Same precedence as every list in the old client: superseded > no result > the server state.
const effState = (c) => (c.superseded || (c.facts && c.facts.superseded_by) ? 'SUPERSEDED' : c.no_result || (c.facts && c.facts.noResult) ? 'NO_RESULT' : c.state);
const summarise = (c) => ({
  noResult: !c.patterns.length,
  superseded_by: c.superseded_by,
  parts: c.patterns.map((p) => ({ part: p.part, cls: p.class })),
  replay: c.replay ? c.replay.matches : null,
});

// ---------------------------------------------------------------- shared fragments
function panel(title, body, o = {}) {
  return el('section', { class: 'panel', id: o.id },
    el('header', { class: 'panel-h' }, el('h2', {}, title), o.count !== undefined ? el('span', { class: 'cnt' }, String(o.count)) : null, o.tools || null),
    el('div', { class: 'panel-b' }, body),
    o.foot ? el('div', { class: 'panel-b' }, o.foot) : null);
}
function table(id, cols, rows) {
  return el('table', { class: 'table', id },
    el('thead', {}, el('tr', {}, cols.map((c) => el('th', {}, c)))),
    el('tbody', {}, rows.map((cells) => el('tr', {}, cells.map((c) => el('td', {}, c))))));
}
function alertBox(host, content, kind = 'bad', id) {
  host.textContent = '';
  const box = el('div', { class: `alert ${kind}`, role: 'alert', id }, content);
  host.append(box);
  return box;
}
function textField(id, label, o = {}) {
  const input = el('input', {
    id, name: o.name || id, type: 'text', inputmode: o.mode || 'decimal', autocomplete: 'off',
    value: o.value ?? '', 'aria-describedby': o.hint ? `${id}-hint` : null,
  });
  const wrap = el('div', { class: 'field' }, el('label', { for: id }, label), input,
    o.hint ? el('p', { class: 'sub', id: `${id}-hint` }, o.hint) : null);
  return { wrap, input };
}
function checkField(id, label) {
  const input = el('input', { type: 'checkbox', id, name: id, checked: true });
  return { wrap: el('label', { class: 'chk', for: id }, input, ` ${label}`), input };
}
function filterField(id, label) {
  return el('div', { class: 'row' }, el('label', { class: 'sub', for: id }, label),
    el('input', { id, type: 'search', autocomplete: 'off', dataset: { filter: id } }));
}
function wireFilter(input, tableId, cnt) {
  const total = cnt ? cnt.textContent : '';
  on(input, 'input', () => {
    const body = $(`#${tableId} tbody`, root); if (!body) return;
    const prev = $('tr[data-nomatch]', body); if (prev) prev.remove();
    const q = input.value.trim().toLowerCase(), rows = $$('tr', body);
    let shown = 0;
    rows.forEach((tr) => { tr.hidden = !!q && !tr.textContent.toLowerCase().includes(q); if (!tr.hidden) shown++; });
    if (cnt) cnt.textContent = q ? t('flt.count', { a: shown, b: rows.length }) : total;
    if (q && !shown) {
      const clear = el('button', { class: 'btn btn-text', type: 'button' }, t('flt.clear'));
      body.append(el('tr', { dataset: { nomatch: '1' } }, el('td', { colspan: 20 },
        el('span', { html: h('flt.nomatch', { q: esc(input.value.trim()) }) }), ' ', clear)));
      on(clear, 'click', () => { input.value = ''; input.dispatchEvent(new Event('input')); input.focus(); });
    }
  });
}

// ---------------------------------------------------------------- loading
async function load(id) {
  const [p, users] = await Promise.all([
    api(`/api/projects/${id}`),
    role() === 'admin' ? api('/api/users') : Promise.resolve([]),
  ]);
  const cs = p.calculations || [];
  // The list API carries the state but not the parts or the replay result: each job is read once, as before.
  const facts = await Promise.all(cs.map((c) => api(`/api/calculations/${c.id}`).then(summarise, () => null)));
  cs.forEach((c, i) => { c.facts = facts[i]; });
  return { p, users: users || [] };
}
async function reload() {
  const my = ++seq;
  try {
    const d = await load(projectId);
    if (my !== seq || !root) return;
    data = d; render();
  } catch (err) { if (my === seq && root) renderError(err); }
}
function renderError(err) {
  root.textContent = '';
  const retry = el('button', { class: 'btn pri', type: 'button' }, t('retry'));
  on(retry, 'click', () => reload());
  root.append(panel(t('err.title'), el('div', { class: 'alert bad', role: 'alert' },
    el('p', {}, t('err.t')),
    el('p', { lang: 'en' }, String((err && err.message) || err)),
    el('div', { class: 'row' }, retry, el('a', { class: 'btn', href: '#/workspace/projects' }, t('jobs.all'))))));
}

// ---------------------------------------------------------------- header, jobs
function header(p) {
  return el('div', { class: 'panel-h' },
    el('div', {},
      el('h1', {}, ud(projName(p.name))),
      el('p', { class: 'sub', html: `${p.description ? `${esc(p.description)} · ` : ''}${h('project.meta', { id: p.id, by: esc(p.created_by), when: when(p.created_at) })}` })),
    role() === 'viewer' ? null : el('a', { class: 'btn pri', href: `#/math/${p.id}` }, t('new.calc')));
}
function partsCell(c) {
  if (!c.facts) return el('span', { class: 'sub' }, '—');
  if (c.facts.noResult) return el('span', { class: 'sub' }, t('parts.none'));
  return frag(...c.facts.parts.map((p, i) => frag(i ? ', ' : '', partName(p.part))));
}
function stateCell(c) {
  return frag(stateChip(effState(c)),
    c.open_obligations && !c.superseded ? frag(' ', chip(t('obl.open.n', { n: c.open_obligations }), 'warn')) : null);
}
function jobsPanel(p) {
  const rows = p.calculations || [];
  const cnt = el('span', { class: 'cnt' }, String(rows.length));
  const filter = rows.length ? filterField('flt-jobs', t('jobs.filter')) : null;
  const body = rows.length
    ? table('tbl-jobs', [t('th.job'), t('th.family'), t('th.parts'), t('th.state'), t('th.replay'), t('th.entered')],
      rows.map((c) => [
        el('a', { href: `#/job/${c.id}` }, t('job.n', { id: c.id })),
        el('span', { title: String(c.family ?? '') }, famShort(c.family)),
        partsCell(c),
        stateCell(c),
        replayCell(c.facts ? c.facts.replay : null),
        byWhen(c.created_by, c.created_at),
      ]))
    : el('div', { class: 'alert' }, el('p', {}, t('jobs.none.t')), el('p', { class: 'sub' }, t('jobs.none.p')),
      el('div', { class: 'row' },
        role() === 'viewer' ? null : el('a', { class: 'btn', href: '#/workspace/projects' }, t('jobs.none.cta')),
        role() === 'viewer' ? null : el('a', { class: 'btn pri', href: `#/math/${p.id}` }, t('new.calc'))));
  const node = panel(t('project.jobs'), body, { tools: filter });
  const head = $('.panel-h', node); if (head) head.append(cnt);
  if (filter) wireFilter($('input', filter), 'tbl-jobs', cnt);
  return node;
}

// ---------------------------------------------------------------- documents and upload
function docRow(p, d) {
  const fname = decodeName(d.filename);
  return [
    frag(ud(fname),
      d.extension_mismatch ? frag(' ', chip(t('doc.mismatch'), 'warn')) : null,
      d.conversion_id ? frag(el('br'), el('a', { href: `#/r/${d.conversion_id}` }, chip(t('conv.label'), 'warn'))) : null,
      d.detected_type === 'pdf' && role() !== 'viewer'
        ? frag(el('br'), el('button', {
          class: 'btn', type: 'button', dataset: { convert: String(d.id) },
          'aria-label': t('conv.action.aria', { file: fname }),
        }, t('conv.action')))
        : null),
    mono(d.detected_type),
    String(d.size),
    short(d.sha256),
    byWhen(d.created_by, d.created_at),
  ];
}
function uploadForm(p) {
  const input = el('input', { id: 'up-file', name: 'file', type: 'file', 'aria-label': t('doc.file.aria') });
  const msg = el('div', { id: 'up-msg' });
  const go = el('button', { class: 'btn', type: 'submit' }, t('doc.store'));
  const form = el('form', { class: 'calc-form', id: 'up', novalidate: true },
    el('div', { class: 'field' }, el('label', { for: 'up-file' }, t('doc.choose')), input), msg, go);
  on(input, 'change', () => { msg.textContent = ''; });
  on(form, 'submit', async (e) => {
    e.preventDefault();
    const file = input.files[0];
    if (!file) { alertBox(msg, t('doc.e.nofile'), 'bad', 'up-err'); input.setAttribute('aria-invalid', 'true'); input.setAttribute('aria-describedby', 'up-err'); return; }
    input.removeAttribute('aria-invalid');
    busy(go, true); go.textContent = t('storing');
    try {
      const r = await uploadFile(`/api/projects/${p.id}/documents`, file);
      toast(t('doc.stored', { type: r && r.detected_type, interp: r && r.interpretation }), 'ok');
      await reload();
    } catch (err) {
      alertBox(msg, el('p', { lang: 'en' }, String((err && err.message) || err)), 'bad');
    } finally { busy(go, false); go.textContent = t('doc.store'); }
  });
  return form;
}
function docsPanel(p) {
  const docs = p.documents || [];
  const body = docs.length
    ? table('tbl-docs', [t('th.file'), t('th.dtype'), t('th.size'), t('th.sha'), t('th.uploaded')], docs.map((d) => docRow(p, d)))
    : el('div', { class: 'alert' }, el('p', {}, t('docs.none.t')), el('p', { class: 'sub' }, t('docs.none.p')));
  const foot = frag(
    role() === 'viewer' ? null : uploadForm(p),
    el('p', { class: 'sub', html: h('doc.accepted') }),
    el('p', { class: 'sub', html: h('doc.snapshot', { id: p.snapshot ? `<span class="mono">${esc(p.snapshot)}</span>` : esc(t('doc.snapshot.none')) }) }),
  );
  return panel(t('project.docs'), body, { count: docs.length, foot });
}

// ---------------------------------------------------------------- PDF conversions (read-only records)
function convPanel(p) {
  const cs = p.conversions || [];
  if (!cs.length) return null;
  const rows = cs.map((c) => [
    el('a', { href: `#/r/${c.id}` }, String(c.id)),
    `#${c.source_document}`,
    String(c.page),
    c.status === 'converted'
      ? chip(t('conv.label'), 'warn')
      : frag(chip(t(c.status === 'limit_exceeded' ? 'conv.st.limit' : 'conv.st.refused'), 'bad'), ' ', mono(c.code || '')),
    byWhen(c.created_by, c.created_at),
  ]);
  return panel(t('conv.panel'), table('tbl-conv', ['#', t('conv.th.doc'), t('conv.th.page'), t('conv.th.result'), t('th.created')], rows), { count: cs.length });
}

// ---------------------------------------------------------------- starting a conversion (old `convert` view)
function openConvert(p, doc) {
  const host = $('#convert-host', root); if (!host) return;
  host.textContent = '';
  const fname = decodeName(doc.filename);
  const head = el('header', { class: 'panel-h' }, el('h2', {}, t('conv.title')), el('span', { class: 'sub' }, `${fname} · ${String(doc.sha256 || '').slice(0, 12)}…`));
  const rules = el('div', { class: 'alert warn', html: h('conv.rules') });
  const stop = (key) => host.append(el('section', { class: 'panel' }, head, el('div', { class: 'panel-b' }, rules, el('div', { class: 'alert bad' }, t(key)))));
  if (doc.detected_type !== 'pdf') { stop('conv.e.notpdf'); return; }
  if (role() === 'viewer') { stop('conv.e.viewer'); return; }

  const page = textField('cv-page', t('conv.f.page'), { mode: 'numeric', value: '1' });
  const scale = textField('cv-scale', t('conv.f.scale'), { hint: t('conv.f.scale.hint') });
  const tol = textField('cv-tol', t('conv.f.tol'), { hint: t('conv.f.tol.hint') });
  const kind = el('select', { id: 'cv-kind', name: 'kind' },
    el('option', { value: 'ratio' }, t('conv.f.kind.ratio')),
    el('option', { value: 'mm_per_point' }, t('conv.f.kind.mmpt')));
  const kindField = el('div', { class: 'field' }, el('label', { for: 'cv-kind' }, t('conv.f.kind')), kind);
  const fill = checkField('cv-fill', t('conv.f.fill'));
  const text = checkField('cv-text', t('conv.f.text'));
  const msg = el('div', { id: 'cv-msg' });
  const go = el('button', { class: 'btn pri', type: 'submit' }, t('conv.submit'));
  const cancel = el('button', { class: 'btn', type: 'button' }, t('cancel'));
  const form = el('form', { class: 'calc-form', id: 'cv', novalidate: true },
    el('div', { class: 'calc-params' }, page.wrap, kindField, scale.wrap, tol.wrap),
    el('div', { class: 'row' }, fill.wrap, text.wrap), msg, el('div', { class: 'row' }, go, cancel));
  host.append(el('section', { class: 'panel' }, head, el('div', { class: 'panel-b' }, rules, form)));
  on(cancel, 'click', () => { host.textContent = ''; });
  on(scale.input, 'input', () => { scale.input.removeAttribute('aria-invalid'); scale.input.setAttribute('aria-describedby', 'cv-scale-hint'); });
  on(form, 'submit', async (e) => {
    e.preventDefault();
    msg.textContent = '';
    const raw = (i) => String(i.value).trim();
    // The scale is never guessed: without it nothing is sent.
    if (raw(scale.input) === '') {
      alertBox(msg, t('conv.e.scale'), 'bad', 'cv-err');
      scale.input.setAttribute('aria-invalid', 'true');
      scale.input.setAttribute('aria-describedby', 'cv-err');
      scale.input.focus();
      return;
    }
    const numOrNull = (v) => (v === '' ? null : Number(v));
    const pageN = Number(raw(page.input) || '1'), scaleN = numOrNull(raw(scale.input)), tolN = numOrNull(raw(tol.input));
    const body = {
      page: Number.isInteger(pageN) ? pageN : raw(page.input),
      scale_kind: kind.value,
      scale_value: Number.isFinite(scaleN) ? scaleN : raw(scale.input),
      tolerance_mm: tolN === null ? null : Number.isFinite(tolN) ? tolN : raw(tol.input),
      include_fill: fill.input.checked,
      include_text: text.input.checked,
    };
    busy(go, true); go.textContent = t('conv.running');
    try {
      const r = await api(`/api/documents/${doc.id}/pdf-to-dxf`, { method: 'POST', json: body });
      toast(t('conv.done', { id: r.conversion.id }), 'warn');
      location.hash = `#/r/${r.conversion.id}`;
    } catch (err) {
      const a = err && err.detail;
      if (a && a.code) {
        alertBox(msg, frag(
          el('p', { html: h('conv.refused', { code: `<span class="mono">${esc(a.code)}</span>`, limit: a.limit ? `<span class="mono">${esc(a.limit)}</span>` : '—' }) }),
          el('p', { lang: 'en' }, String(a.message || a.error || '')),
          a.conversion ? el('a', { href: `#/r/${a.conversion.id}` }, t('conv.refused.rec', { id: a.conversion.id })) : null,
        ), 'bad');
      } else {
        alertBox(msg, el('p', { lang: 'en' }, String((err && err.message) || err)), 'bad');
      }
    } finally { busy(go, false); go.textContent = t('conv.submit'); }
  });
  scale.input.focus();
}

// ---------------------------------------------------------------- project access
function membersPanel(p, users) {
  const members = p.members || [];
  const admin = role() === 'admin';
  const msg = el('div', { id: 'mem-msg' });
  const send = async (username, remove) => {
    try {
      await api(`/api/projects/${p.id}/members`, { method: 'POST', json: { username, remove } });
      toast(t(remove ? 'member.removed' : 'member.added', { user: username }), 'ok');
      await reload();
    } catch (err) { alertBox(msg, el('p', { lang: 'en' }, String((err && err.message) || err)), 'bad'); }
  };
  const list = members.length
    ? el('ul', { class: 'il' }, members.map((m) => {
      const li = el('li', {}, el('span', { class: 'l1' }, mono(m)));
      if (admin) {
        const btn = el('button', { class: 'btn btn-text', type: 'button', 'aria-label': t('member.remove.aria', { user: m }) }, t('member.remove'));
        on(btn, 'click', () => {
          if (!btn.dataset.armed) {
            btn.dataset.armed = '1';
            btn.textContent = t('member.confirm', { user: m });
            const id = setTimeout(() => {
              timers.delete(id);
              if (btn.isConnected) { delete btn.dataset.armed; btn.textContent = t('member.remove'); }
            }, 5000);
            timers.add(id);
            return;
          }
          send(m, true);
        });
        li.append(' ', btn);
      }
      return li;
    }))
    : el('p', { class: 'sub' }, t('members.none'));
  let form = null;
  if (admin) {
    const candidates = users.filter((u) => u.active && !members.includes(u.username));
    const sel = el('select', { id: 'mem-user', name: 'u' },
      candidates.length
        ? candidates.map((u) => el('option', { value: u.username }, t('member.opt', { user: u.username, role: roleText(u.role) })))
        : el('option', { value: '' }, t('member.all')));
    const go = el('button', { class: 'btn', type: 'submit', disabled: !candidates.length }, t('member.add'));
    form = el('form', { class: 'calc-form', id: 'mem', novalidate: true },
      el('div', { class: 'field' }, el('label', { for: 'mem-user' }, t('member.add.label')), sel), msg, go);
    on(form, 'submit', (e) => { e.preventDefault(); if (sel.value) send(sel.value, false); });
  }
  return panel(t('project.access'), frag(list, el('p', { class: 'sub' }, t('members.hint')), form || msg), { count: members.length });
}

// ---------------------------------------------------------------- render
function render() {
  const { p, users } = data;
  root.textContent = '';
  const page = frag(
    header(p),
    jobsPanel(p),
    docsPanel(p),
    el('div', { id: 'convert-host' }),
    convPanel(p),
    membersPanel(p, users),
  );
  root.append(page);
  $$('[data-convert]', root).forEach((btn) => {
    const doc = (p.documents || []).find((d) => String(d.id) === btn.dataset.convert);
    if (doc) on(btn, 'click', () => openConvert(p, doc));
  });
}

// ---------------------------------------------------------------- mount / unmount
export async function mount(container, params = {}) {
  unmount();
  root = container;
  ac = new AbortController();
  projectId = String(params.id ?? params.projectId ?? '').trim();
  const my = ++seq;
  root.textContent = '';
  root.append(el('p', { class: 'sub' }, t('loading')));
  if (!/^\d+$/.test(projectId)) { renderError({ message: 'not found' }); return; }
  try {
    const d = await load(projectId);
    if (my !== seq || !root) return;
    data = d; render();
  } catch (err) { if (my === seq && root) renderError(err); }
}

export function unmount() {
  seq++;
  if (ac) { ac.abort(); ac = null; }
  for (const id of timers) clearTimeout(id);
  timers.clear();
  if (root) root.textContent = '';
  root = null;
  data = null;
  projectId = null;
}
