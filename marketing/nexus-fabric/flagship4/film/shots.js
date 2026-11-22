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
    st.tube('true', hole5, 4.5, C3.blue, true, 720);
    st.tube('amberSeam', hole5, 3.6, C3.amber, true, 720);
    // the branch saddle edge (job 5): child of the branch part so it follows the branch plate when it moves
    P.branch5.set(1);
    const sad5 = D.pat.branch5.curves.saddle_cut.filter((_, i) => i % 1 === 0).map(([x, y]) => P.branch5.at(x, y, 1).toArray());
    st.tube('saddle', sad5, 3.4, 0xff7a1a, false, 720); P.branch5.grp.add(st.tubes.saddle.mesh);
    st.tube('one', hole5, 3.6, C3.green, true, 720);
    // the wrong curve (hook): job 5's own branch plate, its computed saddle set a quarter turn out of orientation about
    // the branch axis (a marking/orientation error), then slid down its axis to first contact with the header. The cut
    // is the product's; only its placement is wrong. Slide d* = min over saddle points of (y0 - sqrt(R^2 - z0^2)) / sin(beta)
    // (a point reaches the header radius R after moving that far along -a). Of the two quarter turns, the one whose
    // largest gap faces the camera (+Z) is used; the joint is symmetric about the XY plane, so both are the same error.
    const b = req5.beta_deg * deg, a = new THREE.Vector3(Math.cos(b), Math.sin(b), 0), R5 = req5.header_radius;
    const O5 = new THREE.Vector3(req5.branch_station, 0, req5.delta);
    const sad5v = D.pat.branch5.curves.saddle_cut.map(([x, y]) => P.branch5.at(x, y, 1));
    let best = null;
    for (const ang of [Math.PI / 2, -Math.PI / 2]) {
      const q = new THREE.Quaternion().setFromAxisAngle(a, ang);
      const pts = sad5v.map((p) => p.clone().sub(O5).applyQuaternion(q).add(O5));
      let dmin = Infinity; for (const p of pts) dmin = Math.min(dmin, (p.y - Math.sqrt(R5 * R5 - p.z * p.z)) / Math.sin(b));
      const shift = a.clone().multiplyScalar(-dmin), placed = pts.map((p) => p.clone().add(shift));
      let gk = 0, gmax = -1; placed.forEach((p, k) => { const g = Math.hypot(p.y, p.z) - R5; if (g > gmax) { gmax = g; gk = k; } });
      if (!best || placed[gk].z > best.placed[best.gk].z) best = { q, shift, placed, gk, gmax };
    }
    this.wrongQ = best.q; this.wrongPos = O5.clone().sub(O5.clone().applyQuaternion(best.q)).add(best.shift);
    this.wrongPts = best.placed; this.gapPt = best.placed[best.gk]; this.gapMax = best.gmax;
    st.tube('wrong', best.placed.map((p) => p.toArray()), 6.0, C3.red, false, 720);
    // the gap itself: a ribbon from each point of the misplaced cut edge down to the header surface (radial foot)
    { const n = best.placed.length, pos = new Float32Array(n * 6), idx = [];
      best.placed.forEach((p, k) => { const r = Math.hypot(p.y, p.z); pos.set([p.x, p.y, p.z, p.x, p.y * R5 / r, p.z * R5 / r], k * 6); if (k) { const i = 2 * k; idx.push(i - 2, i - 1, i, i, i - 1, i + 1); } });
      const g = new THREE.BufferGeometry(); g.setAttribute('position', new THREE.BufferAttribute(pos, 3)); g.setIndex(idx);
      const m = new THREE.Mesh(g, new THREE.MeshBasicMaterial({ color: 0xff3b30, transparent: true, opacity: 0.6, side: THREE.DoubleSide, depthWrite: false, toneMapped: false }));
      m.renderOrder = 3; m.frustumCulled = false; st.root.add(m); this.gapRibbon = m; m.visible = false; }
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
    const st = this.st; for (const p of Object.values(st.parts)) { p.grp.visible = false; p.grp.position.set(0, 0, 0); p.grp.scale.set(1, 1, 1); p.grp.rotation.set(0, 0, 0); p.grp.quaternion.identity(); p.mesh.material = st.steel; }
    for (const t of Object.values(st.tubes)) { t.draw(0); t.mesh.position.set(0, 0, 0); t.mesh.scale.set(1, 1, 1); }
    if (this.gapRibbon) { this.gapRibbon.visible = false; this.gapRibbon.position.set(0, 0, 0); }
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
  label(o, V, s, t, t0, t1, row = 0) { if (!V.labels) return; o.block([s], 120, 1046 - row * 54, 0, { size: LS, weight: 500, color: COL.ink3, bg: 'rgba(244,245,247,0.82)' }, t, t0, t1); }

  // ================================================================== 1 · 0-4 s · the wrong curve
  s1(t) {
    const st = this.st, P = st.parts, b = this.req5.beta_deg * deg, A = new THREE.Vector3(Math.cos(b), Math.sin(b), 0);
    // the misplaced branch comes down its axis and lands at 0.9 s; on beat 2 (1.0 s) the gap and the wrong edge light up
    const land = spring(seg(t, 0.0, 0.9), 0.6);
    this.tee({ b: 'branch5' });
    P.branch5.grp.quaternion.copy(this.wrongQ); P.branch5.grp.position.copy(this.wrongPos).addScaledVector(A, 420 * (1 - land));
    const g = this.gapPt;
    // camera: the joint, slowly orbiting; on 2.5 s ("doesn't fit") it snaps in to the widest gap
    const u = t / 4, snap = spring(seg(t, 2.45, 2.85), 0.75);
    const c0 = { tx: 640, ty: 90, tz: 0, az: 18 + 10 * u, el: 12 + 4 * u, dist: 2150 - 250 * u, fov: 24, sx: 120, sy: 140 };
    const c1 = { tx: g.x, ty: g.y - 10, tz: g.z, az: 30 + 6 * seg(t, 2.8, 4), el: 16, dist: 1000 - 60 * seg(t, 2.8, 4), fov: 24, sx: 330, sy: 170 };
    this.cam(lerpCam(c0, c1, snap));
    const on = seg(t, 0.95, 1.05);
    st.tubes.wrong.draw(t >= 0.95 ? 1 : 0, on);
    this.gapRibbon.visible = t >= 0.95; this.gapRibbon.material.opacity = 0.62 * on * (0.85 + 0.15 * Math.cos((t - 1) * Math.PI * 4));
    return (o, V) => {
      const S = V.S;
      this.head(o, V, S.hook, t, 0.3, 3.95);
      // a pulse ring on the widest gap at 1.0 s and again on the snap (2.5 s)
      for (const tp of [1.0, 2.5]) { const k = seg(t, tp, tp + 0.5); if (k > 0 && k < 1) { const p = this.P(g); o.ring(p[0], p[1], 20 + 90 * outCubic(k), 6, COL.red, 1 - k); } }
      this.label(o, V, S.label_vis, t, 1.0, 3.95);
    };
  }

  // ================================================================== 2 · 4-8 s · the true curve, drawn on the steel
  s2(t) {
    const st = this.st, P = st.parts, b = this.req5.beta_deg * deg, A = new THREE.Vector3(Math.cos(b), Math.sin(b), 0);
    const out = smooth(seg(t, 4.0, 4.45));                                   // the misplaced branch leaves along its axis, its red edge with it
    this.tee({ b: 'branch5' });
    const away = A.clone().multiplyScalar(1100 * out * out);
    P.branch5.grp.quaternion.copy(this.wrongQ); P.branch5.grp.position.copy(this.wrongPos).add(away);
    if (out >= 1) P.branch5.grp.visible = false;
    st.tubes.wrong.draw(1, 1 - seg(t, 4.1, 4.35)); st.tubes.wrong.mesh.position.copy(away);
    this.gapRibbon.visible = t < 4.25; this.gapRibbon.material.opacity = 0.62 * (1 - seg(t, 4.0, 4.2));
    const g = this.gapPt, u = seg(t, 4, 8), mv = quint(seg(t, 4.0, 5.2));
    const c0 = { tx: g.x, ty: g.y - 10, tz: g.z, az: 36, el: 16, dist: 940, fov: 24, sx: 330, sy: 170 };
    const c1 = { tx: 690, ty: 150, tz: 0, az: 18 + 16 * u, el: 38, dist: 1450 - 100 * u, fov: 24, sx: 260, sy: 160 };
    this.cam(lerpCam(c0, c1, mv));
    // the true intersection curve (job 5's opening) draws on the header surface, point by point
    st.tubes.true.draw(spring(seg(t, 4.25, 5.2), 0.9), 1);
    return (o, V) => {
      const S = V.S;
      this.head(o, V, S.question, t, 4.0, 7.95);
      // a marker travels the curve (6.25-7.75 s, the music's draw)
      const m = seg(t, 6.25, 7.75);
      if (m > 0 && m < 1) { const n = this.hole5.length, p = this.hole5[Math.floor(m * n) % n]; const s = this.P(p); o.dot(s[0], s[1], 12, '#ffffff', 0.95); o.ring(s[0], s[1], 12, 5, COL.blue, 1); }
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
    st.tubes.true.draw(t < 8.5 ? 1 : 0, 1 - seg(t, 8.3, 8.48));
    // camera: whole tee on the left for the callouts (labels in a column on the right), then straight above the flat
    // plate where the application's preview will show it (match dissolve at 10.0 s)
    const fit = this.appFit();
    const c0 = { tx: 620, ty: 60, tz: 0, az: 24, el: 20, dist: 3500, fov: 24, sx: -230, sy: 40 };
    const c0b = { tx: 620, ty: 60, tz: 0, az: 30, el: 23, dist: 3350, fov: 24, sx: -230, sy: 40 };
    const ctop = { tx: 600, ty: 150, tz: 0, az: 0, el: 89.9, dist: fit.dist, fov: 24, sx: fit.sx, sy: fit.sy };
    this.cam(lerpCam(lerpCam(c0, c0b, seg(t, 8, 9.6)), ctop, quint(seg(t, 9.6, 10.0))));
    return (o, V) => {
      const S = V.S;
      if (t >= 9.88) this.appWindow(o, V, t);
      if (t < 10) {
        this.head(o, V, S.brand_name, t, 8.2, 9.2);
        this.head(o, V, S.product_a, t, 9.2, 9.95);
        const cA = clamp(1 - seg(t, 9.5, 9.62));
        if (cA > 0) this.callouts(o, V, t, cA);
        this.label(o, V, S.label_vis, t, 8.0, 9.95);
      }
    };
  }
  // The real result screen (job #5): the crop is the plate in the preview with its rulers and dimensions, read from
  // the frame (plate rectangle = where the preview draws x 0..1200, y 0..942); it pushes in to the opening and the
  // 942.478 dimension over the two seconds. The gates panel (right of the preview) is never in the crop.
  appPlate(lang) { return lang === 'ja' ? [609, 265, 1379, 868] : [609, 253, 1379, 856]; }
  appView(lang, t) {
    const pl = this.appPlate(lang), W0 = 906, H0 = 715;
    const c0 = [pl[0] - 64 + W0 / 2, pl[1] - 62 + H0 / 2];                          // whole plate, rulers, both dimensions
    const c1 = [pl[2] - 199, (pl[1] + pl[3]) / 2 + 6];                             // the opening and the 942.478 dimension
    const z = 1 + 0.95 * quint(seg(t, 10.15, 11.75));
    const cx = lerp(c0[0], c1[0], (z - 1) / 0.95), cy = lerp(c0[1], c1[1], (z - 1) / 0.95);
    return { sx: cx - W0 / (2 * z), sy: cy - H0 / (2 * z), sw: W0 / z, sh: H0 / z, X: 498, Y: 272, W: 924, H: 729 };
  }
  appFit() {
    // the 3D plate (1200 mm) must cover the preview's plate at the first frame of the window; px per mm at distance d
    // (fov 24) is 1080 / (2 d tan 12 deg)
    const v = this.appView('en', 10.0), pl = this.appPlate('en'), k = v.W / v.sw;
    const pw = (pl[2] - pl[0]) * k, dist = 1080 / (2 * Math.tan(12 * deg) * (pw / 1200));
    const cx = v.X + ((pl[0] + pl[2]) / 2 - v.sx) * k, cy = v.Y + ((pl[1] + pl[3]) / 2 - v.sy) * k;
    return { dist, sx: cx - 960, sy: cy - 540 };
  }
  appWindow(o, V, t) {
    const v = this.appView(V.lang, t), img = this.foot[V.lang].result, c = o.c, k2 = img.width / 1920;
    const a = seg(t, 9.88, 10.0);
    c.save(); c.globalAlpha = a; c.shadowColor = 'rgba(20,30,50,0.18)'; c.shadowBlur = 40; c.shadowOffsetY = 12; c.fillStyle = '#fff'; o.rr(v.X, v.Y, v.W, v.H, 16); c.fill(); c.shadowColor = 'transparent';
    o.rr(v.X, v.Y, v.W, v.H, 16); c.clip(); c.drawImage(img, v.sx * k2, v.sy * k2, v.sw * k2, v.sh * k2, v.X, v.Y, v.W, v.H); c.restore();
    o.block(V.S.product_b, 120, 210, 140, { size: HS, weight: 650 }, t, 10.0, 12.2);
    this.label(o, V, V.S.label_actual, t, 10.0, 12.2);
  }
  callouts(o, V, t, alpha) {
    const r = this.req5, b = r.beta_deg * deg, prm = B.params, ja = V.lang === 'ja';
    const name = (k) => (prm[k] ? (ja ? prm[k].ja : prm[k].en) : k);
    const v3 = (x, y, z) => new THREE.Vector3(x, y, z);
    const items = [];
    // the job's inputs (job 5 request) on the geometry; every label sits in one column on the right with a leader line
    // to its anchor, so no text lies on the steel. Slots are assigned in the order of the anchors' height on screen.
    { const L = r.header_length, a = this.P(v3(L, 0, 0)), q = this.P(v3(L, -r.header_radius * Math.sin(30 * deg), r.header_radius * Math.cos(30 * deg))); items.push({ t0: 8.5, line: [a, q], dot: a, lead: q, lab: [name('header_radius'), `${mm(r.header_radius)} mm`] }); }
    { const y = -r.header_radius - 60, a = this.P(v3(0, y, 160)), q = this.P(v3(r.branch_station, y, 160)), top = this.P(v3(r.branch_station, -r.header_radius, 120)); items.push({ t0: 8.75, dim: [a, q], tick: [q, top], lead: [(a[0] + q[0]) / 2, (a[1] + q[1]) / 2], lab: [name('branch_station'), `${mm(r.branch_station)} mm`] }); }
    { const y = -r.header_radius - 60, a = this.P(v3(0, y, 160)), q = this.P(v3(r.header_length, y, 160)); const a2 = [a[0], a[1] + 110], q2 = [q[0], q[1] + 110]; items.push({ t0: 9.0, dim: [a2, q2], ext: [[a, a2], [q, q2]], lead: [lerp(a2[0], q2[0], 0.75), lerp(a2[1], q2[1], 0.75)], lab: [name('header_length'), `${mm(r.header_length)} mm`] }); }
    { const O = v3(r.branch_station, 0, r.delta), A = v3(Math.cos(b), Math.sin(b), 0), e1 = v3(-Math.sin(b), Math.cos(b), 0);
      const c = O.clone().addScaledVector(A, r.s_end), q = c.clone().addScaledVector(e1, -r.branch_radius); const pc = this.P(c), pq = this.P(q);
      items.push({ t0: 9.25, line: [pc, pq], dot: pc, lead: [(pc[0] + pq[0]) / 2, (pc[1] + pq[1]) / 2], lab: [name('branch_radius'), `${mm(r.branch_radius)} mm`] }); }
    { const arc = []; for (let i = 0; i <= 24; i++) { const q = (i / 24) * b; arc.push(this.P(v3(r.branch_station + 230 * Math.cos(q), 230 * Math.sin(q), 0))); }
      items.push({ t0: 9.5, arc, lead: arc[12], lab: [name('beta_deg'), `${mm(r.beta_deg)}°`] }); }
    const order = items.map((it, k) => k).sort((p, q) => items[p].lead[1] - items[q].lead[1]);
    order.forEach((k, slot) => (items[k].at = [1840, 250 + slot * 168]));
    for (const it of items) {
      const u = spring(seg(t, it.t0 - 0.25, it.t0 + 0.2)), a = alpha * seg(t, it.t0 - 0.25, it.t0 - 0.1);
      if (a <= 0) continue;
      if (it.line) { const [p, q] = it.line; o.line([p, [lerp(p[0], q[0], u), lerp(p[1], q[1], u)]], 4, COL.blue, a); o.dot(p[0], p[1], 7, COL.blue, a); }
      if (it.dim) { const [p, q] = it.dim; const m = [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2]; o.line([[lerp(m[0], p[0], u), lerp(m[1], p[1], u)], [lerp(m[0], q[0], u), lerp(m[1], q[1], u)]], 4, COL.blue, a); o.line([[p[0], p[1] - 22], [p[0], p[1] + 22]], 4, COL.blue, a * u); o.line([[q[0], q[1] - 22], [q[0], q[1] + 22]], 4, COL.blue, a * u); }
      if (it.tick) { const [q, top] = it.tick; o.line([q, [lerp(q[0], top[0], u), lerp(q[1], top[1], u)]], 3, COL.blue, a, [10, 10]); }
      if (it.ext) for (const [p, q] of it.ext) o.line([p, [lerp(p[0], q[0], u), lerp(p[1], q[1], u)]], 2, COL.blue, a * 0.6, [6, 8]);
      if (it.arc) { const n = Math.max(2, Math.round(u * it.arc.length)); o.line(it.arc.slice(0, n), 4, COL.blue, a); }
      const [x, y] = it.at, w = Math.max(o.tw(it.lab[0], LS, 500), o.tw(it.lab[1], BS, 650));
      const lx = x - w - 28, ly = y + 8; const e = [lerp(it.lead[0], lx, u), lerp(it.lead[1], ly, u)];
      o.line([it.lead, e], 3, COL.blue, a, [2, 8]); o.dot(it.lead[0], it.lead[1], 7, COL.blue, a);
      o.txt(it.lab[1], x, y + 10 * (1 - u), { size: BS, weight: 650, color: COL.blueT, align: 'right', alpha: a });
      o.txt(it.lab[0], x, y + 52 + 10 * (1 - u), { size: LS, weight: 500, color: COL.ink2, align: 'right', alpha: a });
    }
  }

  // ================================================================== 4 · 12-20 s · five families
  partPlace(i) {
    // one family per slot, cut on the music's accent. Scale per family so each flat pattern spans a similar width on
    // screen (a visual normalisation; no size is stated). rot: Euler XYZ, so the flat pattern lies horizontal and wide.
    const cfg = [
      { key: 'shell', s: 0.50, l: [500, 0, 0], rot: [0, Math.PI / 2, 0], dir: 'unroll', cam: { az: 20, el: 30, dist: 3600, sx: 200, sy: 30, ty: 60 } },
      { key: 'cone', s: 1.15, l: [0, -450, 0], rot: [0, 0, 0], dir: 'roll', cam: { az: 22, el: 30, dist: 3250, sx: 150, sy: 120, ty: -200 } },
      { key: 'ecc', s: 1.15, l: [0, -450, 150], rot: [0, 0, 0], dir: 'unroll', cam: { az: 24, el: 30, dist: 3250, sx: 150, sy: 120, ty: -200 } },
      { key: 'branch4', s: 1.6, l: [730, 225, 0], rot: [-Math.PI / 2, 0, -Math.PI / 3], dir: 'roll', cam: { az: 22, el: 32, dist: 2900, sx: 180, sy: 40, ty: 60 } },
      { key: 'header5', s: 1.0, l: [600, 0, 0], rot: [0, 0, 0], dir: 'unroll', cam: { az: 24, el: 32, dist: 3250, sx: 340, sy: 120, ty: 40 } },
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
      const S = V.S, ja = V.lang === 'ja';
      const names = [S.part_shell, S.part_cone, S.part_ecc, S.part_branch, S.part_header];
      // the family's name stays up for its whole slot (it changes on the cut); the catalogue's long name adds the
      // geometric kind where it gives one in brackets (right / oblique cone frustum)
      this.head(o, V, names[i], t, ts, i < 4 ? te + 0.3 : 19.95);
      const famKey = ['cylinder_shell', 'cone_frustum', 'eccentric_reducer', 'branch_stub', 'header_shell_with_opening'][i];
      const lg = (D.fam[famKey][V.lang] || {}).long || '', mt = lg.match(/[（(]([^）)]+)[）)]/);
      const nLines = names[i].length;
      let y = 210 + (nLines - 1) * 140 + 80;
      if (mt) { const sub = ja ? mt[1] : mt[1][0].toUpperCase() + mt[1].slice(1); o.block([sub], 120, y, 0, { size: BS, weight: 500, color: COL.ink2 }, t, ts, i < 4 ? te + 0.3 : 19.95); y += 70; }
      // the computed outline flashes when the plate locks flat (lock = slot start + 1.1 s) and stays as a thin line while flat
      const flatness = 1 - clamp(part.kappa * 6);
      if (flatness > 0) {
        const lock = pp.dir === 'unroll' ? ts + 1.1 : ts;
        const fl = t >= lock - 0.05 ? Math.exp(-6 * Math.max(0, t - lock)) : 0;
        this.outline(o, pp, part, 3 + 5 * fl, flatness * (0.6 + 0.4 * fl));
      }
      // one dimension of the flat pattern, from the product's quantities (angles converted from radians: deg = rad * 180 / pi)
      const Q = D.pat[pp.key].quantities, QL = D.quantLab;
      const dim = [['developed_width', `${mm(Q.developed_width)} mm`], ['sector_angle', `${mm(Q.sector_angle * 180 / Math.PI)}°`], ['developed_angle', `${mm(Q.developed_angle * 180 / Math.PI)}°`], ['s_max', `${mm(Q.s_max)} mm`], ['opening_perimeter', `${mm(Q.opening_perimeter)} mm`]][i];
      const da = flatness * (pp.dir === 'unroll' ? seg(t, ts + 0.95, ts + 1.15) : 1 - seg(t, ts + 0.6, ts + 0.75));
      if (da > 0.01) {
        const cv = D.pat[pp.key].holes[0] || Object.values(D.pat[pp.key].curves)[0], anc = this.P(this.flatWorld(pp, part, cv[Math.floor(cv.length / 2)]));
        const w = Math.max(o.tw(dim[1], BS, 650), o.tw(QL[dim[0]][V.lang], LS, 500));
        o.line([[120 + w + 24, y + 12], anc], 3, COL.blue, da, [2, 8]); o.dot(anc[0], anc[1], 7, COL.blue, da);
        o.txt(dim[1], 120, y + 20, { size: BS, weight: 650, color: COL.blueT, alpha: da });
        o.txt(QL[dim[0]][V.lang], 120, y + 72, { size: LS, weight: 500, color: COL.ink2, alpha: da });
        y += 150;
      }
      // point by point: the opening's computed points (job 5), one after another
      if (i === 4 && t >= 19.1) {
        const pts = D.pat.header5.holes[0], n = pts.length, m = Math.floor(n * seg(t, 19.1, 19.75));
        for (let j = 0; j < m; j += 4) { const p = this.P(part.at(pts[j][0], pts[j][1])); o.dot(p[0], p[1], 4.5, COL.blue, 0.95); }
        if (m > 0 && m < n) { const p = this.P(part.at(pts[m - 1][0], pts[m - 1][1])); o.dot(p[0], p[1], 12, '#fff', 1); o.ring(p[0], p[1], 12, 4, COL.blue, 1); }
        o.block([fmt(S.points_line, { n: mm(n) })], 120, y + 10, 0, { size: BS, weight: 560, color: COL.blueT }, t, 19.1, 19.95);
      }
      if (V.labels) { o.block(S.limits, 120, 938, 52, { size: LS, weight: 500, color: COL.ink3, bg: 'rgba(244,245,247,0.82)' }, t, 12.0, 19.95); }
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
    const c0 = { tx: 600, ty: 40, tz: 0, az: 32, el: 32, dist: 3250, fov: 24, sx: 340, sy: 120 };
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
      this.label(o, V, S.label_vis, t, 20.0, 25.95);
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
    const rows = this._rows.rows, top = 470, H = 440;
    // the list moves by whole rows: each new row slides in from below with a short spring (0.12 s, inside the 0.125 s
    // tick); rows that leave the top fade out instead of being cut
    const target = (k) => { const r = rows[Math.max(0, k - 1)]; return Math.max(0, r.y + r.h - H); };
    const tk = 21.8 + (n - 1) * 0.125, scroll = lerp(target(n - 1), target(n), spring(seg(t, tk, tk + 0.12), 0.9));
    for (let j = 0; j < n; j++) {
      const r = rows[j], y = top + r.y - scroll; if (y > top + H) continue;
      const fade = clamp((y + r.h * 0.6 - top) / (r.h * 0.6)); if (fade <= 0) continue;
      const ti = clamp((t - (21.8 + j * 0.125)) / 0.2), ent = j === n - 1 ? spring(seg(t, tk, tk + 0.12), 0.9) : 1;
      c.save(); c.globalAlpha *= fade * (0.4 + 0.6 * ent);
      c.fillStyle = 'rgba(255,255,255,0.9)'; o.rr(X, y, W, r.h, 14); c.fill();
      o.tick(X + 44, y + 42, 30, ti, COL.green, 6);
      r.lines.forEach((ln, i) => o.txt(ln, X + 92, y + 56 + i * 50, { size: LS, weight: 500, mono: r.mono, color: COL.ink }));
      c.restore();
    }
    // the disclosure belongs to the checks: under the list, in the panel (labels=1)
    if (V.labels) { const sub = ja ? V.S.checks_sub.split('。').filter(Boolean).map((x) => x + '。') : V.S.checks_sub.replace(' — ', ' —\n').split('\n'); sub.forEach((ln, i) => o.txt(ln, X, 966 + i * 50, { size: LS, weight: 500, color: COL.ink3 })); }
    c.restore();
  }

  // ================================================================== 6 · 26-30 s · DXF written, read back, compared
  s6(t) {
    const st = this.st, P = st.parts;
    const kh = 1 - spring(seg(t, 26.0, 26.45), 0.9), lift = smooth(seg(t, 26.0, 26.3));
    const b = this.req5.beta_deg * deg;
    this.tee({ h: 'header6', kh, b: 'branch5', boff: new THREE.Vector3(Math.cos(b), Math.sin(b), 0).multiplyScalar(900 * lift * lift) });
    if (lift >= 1) P.branch5.grp.visible = false;
    const dr = this.dxfRect();
    // camera straight above the plate, the plate filling the drawing rectangle
    const pw = dr.w, dist = 1080 / (2 * Math.tan(12 * deg) * (pw / 1200));
    const ctop = { tx: 600, ty: 150, tz: 0, az: 0, el: 89.9, dist, fov: 24, sx: dr.x + dr.w / 2 - 960, sy: dr.y + dr.h / 2 - 540 };
    const c0 = { tx: 620, ty: 70, tz: 0, az: 44, el: 17, dist: 3050, fov: 24, sx: -470, sy: 130 };
    this.cam(lerpCam(c0, ctop, quint(seg(t, 26.0, 26.45))));
    st.setShadow({ opacity: 0.6 * (1 - seg(t, 26.25, 26.5)) });
    return (o, V) => {
      const S = V.S, c = o.c;
      const words = S.dxf_words; const wt = [26.0, 26.95, 27.6];
      for (let k = 0; k < 3; k++) this.head(o, V, words[k], t, wt[k], k < 2 ? wt[k + 1] - 0.02 : 29.95);
      // the plate turns into its DXF drawing: a white sheet covers the steel, the entities draw on it
      const sh = seg(t, 26.35, 26.55);
      if (sh > 0) { c.save(); c.globalAlpha = sh; c.fillStyle = '#ffffff'; c.shadowColor = 'rgba(20,30,50,0.12)'; c.shadowBlur = 30; c.shadowOffsetY = 8; o.rr(dr.x - 30, dr.y - 30, dr.w + 60, dr.h + 60, 16); c.fill(); c.restore(); }
      const toS = ([x, y]) => [dr.x + x * dr.w / 1200, dr.y + dr.h - y * dr.w / 1200];
      const ents = this.dxfPaths();
      const wU = seg(t, 26.4, 27.0), rU = seg(t, 26.95, 27.55);
      for (const [pass, u, col, w] of [[0, wU, COL.ink, 4], [1, rU, COL.blue, 4]]) {
        if (u <= 0) continue;
        // every entity draws at the same time, each to the same fraction of its own length
        c.save(); c.strokeStyle = col; c.lineWidth = w; c.lineCap = 'round'; c.lineJoin = 'round';
        for (const pth of ents.paths) { let L = 0; for (let j = 1; j < pth.length; j++) L += Math.hypot(pth[j][0] - pth[j - 1][0], pth[j][1] - pth[j - 1][1]);
          const tot = L * u; let acc = 0; c.beginPath(); c.moveTo(...toS(pth[0]));
          for (let j = 1; j < pth.length; j++) { const d = Math.hypot(pth[j][0] - pth[j - 1][0], pth[j][1] - pth[j - 1][1]);
            if (acc + d >= tot) { const f = d ? (tot - acc) / d : 0; c.lineTo(...toS([lerp(pth[j - 1][0], pth[j][0], f), lerp(pth[j - 1][1], pth[j][1], f)])); break; }
            acc += d; c.lineTo(...toS(pth[j])); }
          c.stroke(); }
        c.restore();
      }
      // compared: the tick locks, and the product's own re-import report says what was compared (job 6, header)
      const lk = spring(seg(t, 27.6, 28.0)), R = D.reimport;
      if (t >= 27.6) { o.dot(dr.x + dr.w - 10, dr.y + 10, 46 * lk, COL.green, 1); o.tick(dr.x + dr.w - 10, dr.y + 12, 44, seg(t, 27.65, 27.9), '#fff', 8);
        o.block([fmt(S.reimport_line, { r: mm(R.entities_read), w: mm(R.entities_written), d: mm(R.max_point_deviation) })], 120, 304, 0, { size: BS, weight: 600, color: COL.greenT }, t, 27.6, 29.95); }
      // the rows of the file stream on the right (group code, value) until the stamp
      this.dxfRows(o, V, t);
      // NOT RELEASED (29.0 s, the music's amber): over the lower part of the sheet, below the opening; the real file
      // name in the column on the right where the rows were
      if (t >= 28.95) {
        const u = spring(seg(t, 28.95, 29.35), 0.6), cx = 640, cy = dr.y + dr.h - 105;
        c.save(); c.translate(cx, cy); c.rotate(-3 * deg); c.scale(1.1 - 0.1 * u, 1.1 - 0.1 * u); c.globalAlpha = seg(t, 28.95, 29.05);
        const s = B.ui[V.lang]['tb.nr'], w = o.tw(s, HS, 750) + 90, h = 170;
        c.fillStyle = 'rgba(255,255,255,0.94)'; o.rr(-w / 2, -h / 2, w, h, 20); c.fill(); c.strokeStyle = COL.amber; c.lineWidth = 10; o.rr(-w / 2, -h / 2, w, h, 20); c.stroke();
        o.txt(s, 0, 47, { size: HS, weight: 750, color: COL.amberT, align: 'center' }); c.restore();
        const nm = D.dxf.name, k = nm.indexOf('_', Math.floor(nm.length / 2) - 4) + 1, aa = seg(t, 29.05, 29.25);
        o.txt(nm.slice(0, k), 1160, 440, { size: LS, mono: true, alpha: aa }); o.txt(nm.slice(k), 1160, 492, { size: LS, mono: true, alpha: aa });
        // the marking the file itself carries (its NF_NOT_RELEASED text entity), line by line
        D.dxf.marking.split(' - ').forEach((ln, j) => o.txt(ln, 1160, 572 + j * 50, { size: LS, mono: true, color: COL.amberT, alpha: seg(t, 29.15 + 0.08 * j, 29.35 + 0.08 * j) }));
      }
      this.label(o, V, S.label_vis, t, 26.0, 29.95);
    };
  }
  dxfRect() { const w = 780, h = w * D.pat.header6.quantities.developed_width / D.pat.header6.quantities.length; return { x: 140, y: 1000 - h - 20, w, h }; }
  dxfPaths() {
    if (this._dp) return this._dp; const paths = [];
    for (const l of D.dxf.lines) paths.push([l.a, l.b]);
    for (const p of D.dxf.poly) paths.push(p.closed ? [...p.pts, p.pts[0]] : p.pts);
    let total = 0; for (const p of paths) for (let j = 1; j < p.length; j++) total += Math.hypot(p[j][0] - p[j - 1][0], p[j][1] - p[j - 1][1]);
    return (this._dp = { paths, total });
  }
  dxfRows(o, V, t) {
    if (t < 26.0) return; const rows = D.dxf.rows, c = o.c, X = 1160, top = 450, lh = 54, vis = 10;
    const pos = (t - 26.0) * 11;                       // rows per second
    const a = (0.6 + 0.4 * seg(t, 26.0, 26.13)) * (1 - seg(t, 28.8, 28.95)); if (a <= 0) return;
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
    c.save(); c.globalAlpha = a;
    const g1 = c.createLinearGradient(0, top - 46, 0, top + 30); g1.addColorStop(0, 'rgba(244,245,247,1)'); g1.addColorStop(1, 'rgba(244,245,247,0)');
    c.fillStyle = g1; c.fillRect(X - 20, top - 46, 700, 76);
    const yb = top + lh * vis - 60, g2 = c.createLinearGradient(0, yb, 0, yb + 70); g2.addColorStop(0, 'rgba(236,237,240,0)'); g2.addColorStop(1, 'rgba(236,237,240,1)');
    c.fillStyle = g2; c.fillRect(X - 20, yb, 700, 70); c.restore();
  }

  // ================================================================== 7 · 30-42 s · AMOEBA
  graphLayout() {
    if (this._gl) return this._gl;
    // the job's 18 records in five columns by level (levels from the edges: longest path to an entered record), as dots
    // in a strip along the bottom of the frame
    const lv = this.lvl, nodes = B.deps.nodes, cols = {};
    for (const n of nodes) (cols[lv[n]] = cols[lv[n]] || []).push(n);
    const pos = {};
    for (const [l, ns] of Object.entries(cols)) { ns.sort(); const L = Number(l), n = ns.length; ns.forEach((id, j) => (pos[id] = { x: 200 + L * 380, y: 892 + (j - (n - 1) / 2) * 42 })); }
    return (this._gl = { pos });
  }
  s7(t) {
    const st = this.st, P = st.parts, j6 = B.job6, corr = j6.corr;
    // the change: header radius 150 -> 158 (35.2 s), the header grows, the branch takes job 6's saddle
    const ch = spring(seg(t, 35.2, 35.75), 0.8);
    const Rh = lerp(corr.old, corr.new, ch);
    this.tee({ h: ch > 0.5 ? 'header6' : 'header5', b: ch > 0.5 ? 'branch6' : 'branch5' });
    const hp = ch > 0.5 ? P.header6 : P.header5, base = ch > 0.5 ? corr.new : corr.old;
    hp.grp.scale.set(1, Rh / base, Rh / base);                         // radial growth about the header axis
    if (ch > 0 && ch < 1) {      // blend the job 5 branch into the job 6 branch while the header grows
      P.branch5.grp.visible = false; P.branch6.grp.visible = true; const g = P.branch6.g, pa = g.attributes.position.array, na = g.attributes.normal.array, q = this.bl;
      for (let i = 0; i < pa.length; i++) { pa[i] = q.p5[i] + (q.p6[i] - q.p5[i]) * ch; na[i] = q.n5[i] + (q.n6[i] - q.n5[i]) * ch; }
      g.attributes.position.needsUpdate = true; g.attributes.normal.needsUpdate = true; P.branch6.kappa = -1;
    }
    // camera: the tee in the upper right (the records strip below, the words on the left); a push-in on the change;
    // back to a hero view for the replay
    const u = seg(t, 30, 42), pushC = spring(seg(t, 35.0, 35.6), 0.9) * (1 - quint(seg(t, 37.6, 38.3)));
    const camA = { tx: 640, ty: 90, tz: 0, az: 22 + 10 * u, el: 18, dist: 3900, fov: 24, sx: 430, sy: -70 };
    const camC = { tx: 560, ty: 90, tz: 0, az: 22 + 10 * u, el: 17, dist: 3750, fov: 24, sx: 400, sy: -40 };
    const camR = { tx: 640, ty: 80, tz: 0, az: 30 + 6 * seg(t, 40.3, 42), el: 20, dist: 2600, fov: 24, sx: 330, sy: 120 };
    const intro = spring(seg(t, 30.0, 30.8), 0.9);
    const camI = { ...camA, dist: 3000, sy: -60 };
    let cm = lerpCam(camI, camA, intro); cm = lerpCam(cm, camC, pushC);
    this.cam(lerpCam(cm, camR, quint(seg(t, 40.3, 40.8))));
    st.setShadow({ opacity: 0.6 });
    // the intersection curve turns amber on the object when it is flagged (36.0 s)
    const am = seg(t, 36.0, 36.15);
    if (am > 0) { const tb = ch > 0.5 ? st.tubes.amber6 : st.tubes.amberSeam; tb.draw(1, am); }
    // replay (40.4-41.3 s): a ring scans the header as the records are checked
    if (t >= 40.4) { const s = st.tubes.scan, k = quint(seg(t, 40.4, 41.3)); s.draw(1, 1 - seg(t, 41.3, 41.5)); s.mesh.position.set(lerp(-20, 1220, k), 0, 0); }
    // anchors on the steel (projected after the camera is set)
    const r = this.req5, b = r.beta_deg * deg, v3 = (x, y, z) => new THREE.Vector3(x, y, z);
    const O = v3(r.branch_station, 0, 0), A = v3(Math.cos(b), Math.sin(b), 0), e1 = v3(-Math.sin(b), Math.cos(b), 0), top = O.clone().addScaledVector(A, r.s_end);
    const an = { 'parameter · header_radius': v3(0, Rh * 0.7, Rh * 0.7), 'parameter · header_dev_radius': v3(150, -Rh * 0.7, Rh * 0.7), 'parameter · header_length': v3(1200, 0, Rh),
      'parameter · branch_station': v3(r.branch_station, -Rh, 40), 'parameter · branch_radius': top.clone().addScaledVector(e1, -r.branch_radius), 'parameter · branch_dev_radius': top.clone().addScaledVector(e1, r.branch_radius),
      'parameter · beta_deg': O.clone().addScaledVector(A, 230), 'parameter · delta': O.clone(), 'parameter · s_end': top, 'convention · surface_reference': v3(900, Rh * 0.7, Rh * 0.7) };
    const anS = {}; for (const [k, v] of Object.entries(an)) anS[k] = this.P(v);
    anS.centre = this.P(v3(600, 0, 0)); anS.rim = this.P(v3(0, Rh * Math.cos(40 * deg), Rh * Math.sin(40 * deg)));
    return (o, V) => this.s7draw(o, V, t, anS);
  }
  s7draw(o, V, t, anS) {
    const S = V.S, j6 = B.job6, corr = j6.corr, ja = V.lang === 'ja', c = o.c;
    this.head(o, V, S.amoeba_big, t, 30.0, 34.95);
    o.block(S.amoeba_def, 120, 310, 62, { size: BS, weight: 500, color: COL.ink2 }, t, 32.0, 34.95);
    this.head(o, V, S.change_big, t, 35.0, 40.35);
    // records strip (30.4-38.3 s)
    if (t < 38.4) this.graph(o, V, t, anS, 1 - seg(t, 38.1, 38.35));
    // the changed value, large, tethered to the header (from 35.0 s): the catalogue's name and old -> new
    if (t >= 35.0 && t < 40.4) {
      const a = (0.6 + 0.4 * seg(t, 35.0, 35.13)) * (1 - seg(t, 40.15, 40.35)), y = 330;
      const nm = B.params[corr.name][V.lang], val = `${mm(corr.old)} → ${mm(corr.new)} ${corr.unit}`;
      const w = Math.max(o.tw(nm, LS, 500), o.tw(val, BS, 700)) + 56;
      c.save(); c.globalAlpha *= a; c.fillStyle = '#eaf2ff'; c.strokeStyle = COL.blue; c.lineWidth = 3; o.rr(120, y - 50, w, 126, 18); c.fill(); c.stroke(); c.restore();
      o.txt(val, 148, y + 8, { size: BS, weight: 700, color: COL.blueT, alpha: a }); o.txt(nm, 148, y + 56, { size: LS, weight: 500, color: COL.ink2, alpha: a });
      const rim = anS.rim; o.line([[120 + w, y + 10], rim], 3, COL.blue, a * seg(t, 35.1, 35.3), [2, 8]); o.dot(rim[0], rim[1], 8, COL.blue, a);
    }
    // what the change reached, one record per beat (36.0-38.0 s): the product's flag order and names
    const fo = B.simple.flagOrder;
    if (t >= 36.0 && t < 38.35) {
      const a0 = 1 - seg(t, 38.1, 38.3);
      fo.forEach((id, k) => { const tk = 36.0 + 0.5 * k; if (t < tk) return; const nd = B.simple.nodes.find((n) => n.id === id), y = 500 + k * 62, uu = spring(seg(t, tk, tk + 0.35));
        o.dot(140, y - 16, 12 * uu, COL.amber, a0); o.txt(ja ? nd.ja : nd.en, 170, y + 8 * (1 - uu), { size: BS, weight: 600, color: COL.amberT, alpha: a0 * seg(t, tk, tk + 0.1) }); });
    }
    // the obligations in plain words (38.1 s): counts from the data, wording from the obligations' own notes
    if (t >= 38.1 && t < 40.4) {
      const a0 = (0.6 + 0.4 * seg(t, 38.1, 38.23)) * (1 - seg(t, 40.15, 40.35));
      const rows = [[B.obligations.length, S.obl_flagged, COL.amberT], [B.oblCounts.discharged, S.obl_cleared, COL.greenT], [B.oblCounts.open, S.obl_open, COL.ink]];
      rows.forEach(([n, s, col], k) => { const y = 500 + k * 66, uu = spring(seg(t, 38.1 + 0.08 * k, 38.5 + 0.08 * k));
        const wN = o.tw(mm(n), BS, 750); o.txt(mm(n), 120, y + 10 * (1 - uu), { size: BS, weight: 750, color: col, alpha: a0 }); o.txt(s, 120 + wN + 20, y + 10 * (1 - uu), { size: BS, weight: 500, color: COL.ink, alpha: a0 }); });
    }
    // the real dependency graph screen (job 6), zoomed into the derived records, 38.4-40.4 s
    if (t >= 38.4 && t < 40.4) this.realGraph(o, V, t);
    // replay
    if (t >= 40.4) {
      const n = Math.min(j6.replay.checked, Math.floor(j6.replay.checked * seg(t, 40.45, 41.2)));
      this.head(o, V, S.replay_big, t, 40.4, 41.95);
      o.block([fmt(S.rec_count, { n: mm(n), f: mm(j6.replay.failing) })], 120, 310, 0, { size: BS, weight: 600, color: COL.ink2 }, t, 40.4, 41.95);
      if (t > 41.2) { const lk = spring(seg(t, 41.2, 41.55)), fa = 1 - seg(t, 41.75, 41.95); o.dot(160, 400, 40 * lk, COL.green, fa); o.tick(160, 402, 38, seg(t, 41.25, 41.5), '#fff', 7, fa); }
    }
    if (t < 38.4 || t >= 40.4) this.label(o, V, S.label_vis, t, t < 38.4 ? 30.0 : 40.4, t < 38.4 ? 38.35 : 41.95);
    else this.label(o, V, S.label_actual, t, 38.4, 40.35);
  }
  graph(o, V, t, anS, gA) {
    const pos = this.graphLayout().pos, c = o.c;
    const flagT = {}; B.simple.flagOrder.forEach((id, k) => (flagT[id] = 36.0 + 0.5 * k));
    const src = B.simple.source;
    // each record leaves the steel and settles in the strip (30.4-31.9 s): parameters from their place on the tee,
    // derived records from the joint
    const t0 = (id, k) => 30.15 + 0.08 * k;
    const ids = B.deps.nodes;
    const at = (id, k) => { const p = pos[id], s0 = anS[id] || anS.centre, uu = spring(seg(t, t0(id, k), t0(id, k) + 0.55), 0.85); return [lerp(s0[0], p.x, uu), lerp(s0[1], p.y, uu), uu]; };
    const P2 = {}; ids.forEach((id, k) => (P2[id] = at(id, k)));
    // links (32.4-33.8 s), from a record to what it depends on
    const eU = seg(t, 32.4, 33.8);
    if (eU > 0) B.deps.edges.forEach((e, k) => {
      const p = P2[e.from], q = P2[e.to]; if (!p || !q) return;
      const uu = clamp(eU * 1.6 - (k / B.deps.edges.length) * 0.6); if (uu <= 0) return;
      const onPath = flagT[e.from] !== undefined && (e.to === src || flagT[e.to] !== undefined), hot = onPath && t >= flagT[e.from] - 0.25;
      const pts = bez([q[0], q[1]], [p[0], p[1]], 24).slice(0, Math.max(2, Math.round(24 * uu)));
      o.line(pts, hot ? 5 : 2.4, hot ? COL.amber : 'rgba(29,29,31,0.30)', gA);
      if (onPath) { const w = seg(t, flagT[e.from] - 0.25, flagT[e.from]); if (w > 0 && w < 1) { const pp = bez([q[0], q[1]], [p[0], p[1]], 24); const k2 = Math.floor(w * 23); o.dot(pp[k2][0], pp[k2][1], 11, COL.amber, gA); } }
    });
    ids.forEach((id, k) => {
      const [x, y, uu] = P2[id]; if (t < t0(id, k)) return;
      const fl = flagT[id] !== undefined && t >= flagT[id], isSrc = id === src && t >= 35.0;
      const col = fl ? COL.amber : isSrc ? COL.blue : '#ffffff', r = isSrc ? 16 : 13;
      o.dot(x, y, r, col, gA); o.ring(x, y, r, 3, fl ? COL.amber : isSrc ? COL.blue : 'rgba(29,29,31,0.6)', gA);
      if (fl) { const rg = Math.exp(-5 * (t - flagT[id])); if (rg > 0.02) o.ring(x, y, 13 + 40 * (1 - rg), 4, COL.amber, gA * rg); }
      if (uu < 1) { const s0 = anS[id] || anS.centre; o.line([[s0[0], s0[1]], [x, y]], 2, 'rgba(10,102,255,0.35)', gA * (1 - uu)); }
    });
  }
  realGraph(o, V, t) {
    const img = this.foot[V.lang].graph, c = o.c, k2 = img.width / 1920;
    // the graph panel of the AMOEBA Control Centre (job 6), zoomed into the derived records (intersection curve, parts,
    // interface, patterns, DXF outputs): application px x 735..1380, y 405..565
    const z = 1.95 + 0.12 * smooth(seg(t, 38.4, 40.4)), sw = 1290 / z, sh = 300 / z, cx = 1052 + 30 * smooth(seg(t, 38.4, 40.4)), cy = 486;
    const X = 530, Y = 680, W = 1290, H = 300;
    const u = spring(seg(t, 38.4, 38.8)), out = seg(t, 40.15, 40.35);
    c.save(); c.globalAlpha = seg(t, 38.4, 38.52) * (1 - out); c.translate(0, 40 * (1 - u));
    c.shadowColor = 'rgba(20,30,50,0.18)'; c.shadowBlur = 40; c.shadowOffsetY = 12; c.fillStyle = '#fff'; o.rr(X, Y, W, H, 16); c.fill(); c.shadowColor = 'transparent';
    o.rr(X, Y, W, H, 16); c.clip(); c.drawImage(img, (cx - sw / 2) * k2, (cy - sh / 2) * k2, sw * k2, sh * k2, X, Y, W, H); c.restore();
  }

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
    this.cam({ tx: 640, ty: 80, tz: 0, az: 44 + 14 * smooth(u), el: 16 + 6 * u, dist: 2850 - 250 * u, fov: 24, sx: -230, sy: 150 });
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
    this.cam(lerpCam({ tx: 640, ty: 80, tz: 0, az: 58, el: 22, dist: 2600, fov: 24, sx: -230, sy: 150 }, { tx: 640, ty: 80, tz: 0, az: 26 + 8 * u, el: 16, dist: 6800, fov: 24, sx: 0, sy: -290 }, k));
    st.setShadow({ opacity: 0.55 });
    return (o, V) => {
      const S = V.S, ja = V.lang === 'ja';
      o.block(S.brand_name, 960, 520, 0, { size: HS, weight: 700, align: 'center', track: 6 }, t, 50.0, 54.5);
      o.block([S.brand_line], 960, 598, 0, { size: BS, weight: 500, align: 'center', color: COL.ink2 }, t, 50.15, 54.5);
      // call to action and both addresses from 50.3 s; the button pulses once on the music's cta (51.6 s)
      if (t >= 50.3) { const uu = spring(seg(t, 50.3, 50.7)), c = o.c, pu = 1 + 0.05 * Math.sin(Math.PI * seg(t, 51.6, 52.0));
        const s = S.cta, w = (o.tw(s, BS, 650) + 110) * pu, h = 100 * pu, x = 960 - w / 2, y = 690 - h / 2 + 12 * (1 - uu), aa = 0.6 + 0.4 * seg(t, 50.3, 50.43);
        c.save(); c.globalAlpha = aa; c.shadowColor = 'rgba(10,102,255,0.35)'; c.shadowBlur = 30; c.shadowOffsetY = 8; c.fillStyle = COL.blue; o.rr(x, y, w, h, h / 2); c.fill(); c.restore();
        o.txt(s, 960, y + h / 2 + 17, { size: BS, weight: 650, color: '#fff', align: 'center', alpha: aa }); }
      o.block([S.contact.join('  ·  ')], 960, 836, 0, { size: BS, weight: 600, align: 'center', color: COL.ink }, t, 50.3, 54.5);
      o.block([S.positioning], 960, 910, 0, { size: LS, weight: 500, align: 'center', color: COL.ink2 }, t, 52.6, 54.5);
      if (V.labels) o.block([S.disclaimer], 960, 1046, 0, { size: LS, weight: 500, align: 'center', color: COL.ink3 }, t, 50.3, 54.5);
      if (ja && S.credit) o.block([S.credit], 1800, 980, 0, { size: LS, weight: 500, align: 'right', color: COL.ink2 }, t, 50.3, 54.5);
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
