// Builds data/film4-data.js (window.NF4) for film 4 from the product's own output, read from flagship3 (read only):
//   ../flagship3/data/film-data.js    window.NF_DATA (catalogue labels, job 6 checks, gates, graph, obligations, counts)
//   ../flagship3/data/en/job-N.json   the product's full patterns (every computed point) for jobs 1..6
//   ../flagship3/data/en/dxf-6-header.dxf   the DXF file the product wrote for job 6's header plate
// Nothing is typed by hand. Where film 4 needs a value the files do not state directly (how a plate wraps onto its
// part in 3D), it is derived here from the data with the formula written next to it, and checked against the data
// (the residuals are printed and stored in NF4.fit).
// usage: node tools/build_film4_data.mjs            (run from flagship4/)
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
const HERE = path.dirname(new URL(import.meta.url).pathname), F = path.resolve(HERE, '..'), F3 = path.resolve(F, '../flagship3');
const log = (...a) => console.log(...a);
const ctx = { window: {} }; vm.runInNewContext(fs.readFileSync(path.join(F3, 'data/film-data.js'), 'utf8'), ctx);
const D = ctx.window.NF_DATA;
const job = (i) => JSON.parse(fs.readFileSync(path.join(F3, `data/en/job-${i}.json`), 'utf8'));
const J = {}; for (let i = 1; i <= 6; i++) J[i] = job(i);

// ---- patterns: every curve and hole as the product computed them (points in mm, plate coordinates)
const pat = (i, part) => {
  const p = J[i].patterns.find((q) => q.part === part); if (!p) throw new Error(`job ${i} has no ${part} pattern`);
  const curves = {}; for (const c of p.curves) curves[c.name] = c.points;
  return { job: i, family: p.family, class: p.class, part, curves, holes: p.holes.map((h) => h.points), quantities: p.quantities, request: J[i].request };
};
const P = { shell: pat(1, 'shell'), cone: pat(2, 'reducer'), ecc: pat(3, 'reducer'), branch4: pat(4, 'branch'),
  header5: pat(5, 'header'), branch5: pat(5, 'branch'), header6: pat(6, 'header'), branch6: pat(6, 'branch') };

// ---- fits: how each plate wraps onto its part (derived, then checked against the data)
const R5 = J[5].request, R6 = J[6].request, deg = Math.PI / 180;
const fit = {};
// Header with branch, world frame (mm): header axis = +X from x = 0 to header_length; the branch axis leaves the header
// axis at (branch_station, 0, delta) in the XY plane at beta to +X: a = (cos b, sin b, 0). On the header plate x runs along
// the axis and y around it, so a plate point (x, y) lies at (x, Rc cos phi, Rc sin phi) with phi = phi0 + s*y/R_dev
// (R_dev = header_dev_radius, Rc = header_radius). phi0 and s (= +1 or -1) are found by asking that the plate's opening
// lands on the branch cylinder: distance of each 3D opening point from the branch axis = branch_radius.
const distToLine = (p, o, a) => { const d = [p[0] - o[0], p[1] - o[1], p[2] - o[2]]; const t = d[0] * a[0] + d[1] * a[1] + d[2] * a[2]; return Math.hypot(d[0] - t * a[0], d[1] - t * a[1], d[2] - t * a[2]); };
function fitHeader(H, Rq) {
  const Rc = Rq.header_radius, Rd = Rq.header_dev_radius, b = Rq.beta_deg * deg, O = [Rq.branch_station, 0, Rq.delta], a = [Math.cos(b), Math.sin(b), 0];
  let best = null;
  for (const s of [1, -1]) for (let k = 0; k < 3600; k++) {
    const phi0 = (k / 3600) * 2 * Math.PI; let worst = 0;
    for (const [x, y] of H.holes[0]) { const ph = phi0 + s * y / Rd; worst = Math.max(worst, Math.abs(distToLine([x, Rc * Math.cos(ph), Rc * Math.sin(ph)], O, a) - Rq.branch_radius)); if (best && worst > best.res) break; }
    if (!best || worst < best.res) best = { s, phi0, res: worst };
  }
  // refine phi0 by golden-section search around the coarse optimum
  const f = (phi0) => Math.max(...H.holes[0].map(([x, y]) => { const ph = phi0 + best.s * y / Rd; return Math.abs(distToLine([x, Rc * Math.cos(ph), Rc * Math.sin(ph)], O, a) - Rq.branch_radius); }));
  let lo = best.phi0 - 0.003, hi = best.phi0 + 0.003; for (let i = 0; i < 60; i++) { const m1 = lo + (hi - lo) * 0.382, m2 = lo + (hi - lo) * 0.618; if (f(m1) < f(m2)) hi = m2; else lo = m1; }
  best.phi0 = (lo + hi) / 2; best.res = f(best.phi0);
  return best;
}
// Branch plate: x runs around the branch (0 .. 2 pi branch_dev_radius), y along the branch axis from the axis meeting point O.
// 3D point = O + y a + r (cos th e1 + sin th e2), th = th0 + s*x/r_dev, e1 = (-sin b, cos b, 0), e2 = (0, 0, 1).
// th0 and s are found by asking that the saddle cut lands on the header cylinder: sqrt(Y^2 + Z^2) = header_radius.
function fitBranch(B, Rq, Rhead) {
  const r = Rq.branch_radius, rd = Rq.branch_dev_radius, b = Rq.beta_deg * deg, O = [Rq.branch_station ?? 0, 0, Rq.delta];
  const a = [Math.cos(b), Math.sin(b), 0], e1 = [-Math.sin(b), Math.cos(b), 0], e2 = [0, 0, 1];
  const pt = (x, y, th0, s) => { const th = th0 + s * x / rd, c = Math.cos(th), n = Math.sin(th); return [0, 1, 2].map((i) => O[i] + y * a[i] + r * (c * e1[i] + n * e2[i])); };
  const f = (th0, s) => Math.max(...B.curves.saddle_cut.map(([x, y]) => { const p = pt(x, y, th0, s); return Math.abs(Math.hypot(p[1], p[2]) - Rhead); }));
  let best = null;
  for (const s of [1, -1]) for (let k = 0; k < 1440; k++) { const th0 = (k / 1440) * 2 * Math.PI; const v = f(th0, s); if (!best || v < best.res) best = { s, th0, res: v }; }
  let lo = best.th0 - 0.005, hi = best.th0 + 0.005; for (let i = 0; i < 60; i++) { const m1 = lo + (hi - lo) * 0.382, m2 = lo + (hi - lo) * 0.618; if (f(m1, best.s) < f(m2, best.s)) hi = m2; else lo = m1; }
  best.th0 = (lo + hi) / 2; best.res = f(best.th0, best.s);
  // the mirror solution (s -> -s, th0 -> pi - th0 ... ) exists because delta = 0 makes the joint symmetric about the XY plane
  return best;
}
fit.header5 = fitHeader(P.header5, R5);
fit.header6 = fitHeader(P.header6, R6);
fit.branch5 = fitBranch(P.branch5, R5, R5.header_radius);
fit.branch6 = fitBranch(P.branch6, R6, R6.header_radius);
fit.branch4 = fitBranch(P.branch4, { ...J[4].request, branch_station: 600 }, J[4].request.header_radius);
for (const k of Object.keys(fit)) log(`fit ${k}: s=${fit[k].s} angle0=${((fit[k].phi0 ?? fit[k].th0) / deg).toFixed(4)} deg, worst residual ${fit[k].res.toExponential(2)} mm`);
for (const k of Object.keys(fit)) if (fit[k].res > 0.05) throw new Error(`fit ${k} does not land on its partner surface`);
// Cone (job 2) sector: polar coordinates about the apex at the plate origin. Checks: slant radii and sector angle.
{ const pts = [...P.cone.curves.large_end, ...P.cone.curves.small_end]; const rho = pts.map(([x, y]) => Math.hypot(x, y)); const psi = pts.map(([x, y]) => Math.atan2(y, x));
  fit.cone = { rhoL: Math.max(...rho), rhoS: Math.min(...rho), psi0: Math.min(...psi), psi1: Math.max(...psi) };
  fit.cone.q = J[2].request.r1 / fit.cone.rhoL;   // sin(half apex angle) = r1 / slant radius of the large end
  log(`cone: rhoL ${fit.cone.rhoL.toFixed(4)} (data ${P.cone.quantities.apex_radius_large}), rhoS ${fit.cone.rhoS.toFixed(4)} (data ${P.cone.quantities.apex_radius_small}), sector ${(fit.cone.psi1 - fit.cone.psi0).toFixed(4)} rad (data ${P.cone.quantities.sector_angle}), q ${fit.cone.q.toFixed(4)} (data ${P.cone.quantities.sin_half_angle})`); }
// Eccentric reducer (job 3): two parallel circles r1 (z = 0, centre 0) and r2 (z = h, centre e on +X). Lines joining equal
// angles meet at the homothety centre A = (r1 c2 - r2 c1) / (r1 - r2) = (r1 e / (r1 - r2), 0, r1 h / (r1 - r2)): an oblique cone.
// Its development is psi(t) = integral of |P'(t) x (P(t) - A)| / |P(t) - A|^2 dt along the large circle P(t) = (r1 cos t, r1 sin t, 0),
// with slant radius rho(t) = |P(t) - A|. Checked against the product's large-end polyline: rho and psi of every point.
{ const q = J[3].request, k = q.r1 / (q.r1 - q.r2), A = [k * q.e, 0, k * q.h];
  const Pt = (t) => [q.r1 * Math.cos(t), q.r1 * Math.sin(t), 0];
  const N = 20000, ts = [], ps = []; let psi = 0;
  for (let i = 0; i <= N; i++) { const t = (i / N) * 2 * Math.PI; ts.push(t); ps.push(psi);
    const p = Pt(t), d = [p[0] - A[0], p[1] - A[1], p[2] - A[2]], dp = [-q.r1 * Math.sin(t), q.r1 * Math.cos(t), 0];
    const cx = [dp[1] * d[2] - dp[2] * d[1], dp[2] * d[0] - dp[0] * d[2], dp[0] * d[1] - dp[1] * d[0]];
    psi += (Math.hypot(...cx) / (d[0] ** 2 + d[1] ** 2 + d[2] ** 2)) * (2 * Math.PI / N); }
  // compare with the product's large-end polyline (polar about the plate origin)
  let worst = 0; const L = P.ecc.curves.large_end;
  for (const [x, y] of L) { const ph = Math.atan2(y, x), rr = Math.hypot(x, y); let j = ps.findIndex((v) => v >= ph); if (j < 0) j = N;
    const p = Pt(ts[j]); worst = Math.max(worst, Math.abs(Math.hypot(p[0] - A[0], p[1] - A[1], p[2] - A[2]) - rr)); }
  fit.ecc = { apex: A, ratio: q.r2 / q.r1, total: ps[N], res: worst };
  log(`ecc: apex ${A.map((v) => v.toFixed(1))}, developed angle ${ps[N].toFixed(4)} (data ${P.ecc.quantities.developed_angle}), worst slant radius residual on the large end ${worst.toFixed(3)} mm (sampling)`); }

// ---- DXF: the file as written (job 6, header plate). Entities parsed; the first rows kept as text for the film.
const dxfTxt = fs.readFileSync(path.join(F3, 'data/en/dxf-6-header.dxf'), 'utf8').split(/\r?\n/).map((l) => l.trim());
const e0 = dxfTxt.indexOf('ENTITIES'); const pairs = []; for (let i = e0 + 1; i + 1 < dxfTxt.length; i += 2) pairs.push([dxfTxt[i], dxfTxt[i + 1]]);
const ents = []; let cur = null;
for (const [c, v] of pairs) { if (c === '0') { cur = { type: v, layer: '', v: {} , pts: [] }; ents.push(cur); continue; } if (!cur) continue;
  if (c === '8') cur.layer = v; else if (c === '1') cur.text = v; else cur.v[c] = Number(v); }
// the format version the file declares in its HEADER ($ACADVER, group 1): AC1009 is the AutoCAD R11/R12 DXF format
const av = dxfTxt.indexOf('$ACADVER'), acadver = av >= 0 ? dxfTxt[av + 2] : null;
if (acadver !== 'AC1009') throw new Error('unexpected DXF version ' + acadver);
const dxf = { name: D.dxf.name, marking: D.dxf.marking, acadver, lines: [], poly: [], texts: [], rows: [] };
let poly = null;
for (const e of ents) {
  if (e.type === 'LINE') dxf.lines.push({ layer: e.layer, a: [e.v['10'], e.v['20']], b: [e.v['11'], e.v['21']] });
  else if (e.type === 'POLYLINE') { poly = { layer: e.layer, closed: (e.v['70'] & 1) === 1, pts: [] }; dxf.poly.push(poly); }
  else if (e.type === 'VERTEX' && poly) poly.pts.push([e.v['10'], e.v['20']]);
  else if (e.type === 'SEQEND') poly = null;
  else if (e.type === 'TEXT') dxf.texts.push(e.text);
}
// rows as group-code / value pairs, from ENTITIES on (for the stream of text in the DXF scene)
for (let i = 0; i < 240 && i < pairs.length; i++) dxf.rows.push(pairs[i]);
// compare the file with the pattern (job 6 header): every DXF vertex against the product's opening points
{ const H = P.header6.holes[0]; let worst = 0; const ps = dxf.poly[0].pts; const n = Math.min(H.length, ps.length);
  for (let i = 0; i < n; i++) worst = Math.max(worst, Math.hypot(H[i][0] - ps[i][0], H[i][1] - ps[i][1]));
  dxf.compare = { vertices: ps.length, patternPoints: H.length, worst };
  log(`dxf: ${dxf.lines.length} LINE, ${dxf.poly.length} POLYLINE with ${ps.length} vertices, ${dxf.texts.length} TEXT; opening vertices vs job 6 pattern: worst ${worst.toExponential(2)} mm`); }

// ---- the rest comes straight from window.NF_DATA
const base = {};
for (const k of ['ui', 'params', 'quants', 'checkJa', 'job6', 'deps', 'simple', 'gates6', 'gateCounts6', 'oblCounts', 'obligations', 'checkNoJa', 'rkinds', 'rels']) base[k] = D[k];
base.job6 = { ...D.job6, records: undefined, recordCount: D.job6.records.length };
base.job5 = { id: D.job5.id, state: D.job5.state, request: D.job5.request };
// ---- added in the fix round: the catalogue's family names, job 6's re-import report (header) and the obligations'
// notes as the product wrote them (for the plain-language counts)
globalThis.document = globalThis.document || { documentElement: { lang: '' } };
globalThis.localStorage = globalThis.localStorage || { getItem: () => null, setItem: () => {} };
const I18N = await import(path.join(F3, 'tools/i18n.mjs'));
const fam = {};
for (const k of ['cylinder_shell', 'cone_frustum', 'eccentric_reducer', 'branch_stub', 'header_shell_with_opening']) {
  fam[k] = {}; for (const L of ['en', 'ja']) { I18N.setLang(L); fam[k][L] = { short: I18N.idLabel('family', k), long: I18N.idLabel('familyLong', k) }; }
}
I18N.setLang('en'); const quantLab = {}; for (const q of ['developed_width', 'sector_angle', 'slant_length', 'developed_angle', 's_max', 'opening_perimeter', 'length']) { quantLab[q] = {}; for (const L of ['en', 'ja']) { I18N.setLang(L); quantLab[q][L] = I18N.idLabel('quantity', q); } }
const sref = {}; for (const L of ['en', 'ja']) { I18N.setLang(L); sref[L] = { label: I18N.t('g.topic.sref'), value: I18N.t('sref.' + J[5].request.surface_reference) }; }
const reimport = J[6].records.filter((r) => r.kind === 'reimport_report' && r.payload.part === 'header').map((r) => r.payload)[0];
if (!reimport) throw new Error('job 6 has no re-import report for the header');
const oblFull = JSON.parse(fs.readFileSync(path.join(F3, 'data/en/obligations.json'), 'utf8')).map((o) => ({ id: o.id, state: o.state, kind: o.kind, label: o.label, note: o.note }));
log('re-import (header):', JSON.stringify(reimport));
log('families:', JSON.stringify(fam));
const out = { built: 'tools/build_film4_data.mjs from ../flagship3/data (film-data.js, en/job-1..6.json, en/dxf-6-header.dxf, en/obligations.json) and ../flagship3/tools/i18n.mjs', base, pat: P, fit, dxf, fam, quantLab, reimport, oblFull, sref };
fs.mkdirSync(path.join(F, 'data'), { recursive: true });
const js = `// generated by tools/build_film4_data.mjs: do not edit\nwindow.NF4 = ${JSON.stringify(out)};\n`;
fs.writeFileSync(path.join(F, 'data/film4-data.js'), js);
log('wrote data/film4-data.js', js.length, 'bytes');
