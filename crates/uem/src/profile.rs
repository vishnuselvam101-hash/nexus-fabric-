//! Profile validation on flattened outlines (Spec 02 ALG-08, simplified).
//!
//! LIMITATION: uses binary64 arithmetic, not exact predicates. Results are
//! reliable for well-separated geometry; near-degenerate contacts are not
//! decided rigorously in this build.

use crate::{Curve, Geom, P2};

/// Start and end point of a curve in traversal order.
pub fn endpoints(c: &Curve) -> (P2, P2) {
    match &c.geom {
        Geom::Line { a, b } => (*a, *b),
        Geom::Arc { c, r, a0, a1 } => (
            P2::new(c.x + r * a0.cos(), c.y + r * a0.sin()),
            P2::new(c.x + r * a1.cos(), c.y + r * a1.sin()),
        ),
        Geom::Polyline { pts, .. } => (pts[0], pts[pts.len() - 1]),
    }
}

/// Exact length of a curve as represented (arcs exact, polylines as chords).
pub fn length(c: &Curve) -> f64 {
    match &c.geom {
        Geom::Line { a, b } => a.dist(*b),
        Geom::Arc { r, a0, a1, .. } => r * (a1 - a0).abs(),
        Geom::Polyline { pts, .. } => pts.windows(2).map(|w| w[0].dist(w[1])).sum(),
    }
}

/// Flatten an outline to a closed polygon (last point not repeated).
/// Arcs are sampled with `arc_segments` chords.
pub fn flatten(outline: &[Curve], arc_segments: usize) -> Vec<P2> {
    let mut pts: Vec<P2> = Vec::new();
    for c in outline {
        let seg: Vec<P2> = match &c.geom {
            Geom::Line { a, b } => vec![*a, *b],
            Geom::Arc { c, r, a0, a1 } => (0..=arc_segments)
                .map(|i| {
                    let a = a0 + (a1 - a0) * i as f64 / arc_segments as f64;
                    P2::new(c.x + r * a.cos(), c.y + r * a.sin())
                })
                .collect(),
            Geom::Polyline { pts, .. } => pts.clone(),
        };
        for p in seg {
            if pts.last().is_none_or(|q| q.dist(p) > 1e-9) {
                pts.push(p);
            }
        }
    }
    if pts.len() > 1 && pts[0].dist(pts[pts.len() - 1]) <= 1e-9 {
        pts.pop();
    }
    pts
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProfileReport {
    /// Largest gap between the end of one curve and the start of the next (mm).
    pub max_gap: f64,
    pub closed: bool,
    pub self_intersections: usize,
    /// Signed area of the flattened polygon (positive = counter-clockwise).
    pub signed_area: f64,
    pub vertices: usize,
}

fn orient(a: P2, b: P2, c: P2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn proper_cross(a: P2, b: P2, c: P2, d: P2) -> bool {
    let (o1, o2, o3, o4) = (
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    );
    (o1 > 0.0) != (o2 > 0.0)
        && (o3 > 0.0) != (o4 > 0.0)
        && o1 != 0.0
        && o2 != 0.0
        && o3 != 0.0
        && o4 != 0.0
}

/// Point-coincidence tolerance of the contact tests, the same as `flatten`'s (mm).
const TOUCH_TOL: f64 = 1e-9;

/// Distance from `p` to the segment `a`–`b` (0 when `p` lies on it).
fn point_segment_distance(p: P2, a: P2, b: P2) -> f64 {
    let (vx, vy) = (b.x - a.x, b.y - a.y);
    let len2 = vx * vx + vy * vy;
    if len2 <= 0.0 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * vx + (p.y - a.y) * vy) / len2).clamp(0.0, 1.0);
    p.dist(P2::new(a.x + t * vx, a.y + t * vy))
}

/// Contact that `proper_cross` does not decide (review RC-13): an endpoint of one
/// segment lies on the other within `TOUCH_TOL`. Covers a T-touch, a crossing
/// through a vertex, a pinch and a collinear overlap — an overlap of positive
/// length always puts an endpoint of one segment inside the other.
fn touch_or_overlap(a: P2, b: P2, c: P2, d: P2) -> bool {
    [a, b]
        .iter()
        .any(|p| point_segment_distance(*p, c, d) <= TOUCH_TOL)
        || [c, d]
            .iter()
            .any(|p| point_segment_distance(*p, a, b) <= TOUCH_TOL)
}

/// Number of crossings, touches and collinear overlaps between non-adjacent
/// edges of the closed polygon `poly` (a touch or an overlap is a
/// self-intersection too, review RC-13). Same predicate and same count as
/// testing every pair, but through a uniform grid so that a polyline of tens of
/// thousands of vertices (adaptive density, review RB-004) is checked in about
/// linear time.
pub fn crossings(poly: &[P2]) -> usize {
    let m = poly.len();
    if m < 4 {
        return 0;
    }
    let seg = |i: usize| (poly[i], poly[(i + 1) % m]);
    let adjacent = |i: usize, j: usize| {
        let (lo, hi) = (i.min(j), i.max(j));
        hi - lo <= 1 || (lo == 0 && hi == m - 1)
    };
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in poly {
        x0 = x0.min(q.x);
        y0 = y0.min(q.y);
        x1 = x1.max(q.x);
        y1 = y1.max(q.y);
    }
    let (w, h) = (x1 - x0, y1 - y0);
    if !(w.is_finite() && h.is_finite()) || m < 64 {
        // small or non-finite input: every pair
        let mut n = 0;
        for i in 0..m {
            for j in (i + 2)..m {
                if !adjacent(i, j) {
                    let ((a, b), (c, d)) = (seg(i), seg(j));
                    if proper_cross(a, b, c, d) || touch_or_overlap(a, b, c, d) {
                        n += 1;
                    }
                }
            }
        }
        return n;
    }
    // about one segment per cell on average, aspect ratio of the bounding box
    let cells = m as f64;
    let aspect = if h > 0.0 && w > 0.0 { w / h } else { 1.0 };
    let nx = ((cells * aspect).sqrt().ceil() as usize).clamp(1, 4096);
    let ny = ((cells / aspect).sqrt().ceil() as usize).clamp(1, 4096);
    let cx = |x: f64| -> usize {
        if w > 0.0 {
            (((x - x0) / w * nx as f64) as usize).min(nx - 1)
        } else {
            0
        }
    };
    let cy = |y: f64| -> usize {
        if h > 0.0 {
            (((y - y0) / h * ny as f64) as usize).min(ny - 1)
        } else {
            0
        }
    };
    // a segment whose box covers many cells (a long plate edge) is kept apart and tested against all
    let limit = 4 * (nx + ny);
    let mut grid: Vec<Vec<u32>> = vec![Vec::new(); nx * ny];
    let mut long: Vec<usize> = Vec::new();
    let mut boxes: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(m);
    for i in 0..m {
        let (a, b) = seg(i);
        // grown by the contact tolerance so a point within TOUCH_TOL of a segment shares a cell with it
        let bx = (
            cx(a.x.min(b.x) - TOUCH_TOL),
            cy(a.y.min(b.y) - TOUCH_TOL),
            cx(a.x.max(b.x) + TOUCH_TOL),
            cy(a.y.max(b.y) + TOUCH_TOL),
        );
        boxes.push(bx);
        if (bx.2 - bx.0 + 1) * (bx.3 - bx.1 + 1) > limit {
            long.push(i);
        } else {
            for gx in bx.0..=bx.2 {
                for gy in bx.1..=bx.3 {
                    grid[gy * nx + gx].push(i as u32);
                }
            }
        }
    }
    let mut n = 0;
    let is_long = |i: usize| long.binary_search(&i).is_ok();
    for (k, &i) in long.iter().enumerate() {
        for j in 0..m {
            // a pair of two long segments is counted once, from the earlier one
            if j == i || adjacent(i, j) || (is_long(j) && long[..k].contains(&j)) {
                continue;
            }
            let ((a, b), (c, d)) = (seg(i), seg(j));
            if proper_cross(a, b, c, d) || touch_or_overlap(a, b, c, d) {
                n += 1;
            }
        }
    }
    for (cell, members) in grid.iter().enumerate() {
        let (gx, gy) = (cell % nx, cell / nx);
        for (p, &i) in members.iter().enumerate() {
            for &j in &members[p + 1..] {
                let (i, j) = (i as usize, j as usize);
                if adjacent(i, j) {
                    continue;
                }
                // each pair is examined in exactly one cell: the lowest-left cell their boxes share
                let (bi, bj) = (boxes[i], boxes[j]);
                if bi.0.max(bj.0) != gx || bi.1.max(bj.1) != gy {
                    continue;
                }
                let ((a, b), (c, d)) = (seg(i), seg(j));
                if proper_cross(a, b, c, d) || touch_or_overlap(a, b, c, d) {
                    n += 1;
                }
            }
        }
    }
    n
}

pub fn check(outline: &[Curve], closure_tol: f64) -> ProfileReport {
    let n = outline.len();
    let mut max_gap: f64 = 0.0;
    for i in 0..n {
        let (_, end) = endpoints(&outline[i]);
        let (start, _) = endpoints(&outline[(i + 1) % n]);
        max_gap = max_gap.max(end.dist(start));
    }
    let poly = flatten(outline, 256);
    let m = poly.len();
    let signed_area = 0.5
        * (0..m)
            .map(|i| poly[i].x * poly[(i + 1) % m].y - poly[(i + 1) % m].x * poly[i].y)
            .sum::<f64>();
    ProfileReport {
        max_gap,
        closed: max_gap <= closure_tol,
        self_intersections: crossings(&poly),
        signed_area,
        vertices: m,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::*;

    #[test]
    fn patterns_are_closed_simple_ccw() {
        for p in [
            cylinder_shell(100.0, 50.0).unwrap(),
            cone_frustum(200.0, 100.0, 300.0).unwrap(),
            eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap(),
            branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap(),
        ] {
            let r = check(&p.outline, 1e-9);
            assert!(r.closed, "{} gap {}", p.family, r.max_gap);
            assert_eq!(r.self_intersections, 0, "{}", p.family);
            assert!(r.signed_area > 0.0, "{} not counter-clockwise", p.family);
        }
    }

    #[test]
    fn detects_bow_tie() {
        use crate::{Curve, Geom, Layer};
        let l = |a: (f64, f64), b: (f64, f64)| Curve {
            name: "e",
            layer: Layer::CutOuter,
            geom: Geom::Line {
                a: P2::new(a.0, a.1),
                b: P2::new(b.0, b.1),
            },
        };
        let bow = vec![
            l((0.0, 0.0), (10.0, 10.0)),
            l((10.0, 10.0), (10.0, 0.0)),
            l((10.0, 0.0), (0.0, 10.0)),
            l((0.0, 10.0), (0.0, 0.0)),
        ];
        assert_eq!(check(&bow, 1e-9).self_intersections, 1);
        let open = vec![l((0.0, 0.0), (10.0, 0.0)), l((10.0, 0.5), (0.0, 0.0))];
        assert!(!check(&open, 1e-9).closed);
    }

    /// The grid search must count exactly what the pairwise search counts.
    #[test]
    fn grid_crossing_count_equals_the_pairwise_count() {
        let brute = |poly: &[P2]| -> usize {
            let m = poly.len();
            let mut n = 0;
            for i in 0..m {
                for j in (i + 2)..m {
                    if i == 0 && j == m - 1 {
                        continue;
                    }
                    if proper_cross(poly[i], poly[(i + 1) % m], poly[j], poly[(j + 1) % m])
                        || touch_or_overlap(poly[i], poly[(i + 1) % m], poly[j], poly[(j + 1) % m])
                    {
                        n += 1;
                    }
                }
            }
            n
        };
        let mut seed = 0x2545F4914F6CDD1Du64;
        let mut rnd = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut with_crossings = 0;
        for case in 0..60 {
            let m = 64 + (rnd() * 700.0) as usize;
            // a wobbly ring, sometimes with long chords across it and extreme aspect ratios
            let (sx, sy) = (10f64.powf(rnd() * 6.0 - 3.0), 10f64.powf(rnd() * 6.0 - 3.0));
            let wobble = if case % 3 == 0 { 0.0 } else { rnd() * 0.8 };
            let mut poly: Vec<P2> = (0..m)
                .map(|i| {
                    let t = crate::TAU * i as f64 / m as f64;
                    let r = 1.0 + wobble * (rnd() - 0.5);
                    P2::new(sx * r * t.cos(), sy * r * t.sin())
                })
                .collect();
            if case % 4 == 1 {
                let k = m / 3;
                poly.swap(k, 2 * k); // two long chords across the ring
            }
            if case % 5 == 2 {
                poly.push(P2::new(-3.0 * sx, 0.3 * sy)); // one very long edge
            }
            let (a, b) = (crossings(&poly), brute(&poly));
            assert_eq!(a, b, "case {case}: {m} vertices");
            if b > 0 {
                with_crossings += 1;
            }
        }
        assert!(
            with_crossings > 20,
            "the test must exercise real crossings: {with_crossings}"
        );
        // a dense simple curve: no crossing, and fast
        let dense: Vec<P2> = (0..50_000)
            .map(|i| {
                let t = crate::TAU * i as f64 / 50_000.0;
                P2::new(300.0 * t.cos(), 200.0 * t.sin())
            })
            .collect();
        assert_eq!(crossings(&dense), 0);
    }

    /// RC-13: contacts that are not proper crossings — a T-touch, a crossing
    /// through a vertex, a pinch, a collinear overlap — are self-intersections
    /// too. Every one of these counted 0 before the endpoint-on-segment test.
    #[test]
    fn touches_and_overlaps_are_self_intersections() {
        let p = P2::new;
        // T-touch: the vertex (5,0) lies inside the non-adjacent bottom edge
        let t_touch = [
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(5.0, 10.0),
            p(5.0, 0.0),
        ];
        assert_eq!(crossings(&t_touch), 1);
        // crossing through a vertex: the edge (10,0)-(0,10) runs through the vertex (5,5)
        let through_vertex = [p(0.0, 0.0), p(5.0, 5.0), p(10.0, 0.0), p(0.0, 10.0)];
        assert_eq!(crossings(&through_vertex), 1);
        // pinch: two loops sharing the vertex (2,2)
        let pinch = [p(0.0, 0.0), p(2.0, 2.0), p(4.0, 0.0), p(2.0, 2.0)];
        assert_eq!(crossings(&pinch), 2);
        // collinear overlap: the left edge runs back over itself between y=2 and y=8
        let overlap = [
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 10.0),
            p(0.0, 10.0),
            p(0.0, 2.0),
            p(8.0, 2.0),
            p(8.0, 8.0),
            p(0.0, 8.0),
        ];
        assert_eq!(crossings(&overlap), 3);
        // the same T-touch through check(), as G11 runs it
        let l = |a: (f64, f64), b: (f64, f64)| Curve {
            name: "e",
            layer: crate::Layer::CutOuter,
            geom: Geom::Line {
                a: p(a.0, a.1),
                b: p(b.0, b.1),
            },
        };
        let outline = vec![
            l((0.0, 0.0), (10.0, 0.0)),
            l((10.0, 0.0), (10.0, 10.0)),
            l((10.0, 10.0), (5.0, 10.0)),
            l((5.0, 10.0), (5.0, 0.0)),
            l((5.0, 0.0), (0.0, 0.0)),
        ];
        let r = check(&outline, 1e-9);
        assert!(r.closed);
        assert_eq!(r.self_intersections, 1);
    }
}
