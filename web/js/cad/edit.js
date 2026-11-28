// NF CAD edit helpers (pure, no DOM): snap candidates on the loaded entities, the bounding centre of a geometry,
// and the operation objects the server's edit route takes. Nothing here changes the drawing: the page sends the
// ops, and the server writes a new version.

// Snap order when two candidates are equally near: end, centre, mid, intersection.
const RANK = { end: 0, center: 1, mid: 2, inter: 3 };

function segIntersect(a, b, c, d) {
  const r = [b[0] - a[0], b[1] - a[1]];
  const s = [d[0] - c[0], d[1] - c[1]];
  const den = r[0] * s[1] - r[1] * s[0];
  if (Math.abs(den) < 1e-12) return null;
  const t = ((c[0] - a[0]) * s[1] - (c[1] - a[1]) * s[0]) / den;
  const u = ((c[0] - a[0]) * r[1] - (c[1] - a[1]) * r[0]) / den;
  if (t < 0 || t > 1 || u < 0 || u > 1) return null;
  return [a[0] + t * r[0], a[1] + t * r[1]];
}

function segments(g) {
  if (g.type === 'line') return [[g.a, g.b]];
  if (g.type === 'poly') {
    const out = [];
    const n = g.v.length;
    const segs = g.closed ? n : n - 1;
    for (let i = 0; i < segs; i++) out.push([[g.v[i][0], g.v[i][1]], [g.v[(i + 1) % n][0], g.v[(i + 1) % n][1]]]);
    return out;
  }
  return [];
}

// Candidates within `radius` (world units) of `p`, from the visible entities.
export function snapCandidates(entities, visible, p, radius) {
  const out = [];
  const add = (kind, pt) => {
    const d = Math.hypot(pt[0] - p[0], pt[1] - p[1]);
    if (d <= radius) out.push({ kind, point: pt, d, rank: RANK[kind] });
  };
  const lines = [];
  for (const e of entities) {
    if (visible && !visible.has(e.layer)) continue;
    const g = e.geom;
    if (g.type === 'line') { add('end', g.a); add('end', g.b); add('mid', [(g.a[0] + g.b[0]) / 2, (g.a[1] + g.b[1]) / 2]); }
    if (g.type === 'circle') add('center', g.c);
    if (g.type === 'arc') {
      add('center', g.c);
      const r = (deg) => [g.c[0] + g.r * Math.cos(deg * Math.PI / 180), g.c[1] + g.r * Math.sin(deg * Math.PI / 180)];
      add('end', r(g.start)); add('end', r(g.end));
    }
    if (g.type === 'poly') {
      const n = g.v.length;
      for (let i = 0; i < n; i++) add('end', [g.v[i][0], g.v[i][1]]);
      for (const [a, b] of segments(g)) add('mid', [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2]);
    }
    for (const s of segments(g)) lines.push(s);
  }
  // intersections of segments near the point only (bounded work)
  const near = lines.filter(([a, b]) =>
    Math.max(a[0], b[0]) >= p[0] - radius && Math.min(a[0], b[0]) <= p[0] + radius &&
    Math.max(a[1], b[1]) >= p[1] - radius && Math.min(a[1], b[1]) <= p[1] + radius
  ).slice(0, 400);
  for (let i = 0; i < near.length; i++) {
    for (let j = i + 1; j < near.length; j++) {
      const x = segIntersect(near[i][0], near[i][1], near[j][0], near[j][1]);
      if (x) add('inter', x);
    }
  }
  if (!out.length) return null;
  out.sort((x, y) => (x.rank - y.rank) || (x.d - y.d));
  // the best rank within the radius wins; among its candidates, the nearest
  const best = out[0];
  return { kind: best.kind, point: best.point };
}

// Centre of the bounding box of a geometry: the default pivot for rotate, mirror and scale.
export function pivotOf(g) {
  let b = null;
  const add = (p) => { b = b ? [Math.min(b[0], p[0]), Math.min(b[1], p[1]), Math.max(b[2], p[0]), Math.max(b[3], p[1])] : [p[0], p[1], p[0], p[1]]; };
  if (g.type === 'point' || g.type === 'text') add(g.at);
  if (g.type === 'line') { add(g.a); add(g.b); }
  if (g.type === 'circle') { add([g.c[0] - g.r, g.c[1] - g.r]); add([g.c[0] + g.r, g.c[1] + g.r]); }
  if (g.type === 'arc') { add(g.c); }
  if (g.type === 'poly') for (const v of g.v) add([v[0], v[1]]);
  if (!b) return null;
  return [(b[0] + b[2]) / 2, (b[1] + b[3]) / 2];
}

// Operation objects for the server (`op` names match the nf-cad edit ops exactly).
export const ops = {
  move: (ids, dx, dy) => ({ op: 'move', ids, dx, dy }),
  rotate: (ids, c, deg) => ({ op: 'rotate', ids, cx: c[0], cy: c[1], deg }),
  mirror: (ids, a, b) => ({ op: 'mirror', ids, a, b }),
  scale: (ids, c, factor) => ({ op: 'scale', ids, cx: c[0], cy: c[1], factor }),
  delete: (ids) => ({ op: 'delete', ids }),
  setLayer: (ids, layer) => ({ op: 'set_layer', ids, layer }),
  addLine: (a, b, layer) => ({ op: 'add', entity: { type: 'line', a, b, layer } }),
};
