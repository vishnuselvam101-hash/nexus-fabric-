//! Plane geometry for the placer. Points are (x, y) in millimetres.

pub type P = (f64, f64);

/// Tolerance for float comparisons, mm.
pub const EPS: f64 = 1e-9;

pub fn area(p: &[P]) -> f64 {
    let n = p.len();
    let mut s = 0.0;
    for i in 0..n {
        let (a, b) = (p[i], p[(i + 1) % n]);
        s += a.0 * b.1 - b.0 * a.1;
    }
    s.abs() / 2.0
}

pub fn bbox(p: &[P]) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in p {
        b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
    }
    b
}

/// Counter-clockwise, bbox minimum moved to the origin, no repeated closing point.
pub fn normalise(p: &[P]) -> Vec<P> {
    let mut v: Vec<P> = p.to_vec();
    if v.len() > 1
        && (v[0].0 - v[v.len() - 1].0).abs() < EPS
        && (v[0].1 - v[v.len() - 1].1).abs() < EPS
    {
        v.pop();
    }
    let signed: f64 = (0..v.len())
        .map(|i| {
            let (a, b) = (v[i], v[(i + 1) % v.len()]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum();
    if signed < 0.0 {
        v.reverse();
    }
    let (x0, y0, _, _) = bbox(&v);
    v.iter().map(|&(x, y)| (x - x0, y - y0)).collect()
}

/// Rotation about the origin by a multiple of 90 degrees (exact for the four steps).
pub fn rotate(p: &[P], deg: u32) -> Vec<P> {
    p.iter()
        .map(|&(x, y)| match deg % 360 {
            0 => (x, y),
            90 => (-y, x),
            180 => (-x, -y),
            270 => (y, -x),
            _ => panic!("rotation must be 0, 90, 180 or 270"),
        })
        .collect()
}

/// The placed shape: rotate, then move so the bounding box starts at (x, y).
pub fn place(normalised: &[P], rot: u32, at: P) -> Vec<P> {
    let r = rotate(normalised, rot);
    let (x0, y0, _, _) = bbox(&r);
    r.iter()
        .map(|&(x, y)| (x - x0 + at.0, y - y0 + at.1))
        .collect()
}

fn seg_dist(a: P, b: P, c: P, d: P) -> f64 {
    fn pt_seg(p: P, a: P, b: P) -> f64 {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l2 = dx * dx + dy * dy;
        let t = if l2 == 0.0 {
            0.0
        } else {
            (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
        };
        (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
    }
    let cross = |o: P, p: P, q: P| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    pt_seg(a, c, d)
        .min(pt_seg(b, c, d))
        .min(pt_seg(c, a, b))
        .min(pt_seg(d, a, b))
}

/// Smallest distance between the boundaries of two polygons (0 when they cross or touch).
pub fn boundary_dist(a: &[P], b: &[P]) -> f64 {
    let mut m = f64::MAX;
    for i in 0..a.len() {
        for j in 0..b.len() {
            m = m.min(seg_dist(
                a[i],
                a[(i + 1) % a.len()],
                b[j],
                b[(j + 1) % b.len()],
            ));
        }
    }
    m
}

/// Ray-casting point in polygon (the point is assumed not on the boundary).
pub fn inside(p: P, poly: &[P]) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

pub fn bbox_gap_ok(a: &[P], b: &[P], clearance: f64) -> bool {
    let (ax0, ay0, ax1, ay1) = bbox(a);
    let (bx0, by0, bx1, by1) = bbox(b);
    ax1 + clearance <= bx0
        || bx1 + clearance <= ax0
        || ay1 + clearance <= by0
        || by1 + clearance <= ay0
}

/// True when two placed shapes keep at least `clearance` between their boundaries and do not overlap.
pub fn clear(a: &[P], b: &[P], clearance: f64) -> bool {
    if bbox_gap_ok(a, b, clearance) {
        return true;
    }
    boundary_dist(a, b) >= clearance - EPS && !inside(a[0], b) && !inside(b[0], a)
}
