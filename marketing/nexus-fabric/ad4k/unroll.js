/* ad4k/unroll.js — shot 3 (6.0–9.5 s): cabinet-oblique header + branch, seam split, developed plate.
   Loaded after film.html core, before shots-a.js. Deterministic: everything derives from `local`.
   World: header axis = x (0..1200 mm), y up, z toward the viewer. Branch axis from (600,0,0) at 60°.
   Plate: X = axial 0..1200 mm (horizontal), Y = circumferential 0..942.478 mm (vertical), seam at Y = 0. */
window.NF_UNROLL = (function () {
  const NF = window.NF, svg = NF.svg, geo = NF.geo, seg = NF.seg, ease = NF.ease, lerp = NF.lerp, clamp = NF.clamp;
  const PI = Math.PI, R = geo.R, r = geo.r, B = geo.beta, sB = Math.sin(B), cB = Math.cos(B);
  const LEN = 1200, PW = geo.plateW, BL = 400, MID = LEN / 2;                   // mm
  const CX = 960, CY = 560, K = .62 * 1920 / LEN;                               // header spans 62% of frame width
  const OB = .45, c35 = Math.cos(35 * PI / 180), s35 = Math.sin(35 * PI / 180); // cabinet oblique
  const P = (p) => [CX + K * (p[0] - MID) + K * OB * p[2] * c35, CY - K * p[1] + K * OB * p[2] * s35];
  const U0 = PI / 2;                                                            // seam angle (Y = 0): generator opposite the branch
  const hs = (x, u) => [x, R * Math.cos(u), R * Math.sin(u)];                   // header ruling
  const e1 = [-sB, cB, 0];                                                      // branch circle basis (e2 = 0,0,1)
  const bs = (s, f) => [MID + s * cB + r * Math.cos(f) * e1[0], s * sB + r * Math.cos(f) * e1[1], r * Math.sin(f)];
  const sOf = (f) => (Math.sqrt(Math.max(0, R * R - Math.pow(r * Math.sin(f), 2))) - r * Math.cos(f) * cB) / sB;
  const uT = Math.atan2(-OB * s35, 1), uB = uT + PI;                            // header tangents (extreme screenY)
  const fT = Math.atan2(-OB * s35, cB), fB = fT + PI;                           // branch tangents
  const NX = 40, NA = 24;                                                       // developed grid: 40 around, 24 along
  const thAt = (i) => U0 + 2 * PI * i / (NX - 1);
  const xAt = (j) => LEN * j / (NA - 1);
  const yAt = (i) => PW * i / (NX - 1);
  let S = null;

  const path = (parent, at) => svg('path', Object.assign({ fill: 'none', 'stroke-linecap': 'round', 'stroke-linejoin': 'round' }, at), parent);
  const dOf = (p) => 'M' + p.map((q) => q[0].toFixed(2) + ' ' + q[1].toFixed(2)).join('L');

  function init(layer) {
    const root = svg('svg', { width: 1920, height: 1080, viewBox: '0 0 1920 1080', style: 'position:absolute;left:0;top:0' }, layer);
    const defs = svg('defs', {}, root);
    const ringH = (x) => { const o = []; for (let i = 0; i <= 72; i++) o.push(hs(x, U0 + 2 * PI * i / 72)); return o; };
    const ringB = (s) => { const o = []; for (let i = 0; i <= 72; i++) o.push(bs(s, 2 * PI * i / 72)); return o; };
    const saddle = []; for (let i = 0; i <= 180; i++) { const f = 2 * PI * i / 180; saddle.push(bs(sOf(f), f)); }
    const ends = [ringH(0), ringH(LEN)], top = ringB(BL);
    const silH = [uT, uB].map((u) => [hs(0, u), hs(LEN, u)]);
    const silB = [fT, fB].map((f) => [bs(sOf(f), f), bs(BL, f)]);
    const seam = [hs(0, U0), hs(LEN, U0)];

    const ps = Math.min(.58 * 1080 / PW, 1300 / LEN);                           // plate fits 58% of frame height
    const PCY = 500;                                                            // centred, lifted clear of the caption
    const Q = (x, y) => [960 + (x - MID) * ps, PCY + (y - PW / 2) * ps];
    const pl = Q(0, 0), pr = Q(LEN, PW), L = pl[0], Rr = pr[0], T = pl[1], Bt = pr[1];

    const gA = svg('g', {}, root), gP = svg('g', { opacity: 0 }, root), gM = svg('g', { opacity: 0 }, root);
    const gE = svg('g', { opacity: 0 }, root), gO = svg('g', { opacity: 0 }, root), gD = svg('g', { opacity: 0 }, root);
    svg('rect', { x: L.toFixed(2), y: T.toFixed(2), width: (Rr - L).toFixed(2), height: (Bt - T).toFixed(2),
      fill: 'var(--panel)', stroke: 'none' }, gP);

    const stat = [];
    const add = (pts, at, t0, t1, closed) => {
      const n = path(gA, Object.assign({ d: dOf(pts.map(P)) + (closed ? 'Z' : ''), pathLength: 1,
        'stroke-dasharray': '1 1', 'stroke-dashoffset': 1 }, at));
      stat.push({ n, t0, t1 }); return n;
    };
    const body = { stroke: 'var(--ink2)', 'stroke-width': 2.4 };
    ends.forEach((e, i) => add(e, body, .05 + i * .08, .62 + i * .08, true));
    silH.forEach((s) => add(s, body, .22, .78));
    add(top, body, .48, 1.00, true);
    silB.forEach((s) => add(s, body, .42, .95));
    const seamN = add(seam, { stroke: 'var(--rule)', 'stroke-width': 2 }, .50, .95);
    add(saddle, { stroke: 'var(--cyan)', 'stroke-width': 3.2 }, .68, 1.18, true);

    const morph = [];
    const mAdd = (parent, w, q, at, closed) => {
      const m = { n: path(parent, at), a: w.map(P), b: q, closed: !!closed, done: false }; morph.push(m); return m;
    };
    const edgeAt = { stroke: 'var(--ink)', 'stroke-width': 2.6 }, meshAt = { stroke: 'var(--rule)', 'stroke-width': 1.4 };
    for (let i = 0; i < NX; i++) {                                              // 40 rulings: constant u, seam edges split
      const u = thAt(i), w = [], q = [], edge = (i === 0 || i === NX - 1);
      for (let j = 0; j < NA; j++) { const x = xAt(j); w.push(hs(x, u)); q.push(Q(x, yAt(i))); }
      if (edge || i % 3 === 0) mAdd(edge ? gE : gM, w, q, edge ? edgeAt : meshAt);
    }
    for (let j = 0; j < NA; j++) {                                              // 24 rings: constant axial x
      const x = xAt(j), w = [], q = [], edge = (j === 0 || j === NA - 1);
      for (let i = 0; i < NX; i++) { w.push(hs(x, thAt(i))); q.push(Q(x, yAt(i))); }
      if (edge || j % 4 === 0) mAdd(edge ? gE : gM, w, q, edge ? edgeAt : meshAt);
    }
    const op = geo.opening(160);                                                // [circumferential, axial] -> swap
    const om = mAdd(gO, op.map((p) => hs(MID + p[1], U0 + p[0] / R)),
      op.map((p) => Q(MID + p[1], p[0])),
      { stroke: 'var(--cyan)', 'stroke-width': 3.2, fill: 'var(--cyan-deep)', 'fill-opacity': 0 }, true);

    // ---- dimensions: 942.478 mm vertical at the right, 1200 mm horizontal below ----
    const dx = Rr + 58, dy = Bt + 52, vcy = (T + Bt) / 2;
    svg('path', { d: `M${(Rr + 10).toFixed(2)} ${T.toFixed(2)}H${(dx + 12).toFixed(2)}` +
      `M${(Rr + 10).toFixed(2)} ${Bt.toFixed(2)}H${(dx + 12).toFixed(2)}`,
      stroke: 'var(--rule)', 'stroke-width': 1.6, fill: 'none' }, gD);
    svg('path', { d: `M${L.toFixed(2)} ${(Bt + 10).toFixed(2)}V${(dy + 12).toFixed(2)}` +
      `M${Rr.toFixed(2)} ${(Bt + 10).toFixed(2)}V${(dy + 12).toFixed(2)}`,
      stroke: 'var(--rule)', 'stroke-width': 1.6, fill: 'none' }, gD);
    const dv = path(gD, { d: `M${dx.toFixed(2)} ${T.toFixed(2)}V${Bt.toFixed(2)}`, stroke: 'var(--cyan)',
      'stroke-width': 2, pathLength: 1, 'stroke-dasharray': '1 1', 'stroke-dashoffset': 1 });
    const dh = path(gD, { d: `M${L.toFixed(2)} ${dy.toFixed(2)}H${Rr.toFixed(2)}`, stroke: 'var(--cyan)',
      'stroke-width': 2, pathLength: 1, 'stroke-dasharray': '1 1', 'stroke-dashoffset': 1 });
    const av = svg('path', { d: `M${dx.toFixed(2)} ${T.toFixed(2)}l-6 14h12z M${dx.toFixed(2)} ${Bt.toFixed(2)}l-6 -14h12z`,
      fill: 'var(--cyan)', opacity: 0 }, gD);
    const ah = svg('path', { d: `M${L.toFixed(2)} ${dy.toFixed(2)}l14 -6v12z M${Rr.toFixed(2)} ${dy.toFixed(2)}l-14 -6v12z`,
      fill: 'var(--cyan)', opacity: 0 }, gD);

    const cv = svg('clipPath', { id: 'nfu-dimv' }, defs);
    const wv = svg('rect', { x: (dx + 4).toFixed(2), y: (vcy + 160).toFixed(2), width: 60, height: 0 }, cv);
    const gv = svg('g', { 'clip-path': 'url(#nfu-dimv)' }, gD);
    const tx = dx + 44;
    svg('text', { x: tx.toFixed(2), y: vcy.toFixed(2), transform: `rotate(-90 ${tx.toFixed(2)} ${vcy.toFixed(2)})`,
      'text-anchor': 'middle', 'dominant-baseline': 'central', class: 'mono', 'font-size': 38, 'font-weight': 700,
      fill: 'var(--ink)', 'letter-spacing': '.01em', text: NF.text('s3_dim') }, gv);

    const ch = svg('clipPath', { id: 'nfu-dimh' }, defs);
    const wh = svg('rect', { x: 790, y: (dy + 4).toFixed(2), width: 0, height: 56 }, ch);
    const gh = svg('g', { 'clip-path': 'url(#nfu-dimh)' }, gD);
    svg('text', { x: 960, y: (dy + 40).toFixed(2), 'text-anchor': 'middle', class: 'mono', 'font-size': 38,
      'font-weight': 700, fill: 'var(--ink)', 'letter-spacing': '.01em', text: '1200 mm' }, gh);

    S = { stat, seamN, morph, om, gP, gM, gE, gO, gD, dv, dh, av, ah, wv, wh, vcy };
  }

  function render(local) {
    if (!S) return;
    const t = clamp(local, 0, 3.5);
    const fade = 1 - seg(t, 1.22, 1.58);                                        // wireframe (incl. branch) fades out
    for (const s of S.stat) {
      const p = ease.outCubic(seg(t, s.t0, s.t1));
      s.n.setAttribute('stroke-dashoffset', (1 - p).toFixed(4));
      s.n.setAttribute('opacity', fade.toFixed(3));
    }
    const fl = seg(t, 1.20, 1.27) * (1 - seg(t, 1.33, 1.55));                   // seam flash, then split
    S.seamN.setAttribute('stroke', fl > .02 ? 'var(--cyan)' : 'var(--rule)');
    S.seamN.setAttribute('stroke-width', (2 + 3.4 * fl).toFixed(2));
    S.seamN.setAttribute('opacity', Math.max(fade, fl).toFixed(3));

    const k = ease.inOutCubic(seg(t, 1.2, 2.2)), inn = seg(t, 1.2, 1.42);
    for (const m of S.morph) {
      if (m.done && k >= 1) continue;
      let d = 'M';
      for (let i = 0; i < m.a.length; i++) {
        const A = m.a[i], Bp = m.b[i];
        d += (i ? 'L' : '') + (A[0] + (Bp[0] - A[0]) * k).toFixed(2) + ' ' + (A[1] + (Bp[1] - A[1]) * k).toFixed(2);
      }
      m.n.setAttribute('d', d + (m.closed ? 'Z' : ''));
      m.done = k >= 1;
    }
    S.gP.setAttribute('opacity', (.55 * ease.outCubic(seg(t, 1.85, 2.25))).toFixed(3));
    S.gM.setAttribute('opacity', (inn * lerp(.55, .16, k)).toFixed(3));
    S.gE.setAttribute('opacity', inn.toFixed(3));
    S.gO.setAttribute('opacity', inn.toFixed(3));
    S.om.n.setAttribute('fill-opacity', (0.18 * k).toFixed(3));

    const dp = ease.outCubic(seg(t, 2.24, 2.72)), wp = ease.outCubic(seg(t, 2.34, 2.78));
    const dq = ease.outCubic(seg(t, 2.36, 2.84)), wq = ease.outCubic(seg(t, 2.46, 2.90));
    S.gD.setAttribute('opacity', seg(t, 2.20, 2.32).toFixed(3));
    S.dv.setAttribute('stroke-dashoffset', (1 - dp).toFixed(4));
    S.dh.setAttribute('stroke-dashoffset', (1 - dq).toFixed(4));
    S.av.setAttribute('opacity', clamp(dp * 1.6).toFixed(3));
    S.ah.setAttribute('opacity', clamp(dq * 1.6).toFixed(3));
    S.wv.setAttribute('y', (S.vcy + 160 - 320 * wp).toFixed(2));                // values wipe in whole, no count-up
    S.wv.setAttribute('height', (320 * wp).toFixed(2));
    S.wh.setAttribute('width', (340 * wq).toFixed(2));
  }

  return { init, render };
})();
