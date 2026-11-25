/* NEXUS FABRIC — motion helpers: auto quality, stagger indices, count-up numbers.
   Quality is decided once per page: weak machines (<= 4 GB or <= 4 threads), reduced-motion requests and
   missing WebGL switch off the blur, the spinning edges and the background drift (class fx-off / no-gl on <html>).
   Counts show their final value at once when motion is reduced. Nothing here fetches or invents data. */

const reduce = typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
let quality = 'high';

export function webglAvailable() {
  try {
    const c = document.createElement('canvas');
    return !!(c.getContext('webgl2') || c.getContext('webgl'));
  } catch { return false; }
}

export function initFx() {
  const root = document.documentElement;
  const weak = (navigator.deviceMemory && navigator.deviceMemory <= 4) || (navigator.hardwareConcurrency && navigator.hardwareConcurrency <= 4);
  quality = reduce || weak ? 'low' : 'high';
  root.classList.toggle('fx-off', quality === 'low');
  if (!webglAvailable()) root.classList.add('no-gl');
  return quality;
}

export const fxQuality = () => quality;
export const reducedMotion = () => reduce;

/* count-up: el.textContent animates from 0 (or the current value) to target, 400–900 ms, ease-out. */
export function countUp(el, to, { dec = 0, duration = 700 } = {}) {
  const fmt = (v) => v.toLocaleString(document.documentElement.lang === 'ja' ? 'ja-JP' : 'en-US', { minimumFractionDigits: dec, maximumFractionDigits: dec });
  if (reduce || !Number.isFinite(to)) { el.textContent = fmt(to); return; }
  const t0 = performance.now();
  const step = (now) => {
    const k = Math.min(1, (now - t0) / duration);
    const e = 1 - Math.pow(1 - k, 3);
    el.textContent = fmt(to * e);
    if (k < 1) requestAnimationFrame(step); else el.textContent = fmt(to);
  };
  requestAnimationFrame(step);
}

/* call after a view is mounted: stagger indices for [data-stagger] children, count-ups for [data-count] */
export function enterMotion(root) {
  for (const group of root.querySelectorAll('[data-stagger]')) {
    [...group.children].forEach((child, i) => child.style.setProperty('--i', Math.min(i, 12)));
  }
  for (const el of root.querySelectorAll('[data-count]')) {
    const to = Number(el.dataset.count);
    countUp(el, to, { dec: Number(el.dataset.dec || 0) });
  }
}
