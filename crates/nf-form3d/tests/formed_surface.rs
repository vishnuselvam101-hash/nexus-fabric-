//! Acceptance tests of A-20 (nf-form3d). The patterns are the engine's own output (`uem::develop`) for the
//! parameters of stored compatibility fixtures in crates/nf-core/tests/fixtures/compat_0.4.0.json:
//! entry i = 0 (cylinder_shell, r_dev 100, length 250), entry i = 3 (cone_frustum, r1 200, r2 100, h 300) and
//! entry i = 89 (cone_frustum, r1 230.98152483783582, r2 166.7257557310576, h 39.05917234694148, whose developed
//! angle is above pi, so the seam wrap of the map is exercised).
//!
//! Tolerances are 1e-9 times the largest dimension M of the formed part (mm).

use nf_form3d::{Form3d, Form3dError};
use uem::{Geom, P2, Pattern, TAU, develop};

#[derive(Clone, Copy, Debug)]
enum Dims {
    Cylinder { radius: f64, length: f64 },
    Cone { r1: f64, r2: f64, h: f64 },
}

struct Case {
    pattern: Pattern,
    form: Form3d,
    dims: Dims,
}

impl Case {
    fn tol(&self) -> f64 {
        let m = match self.dims {
            Dims::Cylinder { radius, length } => (2.0 * radius).max(length),
            Dims::Cone { r1, h, .. } => (2.0 * r1).max(h),
        };
        1e-9 * m
    }
    fn quantity(&self, name: &str) -> f64 {
        self.pattern.quantity(name).unwrap()
    }
}

fn cases() -> Vec<Case> {
    let cyl = develop::cylinder_shell(100.0, 250.0).unwrap();
    let cone = develop::cone_frustum(200.0, 100.0, 300.0).unwrap();
    let steep =
        develop::cone_frustum(230.98152483783582, 166.7257557310576, 39.05917234694148).unwrap();
    vec![
        Case {
            form: Form3d::from_pattern(&cyl).unwrap(),
            pattern: cyl,
            dims: Dims::Cylinder {
                radius: 100.0,
                length: 250.0,
            },
        },
        Case {
            form: Form3d::from_pattern(&cone).unwrap(),
            pattern: cone,
            dims: Dims::Cone {
                r1: 200.0,
                r2: 100.0,
                h: 300.0,
            },
        },
        Case {
            form: Form3d::from_pattern(&steep).unwrap(),
            pattern: steep,
            dims: Dims::Cone {
                r1: 230.98152483783582,
                r2: 166.7257557310576,
                h: 39.05917234694148,
            },
        },
    ]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Sum of the signed angles between consecutive points seen from the axis (the swept azimuth, radians).
fn swept_angle(pts: &[[f64; 3]]) -> f64 {
    pts.windows(2)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            (a[0] * b[1] - a[1] * b[0]).atan2(a[0] * b[0] + a[1] * b[1])
        })
        .sum()
}

fn sample(geom: &Geom, n: usize) -> Vec<P2> {
    let t = |i: usize| i as f64 / n as f64;
    match geom {
        Geom::Line { a, b } => (0..=n)
            .map(|i| P2::new(a.x + t(i) * (b.x - a.x), a.y + t(i) * (b.y - a.y)))
            .collect(),
        Geom::Arc { c, r, a0, a1 } => {
            let (c, r, a0, a1) = (*c, *r, *a0, *a1);
            (0..=n)
                .map(|i| {
                    let a = a0 + t(i) * (a1 - a0);
                    P2::new(c.x + r * a.cos(), c.y + r * a.sin())
                })
                .collect()
        }
        Geom::Polyline { pts, .. } => pts.clone(),
    }
}

fn curve(p: &Pattern, name: &str, n: usize) -> Vec<P2> {
    let c = p
        .outline
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no outline curve {name}"));
    sample(&c.geom, n)
}

fn outline(p: &Pattern, n: usize) -> Vec<P2> {
    p.outline.iter().flat_map(|c| sample(&c.geom, n)).collect()
}

/// Plate points of the developed domain on an (n+1) x (n+1) grid.
fn domain(c: &Case, n: usize) -> Vec<P2> {
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=n {
            let (s, t) = (i as f64 / n as f64, j as f64 / n as f64);
            out.push(match c.dims {
                Dims::Cylinder { .. } => {
                    P2::new(c.quantity("developed_width") * s, c.quantity("length") * t)
                }
                Dims::Cone { .. } => {
                    let (l1, l2) = (
                        c.quantity("apex_radius_large"),
                        c.quantity("apex_radius_small"),
                    );
                    P2::polar(l2 + s * (l1 - l2), c.quantity("sector_angle") * t)
                }
            });
        }
    }
    out
}

/// A point well inside the developed domain.
fn interior(c: &Case) -> P2 {
    match c.dims {
        Dims::Cylinder { .. } => P2::new(
            0.3 * c.quantity("developed_width"),
            0.6 * c.quantity("length"),
        ),
        Dims::Cone { .. } => {
            let (l1, l2) = (
                c.quantity("apex_radius_large"),
                c.quantity("apex_radius_small"),
            );
            P2::polar(l2 + 0.4 * (l1 - l2), 0.37 * c.quantity("sector_angle"))
        }
    }
}

/// The coordinate that a seam point and its partner share: v for the cylinder, the distance l from the apex for
/// the cone.
fn shared_key(c: &Case, q: P2) -> f64 {
    match c.dims {
        Dims::Cylinder { .. } => q.y,
        Dims::Cone { .. } => q.x.hypot(q.y),
    }
}

#[test]
fn seam_edges_map_to_the_same_3d_points() {
    for c in cases() {
        let tol = c.tol();
        let mut a = curve(&c.pattern, "seam_a", 64);
        let mut b = curve(&c.pattern, "seam_b", 64);
        assert_eq!(a.len(), b.len());
        a.sort_by(|p, q| shared_key(&c, *p).total_cmp(&shared_key(&c, *q)));
        b.sort_by(|p, q| shared_key(&c, *p).total_cmp(&shared_key(&c, *q)));
        for (p, q) in a.iter().zip(b.iter()) {
            assert!(
                (shared_key(&c, *p) - shared_key(&c, *q)).abs() <= tol,
                "seam partners differ in their shared coordinate: {p:?} {q:?}"
            );
            let d = dist(c.form.map(*p), c.form.map(*q));
            assert!(
                d <= tol,
                "seam points {p:?} and {q:?} map {d:e} mm apart (tol {tol:e})"
            );
        }
    }
}

#[test]
fn every_mapped_vertex_lies_on_the_declared_surface() {
    for c in cases() {
        let tol = c.tol();
        let mut pts = outline(&c.pattern, 720);
        pts.extend(domain(&c, 48));
        for uv in pts {
            let x = c.form.map(uv);
            match c.dims {
                Dims::Cylinder { radius, length } => {
                    let off = (x[0].hypot(x[1]) - radius).abs();
                    assert!(off <= tol, "{uv:?} -> {x:?} is {off:e} mm off the cylinder");
                    assert!(
                        x[2] >= -tol && x[2] <= length + tol,
                        "{uv:?} -> {x:?} outside the length"
                    );
                }
                Dims::Cone { r1, r2, h } => {
                    // the cone r(z) = r2 + (r1 - r2) z / h; the distance to it is at most the radial offset
                    let off = (x[0].hypot(x[1]) - (r2 + (r1 - r2) * x[2] / h)).abs();
                    assert!(off <= tol, "{uv:?} -> {x:?} is {off:e} mm off the cone");
                    assert!(
                        x[2] >= -tol && x[2] <= h + tol,
                        "{uv:?} -> {x:?} outside the frustum"
                    );
                }
            }
        }
    }
}

#[test]
fn closed_form_circumference_and_slant_height() {
    for c in cases() {
        let tol = c.tol();
        match c.dims {
            Dims::Cylinder { radius, .. } => {
                // the bottom edge, u from 0 to 2*pi*R, is one full turn of radius R
                let pts: Vec<[f64; 3]> = curve(&c.pattern, "bottom", 4096)
                    .into_iter()
                    .map(|q| c.form.map(q))
                    .collect();
                let turn = swept_angle(&pts).abs();
                assert!(
                    (radius * turn - TAU * radius).abs() <= tol,
                    "bottom edge sweeps {} mm, expected {} mm",
                    radius * turn,
                    TAU * radius
                );
                assert!(dist(pts[0], pts[pts.len() - 1]) <= tol);
            }
            Dims::Cone { r1, r2, h } => {
                // the large end, developed as an arc of radius l1 and angle phi, is the circle of radius r1
                let arc = match &c
                    .pattern
                    .outline
                    .iter()
                    .find(|c| c.name == "large_end")
                    .unwrap()
                    .geom
                {
                    Geom::Arc { r, a0, a1, .. } => r * (a1 - a0),
                    other => panic!("large_end is not an arc: {other:?}"),
                };
                assert!(
                    (arc - TAU * r1).abs() <= tol,
                    "developed large end {arc} mm, 2*pi*r1 {}",
                    TAU * r1
                );
                let pts: Vec<[f64; 3]> = curve(&c.pattern, "large_end", 4096)
                    .into_iter()
                    .map(|q| c.form.map(q))
                    .collect();
                let turn = swept_angle(&pts).abs();
                assert!(
                    (r1 * turn - arc).abs() <= tol,
                    "formed large end {} mm, developed {arc} mm",
                    r1 * turn
                );
                // slant height: the generatrix from the small to the large end is the straight seam
                let (l1, l2) = (
                    c.quantity("apex_radius_large"),
                    c.quantity("apex_radius_small"),
                );
                let slant = c.quantity("slant_length");
                assert!(
                    ((l1 - l2) - slant).abs() <= tol,
                    "plate generatrix {} mm, slant {slant}",
                    l1 - l2
                );
                let top = c.form.map(P2::new(l1, 0.0));
                let bottom = c.form.map(P2::new(l2, 0.0));
                assert!(
                    (dist(top, bottom) - slant).abs() <= tol,
                    "formed generatrix not the slant height"
                );
                assert!(
                    ((top[2] - bottom[2]) - h).abs() <= tol,
                    "axial height of the generatrix"
                );
                assert!(((bottom[0].hypot(bottom[1])) - r2).abs() <= tol);
                assert!(((top[0].hypot(top[1])) - r1).abs() <= tol);
            }
        }
    }
}

#[test]
fn engine_pattern_and_direct_parameters_map_alike() {
    for c in cases() {
        let direct = match c.dims {
            Dims::Cylinder { radius, length } => Form3d::cylinder(radius, length).unwrap(),
            Dims::Cone { r1, r2, h } => Form3d::cone(r1, r2, h).unwrap(),
        };
        for uv in domain(&c, 16) {
            let d = dist(direct.map(uv), c.form.map(uv));
            assert!(
                d <= c.tol(),
                "{uv:?}: direct and engine route differ by {d:e} mm"
            );
        }
    }
}

#[test]
fn plate_orientation_is_the_outward_normal() {
    // (x, y) of the plate is right-handed with the outward normal, so d(map)/dx x d(map)/dy points outward.
    for c in cases() {
        let m = match c.dims {
            Dims::Cylinder { radius, length } => (2.0 * radius).max(length),
            Dims::Cone { r1, h, .. } => (2.0 * r1).max(h),
        };
        let step = 1e-6 * m;
        let uv = interior(&c);
        let at = |dx: f64, dy: f64| c.form.map(P2::new(uv.x + dx, uv.y + dy));
        let (xp, xm, yp, ym) = (at(step, 0.0), at(-step, 0.0), at(0.0, step), at(0.0, -step));
        let du: [f64; 3] = std::array::from_fn(|k| (xp[k] - xm[k]) / (2.0 * step));
        let dv: [f64; 3] = std::array::from_fn(|k| (yp[k] - ym[k]) / (2.0 * step));
        let n = [
            du[1] * dv[2] - du[2] * dv[1],
            du[2] * dv[0] - du[0] * dv[2],
            du[0] * dv[1] - du[1] * dv[0],
        ];
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let x = c.form.map(uv);
        let expected = match c.dims {
            Dims::Cylinder { .. } => {
                let rho = x[0].hypot(x[1]);
                [x[0] / rho, x[1] / rho, 0.0]
            }
            Dims::Cone { r1, r2, h } => {
                let s = h.hypot(r1 - r2);
                let rho = x[0].hypot(x[1]);
                [h / s * x[0] / rho, h / s * x[1] / rho, -(r1 - r2) / s]
            }
        };
        let cos = (n[0] * expected[0] + n[1] * expected[1] + n[2] * expected[2]) / len;
        assert!(cos > 1.0 - 1e-6, "plate normal not outward: cos {cos}");
    }
}

#[test]
fn invalid_dimensions_are_refused() {
    assert_eq!(
        Form3d::cylinder(0.0, 1.0),
        Err(Form3dError::NotPositive("radius"))
    );
    assert_eq!(
        Form3d::cylinder(1.0, f64::NAN),
        Err(Form3dError::NotPositive("length"))
    );
    assert_eq!(
        Form3d::cone(100.0, 200.0, 10.0),
        Err(Form3dError::RadiiOrder)
    );
    assert_eq!(
        Form3d::cone(100.0, 50.0, -1.0),
        Err(Form3dError::NotPositive("h"))
    );
    let other = develop::cone_frustum(200.0, 100.0, 300.0).unwrap();
    let mut wrong = other.clone();
    wrong.family = "press_bend";
    assert_eq!(
        Form3d::from_pattern(&wrong),
        Err(Form3dError::UnsupportedFamily("press_bend"))
    );
}
