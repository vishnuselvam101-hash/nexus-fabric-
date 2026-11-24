// NEXUS FABRIC flagship2 — film engine. Everything is a pure function of time: no clock, no random state.
// Parts: easing and springs, keyed tracks, the footage player (real captured frames, tiles put back on their
// grid), the product-window camera, the pointer overlay.
'use strict';
const NF = (window.NFE = {});

// ---------------------------------------------------------------- maths
const clamp = (x, a = 0, b = 1) => (x < a ? a : x > b ? b : x);
const lerp = (a, b, u) => a + (b - a) * u;
const seg = (t, a, b) => clamp((t - a) / (b - a));
const E = {
  lin: (u) => u,
  i2: (u) => u * u, o2: (u) => 1 - (1 - u) * (1 - u), io2: (u) => (u < 0.5 ? 2 * u * u : 1 - Math.pow(-2 * u + 2, 2) / 2),
  i3: (u) => u * u * u, o3: (u) => 1 - Math.pow(1 - u, 3), io3: (u) => (u < 0.5 ? 4 * u * u * u : 1 - Math.pow(-2 * u + 2, 3) / 2),
  o4: (u) => 1 - Math.pow(1 - u, 4), io4: (u) => (u < 0.5 ? 8 * u * u * u * u : 1 - Math.pow(-2 * u + 2, 4) / 2),
  o5: (u) => 1 - Math.pow(1 - u, 5), io5: (u) => (u < 0.5 ? 16 * u * u * u * u * u : 1 - Math.pow(-2 * u + 2, 5) / 2),
  oExpo: (u) => (u >= 1 ? 1 : 1 - Math.pow(2, -10 * u)), iExpo: (u) => (u <= 0 ? 0 : Math.pow(2, 10 * u - 10)),
  ioSine: (u) => -(Math.cos(Math.PI * u) - 1) / 2,
};
// unit step response of a damped spring: 0 at tau<=0, settles at 1. w = natural frequency (rad/s), z = damping ratio.
function spring(tau, w = 7, z = 1) {
  if (tau <= 0) return 0;
  if (z >= 1) return 1 - Math.exp(-w * tau) * (1 + w * tau);
  const wd = w * Math.sqrt(1 - z * z);
  return 1 - Math.exp(-z * w * tau) * (Math.cos(wd * tau) + ((z * w) / wd) * Math.sin(wd * tau));
}
// A spring track: keys [{t, w, zeta, ...values}] (zeta = damping ratio; z is an ordinary value, e.g. zoom). The value is the first key plus, for each later key, its change
// times the spring response since that key's time. Motion therefore always starts from rest, carries momentum
// through overlapping keys, and is a closed-form function of t.
function springTrack(keys, logKeys = []) {
  const names = Object.keys(keys[0]).filter((k) => !['t', 'w', 'zeta'].includes(k));
  const val = (k, n) => (logKeys.includes(n) ? Math.log(k[n]) : k[n]);
  return (t) => {
    const o = {};
    for (const n of names) {
      let v = val(keys[0], n), prev = v;
      for (let i = 1; i < keys.length; i++) {
        const k = keys[i]; if (t <= k.t) break;
        const target = k[n] === undefined ? prev : val(k, n);
        v += (target - prev) * spring(t - k.t, k.w ?? 6, k.zeta ?? 1); prev = target;
      }
      o[n] = logKeys.includes(n) ? Math.exp(v) : v;
    }
    return o;
  };
}
// keyed scalar with eased segments: keys [[t, v, ease?], ...]; the ease named on a key shapes the segment that ends at it
function track(keys) {
  return (t) => {
    if (t <= keys[0][0]) return keys[0][1];
    for (let i = 1; i < keys.length; i++) if (t <= keys[i][0]) { const a = keys[i - 1], b = keys[i]; return lerp(a[1], b[1], (E[b[2]] || E.io3)(seg(t, a[0], b[0]))); }
    return keys[keys.length - 1][1];
  };
}
// seeded noise (for static dither and gentle drift): deterministic
function mulberry(a) { return () => { a |= 0; a = (a + 0x6d2b79f5) | 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }
Object.assign(NF, { clamp, lerp, seg, E, spring, springTrack, track, mulberry });

// ---------------------------------------------------------------- footage
// window.NF_FOOTAGE[lang] comes from capture/footage/<lang>/footage.js (written by capture/pack.py).
class Footage {
  constructor(data, dir) {
    this.d = data; this.dir = dir; this.TS = data.tile; this.GX = data.grid[0]; this.GY = data.grid[1]; this.per = data.atlas * data.atlas;
    this.W = this.GX * this.TS; this.H = this.GY * this.TS; this.scale = data.dsf;
    this.full = {}; this.rects = {};
    for (const [name, tk] of Object.entries(data.takes)) {
      const n = this.GX * this.GY, rows = []; let cur = null;
      for (const row of tk.tiles) { if (!cur) cur = Int32Array.from(row); else { cur = cur.slice(); for (let k = 0; k < row.length; k += 2) cur[row[k]] = row[k + 1]; } rows.push(cur); }
      this.full[name] = rows;
      const rr = []; let last = null; for (const r of tk.r) { if (r) last = r; rr.push(last); } this.rects[name] = rr;
    }
    this.cv = document.createElement('canvas'); this.cv.width = this.W; this.cv.height = this.H; this.ctx = this.cv.getContext('2d', { alpha: false });
    this.cur = new Int32Array(this.GX * this.GY).fill(-1); this.shown = null; this.version = 0;
  }
  load() {
    this.atlas = [];
    return Promise.all(Array.from({ length: this.d.atlases }, (_, k) => new Promise((res, rej) => { const im = new Image(); im.onload = () => (im.decode ? im.decode().then(res, res) : res()); im.onerror = () => rej(new Error('atlas ' + k)); im.src = `${this.dir}/a${k}.${this.d.fmt}`; this.atlas[k] = im; })));
  }
  take(name) { return this.d.takes[name]; }
  mark(name, m) { // 'mark', 'mark+3', or a number
    if (typeof m === 'number') return m;
    const [k, off] = String(m).split(/(?=[+-]\d+$)/); const tk = this.d.takes[name];
    const base = k === 'end' ? tk.n - 1 : tk.marks[k]; if (base === undefined) return 0;
    return clamp(base + (off ? Number(off) : 0), 0, tk.n - 1);
  }
  draw(name, f) { // put frame f of a take on the canvas; only tiles that differ from what is shown are drawn
    const rows = this.full[name]; if (!rows) return false; f = clamp(Math.round(f), 0, rows.length - 1);
    const ids = rows[f], TS = this.TS, A = this.d.atlas; let changed = false;
    for (let g = 0; g < ids.length; g++) {
      const id = ids[g]; if (id === this.cur[g]) continue;
      const j = id % this.per, im = this.atlas[(id / this.per) | 0];
      this.ctx.drawImage(im, (j % A) * TS, ((j / A) | 0) * TS, TS, TS, (g % this.GX) * TS, ((g / this.GX) | 0) * TS, TS, TS);
      this.cur[g] = id; changed = true;
    }
    if (changed) this.version++;
    this.shown = [name, f]; return changed;
  }
  rect(name, f, key) { const tk = this.d.takes[name]; if (!tk) return null; f = clamp(Math.round(f), 0, tk.n - 1); const i = tk.keys.indexOf(key); const r = this.rects[name][f]; return i < 0 || !r ? null : r[i]; }
  centre(name, f, key, fx = 0.5, fy = 0.5) { const r = this.rect(name, f, key); return r ? [r[0] + r[2] * fx, r[1] + r[3] * fy] : [960, 540]; }
  pointer(name, ff) { // interpolated pointer state for a fractional frame
    const tk = this.d.takes[name]; const f0 = clamp(Math.floor(ff), 0, tk.n - 1), f1 = Math.min(tk.n - 1, f0 + 1), u = clamp(ff - f0);
    const a = tk.c[f0], b = tk.c[f1]; return { x: lerp(a[0], b[0], u), y: lerp(a[1], b[1], u), down: a[2], cursor: a[3] };
  }
  view(name, f) { const tk = this.d.takes[name]; f = clamp(Math.round(f), 0, tk.n - 1); return tk.v[f]; }           // [s, tx, ty, pvx, pvy, pvw, pvh]
  readout(name, f) { const tk = this.d.takes[name]; f = clamp(Math.round(f), 0, tk.n - 1); return tk.ro[f]; }
  // pattern-space point (mm) -> application pixel, using the viewer transform recorded with that frame
  toApp(name, f, x, y) { const v = this.view(name, f); return v ? [v[3] + v[1] + x * v[0], v[4] + v[2] - y * v[0]] : [0, 0]; }
}
NF.Footage = Footage;

// A reel maps film time to (take, frame). Segments: {t0, t1, take, a, b} play frames a..b linearly over t0..t1
// (a == b holds one frame). Between segments the last frame holds.
class Reel {
  constructor(foot) { this.foot = foot; this.segs = []; }
  // play(t0, take, from, to, rate): rate 1 = the capture's own 30 frames per second
  play(t0, take, from, to, rate = 1) { const a = this.foot.mark(take, from), b = this.foot.mark(take, to); const t1 = t0 + Math.abs(b - a) / (30 * rate); this.segs.push({ t0, t1, take, a, b }); return t1; }
  fit(t0, t1, take, from, to) { const a = this.foot.mark(take, from), b = this.foot.mark(take, to); this.segs.push({ t0, t1, take, a, b }); return t1; }
  hold(t0, t1, take, at) { const a = this.foot.mark(take, at); this.segs.push({ t0, t1, take, a, b: a }); return t1; }
  at(t) { // -> {take, f (fractional)}
    let s = this.segs[0];
    for (const q of this.segs) { if (q.t0 <= t) s = q; else break; }
    const u = s.t1 > s.t0 ? seg(t, s.t0, s.t1) : 1; return { take: s.take, f: lerp(s.a, s.b, u), seg: s };
  }
  // film time at which a mark of a take is reached inside the segment that contains it
  when(take, m) { const f = this.foot.mark(take, m); for (const s of this.segs) if (s.take === take && f >= Math.min(s.a, s.b) && f <= Math.max(s.a, s.b)) return s.a === s.b ? s.t0 : lerp(s.t0, s.t1, (f - s.a) / (s.b - s.a)); return null; }
}
NF.Reel = Reel;

// ---------------------------------------------------------------- the product window
// Camera: the application point (fx, fy) sits at the screen point (sx, sy); z = film px per application px;
// rx, ry, rz in degrees turn the slab about that point; persp = lens distance in px.
function rigTransform(c) {
  return `translate(${c.sx.toFixed(3)}px,${c.sy.toFixed(3)}px) perspective(${c.persp || 2600}px) rotateX(${(c.rx || 0).toFixed(4)}deg) rotateY(${(c.ry || 0).toFixed(4)}deg) rotateZ(${(c.rz || 0).toFixed(4)}deg) scale(${c.z.toFixed(5)}) translate(${(-c.fx).toFixed(3)}px,${(-c.fy).toFixed(3)}px)`;
}
// screen position of an application point when the slab is not rotated (used for match cuts)
const flatProject = (c, ax, ay) => [c.sx + c.z * (ax - c.fx), c.sy + c.z * (ay - c.fy)];
NF.rigTransform = rigTransform; NF.flatProject = flatProject;

const CURSORS = {
  arrow: '<path d="M3 2v19l5.2-4.9 3.3 7.6 3.2-1.4-3.3-7.5H18z" fill="#fff" stroke="#0b0d11" stroke-width="1.6" stroke-linejoin="round"/>',
  hand: '<path d="M9.5 3.2c1.2 0 2 .9 2 2v6.1l.9-.1c.9-.9 2.5-.7 3 .4.9-.7 2.5-.3 2.8.9 1.200-.5 2.600.3 2.600 1.800v4.900c0 3.800-2.700 6.600-6.300 6.600h-1.400c-2.400 0-4-1-5.300-3l-4.400-6.500c-.7-1 .5-2.500 1.900-1.900l2.200 1.300V5.200c0-1.100.8-2 2-2z" fill="#fff" stroke="#0b0d11" stroke-width="1.5" stroke-linejoin="round"/>',
  text: '<path d="M8 3c2 0 3 .8 3.500 1.900C12 3.800 13 3 15 3M8 24c2 0 3-.8 3.500-1.900.5 1.100 1.500 1.900 3.500 1.900M11.500 4.900v17.200" fill="none" stroke="#0b0d11" stroke-width="3.200" stroke-linecap="round"/><path d="M8 3c2 0 3 .8 3.500 1.900C12 3.800 13 3 15 3M8 24c2 0 3-.8 3.500-1.900.5 1.100 1.500 1.900 3.500 1.900M11.500 4.900v17.200" fill="none" stroke="#fff" stroke-width="1.400" stroke-linecap="round"/>',
  cross: '<path d="M12 2v20M2 12h20" stroke="#0b0d11" stroke-width="3.400" stroke-linecap="round"/><path d="M12 2v20M2 12h20" stroke="#fff" stroke-width="1.400" stroke-linecap="round"/>',
};
const HOT = { arrow: [3, 2], hand: [9.5, 3.5], text: [11.5, 13.5], cross: [12, 12] };
const cursorKind = (css) => (/pointer/.test(css) ? 'hand' : /text/.test(css) ? 'text' : /crosshair|cell/.test(css) ? 'cross' : 'arrow');
NF.cursorSvg = (kind) => `<svg viewBox="0 0 26 28" width="26" height="28" style="overflow:visible;filter:drop-shadow(0 1.5px 2px rgba(0,0,0,.55))">${CURSORS[kind]}</svg>`;
NF.cursorKind = cursorKind; NF.cursorHot = HOT;
