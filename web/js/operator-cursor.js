// NEXUS FABRIC — the AI operator's cursor. ES module, no dependencies, no network except the optional follow().
//
// It replays operator events ({seq, step, target, action, value, reason, confidence, flags}) as a cursor that moves to
// the number on the drawing page, a UI element or a CAD canvas point, clicks, types a value and says why.
// Style-neutral: every colour and size is a CSS variable (--op-*) with a fallback to the page theme, so the look can
// follow the chosen mockup without touching this file. Nothing here writes to the server, approves or releases.
//
// The event target is one of
//   { page, bbox? }            a box on a page picture (page pixels); the page is found by [data-op-page="N"]
//   { ui }                     an element id, or [data-op-ui="id"]
//   { cad: { canvas?, entity? } }  a CAD canvas point and/or an entity id (resolved by the host: options.cad)
// An action word this file does not know is shown as a plain pointing step, so new tool kinds need no change here.

import { t } from './i18n.js';

const KNOWN = ['look', 'point', 'click', 'type', 'compute', 'ask'];
const prefersReduced = () => typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const el = (tag, cls, parent) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (parent) parent.append(e);
  return e;
};

/** Screen rectangle of an event target, or null when the host has nothing to show for it. */
export function resolveTarget(target, opts = {}) {
  if (!target) return null;
  if (target.ui) {
    const e = document.getElementById(target.ui) || document.querySelector(`[data-op-ui="${CSS.escape(target.ui)}"]`);
    return e ? e.getBoundingClientRect() : null;
  }
  if (target.page != null) {
    const img = document.querySelector(`[data-op-page="${CSS.escape(String(target.page))}"]`);
    if (!img) return null;
    const r = img.getBoundingClientRect();
    if (!target.bbox) return r;
    // page pixels: stated by the host (data-op-width/height) or the picture's own size
    const nw = Number(img.dataset.opWidth) || img.naturalWidth || r.width;
    const nh = Number(img.dataset.opHeight) || img.naturalHeight || r.height;
    const [x0, y0, x1, y1] = target.bbox;
    const sx = r.width / nw, sy = r.height / nh;
    return new DOMRect(r.left + x0 * sx, r.top + y0 * sy, (x1 - x0) * sx, (y1 - y0) * sy);
  }
  if (target.cad) {
    if (typeof opts.cad === 'function') return opts.cad(target.cad);
    const c = document.querySelector('[data-op-cad]');
    return c ? c.getBoundingClientRect() : null;
  }
  return null;
}

/**
 * The overlay. `options.cad(target.cad) -> DOMRect|null` resolves CAD targets; `options.root` is where the layer is
 * added (default document.body); `options.reduced` forces reduced motion (default: the system setting).
 */
export function createCursor(options = {}) {
  const root = options.root || document.body;
  const reduced = options.reduced ?? prefersReduced();
  const layer = el('div', 'op-layer', root);
  layer.setAttribute('aria-live', 'polite');
  layer.setAttribute('role', 'status');
  layer.dataset.reduced = reduced ? '1' : '0';
  const cursor = el('div', 'op-cursor', layer);
  cursor.setAttribute('aria-hidden', 'true');
  cursor.innerHTML = '<svg viewBox="0 0 16 16" width="20" height="20" focusable="false"><path d="M2 1l11 6.2-4.9 1.3L6.4 13.6z" fill="currentColor" stroke="var(--op-outline, #fff)" stroke-width="1"/></svg>';
  const ripple = el('div', 'op-ripple', layer);
  const chip = el('div', 'op-chip', layer);
  const tip = el('div', 'op-tip', layer);
  const who = el('span', 'op-who', tip);
  who.textContent = t('op.who');
  const act = el('span', 'op-act', tip);
  const why = el('span', 'op-why', tip);
  const meta = el('span', 'op-meta', tip);
  const badge = el('div', 'op-badge', layer);
  badge.hidden = true;
  badge.textContent = t('op.paused');

  let stopped = false, paused = false, seen = 0, lastEvent = null;
  const waiters = [];
  const waitFree = async () => { while (paused && !stopped) await new Promise((r) => waiters.push(r)); };

  const place = (node, x, y) => {
    node.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)`;
  };

  async function show(ev) {
    lastEvent = ev;
    const action = KNOWN.includes(ev.action) ? ev.action : 'point';
    layer.dataset.action = action;
    layer.classList.toggle('op-flagged', Array.isArray(ev.flags) && ev.flags.length > 0);
    layer.classList.toggle('op-ask', action === 'ask');
    act.textContent = t(`op.action.${action}`);
    why.textContent = String(ev.reason || '');
    const bits = [];
    if (typeof ev.confidence === 'number') bits.push(`${t('op.confidence')} ${Math.round(ev.confidence * 100)}%`);
    if (ev.flags && ev.flags.length) bits.push(`${t('op.flagged')}: ${ev.flags.join(', ')}`);
    meta.textContent = bits.join(' · ');
    const rect = resolveTarget(ev.target, options);
    const vw = window.innerWidth, vh = window.innerHeight;
    // a target the page does not show: stay where we are and say so
    const cx = rect ? rect.left + rect.width / 2 : vw / 2;
    const cy = rect ? rect.top + rect.height / 2 : vh - 60;
    layer.dataset.target = rect ? '1' : '0';
    cursor.style.transitionDuration = reduced ? '0ms' : '';
    place(cursor, cx, cy);
    place(chip, Math.min(cx + 14, vw - 160), Math.min(cy + 22, vh - 40));
    place(tip, Math.max(8, Math.min(cx + 14, vw - 330)), Math.max(8, Math.min(cy - 64, vh - 110)));
    layer.classList.add('op-on');
    chip.textContent = '';
    chip.hidden = true;
    if (!reduced) await sleep(380);
    if (stopped) return;
    if (action === 'click' || action === 'compute') {
      place(ripple, cx, cy);
      ripple.classList.remove('op-go');
      void ripple.offsetWidth;
      ripple.classList.add('op-go');
    }
    if (action === 'type' || (action === 'point' && ev.value != null && typeof ev.value !== 'object')) {
      const text = typeof ev.value === 'object' ? JSON.stringify(ev.value) : String(ev.value ?? '');
      chip.hidden = text === '';
      if (reduced) chip.textContent = text;
      else for (const ch of text) { if (stopped) return; chip.textContent += ch; await sleep(35); }
    }
    await sleep(reduced ? 60 : action === 'ask' ? 900 : 320);
    seen = Math.max(seen, ev.seq || 0);
  }

  const api = {
    layer,
    /** Replay events in order; resolves when the last one has been shown. Honors pause() and stop(). */
    async play(events) {
      stopped = false;
      for (const ev of events) {
        await waitFree();
        if (stopped) break;
        await show(ev);
        if (typeof options.onEvent === 'function') options.onEvent(ev);
      }
    },
    pause() { paused = true; badge.hidden = false; },
    resume() { paused = false; badge.hidden = true; waiters.splice(0).forEach((r) => r()); },
    stop() { stopped = true; api.resume(); },
    hide() { layer.classList.remove('op-on'); },
    get lastSeq() { return seen; },
    get last() { return lastEvent; },
    destroy() { stopped = true; layer.remove(); },
    /**
     * Follow a running order: poll the server's event list and show new events as they come. The operator's pause
     * (set by a person) is shown as a badge. Returns a function that stops following.
     */
    follow(orderId, { interval = 1000, fetchImpl = fetch } = {}) {
      let after = 0, live = true, busy = Promise.resolve();
      const tick = async () => {
        if (!live) return;
        try {
          const r = await fetchImpl(`/api/orders/${encodeURIComponent(orderId)}/operator/events?after=${after}`, { credentials: 'same-origin', headers: { 'X-NF-Request': '1' } });
          if (r.ok) {
            const d = await r.json();
            if (d.paused) api.pause(); else if (paused) api.resume();
            const fresh = (d.events || []).filter((e) => e.seq > after);
            if (fresh.length) {
              after = fresh[fresh.length - 1].seq;
              busy = busy.then(() => api.play(fresh));
            }
          }
        } catch { /* the next tick tries again */ }
        if (live) setTimeout(tick, interval);
      };
      tick();
      return () => { live = false; };
    },
  };
  return api;
}
