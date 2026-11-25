//! Triangulation of a plate outline with holes (A-21): ear clipping, with each hole joined to the outline by a bridge
//! (the bridge method). Pure arithmetic on plate coordinates in mm; no I/O.
//!
//! The outline is a simple closed polygon, the holes are simple closed polygons inside it and do not touch each other
//! or the outline. The triangles cover the outline minus the holes exactly (up to rounding), so their area sum is the
//! net area of the plate: `net_area`.
//!
//! The formed parts (`triangulate_formed`, A-21 round 2) are triangulated by slab decomposition: the plate is cut by
//! lines of constant circumferential coordinate into slabs of equal width, the pieces are triangulated by `triangulate`,
//! and every triangle then lies within one slab, so its points are within the chord tolerance of the surface.

use std::collections::HashMap;

use uem::{Geom, P2, Pattern};

use crate::{Form3d, Form3dError};

/// A triangulated plate. `vertices` are the outline vertices followed by the vertices of each hole, in the order the
/// holes were given. A slab triangulation (`triangulate_formed`) lists the vertices of its slabs instead, with the
/// vertices on a shared slab line merged. Triangles are counter-clockwise index triples into `vertices`.
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

/// The outline made counter-clockwise and the holes clockwise, with repeated and closing vertices removed. Refuses an
/// outline or a hole that is not finite, has fewer than three vertices, or has zero area.
fn normalise(outer: &[P2], holes: &[Vec<P2>]) -> Result<(Vec<P2>, Vec<Vec<P2>>), Form3dError> {
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
    Ok((outer, holes))
}

/// Triangulates an outline with holes. The outline and the holes must be simple closed polygons (first vertex not
/// repeated at the end); the outline is made counter-clockwise and the holes clockwise.
pub fn triangulate(outer: &[P2], holes: &[Vec<P2>]) -> Result<Triangulation, Form3dError> {
    let (outer, holes) = normalise(outer, holes)?;

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
/// The circumferential coordinate of `p` along the formed part: x (`along` 0) for a cylinder, y (`along` 1) for a header.
fn coord(p: P2, along: usize) -> f64 {
    if along == 0 { p.x } else { p.y }
}

/// The point of the line `coord = value` on the segment p-q, where `fp` and `fq` are the signed distances of p and q
/// from the line, of opposite signs (or zero at an end point). An end point on the line is returned exactly.
fn line_point(p: P2, q: P2, fp: f64, fq: f64, value: f64, along: usize) -> P2 {
    if fp == 0.0 {
        return p;
    }
    if fq == 0.0 {
        return q;
    }
    let t = fp / (fp - fq);
    let o = if along == 0 {
        p.y + t * (q.y - p.y)
    } else {
        p.x + t * (q.x - p.x)
    };
    if along == 0 {
        P2::new(value, o)
    } else {
        P2::new(o, value)
    }
}

/// Clips the region bounded by `loops` (outer loops counter-clockwise, holes clockwise: the region lies on the left of
/// every edge) to the closed half-plane `coord <= value` (`below`) or `coord >= value`. The new boundary is the kept
/// parts of the loop edges and the chords of the line that lie inside the region (a chord is inside when its midpoint
/// is). The output keeps the convention. Refused: an edge lying along the line, and a boundary that pinches at a point.
fn clip_half_plane(
    loops: &[Vec<P2>],
    along: usize,
    value: f64,
    below: bool,
) -> Result<Vec<Vec<P2>>, Form3dError> {
    let sign = if below { 1.0 } else { -1.0 };
    let f = |p: P2| sign * (coord(p, along) - value);
    let mut segs: Vec<(P2, P2)> = Vec::new();
    let mut on_line: Vec<P2> = Vec::new();
    for poly in loops {
        let n = poly.len();
        for i in 0..n {
            let (p, q) = (poly[i], poly[(i + 1) % n]);
            let (fp, fq) = (f(p), f(q));
            if fp == 0.0 && fq == 0.0 {
                return Err(Form3dError::NotTriangulable(
                    "a plate edge lies along a slab line",
                ));
            }
            if fp == 0.0 {
                on_line.push(p);
            }
            if fq == 0.0 {
                on_line.push(q);
            }
            let (in_p, in_q) = (fp <= 0.0, fq <= 0.0);
            if in_p && in_q {
                segs.push((p, q));
            } else if in_p != in_q {
                let x = line_point(p, q, fp, fq, value, along);
                on_line.push(x);
                let (a, b) = if in_p { (p, x) } else { (x, q) };
                if !same(a, b) {
                    segs.push((a, b));
                }
            }
        }
    }
    // the chords: consecutive points of the line, each kept when its midpoint lies inside the region
    on_line.sort_by(|a, b| coord(*a, 1 - along).total_cmp(&coord(*b, 1 - along)));
    on_line.dedup_by(|a, b| same(*a, *b));
    let want = if below { -1.0 } else { 1.0 };
    for w in on_line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let o = 0.5 * (coord(a, 1 - along) + coord(b, 1 - along));
        let mid = if along == 0 {
            P2::new(value, o)
        } else {
            P2::new(o, value)
        };
        let inside = loops.iter().filter(|l| inside_polygon(l, mid)).count() % 2 == 1;
        if !inside {
            continue;
        }
        // the piece lies on the left: its side of the line is -coord for `below`, +coord otherwise
        let left_along = if along == 0 { -(b.y - a.y) } else { b.x - a.x };
        if left_along * want < 0.0 {
            segs.push((b, a));
        } else {
            segs.push((a, b));
        }
    }
    // chain the directed segments into closed loops; a point with two outgoing segments is a pinch
    let key = |p: P2| ((p.x + 0.0).to_bits(), (p.y + 0.0).to_bits());
    let mut next: HashMap<(u64, u64), usize> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        if next.insert(key(s.0), i).is_some() {
            return Err(Form3dError::NotTriangulable(
                "a slab boundary pinches at a point",
            ));
        }
    }
    let mut used = vec![false; segs.len()];
    let mut out: Vec<Vec<P2>> = Vec::new();
    for s in 0..segs.len() {
        if used[s] {
            continue;
        }
        let mut lp: Vec<P2> = Vec::new();
        let mut cur = s;
        loop {
            used[cur] = true;
            lp.push(segs[cur].0);
            let nxt = *next
                .get(&key(segs[cur].1))
                .ok_or(Form3dError::NotTriangulable(
                    "a slab boundary does not close",
                ))?;
            if nxt == s {
                break;
            }
            if used[nxt] {
                return Err(Form3dError::NotTriangulable(
                    "a slab boundary pinches at a point",
                ));
            }
            cur = nxt;
        }
        out.push(lp);
    }
    Ok(out)
}

/// The vertex index of `p` in `vertices`, appending `p` when no vertex lies within `tol` of it.
///
/// `tol` is the merge tolerance 1e-9 times the largest dimension M of the part. The points merged here are the slab
/// line points computed from the two neighbouring slabs, which differ only by rounding (about 1e-16 M), so 1e-9 M
/// merges them and nothing else: it is far below the chord tolerance (0.01 mm, about 1e-7 M for a 100 mm part) and far
/// above the rounding. `intern` has no triangle in view, so a merge that would collapse a triangle is refused by the
/// caller, `triangulate_formed`, which requires the three corners of every triangle to stay distinct.
fn intern(vertices: &mut Vec<P2>, p: P2, tol: f64) -> usize {
    if let Some(i) = vertices.iter().position(|&v| v.dist(p) <= tol) {
        return i;
    }
    vertices.push(p);
    vertices.len() - 1
}

/// Triangulates an outline with holes on the formed cylinder or header shell, so that every point of every triangle
/// lies within `chord_tol` (mm) of the formed surface.
///
/// Why a slab lies within the bound: on the cylinder the 3D position of a plate point depends only on its
/// circumferential coordinate (the arc length u, at radius R), and on the header only on y. A slab is a band of plate
/// with circumferential coordinate in [lo, hi], of width at most `span = 2 R acos(1 - chord_tol / R)`. Its points map
/// to surface points within one arc of angle span / R, and the chord of such an arc lies at radius R cos(span / 2R) =
/// R - chord_tol from the axis. Every point of a convex combination of them is at least that far from the axis, so a
/// triangle inside the slab is within `chord_tol` of the surface at all its points.
///
/// The method: the plate is cut by the lines of constant circumferential coordinate at equal spacing (the slab count
/// is the least that makes the width at most `span`); each piece is an exact clip of the region to its slab
/// (`clip_half_plane`); each piece is triangulated by `triangulate`. No point is added inside a piece, and no
/// approximation is made: a piece with a pinch, or an edge along a cut, is refused. Refused as well: a cone (no chord
/// bound is stated for its map here), a `chord_tol` that is not positive, and one at or above half the radius.
pub fn triangulate_formed(
    form: &Form3d,
    outer: &[P2],
    holes: &[Vec<P2>],
    chord_tol: f64,
) -> Result<Triangulation, Form3dError> {
    let (radius, along) = match *form {
        Form3d::Cylinder { radius, .. } => (radius, 0),
        Form3d::Header { radius, .. } => (radius, 1),
        Form3d::Cone { .. } => {
            return Err(Form3dError::NotTriangulable(
                "the cone map has no chord-bounded triangulation",
            ));
        }
    };
    if !(chord_tol.is_finite() && chord_tol > 0.0) {
        return Err(Form3dError::NotPositive("chord_tol"));
    }
    if chord_tol >= 0.5 * radius {
        return Err(Form3dError::NotTriangulable(
            "the chord tolerance must be below half the radius",
        ));
    }
    let span = 2.0 * radius * (1.0 - chord_tol / radius).acos();
    let (outer, holes) = normalise(outer, holes)?;
    let (c_min, c_max) = outer
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| {
            let c = coord(p, along);
            (lo.min(c), hi.max(c))
        });
    let width = c_max - c_min;
    let count = ((width / span).ceil() as usize).max(1);
    let step = width / count as f64;
    let tol = 1e-9 * form.largest_dimension();
    let mut loops: Vec<Vec<P2>> = Vec::with_capacity(1 + holes.len());
    loops.push(outer);
    loops.extend(holes);
    let mut vertices: Vec<P2> = Vec::new();
    let mut triangles: Vec<[usize; 3]> = Vec::new();
    for k in 0..count {
        let lo = c_min + k as f64 * step;
        let hi = if k + 1 == count {
            c_max
        } else {
            c_min + (k + 1) as f64 * step
        };
        let mut piece = loops.clone();
        if k + 1 < count {
            piece = clip_half_plane(&piece, along, hi, true)?;
        }
        if k > 0 {
            piece = clip_half_plane(&piece, along, lo, false)?;
        }
        // the piece's loops: counter-clockwise outlines, and clockwise holes, each hole in the outline around it
        let (p_outers, p_holes): (Vec<Vec<P2>>, Vec<Vec<P2>>) =
            piece.into_iter().partition(|l| signed_area(l) > 0.0);
        let mut assigned: Vec<Vec<Vec<P2>>> = vec![Vec::new(); p_outers.len()];
        for h in p_holes {
            let probe = h
                .first()
                .copied()
                .ok_or(Form3dError::NotTriangulable("a slab boundary is empty"))?;
            let host = (0..p_outers.len())
                .filter(|&o| inside_polygon(&p_outers[o], probe))
                .min_by(|&a, &b| signed_area(&p_outers[a]).total_cmp(&signed_area(&p_outers[b])))
                .ok_or(Form3dError::NotTriangulable(
                    "a slab hole lies in no slab outline",
                ))?;
            assigned[host].push(h);
        }
        for (outline, hs) in p_outers.iter().zip(assigned.iter()) {
            let t = triangulate(outline, hs)?;
            let map: Vec<usize> = t
                .vertices
                .iter()
                .map(|&p| intern(&mut vertices, p, tol))
                .collect();
            for tr in &t.triangles {
                let g = [map[tr[0]], map[tr[1]], map[tr[2]]];
                // a merge within the tolerance must not collapse a triangle: its three corners stay distinct
                if g[0] == g[1] || g[1] == g[2] || g[2] == g[0] {
                    return Err(Form3dError::NotTriangulable(
                        "a slab vertex merge collapses a triangle",
                    ));
                }
                triangles.push(g);
            }
        }
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
