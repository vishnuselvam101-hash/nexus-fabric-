//! Triangulation of a plate outline with holes (A-21): ear clipping, with each hole joined to the outline by a bridge
//! (the bridge method). Pure arithmetic on plate coordinates in mm; no I/O.
//!
//! The outline is a simple closed polygon, the holes are simple closed polygons inside it and do not touch each other
//! or the outline. The triangles cover the outline minus the holes exactly (up to rounding), so their area sum is the
//! net area of the plate: `net_area`.

use uem::{Geom, P2, Pattern};

use crate::Form3dError;

/// A triangulated plate. `vertices` are the outline vertices followed by the vertices of each hole, in the order the
/// holes were given. Triangles are counter-clockwise index triples into `vertices`.
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
        let mut pos = None;
        for c in cands {
            let v = vertices[ring[c]];
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

    let triangles = ear_clip(&vertices, ring)?;
    Ok(Triangulation {
        vertices,
        triangles,
    })
}

/// Ear clipping of a weakly simple counter-clockwise ring (repeated indices are allowed for bridge vertices).
fn ear_clip(vertices: &[P2], mut ring: Vec<usize>) -> Result<Vec<[usize; 3]>, Form3dError> {
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
