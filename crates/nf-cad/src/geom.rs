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
            for x in v.iter() {
                add(x.p);
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

/// Default chord tolerance for curves evaluated to polylines (drawing units). A sampled curve deviates from
/// the true curve by at most this much at every segment midpoint.
pub const CHORD_TOL: f64 = 0.001;
/// Cap on the points of one evaluated curve. Above it the curve is not evaluated (reported as a warning).
pub const MAX_CURVE_POINTS: usize = 20_000;

/// Adaptive sampling of `f` over [t0, t1]: 16 uniform spans, then bisection until the midpoint of each span lies
/// within `tol` of its chord. Returns every sample, including both ends. Errors when more than `max` points are
/// needed.
pub fn sample_curve(f: &dyn Fn(f64) -> P, t0: f64, t1: f64, tol: f64, max: usize) -> Result<Vec<P>, String> {
    fn rec(f: &dyn Fn(f64) -> P, a: f64, b: f64, pa: P, pb: P, tol: f64, depth: u32, out: &mut Vec<P>, max: usize) -> Result<(), String> {
        let m = 0.5 * (a + b);
        let pm = f(m);
        let chord_mid = [(pa[0] + pb[0]) / 2.0, (pa[1] + pb[1]) / 2.0];
        if dist(pm, chord_mid) <= tol || depth >= 24 {
            if out.len() >= max {
                return Err(format!("more than {max} points"));
            }
            out.push(pb);
            return Ok(());
        }
        let _ = pm;
        rec(f, a, m, pa, pm, tol, depth + 1, out, max)?;
        rec(f, m, b, pm, pb, tol, depth + 1, out, max)
    }
    if !(t1 > t0) || !t0.is_finite() || !t1.is_finite() {
        return Err("empty parameter range".into());
    }
    let mut out = vec![f(t0)];
    let n = 16;
    let mut pa = out[0];
    for k in 1..=n {
        let a = t0 + (t1 - t0) * (k - 1) as f64 / n as f64;
        let b = t0 + (t1 - t0) * k as f64 / n as f64;
        let pb = f(b);
        rec(f, a, b, pa, pb, tol, 0, &mut out, max)?;
        pa = pb;
    }
    Ok(out)
}

/// Evaluates a NURBS curve (degree `p`, knot vector `u`, control points `cp` with weights `w`) at `t`, using
/// de Boor on homogeneous coordinates. The valid domain is [u[p], u[n+1]] where n + 1 = cp.len().
pub fn nurbs_at(p: usize, u: &[f64], cp: &[P], w: &[f64], t: f64) -> P {
    let n = cp.len() - 1;
    let k = if t >= u[n + 1] {
        n
    } else {
        let (mut lo, mut hi) = (p, n);
        while lo < hi {
            let mid = (lo + hi + 1) / 2;
            if u[mid] <= t { lo = mid } else { hi = mid - 1 }
        }
        lo
    };
    let mut d: Vec<[f64; 3]> = (0..=p)
        .map(|j| {
            let i = k - p + j;
            [cp[i][0] * w[i], cp[i][1] * w[i], w[i]]
        })
        .collect();
    for r in 1..=p {
        for j in (r..=p).rev() {
            let i = k - p + j;
            let den = u[i + p + 1 - r] - u[i];
            let alpha = if den == 0.0 { 0.0 } else { (t - u[i]) / den };
            for c in 0..3 {
                d[j][c] = (1.0 - alpha) * d[j - 1][c] + alpha * d[j][c];
            }
        }
    }
    let wt = d[p][2];
    if wt == 0.0 { [0.0, 0.0] } else { [d[p][0] / wt, d[p][1] / wt] }
}

/// An affine map x' = a x + c y + e, y' = b x + d y + f.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub m: [f64; 4],
    pub t: P,
}

impl Affine {
    pub fn identity() -> Affine {
        Affine { m: [1.0, 0.0, 0.0, 1.0], t: [0.0, 0.0] }
    }
    pub fn apply(&self, p: P) -> P {
        let [a, b, c, d] = self.m;
        [a * p[0] + c * p[1] + self.t[0], b * p[0] + d * p[1] + self.t[1]]
    }
    /// The linear part applied to a vector (no translation).
    pub fn linear(&self, v: P) -> P {
        let [a, b, c, d] = self.m;
        [a * v[0] + c * v[1], b * v[0] + d * v[1]]
    }
    pub fn det(&self) -> f64 {
        let [a, b, c, d] = self.m;
        a * d - b * c
    }
    /// Composition: first `self`, then `o` (o(self(p))).
    pub fn then(&self, o: &Affine) -> Affine {
        let [a, b, c, d] = self.m;
        let [e, f, g, h] = o.m;
        Affine {
            m: [e * a + g * b, f * a + h * b, e * c + g * d, f * c + h * d],
            t: o.apply(self.t),
        }
    }
    pub fn translate(dx: f64, dy: f64) -> Affine {
        Affine { m: [1.0, 0.0, 0.0, 1.0], t: [dx, dy] }
    }
    /// Rotation by `deg` counter-clockwise about the origin.
    pub fn rotate(deg: f64) -> Affine {
        let (s, c) = deg.to_radians().sin_cos();
        Affine { m: [c, s, -s, c], t: [0.0, 0.0] }
    }
    pub fn scale(sx: f64, sy: f64) -> Affine {
        Affine { m: [sx, 0.0, 0.0, sy], t: [0.0, 0.0] }
    }
    /// Reflection across the line through `a` and `b`.
    pub fn mirror(a: P, b: P) -> Affine {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let (ux, uy) = (dx / l2.sqrt(), dy / l2.sqrt());
        // reflection matrix 2 u u^T - I
        let m = [2.0 * ux * ux - 1.0, 2.0 * ux * uy, 2.0 * ux * uy, 2.0 * uy * uy - 1.0];
        let lin = Affine { m, t: [0.0, 0.0] };
        let ta = lin.linear(a);
        Affine { m, t: [a[0] - ta[0], a[1] - ta[1]] }
    }
    /// True when the map preserves angles and scales lengths by one factor (rotation, mirror, uniform scale).
    pub fn is_similarity(&self) -> Option<f64> {
        let [a, b, c, d] = self.m;
        let s1 = (a * a + b * b).sqrt();
        let s2 = (c * c + d * d).sqrt();
        let orth = (a * c + b * d).abs() <= 1e-9 * s1 * s2;
        if s1 > 0.0 && (s1 - s2).abs() <= 1e-9 * s1 && orth { Some(s1) } else { None }
    }
}

/// Ellipse sample: centre `c`, major axis vector `a` (from the centre), minor/major ratio, parametric angle `t`.
pub fn ellipse_at(c: P, a: P, ratio: f64, t: f64) -> P {
    let bx = -a[1] * ratio;
    let by = a[0] * ratio;
    let (s, co) = t.sin_cos();
    [c[0] + a[0] * co + bx * s, c[1] + a[1] * co + by * s]
}

/// Closed-form check helper used by tests: the complete elliptic integral of the second kind E(k).
pub fn ellipse_perimeter_series(a: f64, b: f64) -> f64 {
    // Ramanujan's approximation, accurate to about 1e-6 relative for b/a in (0, 1].
    let h = ((a - b) / (a + b)).powi(2);
    std::f64::consts::PI * (a + b) * (1.0 + 3.0 * h / (10.0 + (4.0 - 3.0 * h).sqrt()))
}
