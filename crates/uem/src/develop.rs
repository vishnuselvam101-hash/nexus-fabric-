//! Surface developments (Spec 02 §4.1–§4.4). All radii are *reference-surface*
//! radii supplied by the caller: thickness and process allowances are NOT
//! modelled in this build, which is recorded as an assumption on every pattern.

use crate::{
    Curve, DevClass, Geom, Layer, MARKING_SIDE, P2, Pattern, TAU, UemError, finite_pos, gauss,
};

const ASSUME_REF: &str = "Radii are taken as reference-surface (neutral) radii exactly as entered; thickness, neutral-factor, weld gap, shrinkage and kerf are not modelled.";
const ASSUME_MODEL: &str = "Mathematical model from Specification 02 (provisional; not independently reviewed; evidence level EV-1).";

/// A developed sector at or beyond this fraction of a full turn is refused for every conical family:
/// its seam edges would touch or overlap. Implementation limit (no seam clearance rule exists in the
/// specification); it is the value the eccentric reducer always used.
pub const FULL_TURN_LIMIT: f64 = 0.999;

/// Polyline segments per closed curve, and samples per segment used for the deviation estimate.
const SEGMENTS: usize = 720;
const SUB: usize = 4;

/// Largest distance from the exact samples strictly inside each segment to its chord, over all
/// segments. `all` holds SUB samples per segment (index i*SUB is a vertex). An ESTIMATE of the
/// representation error: the true maximum may lie between samples.
fn chord_deviation(all: &[P2]) -> f64 {
    let mut worst: f64 = 0.0;
    for seg in 0..(all.len() - 1) / SUB {
        let (a, b) = (all[seg * SUB], all[(seg + 1) * SUB]);
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = dx.hypot(dy);
        for k in 1..SUB {
            let q = all[seg * SUB + k];
            let d = if len > 0.0 {
                ((q.x - a.x) * dy - (q.y - a.y) * dx).abs() / len
            } else {
                q.dist(a)
            };
            worst = worst.max(d);
        }
    }
    worst
}

fn line(name: &'static str, a: P2, b: P2) -> Curve {
    Curve {
        name,
        layer: Layer::CutOuter,
        geom: Geom::Line { a, b },
    }
}

/// Plain cylindrical shell: rectangle 2*pi*r_dev by length. Spec 02 §4.1.
pub fn cylinder_shell(r_dev: f64, length: f64) -> Result<Pattern, UemError> {
    finite_pos("r_dev", r_dev)?;
    finite_pos("length", length)?;
    let w = TAU * r_dev;
    let (p0, p1, p2, p3) = (
        P2::new(0.0, 0.0),
        P2::new(w, 0.0),
        P2::new(w, length),
        P2::new(0.0, length),
    );
    Ok(Pattern {
        family: "cylinder_shell",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactClosedForm,
        outline: vec![
            line("bottom", p0, p1),
            line("seam_b", p1, p2),
            line("top", p2, p3),
            line("seam_a", p3, p0),
        ],
        holes: vec![],
        eps_num: 4.0 * f64::EPSILON * w.max(length),
        eps_rep: 0.0,
        quantities: vec![
            ("developed_width", w),
            ("length", length),
            ("area", w * length),
        ],
        assumptions: vec![ASSUME_REF, ASSUME_MODEL],
    })
}

/// Height of a mitre cut above the segment mid-plane (Spec 02 §4.1):
/// v(theta) = (rb - rc*cos(theta)) * tan(gamma), theta = 0 at the intrados.
pub fn mitre_cut_height(rb: f64, rc: f64, gamma: f64, theta: f64) -> f64 {
    (rb - rc * theta.cos()) * gamma.tan()
}

/// Right circular cone frustum (concentric reducer). Spec 02 §4.2.
/// r1 > r2 > 0 reference radii at the end planes, h axial height.
pub fn cone_frustum(r1: f64, r2: f64, h: f64) -> Result<Pattern, UemError> {
    finite_pos("r1", r1)?;
    finite_pos("r2", r2)?;
    finite_pos("h", h)?;
    if r1 <= r2 {
        return Err(UemError::OutOfRange {
            param: "r1",
            reason: "requires r1 > r2 (large end first); equal radii is a cylinder".into(),
        });
    }
    let s = h.hypot(r1 - r2);
    let sin_a = (r1 - r2) / s;
    if sin_a < 1e-3 {
        return Err(UemError::OutOfRange { param: "r1-r2", reason: "near-cylindrical cone: apex-based formula is ill-conditioned and the apex-free method is not implemented in this build".into() });
    }
    let (l1, l2) = (r1 / sin_a, r2 / sin_a);
    let phi = TAU * sin_a;
    // Same rule, same limit and same words as the eccentric reducer: a sector within 0.1 % of a full turn
    // closes on itself (its seam edges touch), which is not a producible pattern.
    if phi >= TAU * FULL_TURN_LIMIT {
        return Err(UemError::NotDevelopable(format!(
            "developed angle {phi:.6} rad would overlap"
        )));
    }
    let c = P2::new(0.0, 0.0);
    let outline = vec![
        Curve {
            name: "large_end",
            layer: Layer::CutOuter,
            geom: Geom::Arc {
                c,
                r: l1,
                a0: 0.0,
                a1: phi,
            },
        },
        line("seam_b", P2::polar(l1, phi), P2::polar(l2, phi)),
        Curve {
            name: "small_end",
            layer: Layer::CutOuter,
            geom: Geom::Arc {
                c,
                r: l2,
                a0: phi,
                a1: 0.0,
            },
        },
        line("seam_a", P2::polar(l2, 0.0), P2::polar(l1, 0.0)),
    ];
    Ok(Pattern {
        family: "cone_frustum",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactClosedForm,
        outline,
        holes: vec![],
        eps_num: 8.0 * f64::EPSILON * l1,
        eps_rep: 0.0,
        quantities: vec![
            ("slant_length", s),
            ("sin_half_angle", sin_a),
            ("apex_radius_large", l1),
            ("apex_radius_small", l2),
            ("sector_angle", phi),
            ("area", 0.5 * phi * (l1 * l1 - l2 * l2)),
            ("chord_large_arc", 2.0 * l1 * (phi / 2.0).sin()),
        ],
        assumptions: vec![ASSUME_REF, ASSUME_MODEL],
    })
}

/// Geometry of an oblique circular cone frustum between parallel planes.
#[derive(Clone, Copy, Debug)]
pub struct ObliqueCone {
    pub r1: f64,
    pub r2: f64,
    pub h: f64,
    pub e: f64,
}

impl ObliqueCone {
    /// Apex (a, 0, hh).
    pub fn apex(&self) -> (f64, f64) {
        let k = self.r1 / (self.r1 - self.r2);
        (self.e * k, self.h * k)
    }
    pub fn rho(&self, th: f64) -> f64 {
        let (a, hh) = self.apex();
        (self.r1 * self.r1 - 2.0 * a * self.r1 * th.cos() + a * a + hh * hh).sqrt()
    }
    /// d(psi)/d(theta), Spec 02 eq. (4.1).
    pub fn dpsi(&self, th: f64) -> f64 {
        let (a, hh) = self.apex();
        let d = self.r1 * self.r1 - 2.0 * a * self.r1 * th.cos() + a * a + hh * hh;
        self.r1 * (hh * hh + (self.r1 - a * th.cos()).powi(2)).sqrt() / d
    }
}

/// Developed samples of the oblique cone: (theta, rho, psi) on a uniform grid
/// with `2*n` intervals, plus the quadrature error estimate.
pub fn oblique_cone_samples(g: &ObliqueCone, n: usize) -> (Vec<(f64, f64, f64)>, f64) {
    let m = 2 * n;
    let dth = TAU / m as f64;
    let mut out = Vec::with_capacity(m + 1);
    let (mut psi, mut err) = (0.0, 0.0);
    out.push((0.0, g.rho(0.0), 0.0));
    for i in 0..m {
        let (a, b) = (i as f64 * dth, (i + 1) as f64 * dth);
        let (v, e) = gauss(&|t| g.dpsi(t), a, b, 1);
        psi += v;
        err += e;
        out.push((b, g.rho(b), psi));
    }
    (out, err)
}

/// Oblique circular cone frustum (eccentric reducer). Spec 02 §4.3.
/// Seam at theta = 0 (the side toward which the small end is offset).
pub fn eccentric_reducer(r1: f64, r2: f64, h: f64, e: f64) -> Result<Pattern, UemError> {
    finite_pos("r1", r1)?;
    finite_pos("r2", r2)?;
    finite_pos("h", h)?;
    if !e.is_finite() || e < 0.0 {
        return Err(UemError::OutOfRange {
            param: "e",
            reason: "offset must be finite and >= 0".into(),
        });
    }
    if r1 <= r2 {
        return Err(UemError::OutOfRange { param: "r1", reason: "requires r1 > r2; equal radii with offset (oblique cylinder) is not implemented in this build".into() });
    }
    if (r1 - r2) / h.hypot(r1 - r2) < 1e-3 {
        return Err(UemError::OutOfRange {
            param: "r1-r2",
            reason: "near-cylindrical: apex-free method not implemented in this build".into(),
        });
    }
    let g = ObliqueCone { r1, r2, h, e };
    let n = 360;
    let (s, qerr) = oblique_cone_samples(&g, n);
    let psi_total = s[2 * n].2;
    if psi_total >= TAU * FULL_TURN_LIMIT {
        return Err(UemError::NotDevelopable(format!(
            "developed angle {psi_total:.6} rad would overlap"
        )));
    }
    let k = r2 / r1;
    let outer_all: Vec<P2> = s.iter().map(|&(_, rho, psi)| P2::polar(rho, psi)).collect();
    // even samples are polyline vertices; odd samples measure the chord deviation
    let mut eps_rep: f64 = 0.0;
    let mut outer = Vec::with_capacity(n + 1);
    for i in 0..=n {
        outer.push(outer_all[2 * i]);
        if i < n {
            let (a, b, mid) = (outer_all[2 * i], outer_all[2 * i + 2], outer_all[2 * i + 1]);
            let chord_mid = P2::new(0.5 * (a.x + b.x), 0.5 * (a.y + b.y));
            eps_rep = eps_rep.max(mid.dist(chord_mid));
        }
    }
    let inner: Vec<P2> = outer
        .iter()
        .rev()
        .map(|p| P2::new(p.x * k, p.y * k))
        .collect(); // traversed back toward theta = 0
    let rho_max = s.iter().map(|x| x.1).fold(0.0, f64::max);
    let area = {
        let (v, _) = gauss(&|t| 0.5 * g.rho(t).powi(2) * g.dpsi(t), 0.0, TAU, 720);
        v * (1.0 - k * k)
    };
    let outline = vec![
        Curve {
            name: "large_end",
            layer: Layer::CutOuter,
            geom: Geom::Polyline {
                pts: outer.clone(),
                eps_rep,
            },
        },
        line("seam_b", outer[n], inner[0]),
        Curve {
            name: "small_end",
            layer: Layer::CutOuter,
            geom: Geom::Polyline {
                pts: inner.clone(),
                eps_rep: eps_rep * k,
            },
        },
        line("seam_a", inner[n], outer[0]),
    ];
    Ok(Pattern {
        family: "eccentric_reducer",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactNumeric,
        outline,
        holes: vec![],
        eps_num: qerr * rho_max + 16.0 * f64::EPSILON * rho_max,
        eps_rep,
        quantities: vec![
            ("developed_angle", psi_total),
            ("rho_at_0", g.rho(0.0)),
            ("rho_at_180", g.rho(std::f64::consts::PI)),
            ("area", area),
        ],
        assumptions: vec![
            ASSUME_REF,
            ASSUME_MODEL,
            "Wall inclination varies around an oblique cone; a uniform reference-surface offset is a simplification (Spec 02 research item R-05).",
            "Seam placed at theta = 0; no seam-placement rule applied.",
        ],
    })
}

/// Cylinder-on-cylinder branch, upper sheet. Spec 02 eq. (4.3).
#[derive(Clone, Copy, Debug)]
pub struct Branch {
    /// Header contact radius.
    pub big_r: f64,
    /// Branch contact radius.
    pub r: f64,
    /// Angle between axes (rad), 0 < beta < pi.
    pub beta: f64,
    /// Offset of branch axis from header axis.
    pub delta: f64,
}

impl Branch {
    pub fn s(&self, phi: f64) -> f64 {
        let y = self.delta + self.r * phi.cos();
        ((self.big_r * self.big_r - y * y).sqrt() - self.r * phi.sin() * self.beta.cos())
            / self.beta.sin()
    }
    /// 3D point on the branch surface (header axis = x axis).
    pub fn point(&self, phi: f64, s: f64) -> [f64; 3] {
        let (sb, cb) = self.beta.sin_cos();
        [
            s * cb - self.r * phi.sin() * sb,
            self.delta + self.r * phi.cos(),
            s * sb + self.r * phi.sin() * cb,
        ]
    }
}

/// Branch stub pattern: square end at distance `s_end` along the branch axis,
/// saddle cut against the header. `r_dev` is the branch development radius.
pub fn branch_stub(
    big_r: f64,
    r: f64,
    r_dev: f64,
    beta_deg: f64,
    delta: f64,
    s_end: f64,
) -> Result<Pattern, UemError> {
    finite_pos("header_radius", big_r)?;
    finite_pos("branch_radius", r)?;
    finite_pos("branch_dev_radius", r_dev)?;
    finite_pos("s_end", s_end)?;
    if !delta.is_finite() {
        return Err(UemError::OutOfRange {
            param: "delta",
            reason: "must be finite".into(),
        });
    }
    let beta = beta_deg.to_radians();
    if !(beta.sin() > 0.17 && beta_deg > 0.0 && beta_deg < 180.0) {
        return Err(UemError::OutOfRange { param: "beta", reason: "angle between axes must be within about 10..170 deg in this build (grazing intersections are ill-conditioned; limit is an implementation choice, not an engineering rule)".into() });
    }
    if delta.abs() + r >= big_r * (1.0 - 1e-6) {
        return Err(UemError::OutOfRange { param: "delta+r", reason: "requires |delta| + r < R: tangent and partial intersections (including the equal-diameter tee) are not implemented in this build".into() });
    }
    let b = Branch {
        big_r,
        r,
        beta,
        delta,
    };
    let n = SEGMENTS;
    let dphi = TAU / (SUB * n) as f64;
    let sv: Vec<f64> = (0..=SUB * n).map(|i| b.s(i as f64 * dphi)).collect();
    let s_max = sv.iter().cloned().fold(f64::MIN, f64::max);
    let s_min = sv.iter().cloned().fold(f64::MAX, f64::min);
    if s_end <= s_max {
        return Err(UemError::OutOfRange {
            param: "s_end",
            reason: format!("square end must lie beyond the saddle: s_end > {s_max:.6}"),
        });
    }
    let w = TAU * r_dev;
    // Spec 02 §4.4: the branch pattern is (r·φ, s). With φ increasing to the right and s upward the pattern
    // normal is the outward surface normal: the view from OUTSIDE (MARKING_SIDE). The saddle cut is at the
    // bottom, the square end at v = s_end. (Until engine 0.2.0 this was drawn as (r·φ, s_end − s), its
    // mirror image, while the header plate was drawn from outside: validation defect D1.)
    let all: Vec<P2> = (0..=SUB * n)
        .map(|i| P2::new(r_dev * i as f64 * dphi, sv[i]))
        .collect();
    let eps_rep = chord_deviation(&all);
    let mut cut: Vec<P2> = (0..=n).map(|i| all[SUB * i]).collect();
    cut[n] = P2::new(w, cut[0].y); // the seam edges are the same ruling: close exactly
    let (p_top_r, p_top_l) = (P2::new(w, s_end), P2::new(0.0, s_end));
    // counter-clockwise: cut (left to right) -> right seam (up) -> square end (right to left) -> left seam (down)
    let outline = vec![
        Curve {
            name: "saddle_cut",
            layer: Layer::CutOuter,
            geom: Geom::Polyline {
                pts: cut.clone(),
                eps_rep,
            },
        },
        line("seam_b", cut[n], p_top_r),
        line("square_end", p_top_r, p_top_l),
        line("seam_a", p_top_l, cut[0]),
    ];
    Ok(Pattern {
        family: "branch_stub",
        marking_side: MARKING_SIDE,
        class: DevClass::ExactClosedForm,
        outline,
        holes: vec![],
        eps_num: 16.0 * f64::EPSILON * (big_r + s_end) / beta.sin(),
        eps_rep,
        quantities: vec![
            ("developed_width", w),
            ("s_min", s_min),
            ("s_max", s_max),
            ("s_at_0", sv[0]),
            ("s_at_90", sv[SUB * n / 4]),
        ],
        assumptions: vec![
            ASSUME_REF,
            ASSUME_MODEL,
            "Contact radii (which wall surfaces define the fit: set-on / set-in) are entered directly; no joint rule applied.",
            "Weld preparation and root gap are not modelled.",
            "Seam at branch azimuth phi = 0.",
        ],
    })
}

/// Header shell (rolled from plate) with the opening for a set-on branch.
/// Spec 02 §4.4: the opening is the same intersection curve as the branch cut,
/// developed on the header. Shell: x along the header axis in [0, length];
/// v = r_dev_header * (omega + pi), i.e. the longitudinal seam is diametrically
/// opposite the branch side. `station` is the header-axis position of the
/// point where the branch axis is closest to the header axis.
pub fn header_with_opening(
    b: &Branch,
    r_dev_header: f64,
    length: f64,
    station: f64,
) -> Result<Pattern, UemError> {
    finite_pos("header_dev_radius", r_dev_header)?;
    finite_pos("header_length", length)?;
    if !station.is_finite() {
        return Err(UemError::OutOfRange {
            param: "branch_station",
            reason: "must be finite".into(),
        });
    }
    // (written with partial_cmp so that a NaN angle is refused, exactly as `!(sin > 0.17)` did)
    if b.beta.sin().partial_cmp(&0.17) != Some(std::cmp::Ordering::Greater)
        || b.delta.abs() + b.r >= b.big_r * (1.0 - 1e-6)
    {
        return Err(UemError::OutOfRange {
            param: "branch",
            reason: "branch geometry outside the supported range (see branch stub limits)".into(),
        });
    }
    let mut shell = cylinder_shell(r_dev_header, length)?;
    // The opening is computed in the frame (x along the header axis, v around it), so the plate outline must be
    // in that same frame: length along x, developed circumference along y. Reusing the plain shell's rectangle
    // (circumference along x) put the opening in the wrong place and turned by a quarter turn; see
    // nf-core/tests/regressions/002-header-outline-frame.json.
    let w = TAU * r_dev_header;
    let (p0, p1, p2, p3) = (
        P2::new(0.0, 0.0),
        P2::new(length, 0.0),
        P2::new(length, w),
        P2::new(0.0, w),
    );
    shell.outline = vec![
        line("seam_a", p0, p1),
        line("top", p1, p2),
        line("seam_b", p2, p3),
        line("bottom", p3, p0),
    ];
    let n = SEGMENTS;
    let dphi = TAU / (SUB * n) as f64;
    // v = r_dev·(ω + π) with ω increasing from −y to +y, x along the axis: the pattern normal is the
    // outward normal of the header, the view from OUTSIDE (MARKING_SIDE), like the branch.
    let at = |i: usize| -> P2 {
        let phi = i as f64 * dphi;
        let s = b.s(phi);
        let omega = ((b.delta + b.r * phi.cos()) / b.big_r).asin();
        P2::new(
            station + s * b.beta.cos() - b.r * phi.sin() * b.beta.sin(),
            r_dev_header * (omega + std::f64::consts::PI),
        )
    };
    let all: Vec<P2> = (0..=SUB * n).map(at).collect();
    let eps_rep = chord_deviation(&all);
    let mut pts: Vec<P2> = (0..=n).map(|i| all[SUB * i]).collect();
    pts[n] = pts[0]; // close exactly
    let (xmin, xmax) = pts.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
        (lo.min(p.x), hi.max(p.x))
    });
    if xmin <= 0.0 || xmax >= length {
        return Err(UemError::OutOfRange {
            param: "branch_station",
            reason: format!(
                "opening spans x = {xmin:.3} .. {xmax:.3} and does not lie inside the shell length {length}"
            ),
        });
    }
    let area2: f64 = pts
        .windows(2)
        .map(|w| w[0].x * w[1].y - w[1].x * w[0].y)
        .sum();
    if area2 > 0.0 {
        pts.reverse(); // inner profiles are clockwise
    }
    let perimeter: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    shell.family = "header_shell_with_opening";
    shell.holes = vec![Curve {
        name: "branch_opening",
        layer: Layer::CutInner,
        geom: Geom::Polyline { pts, eps_rep },
    }];
    shell.eps_rep = eps_rep;
    shell.eps_num = shell
        .eps_num
        .max(16.0 * f64::EPSILON * (b.big_r + length) / b.beta.sin());
    shell.quantities.push(("opening_x_min", xmin));
    shell.quantities.push(("opening_x_max", xmax));
    shell.quantities.push(("opening_perimeter", perimeter));
    shell.assumptions.push("Header rolled from plate; longitudinal seam placed diametrically opposite the branch. No seam-to-opening clearance rule is applied.");
    shell.assumptions.push("Set-on joint assumed: opening defined by the branch contact surface meeting the header contact surface. Which wall surfaces these are is an engineering decision entered by the user.");
    Ok(shell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() <= tol
    }

    #[test]
    fn cyl_reference_cases() {
        let p = cylinder_shell(500.0, 1000.0).unwrap(); // CYL-02
        assert!(close(
            p.quantity("developed_width").unwrap(),
            3141.592653590,
            1e-8
        ));
        assert!(close(
            mitre_cut_height(300.0, 100.0, 15f64.to_radians(), 0.0),
            53.58983848622,
            1e-10
        )); // MITRE-01
        assert!(close(
            mitre_cut_height(300.0, 100.0, 15f64.to_radians(), PI / 2.0),
            80.38475772934,
            1e-10
        ));
        assert!(close(
            mitre_cut_height(300.0, 100.0, 15f64.to_radians(), PI),
            107.1796769724,
            1e-9
        ));
        assert!(cylinder_shell(-1.0, 10.0).is_err());
    }

    #[test]
    fn cone_reference_case_cone_01() {
        let p = cone_frustum(200.0, 100.0, 300.0).unwrap();
        assert!(close(
            p.quantity("slant_length").unwrap(),
            316.2277660168,
            1e-9
        ));
        assert!(close(
            p.quantity("apex_radius_large").unwrap(),
            632.4555320337,
            1e-9
        ));
        assert!(close(
            p.quantity("sector_angle").unwrap(),
            1.986917653159,
            1e-11
        ));
        assert!(close(p.quantity("area").unwrap(), 298037.6479739, 1e-6));
        assert!(close(
            p.quantity("chord_large_arc").unwrap(),
            1059.892757121,
            1e-8
        ));
    }

    #[test]
    fn cone_degenerate_and_conditioning() {
        assert!(cone_frustum(100.0, 100.0, 300.0).is_err());
        assert!(matches!(
            cone_frustum(200.0, 199.9, 300.0),
            Err(UemError::OutOfRange { .. })
        )); // CONE-02 refused
        assert!(cone_frustum(200.0, 0.0, 300.0).is_err()); // DG-01
        assert!(cone_frustum(200.0, 100.0, 0.0).is_err()); // DG-02
    }

    #[test]
    fn eccentric_reference_case_ecc_01() {
        let p = eccentric_reducer(200.0, 100.0, 300.0, 100.0).unwrap();
        assert!(close(
            p.quantity("developed_angle").unwrap(),
            1.861891289225,
            1e-10
        ));
        assert!(close(p.quantity("rho_at_0").unwrap(), 600.0, 1e-10));
        assert!(close(
            p.quantity("rho_at_180").unwrap(),
            721.1102550928,
            1e-9
        ));
        assert!(close(p.quantity("area").unwrap(), 304679.8420026, 1e-5));
        assert!(p.eps_rep > 0.0 && p.eps_rep < 0.05);
        let g = ObliqueCone {
            r1: 200.0,
            r2: 100.0,
            h: 300.0,
            e: 100.0,
        };
        let (s, _) = oblique_cone_samples(&g, 360);
        assert!(close(s[180].2, 0.4919675896291, 1e-11)); // psi(90 deg)
        assert!(close(s[360].2, 0.9309456446123, 1e-11)); // psi(180 deg)
    }

    #[test]
    fn eccentric_tends_to_right_cone() {
        // ECC-02: zero offset reproduces CONE-01
        let p = eccentric_reducer(200.0, 100.0, 300.0, 0.0).unwrap();
        assert!(close(
            p.quantity("developed_angle").unwrap(),
            1.986917653159,
            1e-10
        ));
        assert!(close(p.quantity("area").unwrap(), 298037.6479739, 1e-5));
    }

    #[test]
    fn branch_reference_cases() {
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: PI / 2.0,
            delta: 0.0,
        }; // BR-01
        assert!(close(b.s(0.0), 111.803398875, 1e-9));
        assert!(close(b.s(PI / 4.0), 132.2875655532, 1e-9));
        assert!(close(b.s(PI / 2.0), 150.0, 1e-9));
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 0.0,
        }; // BR-03
        assert!(close(b.s(0.0), 129.0994448736, 1e-9));
        assert!(close(b.s(PI / 4.0), 111.9276941188, 1e-9));
        assert!(close(b.s(1.5 * PI), 230.9401076759, 1e-9));
        let b = Branch {
            big_r: 150.0,
            r: 80.0,
            beta: PI / 2.0,
            delta: 40.0,
        }; // BR-04
        assert!(close(b.s(0.0), 90.0, 1e-9));
        assert!(close(b.s(0.75 * PI), 149.0821364201, 1e-9));
    }

    #[test]
    fn header_opening_is_inside_shell_and_clockwise() {
        let b = Branch {
            big_r: 150.0,
            r: 100.0,
            beta: 60f64.to_radians(),
            delta: 0.0,
        };
        let p = header_with_opening(&b, 150.0, 1200.0, 600.0).unwrap();
        assert_eq!(p.holes.len(), 1);
        let r = crate::profile::check(&p.holes, 1e-9);
        assert!(r.closed && r.self_intersections == 0 && r.signed_area < 0.0);
        assert!(
            p.quantity("opening_x_min").unwrap() > 0.0
                && p.quantity("opening_x_max").unwrap() < 1200.0
        );
        assert!(
            header_with_opening(&b, 150.0, 300.0, 100.0).is_err(),
            "opening outside the shell must be refused"
        );
    }

    #[test]
    fn branch_limits() {
        assert!(branch_stub(150.0, 100.0, 100.0, 90.0, 60.0, 400.0).is_err()); // BR-02 partial intersection
        assert!(branch_stub(100.0, 100.0, 100.0, 90.0, 0.0, 400.0).is_err()); // BR-05 equal tee not implemented
        assert!(branch_stub(150.0, 100.0, 100.0, 5.0, 0.0, 4000.0).is_err()); // grazing
        assert!(branch_stub(150.0, 100.0, 100.0, 90.0, 0.0, 100.0).is_err()); // square end inside saddle
        let p = branch_stub(150.0, 100.0, 100.0, 60.0, 0.0, 400.0).unwrap();
        assert!(close(
            p.quantity("developed_width").unwrap(),
            628.318530718,
            1e-9
        ));
    }

    /// Validation defect D6: both conical families refuse a sector that closes on itself, with the same rule.
    #[test]
    fn d6_right_cone_refuses_a_full_turn_like_the_eccentric_reducer() {
        for h in [0.01, 1e-8] {
            let e = cone_frustum(300.0, 50.0, h).unwrap_err();
            assert!(
                matches!(&e, UemError::NotDevelopable(m) if m.contains("would overlap")),
                "{e:?}"
            );
            let e = eccentric_reducer(300.0, 50.0, h, 0.0).unwrap_err();
            assert!(
                matches!(&e, UemError::NotDevelopable(m) if m.contains("would overlap")),
                "{e:?}"
            );
        }
        // just inside the limit both still develop; just beyond both refuse
        let h_at = |turn: f64| 250.0 * (1.0 - turn * turn).sqrt() / turn; // sin(alpha) = turn
        assert!(
            cone_frustum(300.0, 50.0, h_at(0.9985)).is_ok()
                && eccentric_reducer(300.0, 50.0, h_at(0.9985), 0.0).is_ok()
        );
        assert!(
            cone_frustum(300.0, 50.0, h_at(0.9995)).is_err()
                && eccentric_reducer(300.0, 50.0, h_at(0.9995), 0.0).is_err()
        );
    }

    /// Validation defect D3: the representation estimate near tangency. It is an estimate (three interior
    /// samples per segment); this measures it against a dense sampling of the exact curve.
    #[test]
    fn d3_representation_estimate_near_tangency_is_close_to_the_dense_truth() {
        for (r, mid_only_ratio) in [(150.0 * (1.0 - 1.1e-6), 1.21), (150.0 * (1.0 - 1e-5), 1.04)] {
            let b = Branch {
                big_r: 150.0,
                r,
                beta: 45f64.to_radians(),
                delta: 0.0,
            };
            let p = branch_stub(150.0, r, r, 45.0, 0.0, 600.0).unwrap();
            // dense truth: 64 samples per segment against the chord
            let (n, m) = (SEGMENTS, 64);
            let dphi = TAU / (n * m) as f64;
            let dense: Vec<P2> = (0..=n * m)
                .map(|i| P2::new(r * i as f64 * dphi, b.s(i as f64 * dphi)))
                .collect();
            let mut truth: f64 = 0.0;
            for seg in 0..n {
                let (a, c) = (dense[seg * m], dense[(seg + 1) * m]);
                let (dx, dy) = (c.x - a.x, c.y - a.y);
                for k in 1..m {
                    let q = dense[seg * m + k];
                    truth = truth.max(((q.x - a.x) * dy - (q.y - a.y) * dx).abs() / dx.hypot(dy));
                }
            }
            let ratio = truth / p.eps_rep;
            assert!(
                ratio >= 1.0 - 1e-9,
                "an estimate from fewer samples cannot exceed the dense value"
            );
            assert!(
                ratio < 1.08,
                "estimate {} against dense {truth}: ratio {ratio} (the midpoint-only estimate was off by {mid_only_ratio})",
                p.eps_rep
            );
        }
    }
}
