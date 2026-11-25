//! Triangulation of a plate outline with holes (A-21): ear clipping, with each hole joined to the outline by a bridge
//! (the bridge method). Pure arithmetic on plate coordinates in mm; no I/O.
//!
//! The outline is a simple closed polygon, the holes are simple closed polygons inside it and do not touch each other
//! or the outline. The triangles cover the outline minus the holes exactly (up to rounding), so their area sum is the
//! net area of the plate: `net_area`.

use std::collections::{HashMap, HashSet};

use uem::{Geom, P2, Pattern};

use crate::{Form3d, Form3dError};

/// A triangulated plate. `vertices` are the outline vertices followed by the vertices of each hole, in the order the
/// holes were given, then any midpoints added by `triangulate_formed`. Triangles are counter-clockwise index triples
/// into `vertices`.
#[derive(Clone, Debug, PartialEq)]
pub struct Triangulation {
    pub vertices: Vec<P2>,
    pub triangles: Vec<[usize; 3]>,
}

/// Cross product (b - o) x (c - o): positive when o, b, c turn counter-clockwise.
fn cross(o: P2, b: P2, c: P2) -> f64 {
    (b.x - o.x) * (c.y - o.y) - (b.y - o.y) * (c.x - o.x)
}

/// Signed area of a closed polygon (positive counter-clockwise), relative to its first vertex.
pub fn signed_area(poly: &[P2]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let o = poly[0];
    0.5 * (0..n)
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            (a.x - o.x) * (b.y - o.y) - (b.x - o.x) * (a.y - o.y)
        })
        .sum::<f64>()
}

/// Net area (mm²) of an outline with holes: the outline's area minus the holes' areas.
pub fn net_area(outer: &[P2], holes: &[Vec<P2>]) -> f64 {
    signed_area(outer).abs() - holes.iter().map(|h| signed_area(h).abs()).sum::<f64>()
}

/// Even-odd point-in-polygon test (boundary points may go either way).
fn inside_polygon(poly: &[P2], p: P2) -> bool {
    let n = poly.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) * (b.x - a.x) / (b.y - a.y) {
            inside = !inside;
        }
    }
    inside
}

/// True when the open segments p-q and a-b cross at a single point strictly inside both.
fn properly_cross(p: P2, q: P2, a: P2, b: P2) -> bool {
    let (d1, d2) = (cross(p, q, a), cross(p, q, b));
    let (d3, d4) = (cross(a, b, p), cross(a, b, q));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

fn same(a: P2, b: P2) -> bool {
    a.x == b.x && a.y == b.y
}

/// The bridge p-q is usable when it crosses no obstacle edge, touches no obstacle vertex in its interior, and its
/// midpoint lies inside the outline and outside every hole.
fn bridge_ok(p: P2, q: P2, obstacles: &[Vec<P2>], outer: &[P2], holes: &[Vec<P2>]) -> bool {
    for poly in obstacles {
        let n = poly.len();
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            if properly_cross(p, q, a, b) {
                return false;
            }
            // an obstacle vertex lying on the bridge (other than its end points) also blocks it
            if !same(a, p) && !same(a, q) && cross(p, q, a).abs() == 0.0 {
                let within = a.x >= p.x.min(q.x)
                    && a.x <= p.x.max(q.x)
                    && a.y >= p.y.min(q.y)
                    && a.y <= p.y.max(q.y);
                if within {
                    return false;
                }
            }
        }
    }
    let mid = P2::new(0.5 * (p.x + q.x), 0.5 * (p.y + q.y));
    inside_polygon(outer, mid) && !holes.iter().any(|h| inside_polygon(h, mid))
}

/// True when direction `d` from `v` lies strictly inside the interior wedge of a counter-clockwise ring at `v`, where
/// `a` is the previous and `b` the next ring vertex. The interior is swept counter-clockwise from `b - v` to `a - v`.
fn in_wedge(a: P2, v: P2, b: P2, d: P2) -> bool {
    let (u1, u2) = (P2::new(b.x - v.x, b.y - v.y), P2::new(a.x - v.x, a.y - v.y));
    let w = P2::new(d.x - v.x, d.y - v.y);
    let cr = |p: P2, q: P2| p.x * q.y - p.y * q.x;
    if cross(a, v, b) >= 0.0 {
        // convex or straight corner: the wedge is narrower than a half turn
        cr(u1, w) > 0.0 && cr(w, u2) > 0.0
    } else {
        // reflex corner: strictly inside unless inside the convex exterior wedge from u2 to u1
        !(cr(u2, w) >= 0.0 && cr(w, u1) >= 0.0)
    }
}

/// Triangulates an outline with holes. The outline and the holes must be simple closed polygons (first vertex not
/// repeated at the end); the outline is made counter-clockwise and the holes clockwise.
pub fn triangulate(outer: &[P2], holes: &[Vec<P2>]) -> Result<Triangulation, Form3dError> {
    let clean = |poly: &[P2]| -> Vec<P2> {
        let mut out: Vec<P2> = Vec::with_capacity(poly.len());
        for &p in poly {
            if out.last().is_none_or(|&l| !same(l, p)) {
                out.push(p);
            }
        }
        while out.len() > 1 && same(out[0], out[out.len() - 1]) {
            out.pop();
        }
        out
    };
    let mut outer = clean(outer);
    if outer.len() < 3 || outer.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err(Form3dError::NotTriangulable(
            "outline needs at least three finite vertices",
        ));
    }
    if signed_area(&outer) == 0.0 {
        return Err(Form3dError::NotTriangulable("outline has zero area"));
    }
    if signed_area(&outer) < 0.0 {
        outer.reverse();
    }
    let mut holes: Vec<Vec<P2>> = holes.iter().map(|h| clean(h.as_slice())).collect();
    for h in holes.iter_mut() {
        if h.len() < 3 || h.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
            return Err(Form3dError::NotTriangulable(
                "hole needs at least three finite vertices",
            ));
        }
        if signed_area(h) == 0.0 {
            return Err(Form3dError::NotTriangulable("hole has zero area"));
        }
        if signed_area(h) > 0.0 {
            h.reverse();
        }
    }

    // vertices: outline first, then holes
    let mut vertices: Vec<P2> = outer.clone();
    let mut hole_idx: Vec<Vec<usize>> = Vec::new();
    for h in &holes {
        let start = vertices.len();
        vertices.extend(h.iter().copied());
        hole_idx.push((start..vertices.len()).collect());
    }
    let mut ring: Vec<usize> = (0..outer.len()).collect();

    // bridge the holes, the one with the largest x first
    let mut order: Vec<usize> = (0..holes.len()).collect();
    order.sort_by(|&a, &b| {
        let mx = |h: &Vec<P2>| h.iter().map(|p| p.x).fold(f64::MIN, f64::max);
        mx(&holes[b]).total_cmp(&mx(&holes[a]))
    });
    for (done, &hi) in order.iter().enumerate() {
        let hv = &hole_idx[hi];
        // the hole vertex with the largest x (ties: the lowest y)
        let m_pos = (0..hv.len())
            .max_by(|&a, &b| {
                let (pa, pb) = (vertices[hv[a]], vertices[hv[b]]);
                pa.x.total_cmp(&pb.x).then(pb.y.total_cmp(&pa.y))
            })
            .unwrap_or(0);
        let m = vertices[hv[m_pos]];
        // the hole's neighbours of m (the hole is clockwise): the bridge must leave m outside the hole's interior
        let hn = hv.len();
        let (m_prev, m_next) = (
            vertices[hv[(m_pos + hn - 1) % hn]],
            vertices[hv[(m_pos + 1) % hn]],
        );
        // obstacles: the ring and the holes not yet merged
        let mut obstacles: Vec<Vec<P2>> = vec![ring.iter().map(|&i| vertices[i]).collect()];
        for &oi in order.iter().skip(done + 1) {
            obstacles.push(holes[oi].clone());
        }
        obstacles.push(holes[hi].clone());
        let mut cands: Vec<usize> = (0..ring.len()).collect();
        cands.sort_by(|&a, &b| {
            let da = m.dist(vertices[ring[a]]);
            let db = m.dist(vertices[ring[b]]);
            da.total_cmp(&db)
        });
        let rn = ring.len();
        let mut pos = None;
        for c in cands {
            // a repeated ring vertex has one copy per wedge: the bridge must enter the copy whose wedge holds it
            let v = vertices[ring[c]];
            let (r_prev, r_next) = (
                vertices[ring[(c + rn - 1) % rn]],
                vertices[ring[(c + 1) % rn]],
            );
            if !in_wedge(r_prev, v, r_next, m) || in_wedge(m_next, m, m_prev, v) {
                continue;
            }
            if bridge_ok(m, v, &obstacles, &outer, &holes) {
                pos = Some(c);
                break;
            }
        }
        let pos = pos.ok_or(Form3dError::NotTriangulable(
            "no bridge from the hole to the outline",
        ))?;
        // the hole cycle starts at m, runs once round, and returns to m; then the outline vertex is repeated
        let mut merged: Vec<usize> = ring[..=pos].to_vec();
        for k in 0..=hv.len() {
            merged.push(hv[(m_pos + k) % hv.len()]);
        }
        merged.push(ring[pos]);
        merged.extend_from_slice(&ring[pos + 1..]);
        ring = merged;
    }

    let net = net_area(&outer, &holes);
    let triangles = ear_clip(&vertices, ring, net)?;
    Ok(Triangulation {
        vertices,
        triangles,
    })
}

/// Ear clipping of a weakly simple counter-clockwise ring (repeated indices are allowed for bridge vertices). `net` is
/// the net area of the plate; the ring must enclose it, so that the corners dropped below carried no area.
fn ear_clip(
    vertices: &[P2],
    mut ring: Vec<usize>,
    net: f64,
) -> Result<Vec<[usize; 3]>, Form3dError> {
    let p = |i: usize| vertices[i];
    let mut tris = Vec::new();
    // zero-area corners (collinear or repeated points) carry no area: drop them first
    let mut changed = true;
    while changed && ring.len() > 3 {
        changed = false;
        let n = ring.len();
        for i in 0..n {
            let (a, b, c) = (p(ring[(i + n - 1) % n]), p(ring[i]), p(ring[(i + 1) % n]));
            let scale = a.dist(b) * b.dist(c);
            if cross(a, b, c).abs() <= 1e-12 * scale || scale == 0.0 {
                ring.remove(i);
                changed = true;
                break;
            }
        }
    }
    // a corner that is only nearly straight can carry real area (a sharp sliver): refuse if the drop changed it
    let kept: Vec<P2> = ring.iter().map(|&i| p(i)).collect();
    if net <= 0.0 || (signed_area(&kept) - net).abs() > 1e-9 * net {
        return Err(Form3dError::NotTriangulable(
            "dropping a near-collinear corner changes the plate area",
        ));
    }
    let mut i = 0usize;
    let mut stalled = 0usize;
    while ring.len() > 3 {
        let n = ring.len();
        let (pi, ci, ni) = ((i + n - 1) % n, i % n, (i + 1) % n);
        let (a, b, c) = (ring[pi], ring[ci], ring[ni]);
        let ear = cross(p(a), p(b), p(c)) > 0.0 && {
            let mut blocked = false;
            for q in 0..n {
                if q == pi || q == ci || q == ni {
                    continue;
                }
                let x = p(ring[q]);
                if same(x, p(a)) || same(x, p(b)) || same(x, p(c)) {
                    continue;
                }
                let qp = p(ring[(q + n - 1) % n]);
                let qn = p(ring[(q + 1) % n]);
                // only a reflex corner can lie inside an ear
                if cross(qp, x, qn) > 0.0 {
                    continue;
                }
                let (d1, d2, d3) = (
                    cross(p(a), p(b), x),
                    cross(p(b), p(c), x),
                    cross(p(c), p(a), x),
                );
                if d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0 {
                    blocked = true;
                    break;
                }
            }
            !blocked
        };
        if ear {
            tris.push([a, b, c]);
            ring.remove(ci);
            i = if ci == 0 { ring.len() - 1 } else { ci - 1 };
            stalled = 0;
        } else {
            i = (i + 1) % n;
            stalled += 1;
            if stalled > 2 * n {
                return Err(Form3dError::NotTriangulable(
                    "no ear found: the outline or a hole is not simple",
                ));
            }
        }
    }
    if ring.len() == 3 {
        let (a, b, c) = (p(ring[0]), p(ring[1]), p(ring[2]));
        if cross(a, b, c) > 0.0 {
            tris.push([ring[0], ring[1], ring[2]]);
        } else if cross(a, b, c) < 0.0 {
            return Err(Form3dError::NotTriangulable(
                "final triangle is clockwise: the bridge is wrong",
            ));
        }
    }
    Ok(tris)
}

/// Boundary splitting passes allowed by `triangulate_formed`. Each pass halves the long boundary sides, so an ordinary
/// plate needs about ten.
const MAX_BOUNDARY_PASSES: usize = 64;

/// Splits a counter-clockwise triangle `t` at the midpoints `m[k]` of its edges `(t[k], t[k + 1])` (None: not split).
/// Every split edge is shared with the neighbour, which is split at the same midpoint, so the mesh stays conforming.
fn split_triangle(t: [usize; 3], m: [Option<usize>; 3], out: &mut Vec<[usize; 3]>) {
    match m {
        [None, None, None] => out.push(t),
        [Some(m0), Some(m1), Some(m2)] => {
            out.push([t[0], m0, m2]);
            out.push([m0, t[1], m1]);
            out.push([m2, m1, t[2]]);
            out.push([m0, m1, m2]);
        }
        [Some(mid), None, None] => {
            out.push([t[0], mid, t[2]]);
            out.push([mid, t[1], t[2]]);
        }
        [None, Some(mid), None] => {
            out.push([t[1], mid, t[0]]);
            out.push([mid, t[2], t[0]]);
        }
        [None, None, Some(mid)] => {
            out.push([t[2], mid, t[1]]);
            out.push([mid, t[0], t[1]]);
        }
        [Some(a), Some(b), None] => {
            out.push([a, t[1], b]);
            out.push([t[0], a, b]);
            out.push([t[0], b, t[2]]);
        }
        [None, Some(a), Some(b)] => {
            out.push([a, t[2], b]);
            out.push([t[1], a, b]);
            out.push([t[1], b, t[0]]);
        }
        [Some(a), None, Some(b)] => {
            out.push([b, t[0], a]);
            out.push([t[2], b, a]);
            out.push([t[2], a, t[1]]);
        }
    }
}

/// The vertex of `t` that is neither `a` nor `b`.
fn opposite(t: [usize; 3], a: usize, b: usize) -> Option<usize> {
    t.into_iter().find(|&v| v != a && v != b)
}

/// True when `d` lies strictly inside the circumcircle of the counter-clockwise triangle `a b c` (with a margin
/// against rounding, so that cocircular points do not flip back and forth).
fn in_circle(a: P2, b: P2, c: P2, d: P2) -> bool {
    let (adx, ady) = (a.x - d.x, a.y - d.y);
    let (bdx, bdy) = (b.x - d.x, b.y - d.y);
    let (cdx, cdy) = (c.x - d.x, c.y - d.y);
    let (ad, bd, cd) = (
        adx * adx + ady * ady,
        bdx * bdx + bdy * bdy,
        cdx * cdx + cdy * cdy,
    );
    let det =
        adx * (bdy * cd - bd * cdy) - ady * (bdx * cd - bd * cdx) + ad * (bdx * cdy - bdy * cdx);
    let scale = ad.max(bd).max(cd);
    det > 1e-12 * scale * scale
}

/// One pass splitting every boundary side (a side with no triangle across it) whose circumferential extent exceeds
/// `span`, at its midpoint. Returns the refined triangles, or None when no boundary side is too long.
fn split_boundary_pass(
    vertices: &mut Vec<P2>,
    tris: &[[usize; 3]],
    circ: fn(P2) -> f64,
    span: f64,
) -> Option<Vec<[usize; 3]>> {
    let sides: HashSet<(usize, usize)> = tris
        .iter()
        .flat_map(|t| (0..3).map(move |k| (t[k], t[(k + 1) % 3])))
        .collect();
    let mut out = Vec::with_capacity(tris.len() + 16);
    let mut changed = false;
    for &t in tris {
        let mut m = [None; 3];
        for (k, slot) in m.iter_mut().enumerate() {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            if !sides.contains(&(b, a)) && (circ(vertices[a]) - circ(vertices[b])).abs() > span {
                let (pa, pb) = (vertices[a], vertices[b]);
                vertices.push(P2::new(0.5 * (pa.x + pb.x), 0.5 * (pa.y + pb.y)));
                *slot = Some(vertices.len() - 1);
                changed = true;
            }
        }
        split_triangle(t, m, &mut out);
    }
    changed.then_some(out)
}

/// Lawson edge flips: an interior edge is flipped while the opposite vertex lies strictly inside the circumcircle of
/// its triangle and the flip keeps both new triangles counter-clockwise. Boundary sides are never flipped, so the
/// triangles keep covering the same plate. Refuses when the flips do not settle within their budget.
fn lawson_flips(vertices: &[P2], tris: &mut [[usize; 3]]) -> Result<(), Form3dError> {
    // owner[(a, b)] is the triangle with the counter-clockwise side a -> b
    let mut owner: HashMap<(usize, usize), usize> = HashMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            owner.insert((t[k], t[(k + 1) % 3]), ti);
        }
    }
    let mut stack: Vec<(usize, usize)> = owner.keys().copied().collect();
    // a fan from the ear clipping can need one flip per vertex for each vertex, so the budget is quadratic
    let mut budget = 4 * vertices.len() * vertices.len() + 1000;
    while let Some((a, b)) = stack.pop() {
        let (Some(&t1), Some(&t2)) = (owner.get(&(a, b)), owner.get(&(b, a))) else {
            continue;
        };
        let (Some(c), Some(d)) = (opposite(tris[t1], a, b), opposite(tris[t2], a, b)) else {
            continue;
        };
        let (pa, pb, pc, pd) = (vertices[a], vertices[b], vertices[c], vertices[d]);
        if !in_circle(pa, pb, pc, pd) || cross(pa, pd, pc) <= 0.0 || cross(pd, pb, pc) <= 0.0 {
            continue;
        }
        if budget == 0 {
            return Err(Form3dError::NotTriangulable(
                "edge flips did not settle within their budget",
            ));
        }
        budget -= 1;
        // a b c and b a d become a d c and d b c
        tris[t1] = [a, d, c];
        tris[t2] = [d, b, c];
        owner.remove(&(a, b));
        owner.remove(&(b, a));
        for (side, t) in [
            ((a, d), t1),
            ((d, c), t1),
            ((c, a), t1),
            ((d, b), t2),
            ((b, c), t2),
            ((c, d), t2),
        ] {
            owner.insert(side, t);
        }
        stack.extend([(a, d), (d, b), (b, c), (c, a)]);
    }
    Ok(())
}

/// Triangulates an outline with holes on the formed cylinder or header shell, so that the formed part is within
/// `chord_tol` (mm) of its surface.
///
/// Why a chord bound follows from the plate: on the cylinder the chord of two surface points whose circumferential
/// plate coordinates differ by `d` (arc length) lies at radius R cos(d / 2R) from the axis. With
/// `span = 2 R acos(1 - chord_tol / R)`, every triangle side with circumferential extent at most `span` is within
/// `chord_tol` of the surface. A triangle whose three sides each span at most `span` has its vertices in one arc of
/// at most `span` (for `span < 2 R pi / 3`, which `chord_tol < R / 2` gives), so its centroid and every point of it
/// lie within `chord_tol` too.
///
/// The method: the plate is triangulated by `triangulate`; the boundary sides longer than `span` are split at their
/// midpoints; then Lawson flips make the interior Delaunay. This bounds plates whose two boundary rows are dense along
/// the circumference, such as the cylinder shell. A plate with an interior that has no points of its own (the branch
/// stub, the header with its opening) keeps a side longer than `span` after the flips, and is refused: no interior
/// point is added here, so no approximation is hidden. Refused as well: a cone (no chord bound is stated for its map
/// here), a `chord_tol` that is not positive, and one at or above half the radius.
pub fn triangulate_formed(
    form: &Form3d,
    outer: &[P2],
    holes: &[Vec<P2>],
    chord_tol: f64,
) -> Result<Triangulation, Form3dError> {
    let radius = match *form {
        Form3d::Cylinder { radius, .. } | Form3d::Header { radius, .. } => radius,
        Form3d::Cone { .. } => {
            return Err(Form3dError::NotTriangulable(
                "the cone map has no chord-bounded triangulation",
            ));
        }
    };
    let circ: fn(P2) -> f64 = if matches!(form, Form3d::Header { .. }) {
        |p| p.y
    } else {
        |p| p.x
    };
    if !(chord_tol.is_finite() && chord_tol > 0.0) {
        return Err(Form3dError::NotPositive("chord_tol"));
    }
    if chord_tol >= 0.5 * radius {
        return Err(Form3dError::NotTriangulable(
            "the chord tolerance must be below half the radius",
        ));
    }
    let span = radius * 2.0 * (1.0 - chord_tol / radius).acos();
    let base = triangulate(outer, holes)?;
    let mut vertices = base.vertices;
    let mut triangles = base.triangles;
    let mut settled = false;
    for _ in 0..MAX_BOUNDARY_PASSES {
        match split_boundary_pass(&mut vertices, &triangles, circ, span) {
            Some(next) => triangles = next,
            None => {
                settled = true;
                break;
            }
        }
    }
    if !settled {
        return Err(Form3dError::NotTriangulable(
            "the boundary did not settle under the chord bound",
        ));
    }
    // the flips make the interior Delaunay; a side that is still too long is refused below
    lawson_flips(&vertices, &mut triangles)?;
    let too_long = triangles.iter().any(|t| {
        (0..3).any(|k| (circ(vertices[t[k]]) - circ(vertices[t[(k + 1) % 3]])).abs() > span)
    });
    if too_long {
        return Err(Form3dError::NotTriangulable(
            "the chord bound is not reached by the boundary splits and edge flips: a side is still too long",
        ));
    }
    Ok(Triangulation {
        vertices,
        triangles,
    })
}

/// The outline and holes of an engine pattern, as closed plate polygons (mm). Line and polyline edges are taken
/// exactly; an arc edge cannot be triangulated exactly, so the pattern is refused.
pub fn pattern_polygons(pattern: &Pattern) -> Result<(Vec<P2>, Vec<Vec<P2>>), Form3dError> {
    let family = pattern.family;
    let mut outer: Vec<P2> = Vec::new();
    for curve in &pattern.outline {
        let pts = match &curve.geom {
            Geom::Line { a, b } => vec![*a, *b],
            Geom::Polyline { pts, .. } => pts.clone(),
            Geom::Arc { .. } => {
                return Err(Form3dError::NotFormable {
                    family,
                    reason: "an arc edge is not triangulated exactly",
                });
            }
        };
        for p in pts {
            if outer.last().is_none_or(|&l| !same(l, p)) {
                outer.push(p);
            }
        }
    }
    while outer.len() > 1 && same(outer[0], outer[outer.len() - 1]) {
        outer.pop();
    }
    let mut holes = Vec::new();
    for curve in &pattern.holes {
        match &curve.geom {
            Geom::Polyline { pts, .. } => {
                let mut h = pts.clone();
                while h.len() > 1 && same(h[0], h[h.len() - 1]) {
                    h.pop();
                }
                holes.push(h);
            }
            _ => {
                return Err(Form3dError::NotFormable {
                    family,
                    reason: "a hole is not a closed polyline",
                });
            }
        }
    }
    Ok((outer, holes))
}
