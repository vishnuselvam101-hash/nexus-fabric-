// Film 4 geometry: meshes built from the product's own plate outlines (NF4.pat), and the wrap that rolls a flat
// plate onto its part. Pure functions, no randomness: the same input gives the same vertices.
//
// Plate meshes are structured "column" meshes: a column at plate x carries one or two y-intervals; the interval ends
// are the product's curves (the saddle cut, the opening), interpolated linearly between the product's points. So the
// mesh boundary IS the computed outline: the opening in the header plate and the saddle edge of the branch plate.
//
// Wrap (exact for cylinders; a developable bend, lengths on the plate are kept): a plate point at along-axis distance u
// and arc distance v from the anchor line (the plate line that stays put) on a cylinder of radius R, bent to curvature
// k = kappa / R (kappa 0 = flat, 1 = rolled), sits at
//   P = O + u a + R n_a + rho sin(v / rho) t_a - rho (1 - cos(v / rho)) n_a,      rho = R / kappa,
// with outward normal  N = cos(v / rho) n_a + sin(v / rho) t_a.  At kappa = 1 this is O + u a + R (cos(v/R) n_a + sin(v/R) t_a),
// the cylinder; at kappa -> 0 it is the tangent plane at the anchor line (O + u a + R n_a + v t_a).
'use strict';
export const lerp = (a, b, u) => a + (b - a) * u;

// interpolate a polyline y(x) at x (points sorted by x)
function interpY(chain, x) {
  if (x <= chain[0][0]) return chain[0][1];
  for (let i = 1; i < chain.length; i++) if (chain[i][0] >= x) { const [x0, y0] = chain[i - 1], [x1, y1] = chain[i]; return x1 === x0 ? y1 : y0 + (y1 - y0) * (x - x0) / (x1 - x0); }
  return chain[chain.length - 1][1];
}

// columns -> {plate: Float32Array (x, y per vertex), index, nV}; every column has the same number of intervals
export function columnMesh(cols, rows) {
  const nI = cols[0].iv.length, perCol = nI * (rows + 1), nV = cols.length * perCol;
  const plate = new Float32Array(nV * 2); let k = 0;
  for (const c of cols) for (const [a, b] of c.iv) for (let j = 0; j <= rows; j++) { plate[k++] = c.x; plate[k++] = a + (b - a) * (j / rows); }
  const idx = [];
  for (let i = 0; i + 1 < cols.length; i++) for (let s = 0; s < nI; s++) for (let j = 0; j < rows; j++) {
    const p = i * perCol + s * (rows + 1) + j, q = p + perCol;
    idx.push(p, q, p + 1, p + 1, q, q + 1);
  }
  return { plate, index: nV > 65535 ? new Uint32Array(idx) : new Uint16Array(idx), nV };
}

// header plate: rectangle [0, L] x [0, W] with one x-monotone opening (the product's opening polyline)
export function headerColumns(hole, L, W, dx = 12) {
  let iL = 0, iR = 0; hole.forEach((p, i) => { if (p[0] < hole[iL][0]) iL = i; if (p[0] > hole[iR][0]) iR = i; });
  const n = hole.length, walk = (from, to) => { const out = []; for (let i = from; ; i = (i + 1) % n) { out.push(hole[i]); if (i === to) break; } return out; };
  let A = walk(iL, iR), B = walk(iR, iL).reverse();
  const meanY = (c) => c.reduce((s, p) => s + p[1], 0) / c.length;
  if (meanY(A) > meanY(B)) [A, B] = [B, A];                       // A = lower chain, B = upper chain
  for (const c of [A, B]) for (let i = 1; i < c.length; i++) if (c[i][0] < c[i - 1][0] - 1e-9) throw new Error('opening is not x-monotone');
  const xmin = hole[iL][0], xmax = hole[iR][0], yL = hole[iL][1], yR = hole[iR][1];
  const xs = [];
  const nl = Math.max(2, Math.ceil(xmin / dx)); for (let i = 0; i < nl; i++) xs.push({ x: (xmin * i) / nl, side: 'L' });
  const mid = [...new Set([...A, ...B].map((p) => p[0]))].sort((a, b) => a - b);
  for (const x of mid) xs.push({ x, side: 'M' });
  const nr = Math.max(2, Math.ceil((L - xmax) / dx)); for (let i = 1; i <= nr; i++) xs.push({ x: xmax + ((L - xmax) * i) / nr, side: 'R' });
  const cols = [];
  for (const { x, side } of xs) {
    if (side === 'L') cols.push({ x, iv: [[0, yL], [yL, W]] });
    else if (side === 'R') cols.push({ x, iv: [[0, yR], [yR, W]] });
    else { const lo = interpY(A, x), hi = interpY(B, x); cols.push({ x, iv: [[0, lo], [hi, W]] }); }
  }
  // drop columns closer than 0.05 mm (duplicates at the chain ends)
  return cols.filter((c, i) => i === 0 || c.x - cols[i - 1].x > 0.05);
}

// branch plate: x around (0 .. W), from the saddle cut s(x) up to the square end
export function branchColumns(saddle, sEnd, step = 2) {
  const cols = []; for (let i = 0; i < saddle.length; i += step) cols.push({ x: saddle[i][0], iv: [[saddle[i][1], sEnd]] });
  const last = saddle[saddle.length - 1]; if (cols[cols.length - 1].x < last[0]) cols.push({ x: last[0], iv: [[last[1], sEnd]] });
  return cols;
}
// a plain rectangle plate (cylindrical shell): x around 0 .. W, y along 0 .. L
export function rectColumns(W, L, n) { const cols = []; for (let i = 0; i <= n; i++) cols.push({ x: (W * i) / n, iv: [[0, L]] }); return cols; }

// ---- wrap: plate (u along, v arc from anchor) -> position/normal, into out arrays at offset o
export function wrapPoint(F, R, kappa, u, v, P, N, o) {
  const [O, a, na, ta] = [F.O, F.a, F.na, F.ta];
  if (kappa < 1e-5) {
    for (let i = 0; i < 3; i++) { P[o + i] = O[i] + u * a[i] + R * na[i] + v * ta[i]; N[o + i] = na[i]; }
    return;
  }
  const rho = R / kappa, th = v / rho, c = Math.cos(th), s = Math.sin(th);
  for (let i = 0; i < 3; i++) {
    P[o + i] = O[i] + u * a[i] + R * na[i] + rho * s * ta[i] - rho * (1 - c) * na[i];
    N[o + i] = c * na[i] + s * ta[i];
  }
}

// ---- cone sector (right cone, exact): plate polar (rho, psi) about the apex. With q' = sin(half apex angle) running from
// 1 (flat) to q (the cone): azimuth phi = (psi - psi_a) / q', distance from the axis r = rho q', drop along the axis
// rho sqrt(1 - q'^2). Lengths on the plate are kept for every q' (each state is a cone developed from the same sector).
export function conePoint(rho, psi, psiA, qq, P, N, o) {
  const phi = (psi - psiA) / qq, r = rho * qq, h = rho * Math.sqrt(Math.max(0, 1 - qq * qq)), ca = Math.sqrt(Math.max(0, 1 - qq * qq));
  // local frame: axis +Y (apex up), anchor direction +Z, tangent +X
  P[o] = r * Math.sin(phi); P[o + 1] = -h; P[o + 2] = r * Math.cos(phi);
  N[o] = ca * Math.sin(phi); N[o + 1] = qq; N[o + 2] = ca * Math.cos(phi);
}

// ---- eccentric reducer (oblique cone, apex A). psi(t) along the large circle (formula in tools/build_film4_data.mjs).
export function eccTable(req, N = 4000) {
  const k = req.r1 / (req.r1 - req.r2), A = [k * req.e, 0, k * req.h];
  const ts = new Float64Array(N + 1), ps = new Float64Array(N + 1); let psi = 0;
  for (let i = 0; i <= N; i++) { const t = (i / N) * 2 * Math.PI; ts[i] = t; ps[i] = psi;
    const p = [req.r1 * Math.cos(t), req.r1 * Math.sin(t), 0], d = [p[0] - A[0], p[1] - A[1], p[2] - A[2]], dp = [-req.r1 * Math.sin(t), req.r1 * Math.cos(t), 0];
    const cx = [dp[1] * d[2] - dp[2] * d[1], dp[2] * d[0] - dp[0] * d[2], dp[0] * d[1] - dp[1] * d[0]];
    psi += (Math.hypot(cx[0], cx[1], cx[2]) / (d[0] * d[0] + d[1] * d[1] + d[2] * d[2])) * (2 * Math.PI / N); }
  const tOf = (ph) => { let lo = 0, hi = N; while (hi - lo > 1) { const m = (lo + hi) >> 1; if (ps[m] < ph) lo = m; else hi = m; } const u = ps[hi] > ps[lo] ? (ph - ps[lo]) / (ps[hi] - ps[lo]) : 0; return ts[lo] + (ts[hi] - ts[lo]) * Math.max(0, Math.min(1, u)); };
  return { A, total: ps[N], tOf };
}

// smoothing of a closed or open 3D polyline into a tube is done by three.js (CatmullRomCurve3 / TubeGeometry)
