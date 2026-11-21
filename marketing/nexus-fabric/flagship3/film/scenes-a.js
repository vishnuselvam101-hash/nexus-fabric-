// Scenes 1-5 (0-26 s). Revision 4: one hero per slot; every word on screen 40 px or more (source labels 26 px); the real
// application appears only in its two moments, whole and labelled. The parts and the joint are drawn from the product's
// outlines (jobs 1 to 5), aligned for morphing. Each scene draws only inside its own time: main.js dissolves between them.
(function () {
  'use strict';
  const F = window.F3, ctx = F.ctx, C = F.C, D = F.D, G = D.geo;

  // a flat outline fitted into a box (aspect kept), centred on (cx, cy), flipped to screen coordinates
  const fit = (pts, box, cx, cy) => {
    let a = [Infinity, Infinity, -Infinity, -Infinity];
    for (const p of pts) { a[0] = Math.min(a[0], p[0]); a[1] = Math.min(a[1], p[1]); a[2] = Math.max(a[2], p[0]); a[3] = Math.max(a[3], p[1]); }
    const K = Math.min(box[0] / (a[2] - a[0]), box[1] / (a[3] - a[1])), ux = (a[0] + a[2]) / 2, vy = (a[1] + a[3]) / 2;
    return pts.map((p) => [cx + (p[0] - ux) * K, cy - (p[1] - vy) * K]);
  };

  // ---------- part geometry: the five parts from the product's own outlines, aligned for morphing (as revision 3c)
  function bboxOf(pts) { const a = [Infinity, Infinity, -Infinity, -Infinity]; for (const p of pts) { a[0] = Math.min(a[0], p[0]); a[1] = Math.min(a[1], p[1]); a[2] = Math.max(a[2], p[0]); a[3] = Math.max(a[3], p[1]); } return a; }
  function area2(pts) { let s = 0; for (let i = 0; i < pts.length; i++) { const a = pts[i], b = pts[(i + 1) % pts.length]; s += a[0] * b[1] - b[0] * a[1]; } return s; }
  function alignTo(ref, pts) {          // cyclic shift and orientation so that pts[i] corresponds to ref[i] (on normalised shapes)
    const N = ref.length, bR = bboxOf(ref), bP = bboxOf(pts);
    const sR = Math.max(bR[2] - bR[0], bR[3] - bR[1]), sP = Math.max(bP[2] - bP[0], bP[3] - bP[1]);
    const cR = [(bR[0] + bR[2]) / 2, (bR[1] + bR[3]) / 2], cP = [(bP[0] + bP[2]) / 2, (bP[1] + bP[3]) / 2];
    const nR = ref.map((p) => [(p[0] - cR[0]) / sR, (p[1] - cR[1]) / sR]);
    const ori = Math.sign(area2(ref)) === Math.sign(area2(pts)) ? pts : pts.slice().reverse();
    const nP = ori.map((p) => [(p[0] - cP[0]) / sP, (p[1] - cP[1]) / sP]);
    let best = 0, bc = Infinity;
    for (let s = 0; s < N; s++) { let c = 0; for (let i = 0; i < N && c < bc; i++) { const a = nR[i], b = nP[(i + s) % N]; c += (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2; } if (c < bc) { bc = c; best = s; } }
    return Array.from({ length: N }, (_, i) => ori[(i + best) % N]);
  }
  let PARTS = null;
  function parts() {
    if (PARTS) return PARTS;
    const sa = (G.cone.request.r1 - G.cone.request.r2) / Math.hypot(G.cone.request.h, G.cone.request.r1 - G.cone.request.r2);
    const defs = [
      { key: 'shell', g: G.shell, label: 'part_shell', map: 'cylV', r: 500 },
      { key: 'cone', g: G.cone, label: 'part_cone', map: 'cone', sa },
      { key: 'ecc', g: G.ecc, label: 'part_ecc', map: 'cone', sa },
      { key: 'branch', g: G.branch, label: 'part_branch', map: 'cylV', r: 100 },
      { key: 'header', g: G.header, label: 'part_header', map: 'cylH', r: 150, hole: true },
    ];
    let prev = null;
    PARTS = defs.map((d) => {
      let out = d.g.outline.map((p) => [p[0], p[1]]);
      if (prev) out = alignTo(prev, out);
      prev = out;
      const bb = bboxOf(out), P = { ...d, out, bb, hole: d.hole ? d.g.holes[0].map((p) => [p[0], p[1]]) : null };
      P.phi0 = Math.atan2(out[0][1], out[0][0]);
      return P;
    });
    return PARTS;
  }
  // mm -> 3D (mm): cylinders roll the developed length around the axis; the cone is exact for the developed sector
  function map3(P, u, v) {
    if (P.map === 'cylV') { const th = (u - P.bb[0]) / P.r; return [P.r * Math.sin(th), v - (P.bb[1] + P.bb[3]) / 2, P.r * Math.cos(th)]; }
    if (P.map === 'cylH') { const th = (v - P.bb[1]) / P.r; return [u - (P.bb[0] + P.bb[2]) / 2, P.r * Math.sin(th), P.r * Math.cos(th)]; }
    const s = Math.hypot(u, v); let ph = Math.atan2(v, u) - P.phi0;
    while (ph > Math.PI) ph -= 2 * Math.PI; while (ph < -Math.PI) ph += 2 * Math.PI;
    const th = ph / P.sa, rs = s * P.sa;
    return [rs * Math.sin(th), -s * Math.sqrt(1 - P.sa * P.sa), rs * Math.cos(th)];
  }
  const proj = (X, Y, Z, K, cx, cy) => [cx + X * K, cy - Y * K * 0.96 + Z * K * 0.26];
  function flatPx(P, pts, box, cx, cy) {                 // flat plate in px, fitted to the box, centred on (cx, cy)
    const bw = P.bb[2] - P.bb[0], bh = P.bb[3] - P.bb[1], K = Math.min(box[0] / bw, box[1] / bh);
    const ux = (P.bb[0] + P.bb[2]) / 2, vy = (P.bb[1] + P.bb[3]) / 2;
    return pts.map((p) => [cx + (p[0] - ux) * K, cy - (p[1] - vy) * K]);
  }
  function rollInfo(P, pts, box) {                       // scale and centre of the rolled part, fitted to the box
    let a = [Infinity, Infinity, -Infinity, -Infinity];
    for (const p of pts) { const q = map3(P, p[0], p[1]); const s = proj(q[0], q[1], q[2], 1, 0, 0); a = [Math.min(a[0], s[0]), Math.min(a[1], s[1]), Math.max(a[2], s[0]), Math.max(a[3], s[1])]; }
    return { K: Math.min(box[0] / (a[2] - a[0]), box[1] / (a[3] - a[1])), mx: (a[0] + a[2]) / 2, my: (a[1] + a[3]) / 2 };
  }
  function rollPx(P, pts, box, cx, cy) {
    const I = rollInfo(P, pts, box);
    return pts.map((p) => { const q = map3(P, p[0], p[1]); const s = proj(q[0], q[1], q[2], I.K, 0, 0); return [cx + s[0] - I.mx * I.K, cy + s[1] - I.my * I.K]; });
  }
  const lerpPts = (A, B, u) => A.map((p, i) => [F.lerp(p[0], B[i][0], u), F.lerp(p[1], B[i][1], u)]);
  function shapeOf(P, boxF, boxR, cx, cy) {             // flat plate in boxF, rolled part in boxR (both centred on cx, cy)
    if (!P._c) P._c = {};
    const key = boxF.join(',') + '|' + boxR.join(',') + '@' + cx + ',' + cy;
    if (!P._c[key]) P._c[key] = { flat: flatPx(P, P.out, boxF, cx, cy), roll: rollPx(P, P.out, boxR, cx, cy),
      hf: P.hole ? flatPx(P, P.hole, boxF, cx, cy) : null, hr: P.hole ? rollPx(P, P.hole, boxR, cx, cy) : null };
    return P._c[key];
  }
  // convex hull: a rolled cylinder or cone projects to a convex silhouette; its steel is the hull of the rolled outline
  function hullOf(pts) {
    const P = pts.slice().sort((a, b) => a[0] - b[0] || a[1] - b[1]);
    const cr = (o, a, b) => (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    const lo = [], up = [];
    for (const p of P) { while (lo.length >= 2 && cr(lo[lo.length - 2], lo[lo.length - 1], p) <= 0) lo.pop(); lo.push(p); }
    for (let i = P.length - 1; i >= 0; i--) { const p = P[i]; while (up.length >= 2 && cr(up[up.length - 2], up[up.length - 1], p) <= 0) up.pop(); up.push(p); }
    up.pop(); lo.pop(); return lo.concat(up);
  }
  // a plate, or a rolled part: the cut line; once rolled, the light-metal silhouette; the opening is an accent outline, never filled
  function drawPart(pts, hole, lam, rollPts) {
    if (lam > 0.001 && rollPts) {
      const hl = hullOf(rollPts), xs = hl.map((p) => p[0]), x0 = Math.min(...xs), x1 = Math.max(...xs);
      const g = ctx.createLinearGradient(x0, 0, x1, 0);
      g.addColorStop(0, '#9AA4B1'); g.addColorStop(0.3, '#EEF1F5'); g.addColorStop(0.55, '#C9D0D9'); g.addColorStop(0.8, '#E4E8EE'); g.addColorStop(1, '#9AA4B1');
      ctx.save(); ctx.globalAlpha = lam; F.polyFill(hl, g); ctx.restore();
    }
    ctx.save(); ctx.globalAlpha = 1 - lam * 0.5; F.line(pts, { closed: true, color: lam < 0.5 ? C.cut : C.ink2, lw: 5 }); ctx.restore();
    if (hole) F.line(hole, { closed: true, color: C.cut, lw: 5 });
  }

  // ---------- the steel tee: the one hero object. Drawn once, here; the hook and the checks scene use this same drawing.
  const TEE = { x0: 230, x1: 1690, yc: 560, R: 175, xb: 1000, rb: 115, topB: 120 };
  function drawTee(o) {
    const P = TEE, dx = o.dx || 0, dy = o.dy || 0, lift = o.lift || 0;
    const seat = P.yc - P.R, foot = seat - lift + dy, xb = P.xb + dx, x0 = P.x0 + dx, x1 = P.x1 + dx;
    const gb = ctx.createLinearGradient(xb - P.rb, 0, xb + P.rb, 0);
    gb.addColorStop(0, '#8E98A6'); gb.addColorStop(0.3, '#EEF1F5'); gb.addColorStop(0.55, '#C9D0D9'); gb.addColorStop(0.85, '#A3ACB8'); gb.addColorStop(1, '#7B8694');
    ctx.fillStyle = gb; ctx.fillRect(xb - P.rb, P.topB + dy, 2 * P.rb, foot - (P.topB + dy));
    ctx.fillStyle = '#DDE2E8'; ctx.beginPath(); ctx.ellipse(xb, P.topB + dy, P.rb, P.rb * 0.3, 0, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = '#2A313C'; ctx.beginPath(); ctx.ellipse(xb, P.topB + dy + 1, P.rb * 0.78, P.rb * 0.22, 0, 0, Math.PI * 2); ctx.fill();
    const gh = ctx.createLinearGradient(0, P.yc - P.R, 0, P.yc + P.R);
    gh.addColorStop(0, '#FBFCFD'); gh.addColorStop(0.3, '#DDE2E8'); gh.addColorStop(0.62, '#B9C1CB'); gh.addColorStop(0.9, '#8E98A6'); gh.addColorStop(1, '#7B8694');
    ctx.fillStyle = gh; ctx.fillRect(x0, P.yc - P.R, x1 - x0, 2 * P.R);
    ctx.fillStyle = '#E4E8EE'; ctx.beginPath(); ctx.ellipse(x1, P.yc, P.R * 0.34, P.R, 0, 0, Math.PI * 2); ctx.fill();
    ctx.fillStyle = '#2A313C'; ctx.beginPath(); ctx.ellipse(x1 + 2, P.yc, P.R * 0.24, P.R * 0.8, 0, 0, Math.PI * 2); ctx.fill();
    ctx.save(); ctx.strokeStyle = C.ink3; ctx.lineWidth = 5; ctx.setLineDash([18, 14]);
    ctx.beginPath(); ctx.ellipse(xb, seat, P.rb, 34, 0, 0, Math.PI); ctx.stroke(); ctx.restore();
    if (o.flash) {
      ctx.save(); ctx.globalAlpha = 0.9; ctx.fillStyle = '#FFFFFF';
      ctx.fillRect(xb - P.rb - 30, (seat + foot) / 2 - 6, 2 * P.rb + 60, 12);
      ctx.globalAlpha = 0.35; ctx.fillRect(xb - P.rb, P.topB + dy, 2 * P.rb, foot - (P.topB + dy)); ctx.fillRect(x0, seat, x1 - x0, 2 * P.R);
      ctx.restore();
    }
    return { seat, foot, xb };
  }

  // ================= SCENE 1: HOOK (0-4 s) =================
  function s1(t) {
    if (t < 0 || t >= 4.0) return;
    const push = 1 + 0.04 * (t / 4.0);
    const dx = -420 * (1 - F.IO(t, 0.0, 0.5));
    const dy = -90 * (1 - F.IO(t, 0.5, 1.0));
    const lift = 14 * F.IO(t, 1.8, 2.2) * (1 - F.IO(t, 3.2, 3.6));
    ctx.save(); ctx.translate(960, 560); ctx.scale(push, push); ctx.translate(-960, -560);
    const J = drawTee({ dx, dy, lift, flash: t >= 2.0 && t < 2.15 });
    ctx.save(); ctx.strokeStyle = C.signal; ctx.lineWidth = 10; ctx.lineCap = 'round';
    ctx.beginPath(); ctx.ellipse(J.xb, J.foot, TEE.rb + 16, 38, 0, 0, Math.PI * F.IO(t, 0.5, 1.0)); ctx.stroke(); ctx.restore();
    ctx.restore();
    F.enter(0.2, () => F.txt(F.on('hook')[0], 960, 930, { size: F.HS, align: 'center' }));
    F.src(F.on('label_vis'), 1.0, 4.0);
  }

  // ================= SCENE 2: QUESTION (4-8 s) =================
  function s2(t) {
    if (t < 4.0 || t >= 8.0) return;
    const pts = fit(G.header.holes[0], [1150, 430], 960, 770);
    F.line(pts, { closed: true, p: F.IO(t, 4.3, 5.3), color: C.cut, lw: 12 });
    if (t >= 5.8) {
      const u = F.U01(t, 5.8, 7.8), idx = Math.min(pts.length - 1, Math.floor(u * (pts.length - 1))), p = pts[idx];
      ctx.save(); ctx.fillStyle = C.cut; ctx.beginPath(); ctx.arc(p[0], p[1], 16, 0, Math.PI * 2); ctx.fill(); ctx.restore();
    }
    F.enter(4.0, () => F.lines(F.on('question'), 120, 300, 140, { size: F.HS, align: 'left' }));
    F.src(F.on('label_vis'), 4.5, 8.0);
  }

  // ================= SCENE 3: PRODUCT (8-12 s) =================
  function s3(t) {
    if (t < 8.0 || t >= 12.0) return;
    if (t < 10.0) {
      F.enter(8.0, () => F.lines([F.on('product_a')[0], F.on('product_b')[0]], 120, 330, 140, { size: F.HS, align: 'left' }));
      const pF = fit(G.header.outline, [560, 380], 1380, 720);
      const hF = fit(G.header.outline.concat(G.header.holes[0]), [560, 380], 1380, 720).slice(G.header.outline.length);
      F.line(pF, { closed: true, p: F.IO(t, 8.5, 9.5), color: C.cut, lw: 6 });
      F.line(hF, { closed: true, p: F.IO(t, 9.5, 10.0), color: C.cut, lw: 5 });
      return;
    }
    const k = 1 + 0.02 * F.IO(t, 11.0, 11.4);
    const bx = 260, by = 16, bw = 1400, bh = 1000;
    ctx.save(); ctx.translate(bx + bw / 2, by + bh / 2); ctx.scale(k, k); ctx.translate(-(bx + bw / 2), -(by + bh / 2));
    F.footCrop('result', 33, 232, 106, 1224, 874, bx, by, bw, bh, 0);
    ctx.restore();
    F.src(F.on('label_actual'), 10.0, 12.0);
  }

  // ================= SCENE 4: FIVE PARTS (12-20 s) =================
  const PART_T = [12.0, 13.5, 15.0, 16.5, 18.0];
  const QPICK = { shell: 'developed_width', cone: 'slant_length', ecc: 'rho_at_0', branch: 'developed_width', header: 'developed_width' };
  function s4(t) {
    if (t < 12.0 || t >= 20.0) return;
    let k = 0; for (let i = 0; i < PART_T.length; i++) if (t >= PART_T[i]) k = i;
    const P = parts()[k], t0 = PART_T[k];
    const S = shapeOf(P, [1500, 380], [1000, 380], 960, 620);
    const lam = F.IO(t, t0 + 0.25, t0 + 1.0);
    const pts = lerpPts(S.flat, S.roll, lam), hole = P.hole ? lerpPts(S.hf, S.hr, lam) : null;
    const beats = Math.max(0, Math.floor((t - t0 - 0.8) / 0.5));
    const ea = 0.6 + 0.4 * F.U01(t, t0, t0 + 4 / 30);
    ctx.save(); ctx.globalAlpha = ea; ctx.translate(960, 620); ctx.rotate(0.035 * beats * lam); ctx.translate(-960, -620);
    drawPart(pts, hole, lam, S.roll);
    ctx.restore();
    const qk = QPICK[P.key], qv = P.g.quantities[qk];
    if (qv !== undefined) F.txt(`${F.qname(qk)}  ${F.mm(qv)} ${F.on('unit_mm')}`, 960, 866, { size: F.BS, w: 500, color: C.ink, align: 'center', a: ea });
    F.enter(t0, () => F.lines(F.wrap(F.on(P.label), F.HS, 1680, 600), 960, 250, 140, { size: F.HS, align: 'center' }));
    F.txt(`${k + 1} / 5`, 1800, 92, { size: F.LS, w: 600, color: C.ink, align: 'right', a: ea });
    const lim = F.on('limits');
    F.txt(lim[0], 960, 935, { size: F.BS, w: 500, color: C.ink, align: 'center', a: ea });
    F.txt(lim[1], 960, 990, { size: F.BS, w: 500, color: C.ink, align: 'center', a: ea });
    F.src(F.on('label_vis'), 12.0, 20.0);
  }

  // ================= SCENE 5: CHECKS (20-26 s) =================
  // the job 6 check entries from the data (the current job): one row per check; the heading and the first counter are there from 20.0
  const ROW0 = 20.0, ROW_STEP = 0.12, ROWS_VIS = 4;
  function s5(t) {
    if (t < 20.0 || t >= 26.0) return;
    const fold = F.IO(t, 20.4, 21.6);
    ctx.save(); ctx.translate(480, 640); ctx.scale(0.5, 0.5); ctx.translate(-960, -560);
    const J = drawTee({ lift: 130 * (1 - fold) });
    if (t >= 21.6) {
      ctx.save(); ctx.globalAlpha = F.IO(t, 21.6, 22.0); ctx.strokeStyle = C.cut; ctx.lineWidth = 20; ctx.lineCap = 'round';
      ctx.beginPath(); ctx.ellipse(TEE.xb, J.seat, TEE.rb, 34, 0, 0, Math.PI * 2); ctx.stroke(); ctx.restore();
    }
    if (t >= 24.0 && t < 24.5) {
      ctx.save(); ctx.globalAlpha = Math.sin(Math.PI * F.U01(t, 24.0, 24.5)); ctx.strokeStyle = C.cut; ctx.lineWidth = 14;
      ctx.beginPath(); ctx.ellipse(TEE.xb, J.seat, TEE.rb + 30, 52, 0, 0, Math.PI * 2); ctx.stroke(); ctx.restore();
    }
    ctx.restore();
    F.lines(F.on('checks_big'), 120, 260, 130, { size: F.HS, align: 'left' });
    const CX = 900, CY = 420, CW = 900, CH = 540;
    F.card(CX, CY, CW, CH, { r: 14 });
    const checks = D.job6.checkList, N = checks.length;
    const nDone = Math.min(N, Math.max(1, Math.floor((t - ROW0) / ROW_STEP) + 1));
    F.txt(F.on('checks_count').replace('{n}', String(nDone)).replace('{b}', String(N)), CX + 60, CY + 100, { size: F.BS, w: 600, color: C.ink });
    const maxName = CW - 130 - 40;
    for (let i = 0; i < nDone; i++) {
      const first = Math.max(0, nDone - ROWS_VIS), j = i - first;
      if (j < 0) continue;
      const T = ROW0 + i * ROW_STEP, a = i === 0 ? 1 : F.IO(t, T, T + 0.2), y = CY + 190 + j * 70;
      const c = checks[i], name = F.JA ? (c.ja || c.name) : c.name;
      const beat = Math.floor(t * 2) / 2, pulse = t >= 23.5 && t < 26.0 ? F.pulse(t, beat) : 1;
      ctx.save(); ctx.globalAlpha = a; ctx.fillStyle = C.green; ctx.beginPath(); ctx.arc(CX + 70, y - 14, 17 * pulse, 0, Math.PI * 2); ctx.fill();
      ctx.strokeStyle = C.white; ctx.lineWidth = 4; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
      ctx.beginPath(); ctx.moveTo(CX + 62, y - 14); ctx.lineTo(CX + 68, y - 8); ctx.lineTo(CX + 79, y - 22); ctx.stroke();
      ctx.restore();
      const w = F.tw(name, F.LS, 500), sx = w > maxName ? maxName / w : 1;
      ctx.save(); ctx.globalAlpha = a; ctx.translate(CX + 110, y); ctx.scale(sx, 1); F.txt(name, 0, 0, { size: F.LS, w: 500, color: C.ink }); ctx.restore();
    }
    const sub = F.on('checks_sub');
    F.txt(sub[0], CX + 60, CY + CH - 84, { size: F.LS, w: 500, color: C.ink2 });
    F.txt(sub[1], CX + 60, CY + CH - 36, { size: F.LS, w: 500, color: C.ink2 });
    F.src(F.on('label_sim'), 20.0, 26.0);
  }

  F.scenes.push(s1, s2, s3, s4, s5);
})();
