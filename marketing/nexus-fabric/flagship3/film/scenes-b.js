// Scenes 6-10 (26-54 s). Revision 5 (lead's review): every incoming hero is on screen on its bar frame (60 %) and fully there
// four frames later; no frame inside a slot is empty. Every number, name and verdict comes from window.NF_DATA.
(function () {
  'use strict';
  const F = window.F3, ctx = F.ctx, C = F.C, D = F.D, G = D.geo;
  const seg = (a, b, p) => [a, [F.lerp(a[0], b[0], p), F.lerp(a[1], b[1], p)]];   // a segment drawn part way (p 0..1)
  const fitMap = (pts, box, cx, cy) => {
    let a = [Infinity, Infinity, -Infinity, -Infinity];
    for (const p of pts) { a[0] = Math.min(a[0], p[0]); a[1] = Math.min(a[1], p[1]); a[2] = Math.max(a[2], p[0]); a[3] = Math.max(a[3], p[1]); }
    const K = Math.min(box[0] / (a[2] - a[0]), box[1] / (a[3] - a[1])), ux = (a[0] + a[2]) / 2, vy = (a[1] + a[3]) / 2;
    return (p) => [cx + (p[0] - ux) * K, cy - (p[1] - vy) * K];
  };
  const fit = (pts, box, cx, cy) => { const m = fitMap(pts, box, cx, cy); return pts.map(m); };
  const enterA = (t0) => 0.6 + 0.4 * F.U01(F.t, t0, t0 + 4 / 30);   // the alpha of an incoming element

  // ================= SCENE 6: DXF (26-30 s) =================
  const PL = { x: 150, y: 500, w: 560, h: 420 }, PR = { x: 1210, y: 500, w: 560, h: 420 };
  function s6(t) {
    if (t < 26.0 || t >= 30.0) return;
    F.enter(26.0, () => F.lines(F.on('dxf_big'), 120, 300, 140, { size: F.HS, align: 'left' }));
    const ea = enterA(26.0);
    const mA = fitMap(G.header.outline, [560, 380], PL.x + 280, PL.y + 210), mB = fitMap(G.header6.outline, [560, 380], PR.x + 280, PR.y + 210);
    const pA = G.header.outline.map(mA), pB = G.header6.outline.map(mB);
    const hA = G.header.holes[0].map(mA), hB = G.header6.holes[0].map(mB);
    F.card(PL.x, PL.y, PL.w, PL.h, { sh: false, r: 10 });
    F.card(PR.x, PR.y, PR.w, PR.h, { sh: false, r: 10 });
    ctx.save(); ctx.globalAlpha = ea; F.line(pA, { closed: true, color: C.ink2, lw: 4 }); F.line(hA, { closed: true, color: C.ink2, lw: 3 }); ctx.restore();
    ctx.save(); ctx.globalAlpha = ea; F.line(pB, { closed: true, color: C.ink2, lw: 4 }); F.line(hB, { closed: true, color: C.ink2, lw: 3 }); ctx.restore();
    F.line(pB, { closed: true, p: F.IO(t, 26.6, 27.6), color: C.cut, lw: 9 });
    if (t >= 27.6) {
      F.line(seg([930, 730], [956, 758], F.IO(t, 27.6, 27.85)), { color: C.green, lw: 12 });
      F.line(seg([956, 758], [1004, 702], F.IO(t, 27.85, 28.1)), { color: C.green, lw: 12 });
    }
    const rows = D.dxf.rows.filter((r) => r.length > 0 && r.length < 40);
    ctx.save(); ctx.beginPath(); ctx.rect(150, 934, 1620, 68); ctx.clip();
    ctx.globalAlpha = 1 - F.IO(t, 26.6, 26.9);
    for (let j = 0; j < 4; j++) {
      const y = 972 + j * 40 - Math.max(0, Math.min(1, (t - 26.0) / 0.6)) * 144;   // 6 lines a second: 3.6 rows of 40 px over 26.0-26.6 s
      F.txt(rows[j] || '', 150, y, { size: F.LS, mono: true, color: C.ink2 });
    }
    ctx.restore();
    const sd = F.SP(t, 28.5, 14, 0.8);
    if (t >= 28.5) {
      const yo = -(1 - sd) * 120, pl = F.pulse(t, 29.0);
      ctx.save(); ctx.globalAlpha = Math.min(1, sd); ctx.translate(1620, 332 + yo); ctx.scale(pl, pl); ctx.translate(-1620, -(332 + yo));
      ctx.strokeStyle = C.amber; ctx.lineWidth = 5; F.rr(1450, 300 + yo, 340, 64, 8); ctx.stroke();
      F.txt(F.ui('tb.nr'), 1620, 343 + yo, { size: F.LS, w: 700, color: C.amber, align: 'center' });
      ctx.restore();
      F.txt(D.dxf.name, 960, 968, { size: F.LS, w: 400, color: C.amber, mono: true, align: 'center', a: F.IO(t, 28.5, 28.8) });
    }
    F.src(F.on('label_vis'), 26.0, 30.0);
  }

  // ================= SCENE 7: AMOEBA (30-42 s) =================
  // the simple graph: the corrected parameter and five real nodes on its path. Product legend (web/i18n.js dg.idle):
  // amber = what a record depends on, blue = what a change to it reaches. Downstream records are blue here.
  const SIMPLE = D.simple, NW = 360, NH = 100;
  const NPOS = { 'parameter · header_radius': [120, 650], 'intersection_curve': [520, 650], 'pattern · header': [920, 650],
    'dxf_output · header': [1320, 650], 'part · header': [520, 830], 'interface · header-branch joint': [920, 830] };
  const FLIP = {}; SIMPLE.flagOrder.forEach((id, k) => { FLIP[id] = 34.5 + 0.5 * k; });     // one reached record per beat
  const cen = (id) => [NPOS[id][0] + NW / 2, NPOS[id][1] + NH / 2];
  function drawGraph(t) {
    for (const e of SIMPLE.edges) {
      const T = FLIP[e.from]; if (T === undefined) continue;
      F.line(seg(cen(e.from), cen(e.to), F.IO(t, T, T + 0.4)), { color: C.cut, lw: 5 });
    }
    for (const n of SIMPLE.nodes) {
      const [x, y] = NPOS[n.id], isSrc = n.id === SIMPLE.source, T = FLIP[n.id];
      const appear = isSrc ? 0.6 + 0.4 * F.U01(t, 33.0, 33.0 + 4 / 30) : F.IO(t, 33.3, 33.6);
      if (appear <= 0.001) continue;
      const k = T !== undefined ? F.IO(t, T, T + 0.25) : 0;
      ctx.save(); ctx.globalAlpha = Math.min(1, appear);
      F.card(x, y, NW, NH, { sh: false, r: 10, fill: k > 0.5 ? '#EAF1FC' : C.white, stroke: isSrc ? C.ink : (k > 0 ? C.cut : C.rule) });
      ctx.restore();
      const lines = F.wrap(F.LANG === 'ja' ? n.ja : n.en, F.LS, NW - 30, 500);
      lines.forEach((ln, i) => F.txt(ln, x + NW / 2, y + NH / 2 + (i - (lines.length - 1) / 2) * 44 + 14, { size: F.LS, w: 500, color: k > 0.5 ? C.cut : C.ink, align: 'center', a: Math.min(1, appear) }));
    }
  }
  function beatA(t) {                                                                        // 30-33 s: the definition and three linked records
    F.enter(30.0, () => F.lines(F.on('amoeba_big'), 120, 300, F.HS, { size: F.HS, align: 'left' }));
    F.enter(30.0, () => F.lines(F.on('amoeba_def'), 120, 420, 62, { size: F.BS, align: 'left' }));
    const req = D.job5.request, keys = ['header_radius', 'header_length', 'branch_station'];
    keys.forEach((key, i) => {
      const x = 120 + i * 600, T = 30.5 + 0.5 * i, a = F.IO(t, T, T + 0.25), y = 600 + (1 - a) * 16;
      ctx.save(); ctx.globalAlpha = a;
      F.card(x, y, 480, 230, { sh: false, r: 10 });
      F.wrap(F.pname(key), F.LS, 408, 500).forEach((ln, li) => F.txt(ln, x + 36, y + 64 + li * 46, { size: F.LS, w: 500, color: C.ink2 }));
      F.txt(`${F.mm(req[key])} mm`, x + 36, y + 190, { size: F.BS, w: 600, color: C.ink });
      ctx.restore();
    });
    [[32.0, 0], [32.5, 1]].forEach(([T, i]) => F.line(seg([120 + i * 600 + 480, 715], [120 + (i + 1) * 600, 715], F.IO(t, T, T + 0.3)), { color: C.cut, lw: 6 }));
    F.src(F.on('label_sim'), 30.0, 33.0);
  }
  function beatB(t) {                                                                        // 33-37 s: the change, what it reaches
    F.enter(33.0, () => F.lines(F.on('change_big'), 120, 260, F.HS, { size: F.HS, align: 'left' }));
    const c = D.job6.corr, tot = Object.values(D.oblCounts).reduce((s, v) => s + v, 0);
    if (t >= 33.5) F.txt(F.on('corr_line').replace('{o}', F.mm(c.old)).replace('{n}', F.mm(c.new)), 120, 400, { size: F.BS, w: 600, color: C.ink, a: F.IO(t, 33.5, 33.75) });
    if (t >= 34.0) F.txt(F.on('obl_line').replace('{r}', String(tot)).replace('{d}', String(D.oblCounts.discharged)).replace('{o}', String(D.oblCounts.open)), 120, 470, { size: F.BS, w: 500, color: C.ink2, a: F.IO(t, 34.0, 34.25) });
    drawGraph(t);
    F.txt(F.on('rule_flag'), 120, 1000, { size: F.LS, w: 500, color: C.ink2, a: F.IO(t, 36.5, 36.75) });
    F.src(F.on('label_sim'), 33.0, 37.0);
  }
  function beatC(t) {                                                                        // 37-40 s: the real dependency view, borders kept
    const k = 1 + 0.02 * F.IO(t, 38.0, 38.4) + 0.01 * F.IO(t, 39.0, 39.4);
    ctx.save(); ctx.translate(960, 540); ctx.scale(k, k); ctx.translate(-960, -540);
    F.footCrop('am_post', 250, 0, 0, 1920, 735, 60, 200, 1800, 690, 0);
    ctx.restore();
    F.src(F.on('label_actual'), 37.0, 40.0);
  }
  function beatD(t) {                                                                        // 40-42 s: the count, then the verdict; never empty
    const total = D.job6.replay.checked;
    const tail = F.on('rec_count').replace('{n}', '').trim();
    if (t < 41.0) {
      const n = Math.min(total, Math.floor((t - 40.0) / 0.024) + 1);
      F.enter(40.0, () => { F.txt(String(n), 120, 470, { size: F.HS, w: 600, color: C.ink }); F.txt(n === 1 ? tail.replace('records', 'record') : tail, 120, 540, { size: F.BS, w: 500, color: C.ink2 }); });
    } else {
      F.enter(41.0, () => { F.txt(F.on('replay_big')[0], 120, 300, { size: F.HS, w: 600, color: C.ink }); F.txt(F.on('rec_count').replace('{n}', String(total)), 120, 420, { size: F.BS, w: 500, color: C.ink2 }); });
    }
    F.src(F.on('label_sim'), 40.0, 42.0);
  }
  function s7(t) {
    if (t < 30.0 || t >= 42.0) return;
    if (t < 33.0) beatA(t);
    else if (t < 37.0) beatB(t);
    else if (t < 40.0) beatC(t);
    else beatD(t);
  }

  // ================= SCENE 8: HONESTY (42-46 s) =================
  const ORDER = ['PASS', 'INCONCLUSIVE', 'NOT_IMPLEMENTED'];
  function s8(t) {
    if (t < 42.0 || t >= 46.0) return;
    F.enter(42.0, () => F.lines(F.on('honesty_big'), 120, 260, 140, { size: F.HS, align: 'left' }));
    const st = F.SP(t, 43.0, 12, 0.8);
    if (st > 0.001) {
      ctx.save(); ctx.globalAlpha = Math.min(1, st); ctx.translate(350, 560); ctx.scale(0.94 + 0.06 * st, 0.94 + 0.06 * st);
      ctx.strokeStyle = C.amber; ctx.lineWidth = 5; F.rr(-230, -42, 460, 84, 10); ctx.stroke();
      F.txt(F.ui('state.REVIEW_REQUIRED'), 0, 14, { size: F.LS, w: 700, color: C.amber, align: 'center', tr: 0.04 });
      ctx.restore();
    }
    const cats = ORDER.filter((k) => (D.gateCounts6[k] || 0) > 0);
    cats.forEach((k, i) => {
      const x = 120 + i * 560, n = D.gateCounts6[k];
      const shown = t >= 42.6 ? Math.min(n, Math.floor((t - 42.6) / (0.8 / n)) + 1) : 0;
      const sc = (t >= 44.0 && t < 44.5 ? F.pulse(t, 44.0) : 1) * (t >= 45.0 && t < 45.5 ? F.pulse(t, 45.0) : 1);
      const col = k === 'PASS' ? C.green : (k === 'NOT_IMPLEMENTED' ? C.amber : C.ink);
      ctx.save(); ctx.translate(x, 740); ctx.scale(sc, sc);
      if (shown >= 1) F.txt(String(shown), 0, 0, { size: F.HS, w: 600, color: col });
      ctx.restore();
      const label = k === 'PASS' ? F.on('res_pass') : F.ui('result.' + k);
      F.txt(label, x, 810, { size: F.LS, w: 600, color: C.ink });
      if (k === 'NOT_IMPLEMENTED') F.wrap(F.on('gate_approval'), F.LS, 540, 500).forEach((ln, li) => F.txt(ln, x, 860 + li * 46, { size: F.LS, w: 500, color: C.ink2 }));
    });
    F.txt(F.on('dev_line'), 120, 982, { size: F.BS, w: 600, color: C.ink, a: F.IO(t, 42.6, 42.8) });
    F.src(F.on('label_sim'), 42.0, 46.0);
  }

  // ================= SCENE 9: LANGUAGE AND OFFLINE (46-50 s) =================
  const GRID = ['tab.checks', 'tab.gates', 'tab.records', 'tab.scrutiny', 'tab.graph', 'tab.changes', 'th.replay',
    'state.REVIEW_REQUIRED', 'state.SUPERSEDED', 'result.PASS', 'result.INCONCLUSIVE', 'result.NOT_IMPLEMENTED'];
  function s9(t) {
    if (t < 46.0 || t >= 50.0) return;
    if (t < 48.0) {
      ctx.save(); ctx.globalAlpha = 1 - F.IO(t, 47.8, 47.95);
      F.enter(46.0, () => F.txt(F.on('lang_big')[0], 960, 250, { size: F.HS, align: 'center' }));
      const onJa = t >= 47.0;
      ctx.fillStyle = C.ink; F.rr(onJa ? 960 : 680, 340, 280, 84, 42); ctx.fill();
      F.txt(F.ui('lang_en'), 820, 396, { size: F.LS, w: 600, color: onJa ? C.ink2 : C.white, align: 'center' });
      F.txt(F.ui('lang_ja'), 1100, 396, { size: F.LS, w: 600, color: onJa ? C.white : C.ink2, align: 'center' });
      const appear = enterA(46.0);
      GRID.forEach((k, i) => {
        const c = i % 3, r = Math.floor(i / 3), x = 120 + 580 * c, y = 500 + 110 * r;
        const flip = r < 2 ? 47.0 : 47.5, f = F.IO(t, flip, flip + 0.2);
        ctx.save(); ctx.globalAlpha = appear; F.card(x, y, 520, 90, { sh: false, r: 10 });
        F.txt(D.ui.en[k], x + 260, y + 56, { size: F.LS, w: 500, color: C.ink, align: 'center', a: 1 - f, lang: 'en' });
        F.txt(D.ui.ja[k], x + 260, y + 56, { size: F.LS, w: 600, color: C.ink, align: 'center', a: f, lang: 'ja' });
        ctx.restore();
      });
      ctx.restore();
    }
    if (t >= 48.0) {
      F.enter(48.0, () => F.txt(F.on('offline')[0], 120, 330, { size: F.HS, align: 'left' }));
      const gx = 1480, gy = 760, sl = F.IO(t, 48.5, 49.0), gA = F.IO(t, 48.12, 48.32);
      ctx.save(); ctx.globalAlpha = gA; ctx.strokeStyle = C.ink2; ctx.lineWidth = 22; ctx.lineCap = 'round';
      for (const r of [120, 240, 360]) { ctx.beginPath(); ctx.arc(gx, gy, r, Math.PI * 1.25, Math.PI * 1.75); ctx.stroke(); }
      ctx.fillStyle = C.ink2; ctx.beginPath(); ctx.arc(gx, gy + 20, 24, 0, Math.PI * 2); ctx.fill();
      ctx.restore();
      F.line(seg([gx - 330, gy - 330], [gx + 330, gy + 330], sl), { color: C.signal, lw: 22 });
    }
    F.src(F.on('label_sim'), 46.0, 50.0);
  }

  // ================= SCENE 10: END CARD (50-54 s) =================
  // the owner's two contact addresses sit directly under the call to action; the locked disclaimer stays on screen for the card
  function s10(t) {
    if (t < 50.0 || t >= 54.0) return;
    const mk = window.NFG && window.NFG.mark ? window.NFG.mark() : null;
    if (mk) {
      const S = 7, mx = (x) => 960 + (x - 8) * S, my = (y) => 205 + (y - 8) * S;
      F.line(mk.legs.map((p) => [mx(p[0]), my(p[1])]), { p: F.IO(t, 50.0, 50.3), color: C.ink, lw: 10 });
      F.line(mk.arc.map((p) => [mx(p[0]), my(p[1])]), { p: F.IO(t, 50.2, 50.5), color: C.ink, lw: 10 });
    }
    F.enter(50.0, () => F.txt(F.on('brand_name')[0], 960, 400, { size: F.HS, align: 'center' }));
    F.enter(50.4, () => F.txt(F.on('brand_line'), 960, 470, { size: F.BS, align: 'center', color: C.ink2 }));
    if (t >= 50.4) {
      const sp = F.SP(t, 50.4, 12, 0.8), pl = (t >= 52.0 && t < 52.5 ? F.pulse(t, 52.0) : 1) * (t >= 53.0 && t < 53.5 ? F.pulse(t, 53.0) : 1);
      const sc = (0.92 + 0.08 * Math.min(1, sp)) * pl;
      ctx.save(); ctx.translate(960, 560); ctx.scale(sc, sc);
      ctx.fillStyle = C.ink; F.rr(-380, -55, 760, 110, 55); ctx.fill();
      F.txt(F.on('cta'), 0, 16, { size: F.BS, w: 600, color: C.white, align: 'center' });
      ctx.restore();
    }
    const ca = enterA(50.0);
    F.on('contact').forEach((line, i) => F.txt(line, 960, 672 + i * 46, { size: F.LS, w: 500, color: C.ink, align: 'center', a: ca }));
    F.txt(F.on('disclaimer'), 960, 806, { size: F.LS, w: 600, color: C.ink, align: 'center', a: ca });
    F.txt(F.on('positioning'), 960, 866, { size: F.LS, w: 500, color: C.ink2, align: 'center', a: F.IO(t, 52.5, 53.0) });
    if (F.LANG === 'ja') F.txt(F.on('credit'), 1800, 1040, { size: F.LS, w: 500, color: C.ink2, align: 'right' });
  }

  F.scenes.push(s6, s7, s8, s9, s10);
})();
