// Film 4 overlay: type, labels, callouts and panels drawn on the 2D canvas over the 3D layer.
// Three type sizes only: HS 132 (headline), BS 48 (body), LS 40 (label). Nothing meant to be read is smaller.
'use strict';
export const HS = 132, BS = 48, LS = 40;
export const COL = {
  paper: '#f4f5f7', ink: '#1d1d1f', ink2: '#3a3a3e', ink3: '#5a5a60', line: '#1d1d1f',
  blue: '#0a66ff', blueT: '#0057d9', red: '#e5241b', amber: '#ff9f0a', amberT: '#a85f00', green: '#1fa463', greenT: '#137a43',
  white: '#ffffff',
};
export const FONT = "'Inter Variable', 'Noto Sans JP', sans-serif";
export const FONT_JA = "'Inter Variable', 'Noto Sans JP', sans-serif";   // Latin in Inter, Japanese in Noto Sans JP
export const MONO = "'JetBrains Mono', 'Noto Sans JP', monospace";
export const clamp = (x, a = 0, b = 1) => Math.max(a, Math.min(b, x));
export const seg = (t, a, b) => clamp((t - a) / (b - a));
export const smooth = (u) => u * u * (3 - 2 * u);
export const quint = (u) => (u < 0.5 ? 16 * u ** 5 : 1 - (-2 * u + 2) ** 5 / 2);
export const outCubic = (u) => 1 - (1 - u) ** 3;
// damped spring step response, settled (within 0.2 %) at u = 1: zeta 0.72 (about 4 % overshoot)
export const spring = (u, zeta = 0.72) => {
  if (u <= 0) return 0; if (u >= 1) return 1;
  const w = 9.2, wd = w * Math.sqrt(1 - zeta * zeta), x = u;
  return 1 - Math.exp(-zeta * w * x) * (Math.cos(wd * x) + (zeta * w / wd) * Math.sin(wd * x));
};
export const lerp = (a, b, u) => a + (b - a) * u;

export class Ov {
  constructor(ctx) { this.c = ctx; this.lang = 'en'; }
  font(size, weight, mono, ja) { return `${weight} ${size}px ${mono ? MONO : (ja ? FONT_JA : FONT)}`; }
  tw(s, size, weight = 600, mono = false) { const c = this.c; c.font = this.font(size, weight, mono, this.lang === 'ja'); return c.measureText(s).width; }
  // text at baseline (x, y); o: size, weight, color, align, alpha, mono, track (letter spacing px)
  txt(s, x, y, o = {}) {
    const c = this.c; if (!s) return 0; const size = o.size || BS, wt = o.weight || (size >= HS ? 650 : 500);
    c.save(); c.globalAlpha *= o.alpha ?? 1; c.font = this.font(size, wt, o.mono, this.lang === 'ja'); c.fillStyle = o.color || COL.ink;
    c.textAlign = o.align || 'left'; c.textBaseline = 'alphabetic';
    if (o.track) c.letterSpacing = `${o.track}px`;
    c.fillText(s, x, y); const w = c.measureText(s).width; c.restore(); return w;
  }
  // a text block that enters with a 16 px spring rise and leaves with a fade (t, t0 enter, t1 leave)
  block(lines, x, y, lh, o, t, t0, t1) {
    if (t < t0 || t >= t1) return;
    // on its first frame an incoming block is already there at 60 % (no empty frame on a bar), fully there 4 frames later
    const u = spring(seg(t, t0, t0 + 0.45)), out = 1 - seg(t, t1 - 0.2, t1 - 0.05);
    const a = clamp((0.6 + 0.4 * seg(t, t0, t0 + 4 / 30)) * out);
    lines.forEach((ln, i) => {
      const ui = spring(seg(t, t0 + i * 0.06, t0 + i * 0.06 + 0.45));
      this.txt(ln, x, y + i * lh + 18 * (1 - ui), { ...o, alpha: (o.alpha ?? 1) * a });
    });
    return u;
  }
  line(pts, w, color, alpha = 1, dash) {
    const c = this.c; c.save(); c.globalAlpha *= alpha; c.strokeStyle = color; c.lineWidth = w; c.lineCap = 'round'; c.lineJoin = 'round';
    if (dash) c.setLineDash(dash); c.beginPath(); pts.forEach((p, i) => (i ? c.lineTo(p[0], p[1]) : c.moveTo(p[0], p[1]))); c.stroke(); c.restore();
  }
  dot(x, y, r, color, alpha = 1) { const c = this.c; c.save(); c.globalAlpha *= alpha; c.fillStyle = color; c.beginPath(); c.arc(x, y, r, 0, Math.PI * 2); c.fill(); c.restore(); }
  ring(x, y, r, w, color, alpha = 1) { const c = this.c; c.save(); c.globalAlpha *= alpha; c.strokeStyle = color; c.lineWidth = w; c.beginPath(); c.arc(x, y, r, 0, Math.PI * 2); c.stroke(); c.restore(); }
  rr(x, y, w, h, r) { const c = this.c; c.beginPath(); c.moveTo(x + r, y); c.arcTo(x + w, y, x + w, y + h, r); c.arcTo(x + w, y + h, x, y + h, r); c.arcTo(x, y + h, x, y, r); c.arcTo(x, y, x + w, y, r); c.closePath(); }
  // a pill with text (label size by default), anchored at its left edge, vertical centre cy; returns its width
  pill(s, x, cy, o = {}) {
    const size = o.size || LS, padX = o.padX ?? 22, h = o.h || Math.round(size * 1.55), wt = o.weight || 560;
    const w = this.tw(s, size, wt, o.mono) + 2 * padX; const X = o.align === 'center' ? x - w / 2 : o.align === 'right' ? x - w : x;
    const c = this.c; c.save(); c.globalAlpha *= o.alpha ?? 1;
    if (o.shadow !== false) { c.shadowColor = 'rgba(20,30,50,0.10)'; c.shadowBlur = 18; c.shadowOffsetY = 4; }
    c.fillStyle = o.bg || 'rgba(255,255,255,0.94)'; this.rr(X, cy - h / 2, w, h, h / 2); c.fill(); c.shadowColor = 'transparent';
    if (o.stroke) { c.strokeStyle = o.stroke; c.lineWidth = o.strokeW || 2; c.stroke(); }
    c.restore();
    this.txt(s, X + padX, cy + size * 0.36, { size, weight: wt, color: o.color || COL.ink, alpha: o.alpha ?? 1, mono: o.mono });
    return w;
  }
  // a check mark drawn progressively (u 0..1), centred at (x, y), size s
  tick(x, y, s, u, color, w = 6, alpha = 1) {
    if (u <= 0) return; const p = [[x - 0.5 * s, y], [x - 0.15 * s, y + 0.35 * s], [x + 0.55 * s, y - 0.4 * s]];
    const L1 = Math.hypot(p[1][0] - p[0][0], p[1][1] - p[0][1]), L2 = Math.hypot(p[2][0] - p[1][0], p[2][1] - p[1][1]); const L = (L1 + L2) * clamp(u);
    const pts = [p[0]]; if (L <= L1) pts.push([lerp(p[0][0], p[1][0], L / L1), lerp(p[0][1], p[1][1], L / L1)]); else { pts.push(p[1]); const v = (L - L1) / L2; pts.push([lerp(p[1][0], p[2][0], v), lerp(p[1][1], p[2][1], v)]); }
    this.line(pts, w, color, alpha);
  }
  veil(alpha, color = '244,245,247') { if (alpha <= 0) return; const c = this.c; c.save(); c.fillStyle = `rgba(${color},${alpha})`; c.fillRect(0, 0, 1920, 1080); c.restore(); }
}
