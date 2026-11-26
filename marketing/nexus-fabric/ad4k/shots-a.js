/* NEXUS FABRIC — ad4k/shots-a.js · shots 1–5 (0.000–14.000 s)
   Deterministic: every visual property is derived from local time only.
   Every camera target and every highlight rectangle comes from the measured
   rects.json (capture coordinates, 1920x1080). No estimated positions. */
(function () {
  const clamp = NF.clamp, lerp = NF.lerp, seg = NF.seg, ease = NF.ease;
  const el = NF.el, svg = NF.svg, geo = NF.geo, text = NF.text;
  const JA = LANG === 'ja';
  const CY = '#35D6E8', CD = '#0E7C8A', RU = '#2A333C', INK2 = '#9AA7B2';

  // ---------- measured rectangles (loaded once, synchronously: still deterministic) ----------
  const RECTS = (function () {
    try {
      const q = new XMLHttpRequest();
      q.open('GET', 'rects.json', false); q.send(null);
      if (q.responseText) return JSON.parse(q.responseText);
    } catch (e) { console.error('rects.json', e); }
    return {};
  })();
  NF._rects = NF._rects || RECTS;

  // a capture is only 1920x1080; a measured element may continue below the fold.
  // clip to the visible plate (inset by the frame margin) so a frame is always a closed rectangle.
  const CX0 = 48, CY0 = 44, CX1 = 1872, CY1 = 1036;
  const rect = (screen, key) => {
    const s = RECTS[screen], r = s && s[key];
    if (!r) return null;
    const x0 = Math.max(r[0], CX0), y0 = Math.max(r[1], CY0);
    const x1 = Math.min(r[0] + r[2], CX1), y1 = Math.min(r[1] + r[3], CY1);
    if (x1 <= x0 || y1 <= y0) return null;
    return [x0, y0, x1 - x0, y1 - y0];
  };
  const cxOf = (r) => r[0] + r[2] / 2, cyOf = (r) => r[1] + r[3] / 2;
  // bounding box of two measured rects — still measured, never estimated
  const union = (a, b) => {
    if (!a) return b; if (!b) return a;
    const x0 = Math.min(a[0], b[0]), y0 = Math.min(a[1], b[1]);
    const x1 = Math.max(a[0] + a[2], b[0] + b[2]), y1 = Math.max(a[1] + a[3], b[1] + b[3]);
    return [x0, y0, x1 - x0, y1 - y0];
  };
  // zoom limit: the element (and its labels) must stay wholly inside the 1920x1080 frame
  const fitZoom = (r) => Math.min(1700 / r[2], 980 / r[3]);

  // camera on a measured rect centre, clamped so the rect never leaves the frame
  const camTo = (layer, s, cx, cy, r) => {
    if (r) s = Math.min(s, fitZoom(r));
    s = Math.max(1, s);
    let tx = clamp(960 - cx * s, 1920 - 1920 * s, 0), ty = clamp(540 - cy * s, 1080 - 1080 * s, 0);
    if (r) {
      const L = r[0] * s + tx, R = (r[0] + r[2]) * s + tx, T = r[1] * s + ty, B = (r[1] + r[3]) * s + ty;
      if (L < CX0) tx += CX0 - L; else if (R > CX1) tx -= R - CX1;
      if (T < CY0) ty += CY0 - T; else if (B > CY1) ty -= B - CY1;
      tx = clamp(tx, 1920 - 1920 * s, 0); ty = clamp(ty, 1080 - 1080 * s, 0);
      cx = (960 - tx) / s; cy = (540 - ty) / s;
    }
    layer.cam(s, cx, cy);
  };
  // NF.frame settles its own padding 26 -> 10 px; inset the rect by 2 px so the resting
  // frame is exactly the measured rectangle + 8 px on every side.
  const frameOn = (layer, r, p, key) => {
    key = key || 0;
    if (!r || p <= 0) return NF.frame(layer, 0, 0, 0, 0, 0, key);
    const s = (layer._cam || { s: 1 }).s, i = 2 / s;
    return NF.frame(layer, r[0] + i, r[1] + i, Math.max(1, r[2] - 2 * i), Math.max(1, r[3] - 2 * i), p, key);
  };
  const fadeFrame = (n, k) => { n.style.opacity = (parseFloat(n.style.opacity || 0) * (1 - k)).toFixed(3); return n; };

  // caption: settles from +rise px over .34 s, optional .12 s cross-fade out
  const capIn = (n, l, a, b, rise, dur) => {
    rise = rise === undefined ? 30 : rise; dur = dur || .34;
    const ki = ease.outCubic(seg(l, a, a + dur)), ko = b == null ? 1 : 1 - seg(l, b, b + .12);
    n.style.opacity = (ki * ko).toFixed(3);
    n.style.transform = 'translateY(' + (rise * (1 - ki)).toFixed(2) + 'px)';
  };
  // auto-fit a kinetic word once (fonts are ready by the first render)
  const fit = (n, sp, max, base, lim) => {
    if (n._fs) return n._fs;
    n.style.fontSize = base + 'px';
    n.style.letterSpacing = (JA ? .02 : -.03) + 'em';
    const w = sp.offsetWidth || 1;
    n._fs = Math.min(base * max / w, lim);
    n.style.fontSize = n._fs.toFixed(1) + 'px';
    n.style.letterSpacing = '';
    n._w = sp.offsetWidth;
    return n._fs;
  };
  const upFade = (n, k) => {
    if (k <= 0) return;
    n.style.opacity = (parseFloat(n.style.opacity || 1) * (1 - k)).toFixed(3);
    const tr = n.style.transform;
    n.style.transform = (tr && tr !== 'none' ? tr + ' ' : '') + 'translateY(' + (-46 * k).toFixed(1) + 'px)';
  };
  const sheet = (parent) => svg('svg', { width: 1920, height: 1080, viewBox: '0 0 1920 1080',
    style: 'position:absolute;left:0;top:0;pointer-events:none' }, parent);

  // ---------- 1 · 0.0–3.0 graphic: three slams + self-drawing ruler ----------
  SHOTS.push({ a: 0, b: 3,
    init(L) {
      el('div', { style: 'position:absolute;left:0;top:0;width:1920px;height:1080px;background:' +
        'radial-gradient(1240px 640px at 50% 46%,rgba(14,124,138,.20),rgba(11,14,17,0) 72%)' }, L);
      this.modes = ['mask', 'stretch', 'track'];
      this.words = ['s1_a', 's1_b', 's1_c'].map((id) => {
        const n = el('div', { class: 'slam' }, L); return { n: n, sp: el('span', { text: text(id) }, n) };
      });
    },
    render(t, l) {
      NF.status(false);
      let p = 0; for (let i = 0; i < 3; i++) p += ease.outCubic(seg(l, i + .02, i + .66)) / 3;
      NF.rule(p);
      const ex = seg(l, 2.86, 3.0);
      NF._overlay.style.opacity = (p > 0 ? 1 - ex : 0).toFixed(3);
      this.words.forEach((w, i) => {
        if (!(l >= i && l < i + 1)) { w.n.style.opacity = 0; return; }
        fit(w.n, w.sp, 1680, JA ? 130 : 150, JA ? 260 : 300);
        NF.slam(w.n, seg(l, i, i + (i === 0 ? .28 : .34)), this.modes[i]);
        upFade(w.n, seg(l, i + .86, i + 1.0));
      });
    } });

  // ---------- 2 · 3.0–6.0 assembly form: push into the nine dimension fields ----------
  SHOTS.push({ a: 3, b: 6,
    init(L) {
      this.name = JA ? '20-ja-new-calculation' : '04-new-calculation';
      this.s = NF.screen(this.name); L.appendChild(this.s);
      this.rp = rect(this.name, 'form_params');      // the nine dimension fields
      this.rf = rect(this.name, 'family_select');    // header shell with branch
      this.c1 = NF.caption(L, 's2_a'); this.c2 = NF.caption(L, 's2_b');
    },
    render(t, l) {
      const rp = this.rp, rf = this.rf;
      const sA = rp ? Math.min(1.9, fitZoom(rp)) : 1;
      const sB = rf ? Math.min(1.9, fitZoom(rf)) : 1;
      const push = ease.outCubic(seg(l, .1, .9));
      const move = ease.inOutCubic(seg(l, 2.05, 2.6));
      const drift = 1 + .004 * seg(l, .9, 2.05);
      let s = lerp(1, sA, push) * drift;
      let cx = rp ? lerp(960, cxOf(rp), push) : 960;
      let cy = rp ? lerp(540, cyOf(rp), push) : 540;
      if (rf && move > 0) { s = lerp(s, sB, move); cx = lerp(cx, cxOf(rf), move); cy = lerp(cy, cyOf(rf), move); }
      // guard only at rest: while moving, the interpolation itself carries the framing
      camTo(this.s, s, cx, cy, move >= 1 ? rf : (move <= 0 && push >= 1 ? rp : null));
      fadeFrame(frameOn(this.s, rp, seg(l, .72, 1.12), 0), seg(l, 1.95, 2.2));
      frameOn(this.s, rf, seg(l, 2.25, 2.6), 1);
      capIn(this.c1, l, .15, 2.0); capIn(this.c2, l, 2.1, null);
      NF.status(true);
    } });

  // ---------- 3 · 6.0–9.5 header + branch → seam split → developed plate → real viewer ----------
  SHOTS.push({ a: 6, b: 9.5,
    init(L) {
      this.name = JA ? '16-ja-flat-pattern' : '06-job-flat-pattern';
      this.s = NF.screen(this.name);
      L.appendChild(this.s); this.s.style.opacity = 0;
      this.rp = rect(this.name, 'pattern_svg');
      // all drawing lives in ad4k/unroll.js — this shot only hosts it and dissolves to the capture
      this.draw = el('div', { style: 'position:absolute;left:0;top:0;width:1920px;height:1080px' }, L);
      window.NF_UNROLL.init(this.draw);
      this.c1 = NF.caption(L, 's3_a'); this.c2 = NF.caption(L, 's3_b');
    },
    render(t, l) {
      window.NF_UNROLL.render(Math.min(l, 3.5));
      // match-dissolve to the capture over the last 1.3 s, then push on the developed pattern
      const dis = seg(l, 2.2, 3.5);
      this.draw.style.opacity = (1 - dis).toFixed(3);
      this.s.style.opacity = dis.toFixed(3);
      const rp = this.rp;
      const zs = rp ? Math.min(1.6, fitZoom(rp)) : 1;
      camTo(this.s, lerp(1, zs, ease.outCubic(seg(l, 2.45, 3.4))),
        rp ? cxOf(rp) : 960, rp ? cyOf(rp) : 540, rp);
      frameOn(this.s, rp, seg(l, 2.85, 3.25), 0);
      capIn(this.c1, l, .35, 2.35); capIn(this.c2, l, 2.5, null);
      NF.status(dis > .02);
    } });

  // ---------- 4 · 9.5–10.5 CHECKED: caliper jaws close and stop dead ----------
  SHOTS.push({ a: 9.5, b: 10.5,
    init(L) {
      el('div', { style: 'position:absolute;left:0;top:0;width:1920px;height:1080px;background:' +
        'radial-gradient(1100px 560px at 50% 50%,rgba(14,124,138,.18),rgba(11,14,17,0) 70%)' }, L);
      const S = sheet(L); this.S = S;
      this.beam = svg('path', { fill: 'none', stroke: RU, 'stroke-width': 2 }, S);
      this.ticks = svg('path', { fill: 'none', stroke: RU, 'stroke-width': 1.5 }, S);
      this.jl = svg('path', { fill: 'none', stroke: CY, 'stroke-width': 4 }, S);
      this.jr = svg('path', { fill: 'none', stroke: CY, 'stroke-width': 4 }, S);
      this.n = el('div', { class: 'slam' }, L); this.sp = el('span', { text: text('s4_a') }, this.n);
    },
    render(t, l) {
      NF.status(false);
      fit(this.n, this.sp, 1000, JA ? 150 : 170, JA ? 300 : 340);
      NF.slam(this.n, seg(l, 0, .30), 'stretch');
      const ex = seg(l, .86, 1.0);
      upFade(this.n, ex);
      const half = (this.n._w || 600) / 2, k = ease.outCubic(seg(l, 0, .32));
      const xl = lerp(120, 960 - half - 26, k), xr = lerp(1800, 960 + half + 26, k);
      this.jl.setAttribute('d', 'M' + xl.toFixed(1) + ' 378V702M' + xl.toFixed(1) + ' 378h46M' + xl.toFixed(1) + ' 702h46');
      this.jr.setAttribute('d', 'M' + xr.toFixed(1) + ' 378V702M' + xr.toFixed(1) + ' 378h-46M' + xr.toFixed(1) + ' 702h-46');
      this.beam.setAttribute('d', 'M120 744H1800');
      let d = ''; for (let x = 120; x <= 1800; x += 40) d += 'M' + x + ' 744V' + ((x - 120) % 200 === 0 ? 726 : 734);
      this.ticks.setAttribute('d', d);
      this.S.style.opacity = (clamp(k * 3) * (1 - ex)).toFixed(3);
    } });

  // ---------- 5 · 10.5–14.0 the real checks list: primary / reference / difference / tolerance ----------
  SHOTS.push({ a: 10.5, b: 14,
    init(L) {
      // re-taken captures: the checks list itself (heading, count chip, one card per check)
      this.name = JA ? '21b-ja-job-checks' : '07b-job-checks';
      this.s = NF.screen(this.name); L.appendChild(this.s);
      this.r1 = rect(this.name, 'first_check');          // first check card
      this.rv = rect(this.name, 'first_check_values');   // its Primary / Reference / Difference / Tolerance
      this.r2 = rect(this.name, 'second_check');         // the 942.478 mm plate check
      this.pair = union(this.r1, this.r2);               // both cards, measured bounds
      this.c1 = NF.caption(L, 's5_a'); this.c2 = NF.caption(L, 's5_b'); this.c3 = NF.caption(L, 's5_c');
    },
    render(t, l) {
      const pair = this.pair, r2 = this.r2, ra = this.rv || this.r1;
      // zoom limits come from the measured widths: 1700 / w (and 980 / h)
      const sOpen = pair ? Math.min(1.6, fitZoom(pair)) : 1;
      const sHi = r2 ? Math.min(1.9, fitZoom(r2)) : sOpen;
      const open = ease.outCubic(seg(l, .05, .95));      // 1.0x -> both cards
      const push = ease.inOutCubic(seg(l, 1.35, 2.05));  // -> second check card
      const pull = ease.inOutCubic(seg(l, 2.60, 3.15));  // short pull back to the record
      const drift = 1 + .004 * seg(l, .95, 1.35);
      let s = lerp(1, sOpen, open) * drift;
      let cx = pair ? lerp(960, cxOf(pair), open) : 960;
      let cy = pair ? lerp(540, cyOf(pair), open) : 540;
      if (r2 && push > 0) { s = lerp(s, sHi, push); cx = lerp(cx, cxOf(r2), push); cy = lerp(cy, cyOf(r2), push); }
      if (pair && pull > 0) { s = lerp(s, sOpen, pull); cx = lerp(cx, cxOf(pair), pull); cy = lerp(cy, cyOf(pair), pull); }
      // guard only at rest; the interpolation carries the framing while the camera moves
      const rest = pull >= 1 ? pair : (push >= 1 ? r2 : (push <= 0 && open >= 1 ? pair : null));
      camTo(this.s, s, cx, cy, rest);
      // closed cyan rectangle on the first card's values, then on the second check card
      fadeFrame(frameOn(this.s, ra, seg(l, .50, .90), 0), seg(l, 1.28, 1.52));
      frameOn(this.s, r2, seg(l, 1.70, 2.10), 1);
      capIn(this.c1, l, .15, 1.0); capIn(this.c2, l, 1.05, 2.5); capIn(this.c3, l, 2.55, null);
      NF.status(true);
    } });
})();
