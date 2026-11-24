// NEXUS FABRIC flagship2 — drawn graphics: the joint as thin shells, the flat patterns, the DXF text and plot,
// the mark. Geometry comes from the job's request and from the product's own vertex lists; the 3D views are
// illustrations made for the film (the product has no 3D view) and are labelled so on screen.
'use strict';
const NFG = (window.NFG = {});
(() => {
  const { clamp, lerp } = window.NFE;
  const TAU = Math.PI * 2;

  // ---- the joint. Axes: X along the header, Z up through the branch, Y across. delta = 0 in the reference job.
  function joint(req) {
    const R = req.header_radius, r = req.branch_radius, beta = (req.beta_deg * Math.PI) / 180;
    const J = { R, r, beta, L: req.header_length, st: req.branch_station, send: req.s_end, cb: Math.cos(beta), sb: Math.sin(beta) };
    // governing equation of the branch cut: distance along the branch axis to the header surface
    J.s = (phi) => (Math.sqrt(R * R - Math.pow(r * Math.cos(phi), 2)) - r * Math.sin(phi) * J.cb) / J.sb;
    // branch pattern point (x = r*phi, y = distance along the branch axis) on a sheet rolled by k (0 flat .. 1 closed)
    J.branch = (x, y, k = 1) => {
      const u = x - Math.PI * r, kap = k / r, lat = kap > 1e-7 ? Math.sin(u * kap) / kap : u, dep = kap > 1e-7 ? (1 - Math.cos(u * kap)) / kap : 0;
      return [J.st + y * J.cb + lat * J.sb, -r + dep, y * J.sb - lat * J.cb];
    };
    J.branchN = (x, k = 1) => { const a = ((x - Math.PI * r) * k) / r, sa = Math.sin(a), ca = Math.cos(a); return [sa * J.sb, -ca, -sa * J.cb]; };   // outward normal
    // header pattern point (x along the axis, y around) on a sheet rolled by k
    J.header = (x, y, k = 1) => {
      const w = y - Math.PI * R, kap = k / R, lat = kap > 1e-7 ? Math.sin(w * kap) / kap : w, dep = kap > 1e-7 ? (1 - Math.cos(w * kap)) / kap : 0;
      return [x, lat, R - dep];
    };
    J.headerN = (y, k = 1) => { const a = ((y - Math.PI * R) * k) / R; return [0, Math.sin(a), Math.cos(a)]; };
    return J;
  }
  NFG.joint = joint;

  // ---- camera: yaw about Z, pitch down, optional perspective. -> [X, Y, depth]
  function camera(c) {
    const cy = Math.cos(c.yaw), sy = Math.sin(c.yaw), cp = Math.cos(c.pitch), sp = Math.sin(c.pitch), T = c.target || [0, 0, 0];
    const f = (p) => {
      const x = p[0] - T[0], y = p[1] - T[1], z = p[2] - T[2];
      const x1 = x * cy - y * sy, y1 = x * sy + y * cy;
      const up = y1 * sp + z * cp, depth = y1 * cp - z * sp;
      const q = c.dist ? c.dist / (c.dist + depth) : 1;
      return [c.cx + c.scale * x1 * q, c.cy - c.scale * up * q, depth];
    };
    f.view = [sy * cp, cy * cp, -sp];                        // world direction the camera looks along
    f.front = (n) => n[0] * f.view[0] + n[1] * f.view[1] + n[2] * f.view[2] < 0;
    return f;
  }
  NFG.camera = camera;

  // ---- strokes with a soft glow (three passes; no canvas filters)
  function stroke(ctx, pts, o = {}) {
    if (pts.length < 2) return;
    const w = o.w ?? 2, a = o.alpha ?? 1; if (a <= 0.003) return;
    ctx.save(); ctx.lineJoin = 'round'; ctx.lineCap = o.cap || 'round';
    if (o.dash) ctx.setLineDash(o.dash);
    const path = () => { ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]); if (o.closed) ctx.closePath(); };
    // no glows: strokes are flat (the film uses no decorative halo)
    ctx.globalAlpha = a; ctx.strokeStyle = o.color; ctx.lineWidth = w; path(); ctx.stroke(); ctx.restore();
  }
  // draw a 3D polyline, splitting it into runs facing the camera (bright) and facing away (faint)
  function stroke3(ctx, cam, pts3, nrm, o = {}) {
    let run = [], state = null; const flush = () => { if (run.length > 1) stroke(ctx, run, state ? o : { ...o, alpha: (o.alpha ?? 1) * (o.back ?? 0.22), glow: 0, w: (o.w ?? 2) * 0.8 }); };
    for (let i = 0; i < pts3.length; i++) {
      const fr = nrm ? cam.front(nrm[i]) : true; const p = cam(pts3[i]);
      if (state !== null && fr !== state) { run.push(p); flush(); run = []; }
      state = fr; run.push(p);
    }
    flush();
  }
  NFG.stroke = stroke; NFG.stroke3 = stroke3;

  // ---- sheets. A sheet is one part of the joint: its pattern outline (product vertices) put on a surface rolled by k.
  // branch: pat.curves = [saddle_cut (x from 0 to 2*pi*r), seam_b, square_end, seam_a]; header: outline + holes[0]
  function branchSheet(J, pat) {
    const saddle = pat.curves.find((c) => /saddle/.test(c.name)) || pat.curves[0];
    const top = Math.max(...pat.curves.flatMap((c) => c.points.map((p) => p[1])));
    const C = 2 * Math.PI * J.r;
    return {
      kind: 'branch', saddle: saddle.points, top, C,
      outline: (n = 1) => [...saddle.points.filter((_, i) => i % n === 0 || i === saddle.points.length - 1), [C, top], [0, top]],
      pos: (x, y, k) => J.branch(x, y, k), nrm: (x, y, k) => J.branchN(x, k),
      gens: (m) => Array.from({ length: m }, (_, i) => { const x = (C * (i + 0.5)) / m; const idx = Math.round((x / C) * (saddle.points.length - 1)); return [[x, saddle.points[idx][1]], [x, top]]; }),
      rings: () => [Array.from({ length: 97 }, (_, i) => [(C * i) / 96, top])],
    };
  }
  function headerSheet(J, pat) {
    const hole = pat.holes[0], C = 2 * Math.PI * J.R, L = J.L;
    return {
      kind: 'header', hole: hole ? hole.points : [], C, L,
      outline: () => { const o = []; for (let i = 0; i <= 96; i++) o.push([0, (C * i) / 96]); for (let i = 96; i >= 0; i--) o.push([L, (C * i) / 96]); return o; },
      pos: (x, y, k) => J.header(x, y, k), nrm: (x, y, k) => J.headerN(y, k),
      gens: (m) => Array.from({ length: m }, (_, i) => { const y = (C * (i + 0.5)) / m; return [[0, y], [L, y]]; }),
      rings: () => [0, L].map((x) => Array.from({ length: 97 }, (_, i) => [x, (C * i) / 96])),
    };
  }
  NFG.branchSheet = branchSheet; NFG.headerSheet = headerSheet;

  // position of a pattern point on screen: blend of the flat layout (m = 0) and the rolled sheet seen by the camera (m = 1)
  function place(sheet, cam, lay, k, m) {
    return (p) => {
      const a = [lay.ox + lay.s * p[0], lay.oy - lay.s * p[1]];
      if (m <= 0) return a;
      const b = cam(sheet.pos(p[0], p[1], k)); return [lerp(a[0], b[0], m), lerp(a[1], b[1], m)];
    };
  }
  // draw one sheet. st: {k, m, lay, color, cut (colour of the cut curve), alpha, fill, gens, w, cutGlow, cutAlpha, seamAlpha}
  function drawSheet(ctx, sheet, cam, st) {
    const k = st.k ?? 1, m = st.m ?? 1, P = place(sheet, cam, st.lay || { ox: 0, oy: 0, s: 1 }, k, m), a = st.alpha ?? 1; if (a <= 0.003) return;
    const front = (p) => (m < 0.5 ? true : cam.front(sheet.nrm(p[0], p[1], k)));
    const line = (pts, o) => { let run = [], state = null; const out = (s) => { if (run.length > 1) stroke(ctx, run, s ? o : { ...o, alpha: o.alpha * (st.back ?? 0.2), glow: 0, w: o.w * 0.8 }); }; for (const p of pts) { const fr = front(p), q = P(p); if (state !== null && fr !== state) { run.push(q); out(state); run = []; } state = fr; run.push(q); } out(state); };
    // faint surface fill: the outline as one polygon
    if (st.fill) { const o = sheet.outline(4).map(P); ctx.save(); ctx.globalAlpha = a * st.fill; ctx.fillStyle = st.color; ctx.beginPath(); ctx.moveTo(o[0][0], o[0][1]); for (const q of o) ctx.lineTo(q[0], q[1]); ctx.closePath(); if (sheet.kind === 'header' && sheet.hole.length) { const h = sheet.hole.map(P); ctx.moveTo(h[0][0], h[0][1]); for (const q of h) ctx.lineTo(q[0], q[1]); ctx.closePath(); ctx.fill('evenodd'); } else ctx.fill(); ctx.restore(); }
    const base = { color: st.color, w: st.w ?? 1.6, alpha: a * (st.seamAlpha ?? 0.9), glow: st.glow ?? 0 };
    if (st.gens) for (const g of sheet.gens(st.gens)) line([g[0], [lerp(g[0][0], g[1][0], 0.5), lerp(g[0][1], g[1][1], 0.5)], g[1]], { ...base, alpha: a * 0.16, w: 1, glow: 0 });
    if (sheet.kind === 'branch') {
      const C = sheet.C, top = sheet.top, s0 = sheet.saddle[0][1], s1 = sheet.saddle[sheet.saddle.length - 1][1];
      line(Array.from({ length: 97 }, (_, i) => [(C * i) / 96, top]), base);                                  // square end
      const seam = 1 - clamp((k - 0.9) / 0.1);                                                                 // the two seam edges meet when closed
      line([[0, s0], [0, top]], { ...base, alpha: base.alpha * lerp(0.55, 1, seam) }); if (seam > 0) line([[C, s1], [C, top]], { ...base, alpha: base.alpha * seam });
      line(sheet.saddle, { color: st.cut || st.color, w: (st.w ?? 1.6) * (st.cutW ?? 1.5), alpha: a * (st.cutAlpha ?? 1), glow: st.cutGlow ?? 0.9 });
    } else {
      const C = sheet.C, L = sheet.L;
      for (const r of sheet.rings()) line(r, base);
      const seam = 1 - clamp((k - 0.9) / 0.1);
      line([[0, 0], [L / 2, 0], [L, 0]], { ...base, alpha: base.alpha * lerp(0.55, 1, seam) }); if (seam > 0) line([[0, C], [L / 2, C], [L, C]], { ...base, alpha: base.alpha * seam });
      if (sheet.hole.length) line([...sheet.hole, sheet.hole[0]], { color: st.cut || st.color, w: (st.w ?? 1.6) * (st.cutW ?? 1.5), alpha: a * (st.cutAlpha ?? 1), glow: st.cutGlow ?? 0.9 });
    }
  }
  NFG.place = place; NFG.drawSheet = drawSheet;

  // silhouette generators of a closed cylinder seen by the camera (the two lines where the surface turns away)
  function silhouettes(sheet, cam, nx = 180) {
    const out = [], lat = sheet.kind === 'branch' ? (i) => [(sheet.C * i) / nx, 0] : (i) => [0, (sheet.C * i) / nx];
    let prev = null;
    for (let i = 0; i <= nx; i++) { const p = lat(i), fr = cam.front(sheet.nrm(p[0], p[1], 1)); if (prev !== null && fr !== prev) out.push(i / nx); prev = fr; }
    return out;                                                                                                // fractions of the circumference
  }
  NFG.silhouettes = silhouettes;

  // ---- DXF (R12 subset as the product writes it): group-code / value pairs
  function parseDXF(text) {
    const L = text.split(/\r?\n/), pairs = []; for (let i = 0; i + 1 < L.length; i += 2) pairs.push([parseInt(L[i], 10), L[i + 1].trim(), i]);
    const out = { comment: (pairs.find((p) => p[0] === 999) || [0, ''])[1], lines: [], polylines: [], texts: [], layers: {}, entityLine: {}, rows: L.length };
    let cur = null, poly = null, inEnt = false;
    const close = () => { if (!cur) return; if (cur.type === 'LINE') out.lines.push(cur); else if (cur.type === 'TEXT') out.texts.push(cur); else if (cur.type === 'POLYLINE') { poly = cur; poly.pts = []; out.polylines.push(poly); } else if (cur.type === 'VERTEX' && poly) poly.pts.push([cur.x1, cur.y1]); else if (cur.type === 'SEQEND') poly = null; if (cur.layer && cur.type !== 'VERTEX' && cur.type !== 'SEQEND') out.layers[cur.layer] = (out.layers[cur.layer] || 0) + 1; cur = null; };
    for (const [code, val, row] of pairs) {
      if (code === 2 && val === 'ENTITIES') { inEnt = true; continue; }
      if (!inEnt) continue;
      if (code === 0) { close(); if (val === 'ENDSEC' || val === 'EOF') { inEnt = false; continue; } cur = { type: val, row }; continue; }
      if (!cur) continue;
      if (code === 8) cur.layer = val; else if (code === 10) cur.x1 = +val; else if (code === 20) cur.y1 = +val; else if (code === 11) cur.x2 = +val; else if (code === 21) cur.y2 = +val; else if (code === 40) cur.h = +val; else if (code === 1) cur.text = val; else if (code === 70) cur.flag = +val;
    }
    close(); return out;
  }
  NFG.parseDXF = parseDXF;

  // ---- the product's mark (the glyph in its top bar), as polylines in a 16-unit box
  NFG.mark = () => {
    const legs = [[2, 13], [6, 3], [10, 3], [14, 13]];
    const R = 14, x0 = 4.4, x1 = 11.6, y = 9.5, half = (x1 - x0) / 2, cx = 8, cyc = y + Math.sqrt(R * R - half * half), a0 = Math.atan2(y - cyc, x0 - cx), a1 = Math.atan2(y - cyc, x1 - cx);
    const arc = Array.from({ length: 41 }, (_, i) => { const a = lerp(a0, a1, i / 40); return [cx + R * Math.cos(a), cyc + R * Math.sin(a)]; });
    return { legs, arc };
  };
  NFG.TAU = TAU;
})();
