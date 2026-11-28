// NEXUS FABRIC job, simple view (lane F1): 1 drop PDFs, 2 AMOEBA works, 3 result and download.
// Wired to the real orders API. Shows only what the server returns. A part without a DXF is never shown as ready,
// and this page offers no approve or release control.
import { api, uploadFile, esc, ApiError } from '../api.js';
import { t } from '../i18n.js';
import { createCursor } from '../operator-cursor.js';

const PROJECT = 'Quick conversions';
const ACCEPT = '.pdf,.png,.jpg,.jpeg,.tif,.tiff';
// Ready = computed AND certified with a DXF allowed and no block (the ZIP's DXF rule); a computed but blocked part is not ready
const isReady = (p) => p.state === 'computed' && p.dxf_allowed === true && Array.isArray(p.dxf_blocked_by) && p.dxf_blocked_by.length === 0;
const NEED = ['held_missing', 'refused', 'not_computed'];

let host = null, root = null, cursor = null, stopFollow = null, busy = false, oid = null, files = [];

const $ = (sel) => root.querySelector(sel);
const msgOf = (e) => (e instanceof ApiError ? e.message : (e?.message || 'Unexpected error'));

export async function mount(viewEl, params = {}) {
  host = viewEl;
  oid = params.oid ? Number(params.oid) : null;
  files = [];
  root = document.createElement('section');
  root.className = 'jf1';
  root.setAttribute('aria-labelledby', 'jf1-title');
  host.append(root);
  await render();
}

export function unmount() {
  if (stopFollow) stopFollow();
  if (cursor) cursor.destroy();
  stopFollow = null; cursor = null;
  root?.remove();
  root = null; host = null;
}

const steps = (cur) => [1, 2, 3].map((n) => {
  const state = n < cur ? 'done' : n === cur ? 'current' : 'todo';
  const label = t(['jf1.step1', 'jf1.step2', 'jf1.step3'][n - 1]);
  return `<li class="jf1-step is-${state}" ${n === cur ? 'aria-current="step"' : ''}><span class="jf1-num">${n}</span><span>${esc(label)}</span></li>`;
}).join('');

async function render() {
  if (!root) return;
  if (!oid) return renderIntake();
  let order;
  try { order = await api(`/api/orders/${oid}`); } catch (e) { return renderError(e); }
  const parts = order.parts || [];
  const pages = order.pages || [];
  files = order.files || [];
  // the result is the latest compute (the order's own part state is not updated by a compute)
  const results = await api(`/api/orders/${oid}/compute`);
  const resParts = results?.current?.result?.parts || [];
  const cur = resParts.length ? 3 : 2;
  root.innerHTML = `
    <header class="jf1-head">
      <p class="jf1-kicker mono">${esc(order.name || '')}</p>
      <h1 id="jf1-title">${esc(t('jf1.title'))}</h1>
      <ol class="jf1-steps" aria-label="${esc(t('jf1.progress'))}">${steps(cur)}</ol>
    </header>
    <div class="jf1-panel" data-cur="${cur}">
      ${cur === 3 ? resultHtml(resParts) : ''}
      ${pagesHtml(pages)}
      <div class="jf1-actions">
        <button type="button" class="jf1-btn jf1-primary" data-act="calc" ${busy || !parts.length ? 'disabled' : ''}>${esc(busy ? t('jf1.calcing') : t('jf1.calc'))}</button>
        <button type="button" class="jf1-btn" data-act="new">${esc(t('jf1.restart'))}</button>
      </div>
      ${parts.length ? '' : `<p class="jf1-note">${esc(t('jf1.noParts'))}</p>`}
      <section class="jf1-feed" aria-labelledby="jf1-feed-h">
        <h2 id="jf1-feed-h">${esc(t('jf1.feed'))}</h2>
        <div class="jf1-stage" data-stage><p class="jf1-note" data-none>${esc(t('jf1.feed.none'))}</p></div>
        <ol class="jf1-log" data-log aria-live="polite"></ol>
      </section>
      <div class="jf1-download">
        ${resParts.length ? `<a class="jf1-btn jf1-primary" id="jf1-outputs" href="/api/orders/${oid}/outputs.zip" download>${esc(t('jf1.download'))}</a>` : `<button type="button" class="jf1-btn" disabled>${esc(t('jf1.download'))}</button>`}
        <p class="jf1-err" data-dl-err role="alert" hidden></p>
        <p class="jf1-note">${esc(t('jf1.download.note'))}</p>
      </div>
    </div>`;
  root.querySelector('[data-act="calc"]')?.addEventListener('click', compute);
  root.querySelector('[data-act="new"]')?.addEventListener('click', () => { location.hash = '#/start'; });
  root.querySelector('#jf1-outputs')?.addEventListener('click', download);
  follow();
}

function renderIntake() {
  root.innerHTML = `
    <header class="jf1-head">
      <p class="jf1-kicker mono">01 / 03</p>
      <h1 id="jf1-title">${esc(t('jf1.title'))}</h1>
      <p class="jf1-lead">${esc(t('jf1.lead'))}</p>
      <ol class="jf1-steps" aria-label="${esc(t('jf1.progress'))}">${steps(1)}</ol>
    </header>
    <div class="jf1-panel" data-cur="1">
      <form class="jf1-drop" data-drop>
        <input type="file" id="jf1-files" class="jf1-file" multiple accept="${ACCEPT}" aria-label="${esc(t('jf1.drop'))}">
        <label for="jf1-files" class="jf1-dz" tabindex="0">
          <span class="jf1-dz-main">${esc(t('jf1.drop'))}</span>
          <span class="jf1-note">${esc(t('jf1.drop.hint'))}</span>
        </label>
        <ul class="jf1-list" data-picked aria-live="polite"></ul>
        <p class="jf1-err" data-err role="alert" hidden></p>
        <div class="jf1-actions">
          <button type="submit" class="jf1-btn jf1-primary" data-start disabled>${esc(t('jf1.start'))}</button>
        </div>
      </form>
    </div>`;
  const input = $('#jf1-files');
  const picked = $('[data-picked]');
  const start = $('[data-start]');
  const dz = $('.jf1-dz');
  let chosen = [];
  const choose = (list) => {
    chosen = [...list].filter((f) => /\.(pdf|png|jpe?g|tiff?)$/i.test(f.name));
    picked.innerHTML = chosen.map((f) => `<li class="mono">${esc(f.name)}</li>`).join('');
    start.disabled = !chosen.length;
  };
  input.addEventListener('change', () => choose(input.files));
  dz.addEventListener('dragover', (e) => { e.preventDefault(); dz.classList.add('is-over'); });
  dz.addEventListener('dragleave', () => dz.classList.remove('is-over'));
  dz.addEventListener('drop', (e) => { e.preventDefault(); dz.classList.remove('is-over'); choose(e.dataTransfer?.files || []); });
  $('[data-drop]').addEventListener('submit', async (ev) => {
    ev.preventDefault();
    if (busy || !chosen.length) return;
    busy = true; start.disabled = true; start.textContent = t('jf1.starting');
    try {
      const pid = await projectId();
      const made = await api(`/api/projects/${pid}/orders`, { json: { name: chosen[0].name.replace(/\.[^.]+$/, '').slice(0, 80) } });
      for (const f of chosen) await uploadFile(`/api/orders/${made.id}/files`, f);
      location.hash = `#/start/${made.id}`;
    } catch (e) {
      busy = false; start.disabled = false; start.textContent = t('jf1.start');
      const err = $('[data-err]'); err.hidden = false; err.textContent = t('jf1.err.upload', { msg: msgOf(e) });
    }
  });
}

async function projectId() {
  const res = await api('/api/projects');
  const list = Array.isArray(res?.projects) ? res.projects : Array.isArray(res) ? res : [];
  const found = list.find((p) => p?.name === PROJECT);
  if (found) return found.id;
  const created = await api('/api/projects', { json: { name: PROJECT } });
  return (created?.project ?? created).id;
}

function pagesHtml(pages) {
  if (!pages.length) return `<p class="jf1-note">${esc(t('jf1.noPages'))}</p>`;
  const names = new Map(files.map((f) => [f.id, f.filename]));
  return `<section class="jf1-pages" aria-labelledby="jf1-pages-h">
    <h2 id="jf1-pages-h">${esc(t('jf1.pages', { n: pages.length }))}</h2>
    <ol class="jf1-list">${pages.map((p) => `<li data-op-page="${p.page_no}"><span class="mono">${p.page_no}</span> ${esc(names.get(p.file_id) || '')} <span class="jf1-note">${esc(p.kind || '')}</span></li>`).join('')}</ol>
  </section>`;
}

function resultHtml(parts) {
  const count = (list) => parts.filter((p) => list.includes(p.state)).length;
  // held = every computed part that is not ready (blocked, no DXF); need = open questions. The three add up to the parts.
  const ready = parts.filter(isReady).length, need = count(NEED), held = parts.length - ready - need;
  return `<section class="jf1-result" aria-labelledby="jf1-res-h">
    <h2 id="jf1-res-h">${esc(t('jf1.res.summary', { ready, held, need }))}</h2>
    <dl class="jf1-counts">
      <div><dt>${esc(t('jf1.res.ready'))}</dt><dd class="mono">${ready}</dd></div>
      <div><dt>${esc(t('jf1.res.held'))}</dt><dd class="mono">${held}</dd></div>
      <div><dt>${esc(t('jf1.res.need'))}</dt><dd class="mono">${need}</dd></div>
    </dl>
  </section>`;
}

// Fetch the ZIP first: a refusal (409 stale result, 413 too large, licence) shows the server's message instead of saving a JSON file
async function download(ev) {
  ev.preventDefault();
  const a = ev.currentTarget, err = $('[data-dl-err]');
  err.hidden = true; err.textContent = '';
  try {
    const res = await fetch(a.getAttribute('href'), { headers: { 'X-NF-Request': '1' }, credentials: 'same-origin' });
    if (!res.ok) {
      let detail = {};
      try { detail = await res.json(); } catch { /* not JSON */ }
      throw new ApiError(detail.error || detail.message || res.statusText, { status: res.status, ...detail });
    }
    const blob = await res.blob();
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    const cd = res.headers.get('content-disposition') || '';
    const name = (cd.match(/filename="([^"]+)"/) || [])[1] || 'job-package.zip';
    link.href = url; link.download = name;
    document.body.append(link); link.click(); link.remove();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  } catch (e) {
    err.hidden = false; err.textContent = msgOf(e);
  }
}

async function compute() {
  if (busy) return;
  busy = true; setCalc(true);
  try {
    await api(`/api/orders/${oid}/compute`, { method: 'POST' });
    busy = false;
    await render();
  } catch (e) {
    busy = false; setCalc(false);
    const p = root.querySelector('.jf1-panel');
    const note = document.createElement('p');
    note.className = 'jf1-err'; note.setAttribute('role', 'alert');
    note.textContent = e?.code === 'order_empty' ? t('jf1.noParts') : msgOf(e);
    p.prepend(note);
  }
}

function setCalc(on) {
  const b = root?.querySelector('[data-act="calc"]');
  if (b) { b.disabled = on; b.textContent = on ? t('jf1.calcing') : t('jf1.calc'); }
}

// AMOEBA's events: the cursor shows them on the page; the log lists the same reasons in words.
function follow() {
  if (!root) return;
  const stage = root.querySelector('[data-stage]');
  const log = root.querySelector('[data-log]');
  cursor = createCursor({
    root: stage,
    onEvent(ev) {
      root?.querySelector('[data-none]')?.remove();
      const li = document.createElement('li');
      li.textContent = `${ev.action || ''} ${ev.reason || ''}`.trim();
      log?.append(li);
    },
  });
  stopFollow = cursor.follow(oid);
}

function renderError(e) {
  root.innerHTML = `<div class="error-box"><h2>${esc(t('jf1.err.load'))}</h2><p>${esc(msgOf(e))}</p></div>`;
}

