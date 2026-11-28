// NEXUS FABRIC inline question cards (pick 5B): a plain-words question, the place on the drawing, and 2-3 big buttons.
// Every card comes from a value the server holds as PROPOSED or MISSING, and every answer goes through the orders API
// (decisions, values). A card never answers for the person and offers no approve or release control.
import { api, esc, ApiError } from '../api.js';
import { t, idLabel, lang } from '../i18n.js';

const OPEN = ['PROPOSED', 'MISSING'];
const what = (key) => idLabel('param', key) || String(key).replaceAll('_', ' ');
const msgOf = (e) => (e instanceof ApiError ? e.message : String(e?.message || e));

/** Open questions of an order, from the order JSON: one per PROPOSED or MISSING value of a part that is not excluded. */
export function questionsOf(order, only) {
  const pages = new Map((order.pages || []).map((p) => [p.page_no, p]));
  const out = [];
  for (const part of order.parts || []) {
    if (part.state === 'EXCLUDED' || (only && part.no !== only)) continue;
    for (const v of part.values || []) {
      if (!OPEN.includes(v.state)) continue;
      const pg = pages.get(part.page) || null;
      out.push({ part: part.no, name: part.name, key: v.key, value: v.value, text: v.text, state: v.state, bbox: v.bbox, page: part.page, pageInfo: pg });
    }
  }
  return out;
}

/** Set the crop picture with the style API only (the page's CSP forbids style attributes in markup). */
export function paintCrop(box, oid, q) {
  const pg = q.pageInfo;
  if (!pg || !pg.has_image || !pg.width || !pg.height) { box.textContent = t('jf2.q.nocrop'); box.classList.add('qc-nocrop'); return; }
  const url = `/api/orders/${oid}/pages/${pg.page_no}`;
  box.style.backgroundImage = `url("${url}")`;
  const b = Array.isArray(q.bbox) && q.bbox.length === 4 ? q.bbox : null;
  if (!b) { box.style.backgroundSize = 'cover'; box.style.backgroundPosition = 'top center'; return; }
  const [x0, y0, x1, y1] = b;
  const padX = Math.max(60, (x1 - x0) * 1.2), padY = Math.max(40, (y1 - y0) * 1.2);
  const cx0 = Math.max(0, x0 - padX), cy0 = Math.max(0, y0 - padY);
  const cx1 = Math.min(pg.width, x1 + padX), cy1 = Math.min(pg.height, y1 + padY);
  const cw = cx1 - cx0, ch = cy1 - cy0;
  box.style.aspectRatio = `${cw} / ${ch}`;
  box.style.backgroundSize = `${(pg.width / cw) * 100}% ${(pg.height / ch) * 100}%`;
  box.style.backgroundPosition = `${pg.width > cw ? (cx0 / (pg.width - cw)) * 100 : 0}% ${pg.height > ch ? (cy0 / (pg.height - ch)) * 100 : 0}%`;
  const mark = document.createElement('i');
  mark.className = 'qc-mark'; mark.setAttribute('aria-hidden', 'true');
  mark.style.left = `${((x0 - cx0) / cw) * 100}%`; mark.style.top = `${((y0 - cy0) / ch) * 100}%`;
  mark.style.width = `${((x1 - x0) / cw) * 100}%`; mark.style.height = `${((y1 - y0) / ch) * 100}%`;
  box.append(mark);
}

/** Draw the cards into `host`. `onAnswered()` runs after the server took an answer. */
export function renderQuestions(host, oid, list, { onAnswered } = {}) {
  host.innerHTML = '';
  if (!list.length) {
    const p = document.createElement('p');
    p.className = 'jf1-note'; p.textContent = t('jf2.q.none');
    host.append(p);
    return;
  }
  for (const q of list) host.append(card(oid, q, onAnswered));
}

function card(oid, q, onAnswered) {
  const el = document.createElement('article');
  el.className = 'qc';
  el.dataset.part = q.part; el.dataset.key = q.key; el.dataset.state = q.state;
  const params = { part: q.part, name: q.name, what: what(q.key), value: q.value ?? '' };
  const text = q.state === 'PROPOSED' && q.value != null ? t('jf2.q.read', params) : t('jf2.q.missing', params);
  el.innerHTML = `<div class="qc-crop" role="img" aria-label="${esc(t('jf2.q.crop'))}"></div>
    <div class="qc-body">
      <p class="qc-q" lang="${lang()}">${esc(text)}</p>
      <div class="qc-btns">
        ${q.state === 'PROPOSED' && q.value != null ? `<button type="button" class="qc-btn qc-primary" data-qa="yes">${esc(t('jf2.q.yes'))}</button>` : ''}
        <button type="button" class="qc-btn" data-qa="type">${esc(t(q.state === 'PROPOSED' ? 'jf2.q.no' : 'jf2.q.save'))}</button>
        <button type="button" class="qc-btn" data-qa="hand">${esc(t('jf2.q.hand'))}</button>
      </div>
      <form class="qc-form" hidden>
        <label>${esc(t('jf2.q.number'))} <input type="text" inputmode="decimal" class="qc-num mono" autocomplete="off"></label>
        <button type="submit" class="qc-btn qc-primary">${esc(t('jf2.q.save'))}</button>
      </form>
      <p class="qc-msg" role="status"></p>
    </div>`;
  paintCrop(el.querySelector('.qc-crop'), oid, q);
  const msg = el.querySelector('.qc-msg'), form = el.querySelector('.qc-form'), num = el.querySelector('.qc-num');
  const say = (s, bad) => { msg.textContent = s; msg.classList.toggle('qc-bad', !!bad); };
  const done = async () => { say(t('jf2.q.done')); if (onAnswered) await onAnswered(q); };
  const run = async (fn) => {
    el.querySelectorAll('button').forEach((b) => { b.disabled = true; });
    try { await fn(); } catch (e) { say(msgOf(e), true); el.querySelectorAll('button').forEach((b) => { b.disabled = false; }); }
  };
  el.addEventListener('click', (ev) => {
    const b = ev.target.closest('[data-qa]');
    if (!b) return;
    const a = b.dataset.qa;
    if (a === 'type') { form.hidden = false; num.focus(); }
    if (a === 'yes') run(async () => { await api(`/api/orders/${oid}/decisions`, { json: { kind: 'confirm', part: q.part, key: q.key, reason: 'confirmed on the question card' } }); await done(); });
    if (a === 'hand') run(async () => { await api(`/api/orders/${oid}/decisions`, { json: { kind: 'hand_drawn', part: q.part, reason: `marked on the question card: part ${q.part} ${q.key} is done by hand` } }); await done(); });
  });
  form.addEventListener('submit', (ev) => {
    ev.preventDefault();
    const v = Number(String(num.value).trim().replace(/,/g, ''));
    if (!String(num.value).trim() || !Number.isFinite(v)) return say(t('jf2.q.badnum'), true);
    run(async () => {
      await api(`/api/orders/${oid}/values`, { method: 'PUT', json: { values: [{ part: q.part, key: q.key, value: v, text: String(num.value).trim(), source: 'entered', confirm: true, note: 'typed on the question card' }] } });
      await done();
    });
  });
  return el;
}
