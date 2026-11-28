//! Geometry: the entity model and the exact measurements. Angles are degrees, counter-clockwise from +X.

pub type P = [f64; 2];

/// A polyline vertex; `bulge` is the DXF bulge of the segment that starts here (0 = straight).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vtx {
    pub p: P,
    pub bulge: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Geom {
    Point { at: P },
    Line { a: P, b: P },
    Circle { c: P, r: f64 },
    /// Counter-clockwise from `start` to `end` (degrees).
    Arc { c: P, r: f64, start: f64, end: f64 },
    Poly { v: Vec<Vtx>, closed: bool },
    Text { at: P, height: f64, text: String },
    /// An entity kind phase 1 does not read (INSERT, HATCH, ...). It is listed, never measured.
    Unsupported,
}

pub fn dist(a: P, b: P) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Direction of a -> b in degrees, in [0, 360).
pub fn angle_deg(a: P, b: P) -> f64 {
    (b[1] - a[1]).atan2(b[0] - a[0]).to_degrees().rem_euclid(360.0)
}

/// Unsigned angle between two lines a1-a2 and b1-b2, in [0, 90] (lines have no direction).
pub fn line_angle_deg(a1: P, a2: P, b1: P, b2: P) -> f64 {
    let d = (angle_deg(a1, a2) - angle_deg(b1, b2)).rem_euclid(180.0);
    d.min(180.0 - d)
}

/// Unsigned angle at vertex `v` between the rays v->a and v->b, in [0, 180].
pub fn vertex_angle_deg(v: P, a: P, b: P) -> f64 {
    let d = (angle_deg(v, a) - angle_deg(v, b)).rem_euclid(360.0);
    d.min(360.0 - d)
}

/// Counter-clockwise sweep from `start` to `end`, in [0, 360). A zero sweep is 0 (use a circle for 360).
pub fn sweep_deg(start: f64, end: f64) -> f64 {
    let s = (end - start).rem_euclid(360.0);
    if s < 1e-12 || (360.0 - s) < 1e-12 { 0.0 } else { s }
}

/// True when angle `a` (degrees) lies on the counter-clockwise arc from `start` over `sweep`.
pub fn angle_on_sweep(a: f64, start: f64, sweep: f64) -> bool {
    (a - start).rem_euclid(360.0) <= sweep + 1e-12
}

pub fn arc_length(r: f64, start: f64, end: f64) -> f64 {
    r * sweep_deg(start, end).to_radians()
}

pub fn circle_length(r: f64) -> f64 {
    2.0 * std::f64::consts::PI * r
}

pub fn circle_area(r: f64) -> f64 {
    std::f64::consts::PI * r * r
}

/// Length and signed area term of the segment p -> q with bulge `b`. A straight segment (b = 0) gives its
/// chord and no area term. A bulge arc uses included angle theta = 4 atan(b) and radius chord / (2 sin(theta/2)).
pub fn bulge_segment(p: P, q: P, b: f64) -> (f64, f64) {
    let chord = dist(p, q);
    if b.abs() < 1e-12 || chord == 0.0 {
        return (chord, 0.0);
    }
    let theta = (4.0 * b.atan()).abs();
    let s = (theta / 2.0).sin().abs();
    if s < 1e-15 {
        return (chord, 0.0);
    }
    let r = chord / (2.0 * s);
    let len = r * theta;
    let area = b.signum() * 0.5 * r * r * (theta - theta.sin());
    (len, area)
}

/// Length of a polyline; a closed one includes the closing segment.
pub fn poly_length(v: &[Vtx], closed: bool) -> f64 {
    let n = v.len();
    if n < 2 {
        return 0.0;
    }
    let segs = if closed { n } else { n - 1 };
    (0..segs)
        .map(|i| bulge_segment(v[i].p, v[(i + 1) % n].p, v[i].bulge).0)
        .sum()
}

/// Area enclosed by a closed polyline (absolute value of the shoelace sum plus bulge segment areas).
/// Self-intersections are not checked: the value is the signed-area magnitude.
pub fn poly_area(v: &[Vtx]) -> f64 {
    let n = v.len();
    if n < 2 {
        return 0.0;
    }
    let mut shoe = 0.0;
    let mut bulges = 0.0;
    for i in 0..n {
        let (p, q) = (v[i].p, v[(i + 1) % n].p);
        shoe += p[0] * q[1] - q[0] * p[1];
        bulges += bulge_segment(p, q, v[i].bulge).1;
    }
    (0.5 * shoe + bulges).abs()
}

/// Bounding box [minx, miny, maxx, maxy] of a geometry, or None for an entity without coordinates.
pub fn extents(g: &Geom) -> Option<[f64; 4]> {
    let mut b: Option<[f64; 4]> = None;
    let mut add = |p: P| {
        b = Some(match b {
            None => [p[0], p[1], p[0], p[1]],
            Some(e) => [e[0].min(p[0]), e[1].min(p[1]), e[2].max(p[0]), e[3].max(p[1])],
        });
    };
    match g {
        Geom::Point { at } | Geom::Text { at, .. } => add(*at),
        Geom::Line { a, b } => {
            add(*a);
            add(*b);
        }
        Geom::Circle { c, r } => {
            add([c[0] - r, c[1] - r]);
            add([c[0] + r, c[1] + r]);
        }
        Geom::Arc { c, r, start, end } => {
            let sw = sweep_deg(*start, *end);
            add(point_at(*c, *r, *start));
            add(point_at(*c, *r, *end));
            for k in 0..4 {
                let a = k as f64 * 90.0;
                if angle_on_sweep(a, *start, sw) {
                    add(point_at(*c, *r, a));
                }
            }
        }
        Geom::Poly { v, closed } => {
            let n = v.len();
            let segs = if *closed { n } else { n.saturating_sub(1) };
            for i in 0..n {
                add(v[i].p);
            }
            for i in 0..segs {
                if v[i].bulge.abs() >= 1e-12 {
                    for k in 1..32 {
                        add(bulge_point(v[i].p, v[(i + 1) % n].p, v[i].bulge, k as f64 / 32.0));
                    }
                }
            }
        }
        Geom::Unsupported => {}
    }
    b
}

pub fn point_at(c: P, r: f64, deg: f64) -> P {
    let t = deg.to_radians();
    [c[0] + r * t.cos(), c[1] + r * t.sin()]
}

/// Distance from `p` to the segment a-b.
pub fn seg_dist(p: P, a: P, b: P) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 == 0.0 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0)
    };
    dist(p, [a[0] + t * dx, a[1] + t * dy])
}

/// Distance from `p` to the geometry (the outline for closed shapes, the path for open ones).
pub fn pick_distance(g: &Geom, p: P) -> f64 {
    match g {
        Geom::Point { at } => dist(p, *at),
        Geom::Text { at, height, .. } => {
            // a text is picked near its anchor, within its height
            (dist(p, *at) - height.max(0.0)).max(0.0)
        }
        Geom::Line { a, b } => seg_dist(p, *a, *b),
        Geom::Circle { c, r } => (dist(p, *c) - r).abs(),
        Geom::Arc { c, r, start, end } => {
            let sw = sweep_deg(*start, *end);
            let a = (p[1] - c[1]).atan2(p[0] - c[0]).to_degrees();
            if angle_on_sweep(a, *start, sw) {
                (dist(p, *c) - r).abs()
            } else {
                dist(p, point_at(*c, *r, *start)).min(dist(p, point_at(*c, *r, *end)))
            }
        }
        Geom::Poly { v, closed } => {
            let n = v.len();
            let segs = if *closed { n } else { n.saturating_sub(1) };
            let mut best = f64::INFINITY;
            for i in 0..segs {
                let (a, b) = (v[i].p, v[(i + 1) % n].p);
                let d = if v[i].bulge.abs() < 1e-12 {
                    seg_dist(p, a, b)
                } else {
                    // sample the bulge arc
                    let steps = 64;
                    let mut m = f64::INFINITY;
                    let mut prev = a;
                    for k in 1..=steps {
                        let t = k as f64 / steps as f64;
                        let q = bulge_point(a, b, v[i].bulge, t);
                        m = m.min(seg_dist(p, prev, q));
                        prev = q;
                    }
                    m
                };
                best = best.min(d);
            }
            best
        }
        Geom::Unsupported => f64::INFINITY,
    }
}

/// Point at parameter t (0..1) along the bulge arc from a to b. Signed included angle theta = 4 atan(b):
/// the centre lies on the left of a->b for a counter-clockwise arc (b > 0), on the right otherwise.
fn bulge_point(a: P, b: P, bulge: f64, t: f64) -> P {
    let theta = 4.0 * bulge.atan();
    let chord = dist(a, b);
    if theta.abs() < 1e-12 || chord == 0.0 {
        return [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    }
    let r = chord / (2.0 * (theta / 2.0).sin().abs());
    let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    let left = [-(b[1] - a[1]) / chord, (b[0] - a[0]) / chord];
    let off = r * (theta / 2.0).cos() * theta.signum();
    let c = [mid[0] + left[0] * off, mid[1] + left[1] * off];
    let a0 = (a[1] - c[1]).atan2(a[0] - c[0]);
    let ang = a0 + theta * t;
    [c[0] + r * ang.cos(), c[1] + r * ang.sin()]
}
