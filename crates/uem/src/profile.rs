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
        Geom::Arc { c, r, a0, a1 } => (P2::new(c.x + r * a0.cos(), c.y + r * a0.sin()), P2::new(c.x + r * a1.cos(), c.y + r * a1.sin())),
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
    let (o1, o2, o3, o4) = (orient(a, b, c), orient(a, b, d), orient(c, d, a), orient(c, d, b));
    (o1 > 0.0) != (o2 > 0.0) && (o3 > 0.0) != (o4 > 0.0) && o1 != 0.0 && o2 != 0.0 && o3 != 0.0 && o4 != 0.0
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
    let mut crossings = 0;
    for i in 0..m {
        let (a, b) = (poly[i], poly[(i + 1) % m]);
        for j in (i + 2)..m {
            if i == 0 && j == m - 1 {
                continue; // adjacent through the closing edge
            }
            if proper_cross(a, b, poly[j], poly[(j + 1) % m]) {
                crossings += 1;
            }
        }
    }
    let signed_area = 0.5 * (0..m).map(|i| poly[i].x * poly[(i + 1) % m].y - poly[(i + 1) % m].x * poly[i].y).sum::<f64>();
    ProfileReport { max_gap, closed: max_gap <= closure_tol, self_intersections: crossings, signed_area, vertices: m }
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
        let l = |a: (f64, f64), b: (f64, f64)| Curve { name: "e", layer: Layer::CutOuter, geom: Geom::Line { a: P2::new(a.0, a.1), b: P2::new(b.0, b.1) } };
        let bow = vec![l((0.0, 0.0), (10.0, 10.0)), l((10.0, 10.0), (10.0, 0.0)), l((10.0, 0.0), (0.0, 10.0)), l((0.0, 10.0), (0.0, 0.0))];
        assert_eq!(check(&bow, 1e-9).self_intersections, 1);
        let open = vec![l((0.0, 0.0), (10.0, 0.0)), l((10.0, 0.5), (0.0, 0.0))];
        assert!(!check(&open, 1e-9).closed);
    }
}
