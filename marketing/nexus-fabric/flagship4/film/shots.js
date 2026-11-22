// Film 4 timeline: for a film time t, set the 3D stage and return the overlay drawing for any language/label variant.
// Scene boundaries (locked bar lines at 120 BPM): 0, 4, 8, 12, 20, 26, 30, 42, 46, 50, 54 s. Beat = 0.5 s.
// Music accents used (flagship3/cues.json): 1.0 flash; 6.25-7.75 draw; 8.2 slam; 9.6 press; 12.0/13.5/15.0/16.5/18.0 accents with
// roll +0.6 and lock +1.1; 20.6 fold; 21.9 close; 21.8 + 0.125 n ticks (28); 26.8 stream; 29.0 amber; 34/35.2 press;
// 36.0-38.0 pulse/flags; 38.1 tick; 40.4 count; 42.6 count; 44.6 tag; 46.9 click; 48 slam; 50 mark; 51.6 cta; 52.6 line_in.
'use strict';
import * as THREE from 'three';
import { C3 } from './stage.js';
import { HS, BS, LS, COL, clamp, seg, smooth, quint, spring, lerp, outCubic } from './ov.js';

const D = window.NF4, B = D.base, deg = Math.PI / 180;
const mm = (v) => Math.round(v).toLocaleString('en-US');
const BAR = [0, 4, 8, 12, 20, 26, 30, 42, 46, 50, 54];
const lerpCam = (a, b, u) => { const o = {}; for (const k of Object.keys(a)) o[k] = typeof a[k] === 'number' ? lerp(a[k], b[k] ?? a[k], u) : a[k]; return o; };

export class Shots {
  constructor(st, foot) {
    this.st = st; this.foot = foot;
    const req5 = D.pat.header5.request; this.req5 = req5;
    st.buildTee(); st.buildFamilies();
    const P = st.parts;
    // ---- tubes (curves on the steel), built once from the product's points
    // the true intersection curve: job 5's opening, on the header (rolled), as 3D points
    const hole5 = D.pat.header5.holes[0].map(([x, y]) => P.header5.at(x, y, 1).toArray());
    P.header5.set(1);
    this.hole5 = hole5;
    st.tube('true', hole5, 3.2, C3.blue, true, 720);
    st.tube('lift', hole5, 3.2, C3.blue, true, 720);
    st.tube('amberSeam', hole5, 3.6, C3.amber, true, 720);
    // the branch saddle edge (job 5): child of the branch part so it follows the branch plate when it moves
    P.branch5.set(1);
    const sad5 = D.pat.branch5.curves.saddle_cut.filter((_, i) => i % 1 === 0).map(([x, y]) => P.branch5.at(x, y, 1).toArray());
    st.tube('saddle', sad5, 3.4, 0xff7a1a, false, 720); P.branch5.grp.add(st.tubes.saddle.mesh);
    st.tube('one', hole5, 3.6, C3.green, true, 720);
    // the wrong curve: job 6's branch (saddle computed for a 158 mm header) set on job 5's 150 mm header, slid down its
    // axis to first contact. Shift d* = min over saddle points of (y0 - sqrt(150^2 - z0^2)) / sin(beta) (the point
    // reaches radius 150 when moved by that much along -a).
    P.branch6.set(1);
    const b = req5.beta_deg * deg, a = new THREE.Vector3(Math.cos(b), Math.sin(b), 0), R5 = req5.header_radius;
    const sad6 = D.pat.branch6.curves.saddle_cut.map(([x, y]) => P.branch6.at(x, y, 1));
    let dmin = Infinity; for (const p of sad6) dmin = Math.min(dmin, (p.y - Math.sqrt(R5 * R5 - p.z * p.z)) / Math.sin(b));
    this.wrongShift = a.clone().multiplyScalar(-dmin);
    this.gapMax = Math.max(...sad6.map((p) => Math.hypot(p.clone().add(this.wrongShift).y, p.z) - R5));
    st.tube('wrong', sad6.map((p) => p.clone().add(this.wrongShift).toArray()), 5.0, C3.red, false, 720);
    this.sad6w = sad6.map((p) => p.clone().add(this.wrongShift));
    // replay scan ring around the header (job 6 radius + 6 mm)
    const ring = []; for (let i = 0; i < 96; i++) { const q = (i / 96) * 2 * Math.PI; ring.push([0, (D.pat.header6.request.header_radius + 6) * Math.cos(q), (D.pat.header6.request.header_radius + 6) * Math.sin(q)]); }
    st.tube('scan', ring, 4, C3.blue, true, 96);
    // the job 6 opening on the job 6 header (amber seam after the change)
    P.header6.set(1);
    const hole6 = D.pat.header6.holes[0].map(([x, y]) => P.header6.at(x, y, 1).toArray());
    st.tube('amber6', hole6, 3.6, C3.amber, true, 720);
    this.hole6 = hole6;
    for (const t of Object.values(st.tubes)) t.draw(0);
    // the branch of job 5 and of job 6 have the same mesh structure (721 saddle points each): their rolled vertices can be
    // blended, so the branch follows the header when the radius changes (scene 7)
    P.branch5.set(1); P.branch6.set(1);
    this.bl = { p5: P.branch5.g.attributes.position.array.slice(), n5: P.branch5.g.attributes.normal.array.slice(), p6: P.branch6.g.attributes.position.array.slice(), n6: P.branch6.g.attributes.normal.array.slice() };
    if (this.bl.p5.length !== this.bl.p6.length) throw new Error('branch meshes differ');
    // ---- graph layout (scene 7): levels by longest path to an entered record (from depends on to)
    const deps = B.deps, lvl = {};
    const outs = {}; for (const e of deps.edges) (outs[e.from] = outs[e.from] || []).push(e.to);
    const L = (n) => (lvl[n] !== undefined ? lvl[n] : (lvl[n] = outs[n] ? 1 + Math.max(...outs[n].map(L)) : 0));
    deps.nodes.forEach(L); this.lvl = lvl;
    this.checks = B.job6.checkList;
  }

  // ------------------------------------------------------------------ per-frame 3D state
  reset() {
    const st = this.st; for (const p of Object.values(st.parts)) { p.grp.visible = false; p.grp.position.set(0, 0, 0); p.grp.scale.set(1, 1, 1); p.grp.rotation.set(0, 0, 0); p.mesh.material = st.steel; }
    for (const t of Object.values(st.tubes)) { t.draw(0); t.mesh.position.set(0, 0, 0); t.mesh.scale.set(1, 1, 1); }
    st.setShadow({ cx: 600, cz: 0, size: 3200, floorY: -300, height: 520, opacity: 0.62 });
    st.setLights(600, 60, 0, 1000);
  }
  tee(o = {}) {
    const P = this.st.parts, h = P[o.h || 'header5'], br = o.b === null ? null : P[o.b || 'branch5'];
    h.grp.visible = true; h.set(o.kh ?? 1, o.Rh);
    if (br) { br.grp.visible = true; br.set(o.kb ?? 1); if (o.boff) br.grp.position.copy(o.boff); }
    return { h, br };
  }
  cam(c) { this.st.setCamera(c); this.cur = c; }
  P(v) { return this.st.project(v instanceof THREE.Vector3 ? v : new THREE.Vector3(...v)); }

  frame(t) {
    this.reset(); this.t = t;
    let draw;
    if (t < 4) draw = this.s1(t); else if (t < 8) draw = this.s2(t); else if (t < 12) draw = this.s3(t); else if (t < 20) draw = this.s4(t);
    else if (t < 26) draw = this.s5(t); else if (t < 30) draw = this.s6(t); else if (t < 42) draw = this.s7(t); else if (t < 46) draw = this.s8(t);
    else if (t < 50) draw = this.s9(t); else draw = this.s10(t);
    this.st.render();
    return draw;
  }

  // common pieces of overlay
  head(o, V, lines, t, t0, t1, y = 210, color) { o.block(lines, 120, y, 140, { size: HS, weight: 650, color: color || COL.ink }, t, t0, t1); }
  label(o, V, s, t, t0, t1, row = 0) { if (!V.labels) return; o.block([s], 120, 1046 - row * 54, 0, { size: LS, weight: 500, color: COL.ink3 }, t, t0, t1); }

  // ================================================================== 1 · 0-4 s · the wrong curve
  s1(t) {
    const st = this.st, P = st.parts;
    this.tee({ b: 'branch6' }); P.branch6.grp.position.copy(this.wrongShift);
    const u = t / 4;
    this.cam({ tx: 600 + 60 * u, ty: 70, tz: 0, az: 14 + 16 * smooth(u), el: 8 + 6 * u, dist: 2350 - 450 * smooth(u), fov: 24, sx: 110, sy: 150 + 40 * u });
    const wu = spring(seg(t, 1.0, 1.55));
    st.tubes.wrong.draw(wu, 1);
    return (o, V) => {
      const S = V.S;
      // light through the gap: a soft streak along the projected flank of the gap, from 1.0 s (beat 2)
      const g = seg(t, 1.0, 1.25) * (1 - 0.35 * seg(t, 2.2, 3.8)) * (1 - seg(t, 3.75, 3.95));
      if (g > 0) {
        const c = o.c; c.save(); c.globalCompositeOperation = 'lighter';
        const pts = this.sad6w.filter((p, i) => i % 6 === 0 && p.z > 20).map((p) => this.P(p));
        c.strokeStyle = `rgba(255,250,236,${0.9 * g})`; c.lineWidth = 22; c.filter = 'blur(9px)'; c.beginPath(); pts.forEach((p, i) => (i ? c.lineTo(p[0], p[1]) : c.moveTo(p[0], p[1]))); c.stroke();
        c.filter = 'none'; c.restore();
      }
      this.head(o, V, S.hook, t, 0.3, 3.95);
      // a marker on the gap: the gap's real size is not stated (the curve is job 6's, on job 5's header)
      this.label(o, V, S.label_vis, t, 1.0, 3.95);
    };
  }

  // ================================================================== 2 · 4-8 s · the true curve lifts off
  s2(t) {
    const st = this.st, P = st.parts;
    const out = smooth(seg(t, 4.0, 4.45));                                   // the wrong branch leaves along its axis, its red edge with it
    this.tee({ b: 'branch6' });
    const b = this.req5.beta_deg * deg, away = new THREE.Vector3(Math.cos(b), Math.sin(b), 0).multiplyScalar(1100 * out * out);
    P.branch6.grp.position.copy(this.wrongShift).add(away);
    st.tubes.wrong.draw(1, 1 - seg(t, 4.2, 4.5)); st.tubes.wrong.mesh.position.copy(away);
    if (out >= 1) P.branch6.grp.visible = false;
    const u = seg(t, 4, 8), mv = quint(seg(t, 4.0, 5.4));
    const c0 = { tx: 660, ty: 70, tz: 0, az: 30, el: 14, dist: 1900, fov: 24, sx: 110, sy: 190 };
    const c1 = { tx: 700, ty: 170, tz: 10, az: 22 + 8 * u, el: 34, dist: 1500 - 120 * u, fov: 24, sx: 230, sy: 150 };
    this.cam(lerpCam(c0, c1, mv));
    // the true intersection curve (job 5) draws on the steel, then lifts off and holds
    const d = spring(seg(t, 4.3, 5.1)), lift = spring(seg(t, 5.0, 5.9));
    st.tubes.true.draw(d * (1 - seg(t, 5.0, 5.3)), 1);
    const L = st.tubes.lift; L.draw(t >= 5.0 ? 1 : 0, 1); L.mesh.position.set(0, 90 * lift + 6 * Math.sin((t - 5.9) * 2.2) * seg(t, 5.9, 6.4), 0);
    return (o, V) => {
      const S = V.S;
      this.head(o, V, S.question, t, 4.0, 7.95);
      // a marker travels the lifted curve point by point (6.25-7.75 s, the music's draw)
      const m = seg(t, 6.25, 7.75);
      if (m > 0 && m < 1) {
        const n = this.hole5.length, k = Math.floor(m * n) % n, p = this.hole5[k];
        const s = this.P([p[0], p[1] + L.mesh.position.y, p[2]]);
        o.dot(s[0], s[1], 11, '#ffffff', 0.95); o.ring(s[0], s[1], 11, 4, COL.blue, 1);
      }
      this.label(o, V, S.label_vis, t, 4.0, 7.95);
    };
  }

  // ================================================================== 3 · 8-12 s · dimensions in, flat pattern out
  s3(t) {
    const st = this.st, P = st.parts, r5 = this.req5, b = r5.beta_deg * deg, A = new THREE.Vector3(Math.cos(b), Math.sin(b), 0);
    const land = spring(seg(t, 8.0, 8.45), 0.8);
    const lift = smooth(seg(t, 9.55, 9.95));
    const kh = 1 - spring(seg(t, 9.6, 10.0), 0.9);
    this.tee({ kh, b: 'branch5', boff: A.clone().multiplyScalar(520 * (1 - land) + 900 * lift * lift) });
    if (lift >= 1) P.branch5.grp.visible = false;
    // the lifted ring settles back onto the opening as the branch lands
    const L = st.tubes.lift; const ry = 90 * (1 - smooth(seg(t, 8.0, 8.4))); L.draw(t < 8.5 ? 1 : 0, 1 - seg(t, 8.35, 8.5)); L.mesh.position.set(0, ry, 0);
    // camera: whole tee for the callouts, then straight above the flat plate (match cut to the application's preview)
    const fit = this.appFit(V_LANG_ANY);
    const c0 = { tx: 620, ty: 60, tz: 0, az: 24, el: 20, dist: 3000, fov: 24, sx: 90, sy: 0 };
    const c0b = { tx: 620, ty: 60, tz: 0, az: 28, el: 22, dist: 2850, fov: 24, sx: 90, sy: 0 };
    const ctop = { tx: 600, ty: 150, tz: 0, az: 0, el: 89.9, dist: fit.dist, fov: 24, sx: fit.sx, sy: fit.sy };
    const cm = lerpCam(c0, c0b, seg(t, 8, 9.6));
    this.cam(lerpCam(cm, ctop, quint(seg(t, 9.6, 10.0))));
    return (o, V) => {
      const S = V.S;
      if (t >= 9.88) this.appWindow(o, V, t);
      if (t < 10) {
        this.head(o, V, S.brand_name, t, 8.2, 9.2);
        this.head(o, V, S.product_a, t, 9.2, 9.95);
        // callouts: the job's inputs (job 5 request), on the geometry, one per eighth-beat from 8.5 s
        const cA = clamp(1 - seg(t, 9.5, 9.62));
        if (cA > 0) this.callouts(o, V, t, cA);
        this.label(o, V, S.label_vis, t, 8.0, 9.95);
      }
    };
  }
  // where the application's preview puts the header plate (per language) -> the camera that puts the 3D plate there
  appCrop(lang) {
    // crop along panel borders (application px of the 1920 x 1080 window): properties + preview, above the toast; the
    // gates panel (right) is left out. The plate rectangle is where the preview draws x 0..1200, y 0..942 (read from the frame).
    return lang === 'ja' ? { x: 232, y: 158, w: 1224, h: 832, plate: [609, 265, 1379, 868] } : { x: 232, y: 153, w: 1224, h: 832, plate: [609, 253, 1379, 856] };
  }
  appPlace(lang, t) { const cr = this.appCrop(lang); const k = 0.95 + 0.03 * smooth(seg(t, 10, 12)); const w = cr.w * k, h = cr.h * k;
    // the Japanese frame draws the plate 7 px lower inside the crop: the window moves up by that much so both plates meet the 3D plate
    const dy = ((cr.plate[1] - cr.y) - 100) * k; return { cr, k, X: 1840 - w, Y: 540 - h / 2 - dy, w, h }; }
  appFit() {
    // the plate (1200 mm) must span the preview's plate width; px per mm at distance d (fov 24): 1080 / (2 d tan 12 deg)
    const pl = this.appPlace('en', 10.0), cr = pl.cr, pw = (cr.plate[2] - cr.plate[0]) * pl.k;
    const dist = 1080 / (2 * Math.tan(12 * deg) * (pw / 1200));
    const cx = pl.X + ((cr.plate[0] + cr.plate[2]) / 2 - cr.x) * pl.k, cy = pl.Y + ((cr.plate[1] + cr.plate[3]) / 2 - cr.y) * pl.k;
    return { dist, sx: cx - 960, sy: cy - 540 };
  }
  appWindow(o, V, t) {
    const pl = this.appPlace(V.lang, t), cr = pl.cr, img = this.foot[V.lang].result, c = o.c, k2 = img.width / 1920;
    const a = seg(t, 9.88, 10.0);                     // the window holds to the bar (12.0 s cut)
    c.save(); c.globalAlpha = a; c.shadowColor = 'rgba(20,30,50,0.18)'; c.shadowBlur = 40; c.shadowOffsetY = 12; c.fillStyle = '#fff'; o.rr(pl.X, pl.Y, pl.w, pl.h, 14); c.fill(); c.shadowColor = 'transparent';
    o.rr(pl.X, pl.Y, pl.w, pl.h, 14); c.clip(); c.drawImage(img, cr.x * k2, cr.y * k2, cr.w * k2, cr.h * k2, pl.X, pl.Y, pl.w, pl.h); c.restore();
    const pb = V.lang === 'ja' ? V.S.product_b[0].replace('へ', '\nへ').split('\n') : V.S.product_b[0].split(' ');   // the line set as a poster column beside the window
    o.block(pb, 120, 400, 140, { size: HS, weight: 650 }, t, 10.0, 11.95);
    this.label(o, V, V.S.label_actual, t, 10.0, 11.95);
  }
  callouts(o, V, t, alpha) {
    const P = this.st.parts, r = this.req5, b = r.beta_deg * deg, prm = B.params, ja = V.lang === 'ja';
    const name = (k) => (prm[k] ? (ja ? prm[k].ja : prm[k].en) : k);
    const v3 = (x, y, z) => new THREE.Vector3(x, y, z);
    const items = [];
    // labels sit in fixed screen places with a leader line to their anchor on the steel (no label over the object)
    // 1 header radius: from the axis to the rim at the far end (x = header length)
    { const L = r.header_length, a = this.P(v3(L, 0, 0)), q = this.P(v3(L, -r.header_radius * Math.sin(30 * deg), r.header_radius * Math.cos(30 * deg))); items.push({ t0: 8.5, line: [a, q], dot: a, lead: q, lab: [name('header_radius'), `${mm(r.header_radius)} mm`], at: [1840, 730], align: 'right' }); }
    // 2 branch position (near end to the axes' meeting point) and 3 header length: stacked dimension lines under the header
    { const y = -r.header_radius - 60, a = this.P(v3(0, y, 160)), q = this.P(v3(r.branch_station, y, 160)), top = this.P(v3(r.branch_station, -r.header_radius, 120)); items.push({ t0: 8.75, dim: [a, q], tick: [q, top], lab: [name('branch_station'), `${mm(r.branch_station)} mm`], at: [(a[0] + q[0]) / 2 - 60, (a[1] + q[1]) / 2 - 92], align: 'center' }); }
    { const y = -r.header_radius - 60, a = this.P(v3(0, y, 160)), q = this.P(v3(r.header_length, y, 160)); const a2 = [a[0], a[1] + 120], q2 = [q[0], q[1] + 120]; items.push({ t0: 9.0, dim: [a2, q2], ext: [[a, a2], [q, q2]], lead: [(a2[0] + q2[0]) / 2, (a2[1] + q2[1]) / 2], lab: [name('header_length'), `${mm(r.header_length)} mm`], at: [1840, 900], align: 'right' }); }
    // 4 branch radius: at the square end of the branch
    { const O = v3(r.branch_station, 0, r.delta), A = v3(Math.cos(b), Math.sin(b), 0), e1 = v3(-Math.sin(b), Math.cos(b), 0);
      const c = O.clone().addScaledVector(A, r.s_end), q = c.clone().addScaledVector(e1, -r.branch_radius); const pc = this.P(c), pq = this.P(q);
      items.push({ t0: 9.25, line: [pc, pq], dot: pc, lead: [(pc[0] + pq[0]) / 2, (pc[1] + pq[1]) / 2], lab: [name('branch_radius'), `${mm(r.branch_radius)} mm`], at: [1840, 250], align: 'right' }); }
    // 5 angle between axes: arc at the meeting point
    { const arc = []; for (let i = 0; i <= 24; i++) { const q = (i / 24) * b; arc.push(this.P(v3(r.branch_station + 230 * Math.cos(q), 230 * Math.sin(q), 0))); }
      items.push({ t0: 9.5, arc, lead: arc[12], lab: [name('beta_deg'), `${mm(r.beta_deg)}°`], at: [1840, 520], align: 'right' }); }
    for (const it of items) {
      const u = spring(seg(t, it.t0 - 0.25, it.t0 + 0.2)), a = alpha * seg(t, it.t0 - 0.25, it.t0 - 0.1);
      if (a <= 0) continue;
      if (it.line) { const [p, q] = it.line; o.line([p, [lerp(p[0], q[0], u), lerp(p[1], q[1], u)]], 4, COL.blue, a); o.dot(p[0], p[1], 7, COL.blue, a); }
      if (it.dim) { const [p, q] = it.dim; const m = [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2]; o.line([[lerp(m[0], p[0], u), lerp(m[1], p[1], u)], [lerp(m[0], q[0], u), lerp(m[1], q[1], u)]], 4, COL.blue, a); o.line([[p[0], p[1] - 22], [p[0], p[1] + 22]], 4, COL.blue, a * u); o.line([[q[0], q[1] - 22], [q[0], q[1] + 22]], 4, COL.blue, a * u); }
      if (it.tick) { const [q, top] = it.tick; o.line([q, [lerp(q[0], top[0], u), lerp(q[1], top[1], u)]], 3, COL.blue, a, [10, 10]); o.line([[q[0], q[1] - 22], [q[0], q[1] + 22]], 4, COL.blue, a); }
      if (it.arc) { const n = Math.max(2, Math.round(u * it.arc.length)); o.line(it.arc.slice(0, n), 4, COL.blue, a); }
      let [x, y] = it.at; const al = it.align;
      if (it.ext) for (const [p, q] of it.ext) o.line([p, [lerp(p[0], q[0], u), lerp(p[1], q[1], u)]], 2, COL.blue, a * 0.6, [6, 8]);
      if (it.lead) { const w = Math.max(o.tw(it.lab[0], LS, 500), o.tw(it.lab[1], BS, 650)); const lx = al === 'right' ? x - w - 24 : x - 24, ly = y - 14; const e = [lerp(it.lead[0], lx, u), lerp(it.lead[1], ly, u)]; o.line([it.lead, e], 2.5, COL.blue, a, [2, 8]); o.dot(it.lead[0], it.lead[1], 6, COL.blue, a); }
      if (it.fitRight) { const w = Math.max(o.tw(it.lab[0], LS, 500), o.tw(it.lab[1], BS, 650)); x = Math.min(x, it.fitRight - w); }
      o.txt(it.lab[1], x, y + 10 * (1 - u), { size: BS, weight: 650, color: COL.blueT, align: al, alpha: a });
      o.txt(it.lab[0], x, y + 52 + 10 * (1 - u), { size: LS, weight: 500, color: COL.ink2, align: al, alpha: a });
    }
  }

  // ================================================================== 4 · 12-20 s · five families
  partPlace(i) {
    // one family per slot, cut on the music's accent. Scale per family so each flat pattern spans a similar width on
    // screen (a visual normalisation; no size is stated). rot: Euler XYZ, so the flat pattern lies horizontal and wide.
    const cfg = [
      { key: 'shell', s: 0.50, l: [500, 0, 0], rot: [0, Math.PI / 2, 0], dir: 'unroll', cam: { az: 20, el: 30, dist: 3350, sx: 120, sy: 150, ty: 60 } },
      { key: 'cone', s: 1.15, l: [0, -450, 0], rot: [0, 0, 0], dir: 'roll', cam: { az: 22, el: 30, dist: 3250, sx: 150, sy: 120, ty: -200 } },
      { key: 'ecc', s: 1.15, l: [0, -450, 150], rot: [0, 0, 0], dir: 'unroll', cam: { az: 24, el: 30, dist: 3250, sx: 150, sy: 120, ty: -200 } },
      { key: 'branch4', s: 1.6, l: [730, 225, 0], rot: [-Math.PI / 2, 0, -Math.PI / 3], dir: 'roll', cam: { az: 22, el: 32, dist: 2900, sx: 180, sy: 40, ty: 60 } },
      { key: 'header5', s: 1.0, l: [600, 0, 0], rot: [0, 0, 0], dir: 'unroll', cam: { az: 24, el: 32, dist: 2950, sx: 160, sy: 170, ty: 40 } },
    ][i];
    return { ...cfg, c: [0, 0, 0] };
  }
  s4(t) {
    const st = this.st, P = st.parts, T0 = 12, step = 1.5;
    const i = Math.min(4, Math.floor((t - T0) / step)), ts = T0 + i * step, te = i < 4 ? ts + step : 20;
    const pp = this.partPlace(i), part = P[pp.key];
    // the part arrives on the accent with a short spring turn; the camera drifts in through the slot
    const inU = spring(seg(t, ts, ts + 0.45), 0.75), du = seg(t, ts, te);
    const c = pp.cam;
    this.cam({ tx: 0, ty: c.ty, tz: 0, az: c.az + 8 * du, el: c.el, dist: c.dist * (1.06 - 0.06 * du), fov: 24, sx: c.sx, sy: c.sy });
    st.setShadow({ cx: 0, cz: 0, size: 4200, floorY: -560, height: 760, opacity: 0.55 });
    st.setLights(0, 0, 0, 1100);
    part.grp.visible = true;
    part.grp.scale.setScalar(pp.s * (0.94 + 0.06 * inU));
    part.grp.rotation.set(pp.rot[0], pp.rot[1] - 0.35 * (1 - inU), pp.rot[2]);
    const lw = new THREE.Vector3(...pp.l).multiplyScalar(pp.s * (0.94 + 0.06 * inU)).applyEuler(part.grp.rotation);
    const u = spring(seg(t, ts + 0.6, ts + 1.1), 0.85), kap = pp.dir === 'unroll' ? 1 - u : u;
    // the sectors flatten in the plane of their apex (above the frustum): lower them with the flattening so they stay in frame
    const drop = (pp.key === 'cone' || pp.key === 'ecc') ? (1 - kap) * 330 * pp.s : 0;
    part.grp.position.set(-lw.x, -lw.y - drop, -lw.z);
    part.set(kap);
    return (o, V) => {
      const S = V.S;
      const names = [S.part_shell, S.part_cone, S.part_ecc, S.part_branch, S.part_header];
      this.head(o, V, names[i], t, ts, te - 0.02);
      // the computed outline flashes when the plate locks flat (lock = slot start + 1.1 s) and stays as a thin line while flat
      const flatness = 1 - clamp(part.kappa * 6);
      if (flatness > 0) {
        const lock = pp.dir === 'unroll' ? ts + 1.1 : ts;
        const fl = t >= lock - 0.05 ? Math.exp(-6 * Math.max(0, t - lock)) : 0;
        this.outline(o, pp, part, 3 + 5 * fl, flatness * (0.6 + 0.4 * fl));
      }
      // point by point: the opening's computed points (job 5), one after another
      if (i === 4 && t >= 19.1) {
        const pts = D.pat.header5.holes[0], n = pts.length, m = Math.floor(n * seg(t, 19.1, 19.75));
        for (let j = 0; j < m; j += 4) { const p = this.P(part.at(pts[j][0], pts[j][1])); o.dot(p[0], p[1], 4.5, COL.blue, 0.95); }
        if (m > 0 && m < n) { const p = this.P(part.at(pts[m - 1][0], pts[m - 1][1])); o.dot(p[0], p[1], 12, '#fff', 1); o.ring(p[0], p[1], 12, 4, COL.blue, 1); }
        o.block([fmt(S.points_line, { n: mm(n) })], 120, V.lang === 'ja' ? 300 : 440, 0, { size: BS, weight: 560, color: COL.blueT }, t, 19.1, 19.95);
      }
      if (V.labels) { o.block(S.limits, 120, 938, 52, { size: LS, weight: 500, color: COL.ink3 }, t, 12.0, 19.95); }
      this.label(o, V, S.label_vis, t, 12.0, 19.95);
    };
  }
  outline(o, pp, part, w, a) {
    const pat = D.pat[pp.key === 'header5' ? 'header5' : pp.key === 'branch4' ? 'branch4' : pp.key];
    const curves = [...Object.values(pat.curves), ...pat.holes];
    const c = o.c; c.save(); c.globalAlpha = a; c.strokeStyle = COL.blue; c.lineWidth = w; c.lineJoin = 'round'; c.lineCap = 'round';
    for (const cv of curves) {
      c.beginPath(); const step = Math.max(1, Math.floor(cv.length / 240));
      for (let j = 0; j < cv.length; j += step) { const p = this.P(this.flatWorld(pp, part, cv[j])); j ? c.lineTo(p[0], p[1]) : c.moveTo(p[0], p[1]); }
      const l = cv[cv.length - 1], p = this.P(this.flatWorld(pp, part, l)); c.lineTo(p[0], p[1]);
      if (pat.holes.includes(cv)) c.closePath(); c.stroke();
    }
    c.restore();
  }
  flatWorld(pp, part, [x, y]) {
    // the plate point in world space with the part's current wrap (plates) or in its flat state (cone, eccentric)
    if (part.at) return part.at(x, y);
    const rho = Math.hypot(x, y), psi = Math.atan2(y, x); let a;
    if (pp.key === 'cone') { const f = D.fit.cone; a = psi - (f.psi0 + f.psi1) / 2; } else { a = psi - D.fit.ecc.total / 2; }
    const v = new THREE.Vector3(rho * Math.sin(a), 0, rho * Math.cos(a)); part.grp.updateMatrixWorld(); return v.applyMatrix4(part.grp.matrixWorld);
  }

  // ================================================================== 5 · 20-26 s · fold back, one curve, checks
  s5(t) {
    const st = this.st, P = st.parts;
    const fold = clamp(seg(t, 20.6, 21.9)), kf = quint(fold);
    const bIn = spring(seg(t, 20.0, 20.5));
    // branch plate: slides in flat in front of the header plate (20.0-20.5), then rolls and closes onto it (20.6-21.9)
    const off = new THREE.Vector3(0, 380, 620).multiplyScalar(1 - kf).add(new THREE.Vector3(1500, 0, 0).multiplyScalar(1 - bIn));
    this.tee({ kh: kf, kb: kf, b: 'branch5', boff: off });
    const u = seg(t, 20, 26);
    const c0 = { tx: 600, ty: 40, tz: 0, az: 32, el: 32, dist: 2950, fov: 24, sx: 160, sy: 170 };
    const c1 = { tx: 620, ty: 70, tz: 0, az: 36 + 8 * u, el: 17, dist: 3050, fov: 24, sx: -470, sy: 130 };
    this.cam(lerpCam(c0, c1, quint(seg(t, 20.0, 22.2))));
    // the two cut edges: the header's opening (blue) and the branch's saddle (orange); at 21.9 s they meet: one curve (green)
    const e = seg(t, 21.3, 21.75), m = seg(t, 21.9, 22.25);
    st.tubes.true.draw(t < 22.3 ? 1 : 0, e * (1 - m)); st.tubes.saddle.draw(t < 22.3 ? 1 : 0, e * (1 - m));
    st.tubes.one.draw(t >= 21.9 ? spring(seg(t, 21.9, 22.4)) : 0, 1 - 0.0 * seg(t, 25.6, 25.95));
    return (o, V) => {
      const S = V.S;
      this.head(o, V, S.checks_big, t, 20.0, 21.85);
      this.head(o, V, S.one_curve, t, 21.9, 25.95);
      // flash at the closing
      const fl = Math.exp(-7 * Math.max(0, t - 21.9)) * (t >= 21.9 ? 1 : 0);
      if (fl > 0.02) { const c = o.c; c.save(); c.globalCompositeOperation = 'lighter'; c.filter = 'blur(14px)'; c.strokeStyle = `rgba(160,255,200,${0.7 * fl})`; c.lineWidth = 26; c.beginPath(); this.hole5.forEach((p, i) => { if (i % 8) return; const s = this.P(p); i ? c.lineTo(s[0], s[1]) : c.moveTo(s[0], s[1]); }); c.closePath(); c.stroke(); c.restore(); }
      this.checkPanel(o, V, t);
      if (V.labels) o.block([S.checks_sub], 1840, 1046 - 54, 0, { size: LS, weight: 500, color: COL.ink3, align: 'right' }, t, 21.8, 25.95);
      if (V.labels) o.block([S.label_vis], t < 21.8 ? 120 : 1840, 1046, 0, { size: LS, weight: 500, color: COL.ink3, align: t < 21.8 ? 'left' : 'right' }, t, t < 21.8 ? 20.0 : 21.8, t < 21.8 ? 21.8 : 25.95);
    };
  }
  checkPanel(o, V, t) {
    if (t < 21.8) return;
    const list = this.checks, N = list.length, ja = V.lang === 'ja';
    // one check per 0.125 s from 21.8 s (the music's 28 ticks)
    const n = Math.min(N, Math.floor((t - 21.8) / 0.125 + 1e-6) + 1);
    const a = (0.6 + 0.4 * seg(t, 21.8, 21.93)) * (1 - seg(t, 25.75, 25.95)), X = 1100, W = 740, c = o.c;
    c.save(); c.globalAlpha = a;
    // counter and the verdict count (job 6: checkCounts)
    o.txt(fmt(V.S.checks_count, { n: mm(n), b: mm(N) }), X, 330, { size: BS, weight: 650, color: COL.ink });
    const passN = Math.min(n, B.job6.checkCounts.PASS || 0), pl = `${V.S.res_pass} ${mm(passN)}`;
    o.txt(pl, X + W, 430, { size: BS, weight: 650, color: COL.greenT, align: 'right' });
    c.fillStyle = 'rgba(29,29,31,0.10)'; o.rr(X, 362, W, 10, 5); c.fill(); c.fillStyle = COL.green; o.rr(X, 362, W * n / N, 10, 5); c.fill();
    // rows: newest at the bottom, the list scrolls; names as the application gives them (JA label, or the identifier)
    if (!this._rows || this._rows.lang !== V.lang) {
      const maxW = W - 110; let y = 0; const rows = [];  // W = panel width
      for (const ck of list) { const nm = ja ? (ck.ja || ck.name) : ck.name, mono = !(ja && ck.ja); const lines = wrapText(o, nm, maxW, mono, ja && !!ck.ja); const h = 34 + lines.length * 50; rows.push({ lines, mono, y, h }); y += h + 12; }
      this._rows = { lang: V.lang, rows };
    }
    const rows = this._rows.rows, top = 470, H = 450;
    const last = rows[n - 1], scroll = Math.max(0, last.y + last.h - H);
    const sc = this._sc || 0;
    c.save(); c.beginPath(); c.rect(X - 10, top - 6, W + 20, H + 8); c.clip();
    for (let j = 0; j < n; j++) {
      const r = rows[j], y = top + r.y - scroll; if (y > top + H || y + r.h < top) continue;
      const ti = clamp((t - (21.8 + j * 0.125)) / 0.2);
      c.fillStyle = 'rgba(255,255,255,0.88)'; o.rr(X, y, W, r.h, 14); c.fill();
      o.tick(X + 44, y + 42, 30, ti, COL.green, 6);
      r.lines.forEach((ln, i) => o.txt(ln, X + 92, y + 56 + i * 50, { size: LS, weight: 500, mono: r.mono, color: COL.ink }));
    }
    c.restore();
    // soft fade at the top edge of the list
    const g = c.createLinearGradient(0, top - 6, 0, top + 50); g.addColorStop(0, 'rgba(240,241,244,1)'); g.addColorStop(1, 'rgba(240,241,244,0)');
    if (scroll > 0) { c.fillStyle = g; c.fillRect(X - 10, top - 6, W + 20, 56); }
    c.restore();
  }

  // ================================================================== 6 · 26-30 s · DXF written, read back, compared
  s6(t) {
    const st = this.st, P = st.parts;
    const kh = 1 - spring(seg(t, 26.0, 26.55), 0.9), lift = smooth(seg(t, 26.0, 26.35));
    const b = this.req5.beta_deg * deg;
    this.tee({ h: 'header6', kh, b: 'branch5', boff: new THREE.Vector3(Math.cos(b), Math.sin(b), 0).multiplyScalar(900 * lift * lift) });
    if (lift >= 1) P.branch5.grp.visible = false;
    const dr = this.dxfRect();
    // camera straight above the plate, the plate filling the drawing rectangle
    const pw = dr.w, dist = 1080 / (2 * Math.tan(12 * deg) * (pw / 1200));
    const ctop = { tx: 600, ty: 150, tz: 0, az: 0, el: 89.9, dist, fov: 24, sx: dr.x + dr.w / 2 - 960, sy: dr.y + dr.h / 2 - 540 };
    const c0 = { tx: 620, ty: 70, tz: 0, az: 44, el: 17, dist: 3050, fov: 24, sx: -470, sy: 130 };
    this.cam(lerpCam(c0, ctop, quint(seg(t, 26.0, 26.55))));
    st.setShadow({ opacity: 0.6 * (1 - seg(t, 26.3, 26.6)) });
    return (o, V) => {
      const S = V.S, c = o.c;
      const words = S.dxf_words; const wt = [26.0, 26.95, 27.6];
      for (let k = 0; k < 3; k++) this.head(o, V, words[k], t, wt[k], k < 2 ? wt[k + 1] - 0.02 : 29.95);
      // the plate turns into its DXF drawing: a white sheet covers the steel, the entities draw on it
      const sh = seg(t, 26.5, 26.8);
      if (sh > 0) { c.save(); c.globalAlpha = sh; c.fillStyle = '#ffffff'; c.shadowColor = 'rgba(20,30,50,0.12)'; c.shadowBlur = 30; c.shadowOffsetY = 8; o.rr(dr.x - 30, dr.y - 30, dr.w + 60, dr.h + 60, 16); c.fill(); c.restore(); }
      const toS = ([x, y]) => [dr.x + x * dr.w / 1200, dr.y + dr.h - y * dr.w / 1200];
      const ents = this.dxfPaths();
      const wU = seg(t, 26.55, 27.25), rU = seg(t, 26.95, 27.6);
      for (const [pass, u, col, w] of [[0, wU, COL.ink, 4], [1, rU, COL.blue, 4]]) {
        if (u <= 0) continue; const tot = ents.total * u; let acc = 0;
        c.save(); c.strokeStyle = col; c.lineWidth = w; c.lineCap = 'round'; c.lineJoin = 'round'; if (pass === 1) c.setLineDash([]);
        for (const pth of ents.paths) { if (acc >= tot) break; c.beginPath(); let first = true;
          for (let j = 0; j < pth.length; j++) { const p = toS(pth[j]); if (j > 0) { const q = pth[j - 1]; acc += Math.hypot(pth[j][0] - q[0], pth[j][1] - q[1]); }
            if (first) { c.moveTo(p[0], p[1]); first = false; } else c.lineTo(p[0], p[1]); if (acc >= tot) break; }
          c.stroke(); }
        c.restore();
      }
      // compared: the tick locks
      const lk = spring(seg(t, 27.6, 28.0));
      if (t >= 27.6) { o.dot(dr.x + dr.w - 10, dr.y + 10, 46 * lk, COL.green, 1); o.tick(dr.x + dr.w - 10, dr.y + 12, 44, seg(t, 27.65, 27.9), '#fff', 8); }
      // the rows of the file stream on the right (group code, value), from 26.8 s
      this.dxfRows(o, V, t);
      // NOT RELEASED: stamp with the real file name, 29.0 s
      if (t >= 28.95) {
        const u = spring(seg(t, 28.95, 29.35), 0.6), cx = dr.x + dr.w / 2, cy = dr.y + dr.h / 2 - 30;
        c.save(); c.translate(cx, cy); c.rotate(-6 * deg); c.scale(1.25 - 0.25 * u, 1.25 - 0.25 * u); c.globalAlpha = seg(t, 28.95, 29.05);
        const s = B.ui[V.lang]['tb.nr'], w = o.tw(s, HS, 750) + 90, h = 170;
        c.fillStyle = 'rgba(255,255,255,0.92)'; o.rr(-w / 2, -h / 2, w, h, 20); c.fill(); c.strokeStyle = COL.amber; c.lineWidth = 10; o.rr(-w / 2, -h / 2, w, h, 20); c.stroke();
        o.txt(s, 0, 47, { size: HS, weight: 750, color: COL.amberT, align: 'center' }); c.restore();
        { // the real file name, broken after an underscore onto two lines so it stays inside the drawing
          const nm = D.dxf.name, k = nm.indexOf('_', Math.floor(nm.length / 2) - 4) + 1, l1 = nm.slice(0, k), l2 = nm.slice(k), aa = seg(t, 29.1, 29.3);
          const w = Math.max(o.tw(l1, LS, 500, true), o.tw(l2, LS, 500, true)) + 52, y0 = cy + 110;
          c.save(); c.globalAlpha = aa; c.fillStyle = 'rgba(255,255,255,0.95)'; c.shadowColor = 'rgba(20,30,50,0.10)'; c.shadowBlur = 16; o.rr(cx - w / 2, y0, w, 124, 16); c.fill(); c.restore();
          o.txt(l1, cx, y0 + 52, { size: LS, mono: true, align: 'center', alpha: aa }); o.txt(l2, cx, y0 + 102, { size: LS, mono: true, align: 'center', alpha: aa }); }
      }
      this.label(o, V, S.label_vis, t, 26.0, 29.95);
    };
  }
  dxfRect() { const w = 860, h = w * D.pat.header6.quantities.developed_width / D.pat.header6.quantities.length; return { x: 140, y: 1000 - h - 20, w, h }; }
  dxfPaths() {
    if (this._dp) return this._dp; const paths = [];
    for (const l of D.dxf.lines) paths.push([l.a, l.b]);
    for (const p of D.dxf.poly) paths.push(p.closed ? [...p.pts, p.pts[0]] : p.pts);
    let total = 0; for (const p of paths) for (let j = 1; j < p.length; j++) total += Math.hypot(p[j][0] - p[j - 1][0], p[j][1] - p[j - 1][1]);
    return (this._dp = { paths, total });
  }
  dxfRows(o, V, t) {
    if (t < 26.8) return; const rows = D.dxf.rows, c = o.c, X = 1160, top = 300, lh = 54, vis = 13;
    const pos = (t - 26.8) * 11;                       // rows per second
    const a = seg(t, 26.8, 26.95);
    c.save(); c.globalAlpha = a; c.beginPath(); c.rect(X - 20, top - 46, 700, lh * vis + 10); c.clip();
    const k0 = Math.floor(pos); const fr = pos - k0;
    for (let j = 0; j <= vis; j++) {
      const k = k0 + j; if (k >= rows.length) break; const y = top + (j - fr) * lh;
      const [code, val] = rows[k], isNew = j >= vis - 1;
      o.txt(code.padStart(3, ' '), X, y, { size: LS, mono: true, weight: 500, color: COL.ink3 });
      o.txt(val, X + 130, y, { size: LS, mono: true, weight: 500, color: code === '0' ? COL.blueT : COL.ink });
    }
    c.restore();
    // fade top and bottom edges of the stream
    const g1 = c.createLinearGradient(0, top - 46, 0, top + 30); g1.addColorStop(0, 'rgba(244,245,247,1)'); g1.addColorStop(1, 'rgba(244,245,247,0)');
    c.fillStyle = g1; c.fillRect(X - 20, top - 46, 700, 76);
    const yb = top + lh * vis - 60, g2 = c.createLinearGradient(0, yb, 0, yb + 70); g2.addColorStop(0, 'rgba(236,237,240,0)'); g2.addColorStop(1, 'rgba(236,237,240,1)');
    c.fillStyle = g2; c.fillRect(X - 20, yb, 700, 70);
  }

  // ================================================================== 7 · 30-42 s · AMOEBA
  graphLayout() {
    if (this._gl) return this._gl;
    const lv = this.lvl, nodes = B.deps.nodes, cols = {};
    for (const n of nodes) (cols[lv[n]] = cols[lv[n]] || []).push(n);
    // columns: 0 entered/declared, 1 parameters derived from a convention, 2..4 derived records
    const X = [120, 760, 1500, 1665, 1825], pos = {};
    for (const [l, ns] of Object.entries(cols)) {
      ns.sort();
      const L = Number(l), n = ns.length;
      ns.forEach((id, j) => {
        if (L <= 1) pos[id] = { x: X[L], y: 640 + j * 66, tag: true };
        else pos[id] = { x: X[L], y: 790 + (j - (n - 1) / 2) * 130, tag: false };
      });
    }
    return (this._gl = { pos });
  }
  s7(t) {
    const st = this.st, P = st.parts, j6 = B.job6, corr = j6.corr;
    // the change: header radius 150 -> 158 (35.2 s), the header grows, the branch takes job 6's saddle
    const ch = spring(seg(t, 35.2, 35.75), 0.8);
    const after = t >= 35.2;
    const Rh = lerp(corr.old, corr.new, ch);
    this.tee({ h: ch > 0.5 ? 'header6' : 'header5', Rh: ch > 0.5 ? undefined : undefined, b: ch > 0.5 ? 'branch6' : 'branch5' });
    // radial growth: scale the header about its axis (y, z) by Rh / radius of the plate shown
    const hp = ch > 0.5 ? P.header6 : P.header5, base = ch > 0.5 ? corr.new : corr.old;
    hp.grp.scale.set(1, Rh / base, Rh / base);
    const bp = ch > 0.5 ? P.branch6 : P.branch5; bp.grp.position.set(0, 0, 0);
    if (ch > 0 && ch < 1) {      // blend the job 5 branch into the job 6 branch while the header grows
      P.branch5.grp.visible = false; P.branch6.grp.visible = true; const g = P.branch6.g, pa = g.attributes.position.array, na = g.attributes.normal.array, q = this.bl;
      for (let i = 0; i < pa.length; i++) { pa[i] = q.p5[i] + (q.p6[i] - q.p5[i]) * ch; na[i] = q.n5[i] + (q.n6[i] - q.n5[i]) * ch; }
      g.attributes.position.needsUpdate = true; g.attributes.normal.needsUpdate = true; P.branch6.kappa = -1;
    }
    // camera: the tee upper right, the graph below
    const u = seg(t, 30, 42);
    const camA = { tx: 640, ty: 80, tz: 0, az: 24 + 8 * u, el: 20, dist: 2500, fov: 24, sx: 250, sy: 60 };
    const camB = { tx: 640, ty: 80, tz: 0, az: 24 + 8 * u, el: 18, dist: 4500, fov: 24, sx: 380, sy: -95 };
    const toB = 0.82 + 0.18 * quint(seg(t, 30.0, 32.6)), back = quint(seg(t, 40.3, 40.8));
    this.cam(lerpCam(lerpCam(camA, camB, toB), { ...camA, dist: 2600, sx: 330, sy: 120 }, back));
    st.setShadow({ opacity: 0.6 * (1 - 0.4 * toB * (1 - back)) });
    // the intersection curve turns amber on the object when it is flagged (36.0 s)
    const am = seg(t, 36.0, 36.15);
    if (am > 0) { const tb = ch > 0.5 ? st.tubes.amber6 : st.tubes.amberSeam; tb.draw(1, am); }
    // replay (40.4-41.3 s): a ring scans the header as the records are checked
    if (t >= 40.4) { const s = st.tubes.scan, k = quint(seg(t, 40.4, 41.3)); s.draw(1, 1 - seg(t, 41.3, 41.5)); s.mesh.position.set(lerp(-20, 1220, k), 0, 0); }
    return (o, V) => this.s7draw(o, V, t, Rh, after);
  }
  s7draw(o, V, t, Rh, after) {
    const S = V.S, j6 = B.job6, corr = j6.corr, ja = V.lang === 'ja', c = o.c;
    // headline zone
    this.head(o, V, S.amoeba_big, t, 30.0, 34.95);
    o.block(S.amoeba_def, 120, 300, 62, { size: BS, weight: 500, color: COL.ink2 }, t, 32.0, 34.95);
    this.head(o, V, S.change_big, t, 35.0, 38.35);
    // graph (records and links) from 30.4 s; hidden behind the real screen 38.4-40.4 s; out at 40.3 s
    const gA = 1 - seg(t, 40.2, 40.4);
    if (t < 40.4) this.graph(o, V, t, Rh, gA);
    // the real dependency graph screen (job 6), 38.4-40.4 s
    if (t >= 38.4 && t < 40.4) this.realGraph(o, V, t);
    o.block([fmt(S.obl_line, { r: mm(B.obligations.length), d: mm(B.oblCounts.discharged), o: mm(B.oblCounts.open) })], 120, 300, 0, { size: BS, weight: 600, color: COL.amberT }, t, 38.1, 40.35);
    // replay
    if (t >= 40.4) {
      const n = Math.min(j6.replay.checked, Math.floor(j6.replay.checked * seg(t, 40.45, 41.2)));
      this.head(o, V, S.replay_big, t, 41.0, 41.95);
      o.block([fmt(S.rec_count, { n: mm(n), f: mm(j6.replay.failing) })], 120, 300, 0, { size: BS, weight: 560, color: COL.ink2 }, t, 40.4, 41.95);
      if (t > 41.2) { const lk = spring(seg(t, 41.2, 41.55)), fa = 1 - seg(t, 41.75, 41.95); o.dot(1730, 230, 46 * lk, COL.green, fa); o.tick(1730, 232, 44, seg(t, 41.25, 41.5), '#fff', 8, fa); }
    }
    if (t < 38.4 || t >= 40.4) this.label(o, V, S.label_vis, t, t < 38.4 ? 30.0 : 40.4, t < 38.4 ? 38.35 : 41.95);
    else this.label(o, V, S.label_actual, t, 38.4, 40.35);
  }
  graph(o, V, t, Rh, gA) {
    const G = this.graphLayout(), pos = G.pos, c = o.c, ja = V.lang === 'ja', j6 = B.job6;
    const val = {}; for (const p of j6.params) val[p.name] = p; // job 6 values; header_radius shows the change
    const req5 = this.req5;
    const r = this.req5, b = r.beta_deg * deg, v3 = (x, y, z) => new THREE.Vector3(x, y, z);
    const O = v3(r.branch_station, 0, 0), A = v3(Math.cos(b), Math.sin(b), 0), e1 = v3(-Math.sin(b), Math.cos(b), 0), top = O.clone().addScaledVector(A, r.s_end);
    const Rs = Rh;
    const anchor = { 'parameter · header_radius': v3(0, Rs * 0.7, Rs * 0.7), 'parameter · header_dev_radius': v3(150, -Rs * 0.7, Rs * 0.7), 'parameter · header_length': v3(1200, 0, Rs),
      'parameter · branch_station': v3(r.branch_station, -Rs, 40), 'parameter · branch_radius': top.clone().addScaledVector(e1, -r.branch_radius), 'parameter · branch_dev_radius': top.clone().addScaledVector(e1, r.branch_radius),
      'parameter · beta_deg': O.clone().addScaledVector(A, 230).add(v3(0, 0, 0)), 'parameter · delta': O.clone(), 'parameter · s_end': top, 'convention · surface_reference': v3(900, Rs * 0.7, Rs * 0.7) };
    const ids = Object.keys(pos);
    // order of appearance of the tags: one per eighth-beat from 30.5 s
    const tagOrder = ids.filter((id) => pos[id].tag);
    const showT = (id) => 30.5 + 0.25 * tagOrder.indexOf(id);
    const flagT = {}; B.simple.flagOrder.forEach((id, k) => (flagT[id] = 36.0 + 0.5 * k));
    const src = B.simple.source;
    // edges (draw 32.4-33.8 s), from the record to what it depends on
    const eU = seg(t, 32.4, 33.8);
    if (eU > 0) {
      B.deps.edges.forEach((e, k) => {
        const p = pos[e.from], q = pos[e.to]; if (!p || !q) return;
        const uu = clamp(eU * 1.6 - (k / B.deps.edges.length) * 0.6); if (uu <= 0) return;
        const left = q.x < p.x; const pa = left ? [p.x, p.y] : this.nodePort(o, p, e.from, V, 'in'), qa = this.nodePort(o, q, e.to, V, 'out');
        const onPath = flagT[e.from] !== undefined && (e.to === src || flagT[e.to] !== undefined);
        const hot = onPath && t >= (flagT[e.from] ?? 99) - 0.25;
        const pts = bez(qa, pa, 24).slice(0, Math.max(2, Math.round(24 * uu)));
        o.line(pts, hot ? 4 : 2.2, hot ? COL.amber : 'rgba(29,29,31,0.32)', gA);
        // the pulse: a bright dot runs along each edge on the path just before its node is flagged
        if (onPath) { const t0 = flagT[e.from] - 0.25, w = seg(t, t0, t0 + 0.25); if (w > 0 && w < 1) { const pp = bez(qa, pa, 24); const k2 = Math.floor(w * 23); o.dot(pp[k2][0], pp[k2][1], 9, COL.amber, gA); } }
      });
    }
    // tags (parameter records with their values), tethered to the geometry
    for (const id of ids) {
      const p = pos[id], t0 = p.tag ? showT(id) : 32.6 + 0.08 * (this.lvl[id] - 2);
      if (t < t0) continue;
      const u = spring(seg(t, t0, t0 + 0.4)), a = gA * seg(t, t0, t0 + 0.1);
      if (p.tag) {
        const name = id.split(' · ')[1], pv = val[name];
        let vtxt = pv ? (pv.unit === 'deg' ? `${mm(pv.value)}°` : `${mm(pv.value)} mm`) : '';
        if (name === 'header_radius') vtxt = t < 35.2 ? `${mm(B.job6.corr.old)} mm` : `${mm(B.job6.corr.old)} → ${mm(Math.round(lerp(B.job6.corr.old, B.job6.corr.new, seg(t, 35.2, 35.6))))} mm`;
        if (name === 'surface_reference' || id.startsWith('convention')) vtxt = req5.surface_reference;
        if (name !== 'header_radius' && pv && pv.value !== req5[name]) vtxt = `${mm(req5[name])} mm`;
        const isSrc = id === src, hl = isSrc && t >= 34.9;
        // tether to the geometry (until the camera pulls back and the graph forms)
        const an = anchor[id];
        if (an && t < 40.4) { const s = this.P(an); const wpt = this.nodePort(o, p, id, V, 'in'); const ta = a * (0.55 + 0.45 * (hl ? 1 : 0)); o.line([[wpt[0], wpt[1]], [lerp(wpt[0], s[0], u), lerp(wpt[1], s[1], u)]], hl ? 3.5 : 2, hl ? COL.blue : 'rgba(10,102,255,0.55)', ta, [2, 8]); o.dot(s[0], s[1], 6, COL.blue, a); }
        this.tagPill(o, p.x, p.y + 12 * (1 - u), name, vtxt, a, hl ? 'src' : '');
      } else {
        const fl = flagT[id] !== undefined && t >= flagT[id];
        const ring = fl ? Math.exp(-5 * (t - flagT[id])) : 0;
        o.dot(p.x, p.y, 15 * u, fl ? COL.amber : '#ffffff', a); o.ring(p.x, p.y, 15 * u, 3, fl ? COL.amber : 'rgba(29,29,31,0.55)', a);
        if (ring > 0.02) o.ring(p.x, p.y, 15 + 40 * (1 - ring), 4, COL.amber, a * ring);
      }
    }
    // the label of the record that was flagged last (one at a time)
    const fo = B.simple.flagOrder; let cur = -1; fo.forEach((id, k) => { if (t >= flagT[id]) cur = k; });
    if (cur >= 0 && t < 38.4) {
      const id = fo[cur], p = pos[id], nd = B.simple.nodes.find((n) => n.id === id), s = ja ? nd.ja : nd.en;
      const a = gA * seg(t, flagT[id], flagT[id] + 0.08);
      const w = o.tw(s, LS, 600) + 44; const x = Math.min(1830 - w, p.x - w / 2);
      o.pill(s, x, p.y - 58, { size: LS, weight: 600, color: COL.amberT, alpha: a, stroke: COL.amber, strokeW: 3 });
    }
  }
  tagPill(o, x, cy, name, val, a, kind) {
    const wN = o.tw(name, LS, 500, true), wV = o.tw(val, LS, 650), w = wN + wV + 26 + 44, h = 58, c = o.c;
    c.save(); c.globalAlpha *= a; c.shadowColor = 'rgba(20,30,50,0.10)'; c.shadowBlur = 16; c.shadowOffsetY = 4;
    c.fillStyle = kind === 'src' ? '#eaf2ff' : 'rgba(255,255,255,0.95)'; o.rr(x, cy - h / 2, w, h, 12); c.fill(); c.shadowColor = 'transparent';
    if (kind === 'src') { c.strokeStyle = COL.blue; c.lineWidth = 3; o.rr(x, cy - h / 2, w, h, 12); c.stroke(); }
    c.restore();
    o.txt(name, x + 22, cy + 14, { size: LS, weight: 500, mono: true, color: COL.ink3, alpha: a });
    o.txt(val, x + 22 + wN + 26, cy + 14, { size: LS, weight: 650, color: kind === 'src' ? COL.blueT : COL.ink, alpha: a });
    return w;
  }
  nodePort(o, p, id, V, side) {
    if (!p.tag) return [p.x, p.y];
    // tags: their right edge (outgoing to derived records) at the vertical centre
    const name = id.split(' · ')[1]; const w = o.tw(name, LS, 500, true) + 26 + 44 + o.tw('000 mm', LS, 650);
    return side === 'in' ? [p.x + w, p.y] : [p.x + w, p.y];
  }
  realGraph(o, V, t) {
    const img = this.foot[V.lang].graph, c = o.c, k2 = img.width / 1920;
    // the graph panel of the AMOEBA Control Centre, job 6 (crop along the panel border)
    const cr = V.lang === 'ja' ? this.graphCrop.ja : this.graphCrop.en;
    const k = 1.0 + 0.04 * smooth(seg(t, 38.4, 40.4)), w = cr.w * k, h = cr.h * k;
    const u = spring(seg(t, 38.4, 38.85)), out = seg(t, 40.15, 40.35);
    const X = 960 - w / 2, Y = 360 + 60 * (1 - u) + 40 * out;
    c.save(); c.globalAlpha = seg(t, 38.4, 38.55) * (1 - out);
    c.fillStyle = 'rgba(244,245,247,0.9)'; c.fillRect(0, 0, 1920, 1080);
    c.shadowColor = 'rgba(20,30,50,0.18)'; c.shadowBlur = 40; c.shadowOffsetY = 12; c.fillStyle = '#fff'; o.rr(X, Y, w, h, 14); c.fill(); c.shadowColor = 'transparent';
    o.rr(X, Y, w, h, 14); c.clip(); c.drawImage(img, cr.x * k2, cr.y * k2, cr.w * k2, cr.h * k2, X, Y, w, h); c.restore();
  }
  get graphCrop() { return { en: { x: 256, y: 188, w: 1640, h: 580 }, ja: { x: 256, y: 188, w: 1640, h: 580 } }; }

  // ================================================================== 8 · 42-46 s · what it cannot confirm
  s8(t) {
    const st = this.st, P = st.parts;
    this.tee({ h: 'header6', b: 'branch6' });
    st.tubes.amber6.draw(1, 1);
    const u = seg(t, 42, 46);
    this.cam({ tx: 640, ty: 80, tz: 0, az: 34 + 10 * u, el: 18, dist: 2900, fov: 24, sx: 520, sy: 130 });
    return (o, V) => {
      const S = V.S, ja = V.lang === 'ja';
      o.veil(0.5 * seg(t, 42.0, 42.3));
      this.head(o, V, S.honesty_big, t, 42.0, 44.45);
      this.gates(o, V, t);
      // REVIEW REQUIRED (once), 44.6 s
      if (t >= 44.5) {
        const s = B.ui[V.lang]['state.REVIEW_REQUIRED'], uu = spring(seg(t, 44.5, 44.9), 0.65), c = o.c;
        const w = o.tw(s, HS, 750) + 80, h = 172, x = 120, y = 92;
        c.save(); c.globalAlpha = seg(t, 44.5, 44.6) * (1 - seg(t, 45.8, 45.95)); c.translate(x + w / 2, y + h / 2); c.scale(1.08 - 0.08 * uu, 1.08 - 0.08 * uu); c.translate(-(x + w / 2), -(y + h / 2));
        c.fillStyle = 'rgba(255,255,255,0.92)'; o.rr(x, y, w, h, 20); c.fill(); c.strokeStyle = COL.amber; c.lineWidth = 10; o.rr(x, y, w, h, 20); c.stroke();
        o.txt(s, x + 40, y + 132, { size: HS, weight: 750, color: COL.amberT }); c.restore();
      }
      if (V.labels) o.block([S.dev_line], 120, 1046, 0, { size: LS, weight: 500, color: COL.ink3 }, t, 42.8, 45.95);
    };
  }
  gates(o, V, t) {
    const g = B.gates6, cnt = B.gateCounts6, ja = V.lang === 'ja', c = o.c;
    const groups = [['PASS', COL.green], ['INCONCLUSIVE', '#8e8e93'], ['NOT_IMPLEMENTED', null]];
    const a = seg(t, 42.2, 42.4) * (1 - seg(t, 45.8, 45.95));
    if (a <= 0) return;
    const S = 64, gap = 16, X0 = 120, rowY = [520, 690, 860];
    // tiles in gate order (42.3 s), then they settle into their groups (42.6 s, the music's count)
    const settle = spring(seg(t, 42.6, 43.2), 0.8);
    const idxIn = {}; groups.forEach(([r]) => (idxIn[r] = 0));
    g.forEach((gate, k) => {
      const gi = groups.findIndex(([r]) => r === gate.result), j = idxIn[gate.result]++;
      const x0 = X0 + k * (S + gap), y0 = rowY[1];
      const x1 = X0 + j * (S + gap), y1 = rowY[gi];
      const x = lerp(x0, x1, settle), y = lerp(y0, y1, settle), ap = a * seg(t, 42.3 + k * 0.02, 42.4 + k * 0.02);
      c.save(); c.globalAlpha = ap;
      if (groups[gi][1]) { c.fillStyle = groups[gi][1]; o.rr(x, y - S / 2, S, S, 10); c.fill(); }
      else { c.strokeStyle = COL.ink2; c.lineWidth = 4; c.setLineDash([8, 6]); o.rr(x + 2, y - S / 2 + 2, S - 4, S - 4, 9); c.stroke(); }
      c.restore();
      if (gate.result === 'PASS') o.tick(x + S / 2, y + 2, 32, settle, '#fff', 6, ap);
    });
    // group labels with their counts (from the gates' own results; never merged)
    groups.forEach(([r, col], gi) => {
      const n = cnt[r] || 0, la = a * seg(t, 42.9 + gi * 0.12, 43.1 + gi * 0.12);
      const lab = r === 'PASS' ? V.S.res_pass : B.ui[V.lang]['result.' + r];
      const x = X0 + 7 * (S + gap) + 30;
      o.txt(`${lab}`, x, rowY[gi] + 17, { size: BS, weight: 650, color: r === 'PASS' ? COL.greenT : COL.ink, alpha: la });
      const lw = o.tw(lab, BS, 650);
      o.txt(mm(n), x + lw + 24, rowY[gi] + 17, { size: BS, weight: 650, color: COL.ink3, alpha: la });
      if (r === 'NOT_IMPLEMENTED') o.txt(V.S.gate_approval, x, rowY[gi] + 76, { size: LS, weight: 500, color: COL.ink2, alpha: la });
    });
  }

  // ================================================================== 9 · 46-50 s · English and Japanese, offline
  s9(t) {
    const st = this.st;
    this.tee({ h: 'header6', b: 'branch6' });
    st.tubes.amber6.draw(1, 1 - seg(t, 46.0, 46.4));
    const u = seg(t, 46, 50);
    this.cam({ tx: 640, ty: 80, tz: 0, az: 44 + 14 * smooth(u), el: 16 + 6 * u, dist: 2700 - 250 * u, fov: 24, sx: -60, sy: 150 });
    return (o, V) => {
      const S = V.S;
      o.veil(0.5 * (1 - seg(t, 46.0, 46.35)));
      this.head(o, V, S.lang_big, t, 46.0, 47.95);
      this.head(o, V, S.offline, t, 48.0, 49.95);
      // the application's own labels (catalogue) flip from English to Japanese on 46.9 s, tethered to the tee
      const r = this.req5, b = r.beta_deg * deg, v3 = (x, y, z) => new THREE.Vector3(x, y, z);
      const O = v3(r.branch_station, 0, 0), A = v3(Math.cos(b), Math.sin(b), 0), e1 = v3(-Math.sin(b), Math.cos(b), 0), top = O.clone().addScaledVector(A, r.s_end);
      const R6 = D.pat.header6.request.header_radius;
      const tags = [
        { k: 'branch_radius', an: top.clone().addScaledVector(e1, -r.branch_radius), at: [1560, 330] },
        { k: 'beta_deg', an: O.clone().addScaledVector(A, 250), at: [1560, 470] },
        { k: 'header_radius', an: v3(1200, R6 * 0.7, R6 * 0.7), at: [1560, 760] },
        { k: 'header_length', an: v3(260, -R6 * 0.3, R6 * 0.95), at: [1560, 900] },
      ];
      const flipT = 46.9;
      tags.forEach((tg, k) => {
        const t0 = 46.15 + 0.1 * k, a = seg(t, t0, t0 + 0.15) * (1 - seg(t, 47.85, 47.98));
        if (a <= 0) return;
        const en = B.params[tg.k].en, ja = B.params[tg.k].ja, f = seg(t, flipT + 0.06 * k, flipT + 0.06 * k + 0.3);
        const s = this.P(tg.an), sy = Math.cos(Math.PI * f);              // card flip about its horizontal axis
        const txt = f < 0.5 ? en : ja;
        const lang0 = o.lang; o.lang = f < 0.5 ? 'en' : 'ja';
        const w = o.tw(txt, LS, 560) + 44, x = tg.at[0] - w / 2, y = tg.at[1];
        o.line([[s[0], s[1]], [x, y]], 2.5, 'rgba(10,102,255,0.6)', a, [2, 8]); o.dot(s[0], s[1], 6, COL.blue, a);
        const c = o.c; c.save(); c.translate(0, y); c.scale(1, Math.max(0.02, Math.abs(sy))); c.translate(0, -y);
        o.pill(txt, x, y, { size: LS, weight: 560, alpha: a });
        c.restore(); o.lang = lang0;
      });
      this.label(o, V, S.label_vis, t, 46.0, 47.95);
    };
  }

  // ================================================================== 10 · 50-54 s · end card
  s10(t) {
    const st = this.st;
    this.tee({ h: 'header6', b: 'branch6' });
    const u = smooth(seg(t, 50, 54));
    // the tee rests small above the wordmark
    const k = 0.96 + 0.04 * quint(seg(t, 50.0, 51.0));
    this.cam(lerpCam({ tx: 640, ty: 80, tz: 0, az: 58, el: 22, dist: 2450, fov: 24, sx: -60, sy: 150 }, { tx: 640, ty: 80, tz: 0, az: 26 + 8 * u, el: 16, dist: 6300, fov: 24, sx: 0, sy: -250 }, k));
    st.setShadow({ opacity: 0.55 });
    return (o, V) => {
      const S = V.S, ja = V.lang === 'ja';
      const a0 = seg(t, 50.05, 50.4);
      o.block(S.brand_name, 960, 560, 0, { size: HS, weight: 700, align: 'center', track: 6 }, t, 50.0, 54.5);
      o.block([S.brand_line], 960, 640, 0, { size: BS, weight: 500, align: 'center', color: COL.ink2 }, t, 50.5, 54.5);
      // call to action (51.6 s, the music's cta)
      if (t >= 51.5) { const uu = spring(seg(t, 51.5, 51.9)), c = o.c; const s = S.cta, w = o.tw(s, BS, 600) + 96, h = 92, x = 960 - w / 2, y = 690 + 10 * (1 - uu);
        c.save(); c.globalAlpha = seg(t, 51.5, 51.62); c.fillStyle = COL.blue; o.rr(x, y, w, h, h / 2); c.fill(); c.restore();
        o.txt(s, 960, y + 62, { size: BS, weight: 600, color: '#fff', align: 'center', alpha: seg(t, 51.5, 51.62) }); }
      // contacts (always), under the button
      o.block([S.contact.join('   ·   ')], 960, 860, 0, { size: LS, weight: 500, align: 'center', color: COL.ink }, t, 51.0, 54.5);
      o.block([S.positioning], 960, 922, 0, { size: LS, weight: 500, align: 'center', color: COL.ink2 }, t, 52.6, 54.5);
      if (V.labels) o.block([S.disclaimer], 960, 1046, 0, { size: LS, weight: 500, align: 'center', color: COL.ink3 }, t, 50.3, 54.5);
      if (ja && S.credit) o.block([S.credit], 1800, 990, 0, { size: LS, weight: 500, align: 'right', color: COL.ink2 }, t, 50.3, 54.5);
    };
  }
}
const V_LANG_ANY = 'en';
export const fmt = (s, o) => s.replace(/\{(\w+)\}/g, (_, k) => (o[k] ?? ''));
function bez(p, q, n) { const out = []; const mx = (p[0] + q[0]) / 2; for (let i = 0; i <= n; i++) { const u = i / n, a = (1 - u) ** 3, b = 3 * (1 - u) ** 2 * u, c = 3 * (1 - u) * u * u, d = u ** 3; out.push([a * p[0] + b * mx + c * mx + d * q[0], a * p[1] + b * p[1] + c * q[1] + d * q[1]]); } return out; }
// words broken into lines no wider than maxW: identifiers break after an underscore, Japanese after any character
function wrapText(o, s, maxW, mono, ja) {
  const parts = ja ? [...s] : s.split(/(?<=_)|(?= )/); const out = []; let cur = '';
  for (const p of parts) { const cand = cur + p; if (!cur || o.tw(cand, LS, 500, mono) <= maxW) cur = cand; else { out.push(cur); cur = p.replace(/^ /, ''); } }
  if (cur) out.push(cur); return out;
}
