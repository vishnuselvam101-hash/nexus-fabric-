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

/// Area centroid of a simple polygon.
pub fn centroid(p: &[P]) -> P {
    let n = p.len();
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let (x0, y0) = p[i];
        let (x1, y1) = p[(i + 1) % n];
        let cr = x0 * y1 - x1 * y0;
        a += cr;
        cx += (x0 + x1) * cr;
        cy += (y0 + y1) * cr;
    }
    if a.abs() < EPS {
        return p.iter().fold((0.0, 0.0), |s, q| {
            (s.0 + q.0 / n as f64, s.1 + q.1 / n as f64)
        });
    }
    (cx / (3.0 * a), cy / (3.0 * a))
}

/// Counter-clockwise, area centroid at the origin, no repeated closing point. This is the frame jagua-rs
/// uses: a placement is `rotate(centred, rot) + t` with `t` the centroid's position on the sheet.
pub fn centre(p: &[P]) -> Vec<P> {
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
    let c = centroid(&v);
    v.iter().map(|&(x, y)| (x - c.0, y - c.1)).collect()
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

/// Placement by pose: the centred outline turned by `rot` and moved to centroid position `t`.
pub fn place(centred: &[P], rot: u32, t: P) -> Vec<P> {
    rotate(centred, rot)
        .iter()
        .map(|&(x, y)| (x + t.0, y + t.1))
        .collect()
}

/// Placement by bounding-box corner: the pose `t` whose shape starts at `corner`, and the shape.
pub fn place_corner(centred: &[P], rot: u32, corner: P) -> (P, Vec<P>) {
    let r = rotate(centred, rot);
    let (x0, y0, _, _) = bbox(&r);
    let t = (corner.0 - x0, corner.1 - y0);
    let pts = r.iter().map(|&(x, y)| (x + t.0, y + t.1)).collect();
    (t, pts)
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
