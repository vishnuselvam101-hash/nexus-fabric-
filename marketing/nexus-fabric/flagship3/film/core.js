// NEXUS FABRIC flagship film 3, revision 3. Every frame is drawn on one canvas from the film clock (window.seek(t)).
// Vector pieces come from data/film-data.js (the product's own output); interface pieces are simplified for the film;
// two moments show the real application (footage). No clock, no random numbers: frame f is the same every time.
(function () {
  'use strict';
  const { clamp, E } = window.NFE;
  const LANG = new URLSearchParams(location.search).get('lang') === 'ja' ? 'ja' : 'en';
  const JA = LANG === 'ja';
  const W = 1920, H = 1080;
  const canvas = document.getElementById('gfx');
  // ?scale=2 gives the 4K master: the same 1920x1080 drawing on a 3840x2160 backing store (all coordinates stay in 1920x1080 units)
  const SCALE = new URLSearchParams(location.search).get('scale') === '2' ? 2 : 1;
  canvas.width = 1920 * SCALE; canvas.height = 1080 * SCALE;
  const ctx = canvas.getContext('2d');
  const C = { paper: '#F5F5F7', ink: '#15191F', ink2: '#4A5361', ink3: '#5F6876', rule: '#D5DBE3', rule2: '#A8B1BD',
    white: '#FFFFFF', cv: '#FAF9F5', grid: '#E4E8EE', grid2: '#D2D9E2', cut: '#1E5BCC', signal: '#1E5BCC', amber: '#8A5A00',
    amberSoft: 'rgba(138,90,0,0.08)', green: '#13764A', greenSoft: 'rgba(19,118,74,0.08)', dim: '#2A313C' };
  const FE = '"Inter Variable", "Noto Sans JP", sans-serif';
  const FJ = '"Noto Sans JP", "Inter Variable", sans-serif';
  const FM = '"DejaVu Sans Mono", "Liberation Mono", monospace';
  const F3 = (window.F3 = { LANG, JA, W, H, ctx, C, t: 0, foot: null, scenes: [], S: SCALE });

  // ---- timing: springs with overshoot under 4 %, eases; nothing moves linearly
  F3.U01 = (t, a, b) => clamp((t - a) / (b - a), 0, 1);
  F3.SP = (t, t0, w = 13, z = 0.8) => {                 // damped spring step, 0 -> 1 from t0 (overshoot about 1.5 %)
    const x = t - t0; if (x <= 0) return 0;
    const wd = w * Math.sqrt(1 - z * z);
    return 1 - Math.exp(-z * w * x) * (Math.cos(wd * x) + (z / Math.sqrt(1 - z * z)) * Math.sin(wd * x));
  };
  F3.IO = (t, a, b) => E.io3(F3.U01(t, a, b));
  F3.OUT = (t, a, b) => E.o3(F3.U01(t, a, b));
  F3.IN = (t, a, b) => E.i3(F3.U01(t, a, b));
  F3.lerp = (a, b, u) => a + (b - a) * u;
  F3.bump = (t, t0, dur, amp) => amp * Math.sin(Math.PI * F3.U01(t, t0, t0 + dur));

  // ---- shapes
  F3.rr = (x, y, w, h, r) => {
    r = Math.max(0, Math.min(r, w / 2, h / 2));
    ctx.beginPath(); ctx.moveTo(x + r, y); ctx.arcTo(x + w, y, x + w, y + h, r); ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r); ctx.arcTo(x, y, x + w, y, r); ctx.closePath();
  };
  F3.card = (x, y, w, h, o = {}) => {                     // a white sheet with a soft shadow and a hairline edge (tokens: --sheet, --rule, --r)
    if (o.sh !== false) {
      ctx.save(); ctx.shadowColor = o.shc || 'rgba(21,25,31,0.14)'; ctx.shadowBlur = (o.blur ?? 52) * F3.S; ctx.shadowOffsetY = (o.dy ?? 20) * F3.S;
      ctx.fillStyle = o.fill || C.white; F3.rr(x, y, w, h, o.r ?? 3); ctx.fill(); ctx.restore();
    } else { ctx.fillStyle = o.fill || C.white; F3.rr(x, y, w, h, o.r ?? 3); ctx.fill(); }
    if (o.line !== false) { ctx.strokeStyle = o.stroke || C.rule; ctx.lineWidth = 2; F3.rr(x + 1, y + 1, w - 2, h - 2, o.r ?? 3); ctx.stroke(); }
  };
  F3.line = (pts, o = {}) => {                             // open polyline, optional draw-on progress p (0..1 along the points)
    const n = pts.length, k = Math.max(0, Math.min(n, Math.floor((o.p ?? 1) * (n - 1)) + 1));
    if (k < 2) return;
    ctx.save(); ctx.strokeStyle = o.color || C.cut; ctx.lineWidth = o.lw ?? 6; ctx.lineJoin = 'round'; ctx.lineCap = 'round';
    if (o.dash) ctx.setLineDash(o.dash);
    ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < k; i++) ctx.lineTo(pts[i][0], pts[i][1]);
    if (o.closed && k === n) ctx.closePath();
    ctx.stroke(); ctx.restore();
  };
  F3.polyFill = (pts, fill) => { ctx.save(); ctx.fillStyle = fill; ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]); ctx.closePath(); ctx.fill(); ctx.restore(); };

  // ---- type
  F3.font = (size, wt, lang) => `${wt} ${size}px ${((lang || LANG) === 'ja') ? FJ : FE}`;
  F3.tw = (s, size, wt, tr, lang) => { ctx.save(); ctx.font = F3.font(size, wt, lang); ctx.letterSpacing = ((lang || LANG) === 'ja' ? 0 : (tr ?? 0) * size) + 'px'; const w = ctx.measureText(String(s)).width; ctx.restore(); return w; };
  F3.txt = (s, x, y, o = {}) => {
    if (s === '' || s === null || s === undefined) return;
    const size = o.size ?? 26, lang = o.lang || LANG, wt = o.w ?? (lang === 'ja' ? 700 : 600);
    ctx.save();
    if (o.mono) { ctx.font = `${o.w ?? 400} ${size}px ${FM}`; ctx.letterSpacing = '0px'; }
    else { ctx.font = F3.font(size, wt, lang); ctx.letterSpacing = (lang === 'ja' ? 0 : (o.tr ?? 0) * size) + 'px'; }
    ctx.textAlign = o.align || 'left'; ctx.textBaseline = o.base || 'alphabetic';
    ctx.fillStyle = o.color || C.ink; ctx.globalAlpha = ctx.globalAlpha * (o.a ?? 1);
    ctx.fillText(String(s), x, y); ctx.restore();
  };
  // kinetic type: each word rises out of its own mask (a clip box on the line), springs to rest; o.t0s gives per-word start times
  F3.kin = (lines, o) => {
    const t = F3.t, size = o.size, lang = o.lang || LANG, ja = lang === 'ja';
    const wt = o.w ?? (ja ? 700 : 600), tr = ja ? 0 : (o.tr ?? -0.03) * size, lh = o.lh ?? size * 1.1;
    const cx = o.x ?? 960, y0 = o.y;
    ctx.save(); ctx.font = F3.font(size, wt, lang); ctx.letterSpacing = tr + 'px';
    const sp = ja ? 0 : ctx.measureText(' ').width;
    let k = 0;
    lines.forEach((ln, li) => {
      const words = ja ? [ln] : ln.split(' ');
      const ws = words.map((w) => ctx.measureText(w).width);
      const total = ws.reduce((a, b) => a + b, 0) + sp * (words.length - 1);
      let x = o.align === 'left' ? cx : cx - total / 2;
      const yb = y0 + li * lh;
      words.forEach((w, i) => {
        const t0 = o.t0s ? o.t0s[k] : o.t0 + k * (o.stag ?? 0.07);
        const idx = k++;
        const p = F3.SP(t, t0, o.wn ?? 14, 0.8);
        const q = o.tOut !== undefined && t > o.tOut ? E.i3(F3.U01(t, o.tOut, o.tOut + 0.28)) : 0;
        const yy = yb + (1 - p) * size * 1.3 - q * size * 1.45;
        ctx.save();
        ctx.beginPath(); ctx.rect(x - size * 0.08, yb - size * 0.9, ws[i] + size * 0.16, size * 1.3); ctx.clip();
        ctx.fillStyle = (o.acc && o.acc.indexOf(idx) >= 0) ? C.signal : (o.color || C.ink);
        ctx.fillText(w, x, yy);
        ctx.restore();
        x += ws[i] + sp;
      });
    });
    ctx.restore();
  };
  // ---- footage (real pixels only: cropped, moved; the frame is read from the tile atlases)
  F3.footFrame = (take, f, x, y, w, h, r) => {
    const fo = F3.foot; fo.draw(take, f);
    ctx.save(); F3.rr(x, y, w, h, r ?? 6); ctx.clip();
    ctx.drawImage(fo.cv, 0, 0, fo.W, fo.H, x, y, w, h); ctx.restore();
  };

  // ---- revision 5: three type sizes only (headline 132 px, body 48 px, label 40 px)
  F3.HS = 132; F3.BS = 48; F3.LS = 40;
  // one of the three fixed source labels: 40 px, bottom left, drawn for the life of the element it names (ink3 on paper, about 5.5:1)
  F3.src = (s, t0, t1) => { if (F3.t < t0 || F3.t > t1) return; F3.txt(s, 120, 1040, { size: 40, w: 500, color: C.ink3 }); };
  // a beat-locked pulse: scale 1 -> 1.08 -> 1 over half a second starting on beat time b (1 outside it)
  F3.pulse = (t, b) => 1 + 0.08 * Math.sin(Math.PI * F3.U01(t, b, b + 0.5));
  // an incoming element: on its bar frame it is already there (60 %), fully there four frames later, with a 12 px rise
  F3.enter = (t0, draw) => {
    if (F3.t < t0) return;
    const u = F3.U01(F3.t, t0, t0 + 4 / 30);
    ctx.save(); ctx.globalAlpha = ctx.globalAlpha * (0.6 + 0.4 * u); ctx.translate(0, 12 * (1 - u)); draw(); ctx.restore();
  };
  // several lines of one headline block, drawn together (no word-by-word rise)
  F3.lines = (lines, x, y, lh, o) => lines.forEach((ln, i) => F3.txt(ln, x, y + i * lh, o));
  // ---- revision 4 helpers
  // whole millimetres with thousands separators: one format wherever a length is shown
  F3.mm = (v) => Math.round(v).toLocaleString('en-US');
  // a crop of the real frame: (sx, sy, sw, sh) in CSS pixels of the 1920 x 1080 frame, drawn into the box (x, y, w, h)
  F3.footCrop = (take, f, sx, sy, sw, sh, x, y, w, h, r) => {
    const fo = F3.foot; fo.draw(take, f); const k = fo.W / 1920;
    ctx.save(); F3.rr(x, y, w, h, r ?? 6); ctx.clip();
    ctx.drawImage(fo.cv, sx * k, sy * k, sw * k, sh * k, x, y, w, h); ctx.restore();
  };
  // words broken into lines no wider than maxW
  F3.wrap = (s, size, maxW, wt) => {
    const out = []; let cur = '';
    for (const w of String(s).split(' ')) { const cand = cur ? cur + ' ' + w : w; if (!cur || F3.tw(cand, size, wt) <= maxW) cur = cand; else { out.push(cur); cur = w; } }
    if (cur) out.push(cur); return out;
  };

  // ---- data (window.NF_DATA, built from the product's output and the catalogue)
  F3.D = window.NF_DATA; F3.UI = F3.D.ui[LANG];
  F3.ui = (k) => (F3.UI[k] !== undefined ? F3.UI[k] : k);
  // labels=0 (owner's "clean" variant): the film's own disclosure captions are left out. The product's own states
  // (REVIEW REQUIRED, NOT RELEASED, "Human approval (not implemented)") and the Japanese voice credit always stay.
  const CLEAN = new URLSearchParams(location.search).get('labels') === '0';
  const HIDDEN = new Set(['label_vis', 'label_sim', 'label_actual', 'limits', 'checks_sub', 'dev_line', 'disclaimer', 'rule_flag']);
  F3.on = (k) => { const v = window.NF_ONSCREEN[k]; if (!v) throw new Error('no on-screen text ' + k); const t = v[LANG] !== undefined ? v[LANG] : v.en; if (CLEAN && HIDDEN.has(k)) return Array.isArray(t) ? t.map(() => '') : ''; return t; };
  F3.pname = (n) => (F3.D.params[n] && F3.D.params[n][LANG]) || n;
  F3.qname = (k) => (F3.D.quants[k] && F3.D.quants[k][LANG]) || k;
  F3.rkind = (k) => (F3.D.rkinds[k] && F3.D.rkinds[k][LANG]) || k;
  F3.relname = (r) => (F3.D.rels[r] && F3.D.rels[r][LANG]) || r;
  F3.fmt = (v, d = 3) => Number(v).toFixed(d);
  // the application's number format (web/app.js, n3): zero is 0.000; tiny and huge values are exponents; others 3 decimals
  const NF3 = new Intl.NumberFormat('en-US', { minimumFractionDigits: 3, maximumFractionDigits: 3 });
  F3.n3 = (v) => (v === 0 ? '0.000' : Math.abs(v) < 1e-3 || Math.abs(v) >= 1e12 ? v.toExponential(2) : NF3.format(Math.abs(v) < 5e-4 ? 0 : v));
  F3.ctx = ctx; F3.canvas = canvas;
})();
