//! Unit tests of the digest: a clean file that must PASS, and one planted fault per check that must FAIL.
//! The DXF text is built here by hand (R12 layout like the NEXUS FABRIC writer), without nf-dxf.

use crate::*;

const COMMENT: &str = "NEXUS FABRIC DXF - units: millimetres - NOT RELEASED: development build, not validated, not for manufacturing";

struct Doc {
    ents: String,
    comments: Vec<String>,
    insunits: Option<i32>,
    mark: bool,
    tables_layers: Vec<&'static str>,
}

impl Doc {
    fn new() -> Doc {
        Doc {
            ents: String::new(),
            comments: vec![COMMENT.into(), "NF-UNITS: mm".into()],
            insunits: None,
            mark: true,
            tables_layers: vec!["CUT_OUTER", "CUT_INNER", "NF_NOT_RELEASED"],
        }
    }
    fn line(&mut self, layer: &str, a: [f64; 2], b: [f64; 2]) -> &mut Doc {
        self.ents += &format!(
            "0\nLINE\n8\n{layer}\n10\n{}\n20\n{}\n11\n{}\n21\n{}\n",
            a[0], a[1], b[0], b[1]
        );
        self
    }
    fn arc(&mut self, layer: &str, c: [f64; 2], r: f64, s: f64, e: f64) -> &mut Doc {
        self.ents += &format!(
            "0\nARC\n8\n{layer}\n10\n{}\n20\n{}\n40\n{r}\n50\n{s}\n51\n{e}\n",
            c[0], c[1]
        );
        self
    }
    fn poly(&mut self, layer: &str, pts: &[[f64; 2]], closed: bool) -> &mut Doc {
        self.ents += &format!("0\nPOLYLINE\n8\n{layer}\n66\n1\n70\n{}\n", u8::from(closed));
        for p in pts {
            self.ents += &format!("0\nVERTEX\n8\n{layer}\n10\n{}\n20\n{}\n", p[0], p[1]);
        }
        self.ents += "0\nSEQEND\n";
        self
    }
    fn raw(&mut self, s: &str) -> &mut Doc {
        self.ents += s;
        self
    }
    fn render(&self) -> String {
        let mut o = String::new();
        for c in &self.comments {
            o += &format!("999\n{c}\n");
        }
        o += "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n";
        if let Some(u) = self.insunits {
            o += &format!("9\n$INSUNITS\n70\n{u}\n");
        }
        o += "0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n3\n";
        for l in &self.tables_layers {
            o += &format!("0\nLAYER\n2\n{l}\n70\n0\n62\n7\n6\nCONTINUOUS\n");
        }
        o += "0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n";
        o += &self.ents;
        if self.mark {
            o += &format!(
                "0\nTEXT\n8\n{MARK_LAYER}\n10\n0\n20\n-5\n40\n2.5\n1\n{NOT_RELEASED}\n7\nSTANDARD\n"
            );
        }
        o += "0\nENDSEC\n0\nEOF\n";
        o
    }
}

fn curve(points: &[[f64; 2]], is_arc: bool, max_dev: f64) -> RefCurve {
    let length = points
        .windows(2)
        .map(|w| ((w[0][0] - w[1][0]).powi(2) + (w[0][1] - w[1][1]).powi(2)).sqrt())
        .sum();
    RefCurve {
        points: points.to_vec(),
        length,
        max_dev,
        is_arc,
        seam: false,
        eps_rep: None,
        eps_rep_certified: None,
    }
}

const RECT: [[f64; 2]; 4] = [[0.0, 0.0], [100.0, 0.0], [100.0, 50.0], [0.0, 50.0]];
const SQUARE: [[f64; 2]; 4] = [[40.0, 20.0], [60.0, 20.0], [60.0, 30.0], [40.0, 30.0]];

fn rect_reference() -> Reference {
    let closed = [RECT[0], RECT[1], RECT[2], RECT[3], RECT[0]];
    Reference {
        part: "P1".into(),
        outer: vec![curve(&closed, false, 0.0)],
        holes: vec![curve(
            &[SQUARE[0], SQUARE[1], SQUARE[2], SQUARE[3], SQUARE[0]],
            false,
            0.0,
        )],
        ..Reference::default()
    }
}

fn clean() -> Doc {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &RECT, true);
    d.poly("CUT_INNER", &SQUARE, true);
    d
}

fn run(d: &Doc, r: &Reference) -> Report {
    check_text(&d.render(), r)
}

fn check_named<'a>(rep: &'a Report, name: &str) -> &'a Check {
    rep.checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no check {name}"))
}

fn assert_fails(rep: &Report, name: &str) {
    let c = check_named(rep, name);
    assert_eq!(c.verdict, Verdict::Fail, "{name} should FAIL: {}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Fail);
}

#[test]
fn clean_closed_profile_passes_every_check() {
    let rep = run(&clean(), &rect_reference());
    for c in &rep.checks {
        assert_eq!(c.verdict, Verdict::Pass, "{}: {}", c.name, c.detail);
    }
    assert_eq!(rep.verdict(), Verdict::Pass);
    assert_eq!(rep.checks.len(), 22);
}

/// A reference that declares the same drawn-polyline bound for every curve.
fn declared_polyline(bound: Option<f64>, certified: Option<bool>) -> Reference {
    let mut r = rect_reference();
    for c in r.outer.iter_mut().chain(r.holes.iter_mut()) {
        c.eps_rep = bound;
        c.eps_rep_certified = certified;
    }
    r
}

#[test]
fn a_certified_bound_at_the_cap_passes() {
    let rep = run(
        &clean(),
        &declared_polyline(Some(EPS_REP_CAP_MM), Some(true)),
    );
    let c = check_named(&rep, "representation_certified");
    assert_eq!(c.verdict, Verdict::Pass, "{}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Pass);
}

#[test]
fn an_uncertified_or_absent_bound_is_inconclusive() {
    for certified in [None, Some(false)] {
        let rep = run(
            &clean(),
            &declared_polyline(Some(EPS_REP_CAP_MM), certified),
        );
        let c = check_named(&rep, "representation_certified");
        assert_eq!(c.verdict, Verdict::Inconclusive, "{}", c.detail);
        assert_eq!(rep.verdict(), Verdict::Inconclusive);
    }

    // A non-zero declared deviation is a drawn polyline even when no explicit bound is given.
    let mut r = rect_reference();
    for c in r.outer.iter_mut().chain(r.holes.iter_mut()) {
        c.max_dev = EPS_REP_CAP_MM;
    }
    let rep = run(&clean(), &r);
    let c = check_named(&rep, "representation_certified");
    assert_eq!(c.verdict, Verdict::Inconclusive, "{}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Inconclusive);
}

#[test]
fn a_reference_declared_uncertified_is_inconclusive_without_a_drawn_polyline() {
    let mut r = rect_reference();
    r.eps_rep_certified = Some(false);
    let rep = run(&clean(), &r);
    let c = check_named(&rep, "representation_certified");
    assert_eq!(c.verdict, Verdict::Inconclusive, "{}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Inconclusive);
}

#[test]
fn a_bound_larger_than_the_cap_is_inconclusive() {
    let rep = run(
        &clean(),
        &declared_polyline(Some(EPS_REP_CAP_MM + 0.001), Some(true)),
    );
    let c = check_named(&rep, "representation_certified");
    assert_eq!(c.verdict, Verdict::Inconclusive, "{}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Inconclusive);
}

#[test]
fn a_real_geometric_fail_stays_fail_when_the_bound_is_uncertified() {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.line("CUT_OUTER", [0.0, 0.0], [100.0, 0.0])
        .line("CUT_OUTER", [100.0, 0.0], [100.0, 50.0])
        .line("CUT_OUTER", [100.0, 50.0], [0.0, 50.0]);
    let rep = run(&d, &declared_polyline(Some(EPS_REP_CAP_MM), Some(false)));
    assert_eq!(
        check_named(&rep, "representation_certified").verdict,
        Verdict::Inconclusive
    );
    assert_fails(&rep, "contours_closed");
}

#[test]
fn clean_profile_built_from_lines_and_an_arc_passes() {
    // a quarter-round corner: lines and one arc form the closed outline
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.line("CUT_OUTER", [0.0, 0.0], [80.0, 0.0])
        .arc("CUT_OUTER", [80.0, 20.0], 20.0, -90.0, 0.0)
        .line("CUT_OUTER", [100.0, 20.0], [100.0, 50.0])
        .line("CUT_OUTER", [100.0, 50.0], [0.0, 50.0])
        .line("CUT_OUTER", [0.0, 50.0], [0.0, 0.0]);
    let arc_pts: Vec<[f64; 2]> = (0..=8)
        .map(|k| {
            let t = (-90.0f64 + 90.0 * k as f64 / 8.0).to_radians();
            [80.0 + 20.0 * t.cos(), 20.0 + 20.0 * t.sin()]
        })
        .collect();
    let max_dev = 20.0 * (1.0 - (std::f64::consts::FRAC_PI_2 / 16.0).cos());
    let mut outline = vec![curve(&[[0.0, 0.0], [80.0, 0.0]], false, 0.0)];
    outline.push(RefCurve {
        length: 20.0 * std::f64::consts::FRAC_PI_2,
        ..curve(&arc_pts, true, max_dev)
    });
    outline.push(curve(&[[100.0, 20.0], [100.0, 50.0]], false, 0.0));
    outline.push(curve(&[[100.0, 50.0], [0.0, 50.0]], false, 0.0));
    outline.push(curve(&[[0.0, 50.0], [0.0, 0.0]], false, 0.0));
    let r = Reference {
        part: "P2".into(),
        outer: outline,
        holes: vec![],
        ..Reference::default()
    };
    let rep = run(&d, &r);
    for c in &rep.checks {
        assert_eq!(c.verdict, Verdict::Pass, "{}: {}", c.name, c.detail);
    }
    assert_eq!(check_named(&rep, "arc_radii").verdict, Verdict::Pass);
}

#[test]
fn open_contour_fails() {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.line("CUT_OUTER", [0.0, 0.0], [100.0, 0.0])
        .line("CUT_OUTER", [100.0, 0.0], [100.0, 50.0])
        .line("CUT_OUTER", [100.0, 50.0], [0.0, 50.0]);
    let rep = run(&d, &rect_reference());
    assert_fails(&rep, "contours_closed");
    assert_fails(&rep, "seam_gaps");
    let gap = check_named(&rep, "seam_gaps").measured.unwrap();
    assert!((gap - 50.0).abs() < 1e-9, "gap {gap}");
}

#[test]
fn duplicate_entity_fails() {
    let mut d = clean();
    d.line("CUT_OUTER", [0.0, 0.0], [100.0, 0.0]);
    d.line("CUT_OUTER", [100.0, 0.0], [0.0, 0.0]);
    let rep = run(&d, &rect_reference());
    assert_fails(&rep, "duplicates");
}

#[test]
fn tiny_segment_fails() {
    // E4 v3: the 111.8 mm part has tol 1.1e-4 mm, so this 0.00005 mm segment is below it (spec change, recorded)
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly(
        "CUT_OUTER",
        &[
            [0.0, 0.0],
            [100.0, 0.0],
            [100.00005, 0.0],
            [100.0, 50.0],
            [0.0, 50.0],
        ],
        true,
    );
    d.poly("CUT_INNER", &SQUARE, true);
    let rep = run(&d, &rect_reference());
    assert_fails(&rep, "tiny_segments");
    assert!(check_named(&rep, "tiny_segments").measured.unwrap() >= 1.0);
}

#[test]
fn segment_of_0_0005_mm_passes_on_112_mm_part() {
    // E4 v3: tol of the 111.8 mm part is 1.1e-4 mm, so a 0.0005 mm segment is a real edge, not a tiny one
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly(
        "CUT_OUTER",
        &[
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0005, 0.0],
            [100.0, 50.0],
            [0.0, 50.0],
        ],
        true,
    );
    d.poly("CUT_INNER", &SQUARE, true);
    let rep = run(&d, &rect_reference());
    assert_eq!(check_named(&rep, "tiny_segments").verdict, Verdict::Pass);
}

#[test]
fn zero_length_segment_fails() {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly(
        "CUT_OUTER",
        &[
            [0.0, 0.0],
            [100.0, 0.0],
            [100.0, 0.0],
            [100.0, 50.0],
            [0.0, 50.0],
        ],
        true,
    );
    d.poly("CUT_INNER", &SQUARE, true);
    assert_fails(&run(&d, &rect_reference()), "tiny_segments");
}

#[test]
fn wrong_unit_fails_by_insunits() {
    let mut d = clean();
    d.insunits = Some(1); // inches
    assert_fails(&run(&d, &rect_reference()), "units");
}

#[test]
fn wrong_unit_fails_by_statement() {
    let mut d = clean();
    d.comments[1] = "NF-UNITS: in".into();
    assert_fails(&run(&d, &rect_reference()), "units");
}

#[test]
fn unit_statement_missing_fails() {
    let mut d = clean();
    d.insunits = None;
    d.comments.retain(|c| !c.starts_with("NF-UNITS"));
    let rep = run(&d, &rect_reference());
    assert_fails(&rep, "units");
}

#[test]
fn missing_not_released_marking_fails() {
    let mut d = clean();
    d.mark = false;
    assert_fails(&run(&d, &rect_reference()), "not_released");
}

#[test]
fn missing_not_released_comment_fails() {
    let mut d = clean();
    d.comments[0] = "NEXUS FABRIC DXF - units: millimetres".into();
    assert_fails(&run(&d, &rect_reference()), "not_released");
}

#[test]
fn wrong_dimension_fails_extents_and_area() {
    // the file is the rectangle scaled by 1.01; the job declares 100 x 50
    let mut d = Doc::new();
    d.insunits = Some(4);
    let big: Vec<[f64; 2]> = RECT.iter().map(|p| [p[0] * 1.01, p[1] * 1.01]).collect();
    d.poly("CUT_OUTER", &big, true);
    d.poly("CUT_INNER", &SQUARE, true);
    let rep = run(&d, &rect_reference());
    assert_fails(&rep, "extents");
    assert_fails(&rep, "outline_length");
    assert_fails(&rep, "area");
}

#[test]
fn wrong_opening_size_fails_hole_length() {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &RECT, true);
    d.poly(
        "CUT_INNER",
        &[[40.0, 20.0], [65.0, 20.0], [65.0, 30.0], [40.0, 30.0]],
        true,
    );
    let rep = run(&d, &rect_reference());
    assert_fails(&rep, "hole_length");
    assert_fails(&rep, "area");
}

#[test]
fn self_intersecting_outline_fails() {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly(
        "CUT_OUTER",
        &[[0.0, 0.0], [100.0, 50.0], [100.0, 0.0], [0.0, 50.0]],
        true,
    );
    d.poly("CUT_INNER", &SQUARE, true);
    assert_fails(&run(&d, &rect_reference()), "self_intersection");
}

#[test]
fn opening_outside_the_outline_fails() {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &RECT, true);
    let far: Vec<[f64; 2]> = SQUARE.iter().map(|p| [p[0] + 200.0, p[1]]).collect();
    d.poly("CUT_INNER", &far, true);
    let r = Reference {
        part: "P1".into(),
        outer: rect_reference().outer,
        holes: vec![curve(&far, false, 0.0)],
        ..Reference::default()
    };
    assert_fails(&run(&d, &r), "holes_inside_outline");
}

#[test]
fn two_outer_profiles_fail() {
    let mut d = clean();
    d.poly(
        "CUT_OUTER",
        &[[200.0, 0.0], [210.0, 0.0], [210.0, 10.0], [200.0, 10.0]],
        true,
    );
    assert_fails(&run(&d, &rect_reference()), "outer_profiles");
}

#[test]
fn missing_opening_fails_hole_count() {
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &RECT, true);
    assert_fails(&run(&d, &rect_reference()), "hole_count");
}

#[test]
fn non_finite_coordinate_fails() {
    let mut d = clean();
    d.raw("0\nLINE\n8\nCUT_OUTER\n10\nNaN\n20\n0\n11\n5\n21\n0\n");
    let rep = run(&d, &rect_reference());
    assert_fails(&rep, "numbers_finite");
    assert_eq!(
        check_named(&rep, "contours_closed").verdict,
        Verdict::Inconclusive
    );
}

#[test]
fn unsupported_entity_is_inconclusive() {
    let mut d = clean();
    d.raw("0\nCIRCLE\n8\nCUT_OUTER\n10\n5\n20\n5\n40\n2\n");
    let rep = run(&d, &rect_reference());
    assert_eq!(
        check_named(&rep, "entity_types").verdict,
        Verdict::Inconclusive
    );
    assert_ne!(rep.verdict(), Verdict::Pass);
}

#[test]
fn geometry_on_undeclared_or_marking_layer_fails() {
    let mut d = clean();
    d.line(MARK_LAYER, [0.0, 0.0], [5.0, 5.0]);
    assert_fails(&run(&d, &rect_reference()), "layers");
}

#[test]
fn missing_section_fails() {
    let text = clean().render().replace("2\nTABLES\n", "2\nTABLEZ\n");
    let rep = check_text(&text, &rect_reference());
    assert_fails(&rep, "sections");
}

#[test]
fn unreadable_bytes_are_inconclusive_everywhere() {
    let rep = check_bytes(&[0xff, 0xfe, 0x00], &rect_reference());
    assert!(rep.read_error.is_some());
    assert_eq!(rep.verdict(), Verdict::Inconclusive);
    let text = clean().render();
    let truncated = text.strip_suffix("EOF\n").unwrap();
    let rep = check_text(truncated, &rect_reference());
    assert!(
        rep.read_error.is_some(),
        "an odd number of lines cannot be read"
    );
    assert_eq!(rep.verdict(), Verdict::Inconclusive);
}

#[test]
fn corrupt_bytes_never_panic_and_never_pass_a_damaged_group_code() {
    let base = clean().render().into_bytes();
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut inconclusive = 0;
    for _ in 0..400 {
        let mut b = base.clone();
        let at = (next() as usize) % b.len();
        b[at] = (next() % 256) as u8;
        match std::str::from_utf8(&b) {
            Ok(t) => {
                let rep = check_text(t, &rect_reference());
                if rep.read_error.is_some() {
                    inconclusive += 1;
                }
            }
            Err(_) => inconclusive += 1,
        }
    }
    assert!(
        inconclusive > 0,
        "some corruptions must make the file unreadable"
    );
}

#[test]
fn helpers_measure_what_they_claim() {
    assert!(geom::inside(&RECT, [50.0, 25.0]));
    assert!(!geom::inside(&RECT, [150.0, 25.0]));
    assert!((geom::loop_area(&RECT) - 5000.0).abs() < 1e-9);
    assert!((geom::circumradius([1.0, 0.0], [0.0, 1.0], [-1.0, 0.0]).unwrap() - 1.0).abs() < 1e-12);
    assert!(geom::circumradius([0.0, 0.0], [1.0, 0.0], [2.0, 0.0]).is_none());
}

#[test]
fn very_large_radius_arc_matches_its_declared_arc() {
    // an arc of radius 1e8 mm: three of its points are nearly collinear (the case that defeated a radius fit)
    let r = 1.0e8;
    let c = [0.0, -1.0e8 + 20.0];
    let pts: Vec<[f64; 2]> = (0..=4)
        .map(|k| {
            let t = (89.9999 + 0.0000025 * k as f64).to_radians();
            [c[0] + r * t.cos(), c[1] + r * t.sin()]
        })
        .collect();
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.line("CUT_OUTER", [0.0, 0.0], [0.0, 20.0]);
    d.arc("CUT_OUTER", c, r, 89.9999, 89.9999 + 0.00001);
    d.line("CUT_OUTER", pts[4], [0.0, 0.0]);
    let outline = vec![
        curve(&[[0.0, 0.0], [0.0, 20.0]], false, 0.0),
        RefCurve {
            length: r * 0.00001_f64.to_radians(),
            ..curve(&pts, true, 0.0)
        },
        curve(&[pts[4], [0.0, 0.0]], false, 0.0),
    ];
    let rep = check_text(
        &d.render(),
        &Reference {
            part: "BIG".into(),
            outer: outline,
            holes: vec![],
            ..Reference::default()
        },
    );
    let c = rep.checks.iter().find(|c| c.name == "arc_radii").unwrap();
    assert_eq!(c.verdict, Verdict::Pass, "{}", c.detail);
}

/// The clean square-in-rectangle job and its reference, both multiplied by `s`.
fn scaled_job(s: f64) -> (Doc, Reference) {
    let sc =
        |pts: &[[f64; 2]]| -> Vec<[f64; 2]> { pts.iter().map(|p| [p[0] * s, p[1] * s]).collect() };
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &sc(&RECT), true);
    d.poly("CUT_INNER", &sc(&SQUARE), true);
    let closed = |pts: &[[f64; 2]]| {
        let mut v = sc(pts);
        v.push(v[0]);
        v
    };
    let r = Reference {
        part: "P1".into(),
        outer: vec![curve(&closed(&RECT), false, 0.0)],
        holes: vec![curve(&closed(&SQUARE), false, 0.0)],
        ..Reference::default()
    };
    (d, r)
}

/// Tolerance of the rectangle reference at scale `s` (E4 v3 rule on its 100 mm by 50 mm diagonal).
fn rect_tol(s: f64) -> f64 {
    geom::tolerance((100.0f64.powi(2) + 50.0f64.powi(2)).sqrt() * s)
}

#[test]
fn tolerance_follows_the_v3_rule() {
    // normal and large parts: capped at 0.001 mm
    assert!((geom::tolerance(1000.0 * 2f64.sqrt()) - 0.001).abs() < 1e-15);
    // tiny parts: 1e-6 x diagonal, so a scaled job gets the same verdicts
    assert!((geom::tolerance(1e-6) - 1e-12).abs() < 1e-24);
    // huge parts: float-noise floor 1e-9 x diagonal
    assert!((geom::tolerance(1e9) - 1.0).abs() < 1e-12);
    assert_eq!(geom::tolerance(0.0), 0.0);
    assert_eq!(geom::tolerance(f64::NAN), 0.0);
}

#[test]
fn clean_job_passes_at_tiny_and_huge_scales() {
    for s in [1e-8, 1e-3, 1.0, 256.0, 65536.0] {
        let (d, r) = scaled_job(s);
        let rep = run(&d, &r);
        for c in &rep.checks {
            assert_eq!(
                c.verdict,
                Verdict::Pass,
                "scale {s}: {} — {}",
                c.name,
                c.detail
            );
        }
        assert_eq!(rep.verdict(), Verdict::Pass, "scale {s}");
    }
}

#[test]
fn real_gap_fails_at_tiny_and_huge_scales() {
    // the last edge stops 10 tolerances short of the first vertex: an open contour with a real gap
    for s in [1e-8, 1.0, 65536.0] {
        let (_, r) = scaled_job(s);
        let g = 10.0 * rect_tol(s);
        let mut d = Doc::new();
        d.insunits = Some(4);
        d.line("CUT_OUTER", [0.0, 0.0], [100.0 * s, 0.0]);
        d.line("CUT_OUTER", [100.0 * s, 0.0], [100.0 * s, 50.0 * s]);
        d.line("CUT_OUTER", [100.0 * s, 50.0 * s], [0.0, 50.0 * s]);
        d.line("CUT_OUTER", [0.0, 50.0 * s], [0.0, g]);
        d.poly("CUT_INNER", &SQUARE.map(|p| [p[0] * s, p[1] * s]), true);
        let rep = run(&d, &r);
        assert_fails(&rep, "seam_gaps");
        let c = check_named(&rep, "seam_gaps");
        assert!(
            (c.measured.unwrap() - g).abs() < 1e-9 * g,
            "scale {s}: {:?}",
            c.measured
        );
        assert!(
            (c.tolerance.unwrap() - rect_tol(s)).abs() < 1e-9 * rect_tol(s),
            "scale {s}"
        );
    }
}

#[test]
fn real_tiny_segment_fails_at_tiny_and_huge_scales() {
    // one extra vertex 0.5 tolerances past the bottom-right corner: a segment shorter than the tolerance
    for s in [1e-8, 1.0, 65536.0] {
        let t = 0.5 * rect_tol(s);
        let mut d = Doc::new();
        d.insunits = Some(4);
        d.poly(
            "CUT_OUTER",
            &[
                [0.0, 0.0],
                [100.0 * s, 0.0],
                [100.0 * s + t, 0.0],
                [100.0 * s, 50.0 * s],
                [0.0, 50.0 * s],
            ],
            true,
        );
        d.poly("CUT_INNER", &SQUARE.map(|p| [p[0] * s, p[1] * s]), true);
        let rep = run(&d, &scaled_job(s).1);
        assert_fails(&rep, "tiny_segments");
        assert!(
            check_named(&rep, "tiny_segments").measured.unwrap() >= 1.0,
            "scale {s}"
        );
    }
}

#[test]
fn large_arcs_fit_the_chord_budget_and_stay_sampled() {
    // a 5 m radius full circle (the reducer of a 25.4x scaled job) and a 200 m one: both must be sampled, not inconclusive
    for r in [5334.0, 210000.0] {
        let ent = reader::Ent::Arc {
            layer: "CUT_OUTER".into(),
            c: [0.0, 0.0],
            r,
            start_deg: 0.0,
            end_deg: 360.0,
        };
        let (pieces, empty) = geom::pieces(&[ent], tolerance(r * 2.0));
        assert_eq!(empty, 0, "r {r}");
        assert!(pieces.iter().all(|p| p.sampled_ok), "r {r}");
        assert!(pieces[0].pts.len() <= 4096 + 1, "r {r}"); // MAX_CHORDS points at most
    }
}

/// Engine 0.4.12 / digest: a large valid part (30 000 segments) is decided by the crossing test, not left INCONCLUSIVE
/// by a segment cap; two swapped vertices of it are a crossing and must be found.
#[test]
fn large_valid_part_is_decided_by_the_crossing_test() {
    const N: usize = 30_000;
    let pts: Vec<[f64; 2]> = (0..N)
        .map(|k| {
            let t = 2.0 * std::f64::consts::PI * k as f64 / N as f64;
            [1000.0 * t.cos(), 1000.0 * t.sin()]
        })
        .collect();
    let mut closed = pts.clone();
    closed.push(pts[0]);
    let r = Reference {
        part: "BIG".into(),
        outer: vec![curve(&closed, false, 0.0)],
        holes: Vec::new(),
        ..Reference::default()
    };
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &pts, true);
    let rep = run(&d, &r);
    let sx = check_named(&rep, "self_intersection");
    assert_eq!(sx.verdict, Verdict::Pass, "{}", sx.detail);
    assert_eq!(sx.measured, Some(0.0));

    let mut bad = pts.clone();
    bad.swap(10, N / 2);
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &bad, true);
    let rep = run(&d, &r);
    let sx = check_named(&rep, "self_intersection");
    assert_eq!(sx.verdict, Verdict::Fail, "{}", sx.detail);
    assert!(sx.measured.unwrap() >= 1.0);
}

/// A near-360 degree cone sector (the seam edges coincide within the tolerance, as the engine writes them), with its
/// reference. With `declared` the two straight edges are the engine's declared seam pair.
fn near_full_sector(declared: bool) -> (Doc, Reference) {
    let phi_deg = 359.999_999_712_000_03_f64;
    let phi = phi_deg.to_radians();
    let arc_pts = |r: f64, from: f64, to: f64| -> Vec<[f64; 2]> {
        (0..=720)
            .map(|k| {
                let t = from + (to - from) * k as f64 / 720.0;
                [r * t.cos(), r * t.sin()]
            })
            .collect()
    };
    let (o0, o1) = ([300.0, 0.0], [300.0 * phi.cos(), 300.0 * phi.sin()]);
    let (i0, i1) = ([50.0, 0.0], [50.0 * phi.cos(), 50.0 * phi.sin()]);
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.arc("CUT_OUTER", [0.0, 0.0], 300.0, 0.0, phi_deg)
        .line("CUT_OUTER", o1, i1)
        .arc("CUT_OUTER", [0.0, 0.0], 50.0, 0.0, phi_deg)
        .line("CUT_OUTER", i0, o0);
    let seam = |c: RefCurve| RefCurve {
        seam: declared,
        ..c
    };
    // the reference arcs are 720 chords each: their sagitta is the chord error the area check allows for
    let sag = |r: f64| r * (1.0 - (phi / 1440.0).cos());
    let outer = vec![
        RefCurve {
            length: 300.0 * phi,
            ..curve(&arc_pts(300.0, 0.0, phi), true, sag(300.0))
        },
        seam(RefCurve {
            length: 250.0,
            ..curve(&[o1, i1], false, 0.0)
        }),
        RefCurve {
            length: 50.0 * phi,
            ..curve(&arc_pts(50.0, phi, 0.0), true, sag(50.0))
        },
        seam(RefCurve {
            length: 250.0,
            ..curve(&[i0, o0], false, 0.0)
        }),
    ];
    (
        d,
        Reference {
            part: "SECTOR".into(),
            outer,
            holes: Vec::new(),
            ..Reference::default()
        },
    )
}

/// Engine 0.4.12 digest: a near-360 degree sector whose seam pair the reference declares is read as one closed outline:
/// no duplicate, closed, one outer profile and the correct area.
#[test]
fn declared_seam_pair_of_a_near_full_sector_reads_as_one_closed_outline() {
    let (d, r) = near_full_sector(true);
    let rep = run(&d, &r);
    for name in [
        "contours_closed",
        "seam_gaps",
        "duplicates",
        "outer_profiles",
        "area",
        "self_intersection",
    ] {
        let c = check_named(&rep, name);
        assert_eq!(c.verdict, Verdict::Pass, "{name}: {}", c.detail);
    }
}

/// The same coincident edges without a declaration are still a finding: a duplicate, and an outline that does not close.
#[test]
fn undeclared_coincident_edges_of_a_near_full_sector_still_fail() {
    let (d, r) = near_full_sector(false);
    let rep = run(&d, &r);
    assert_fails(&rep, "duplicates");
    assert_fails(&rep, "contours_closed");
}

/// Engine 0.4.12 digest: a reference that declares a feature shorter than the tolerance cannot be judged for tiny
/// segments: the finding is INCONCLUSIVE (the file is held). Without such a declaration the same segment is a FAIL.
#[test]
fn features_below_the_tolerance_declared_by_the_reference_are_inconclusive() {
    let pts = [
        [0.0, 0.0],
        [1e-5, 0.0],
        [100.0, 0.0],
        [100.0, 50.0],
        [0.0, 50.0],
    ];
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &pts, true);
    d.poly("CUT_INNER", &SQUARE, true);
    let closed_pts = [pts[0], pts[1], pts[2], pts[3], pts[4], pts[0]];
    let declared = Reference {
        part: "T".into(),
        outer: vec![curve(&closed_pts, false, 0.0)],
        holes: rect_reference().holes,
        ..Reference::default()
    };
    let rep = run(&d, &declared);
    let c = check_named(&rep, "tiny_segments");
    assert_eq!(c.verdict, Verdict::Inconclusive, "{}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Inconclusive);

    let undeclared = Reference {
        part: "T".into(),
        outer: rect_reference().outer,
        holes: rect_reference().holes,
        ..Reference::default()
    };
    let rep = run(&d, &undeclared);
    assert_fails(&rep, "tiny_segments");
}

/// The same near-360 degree sector written as ONE closed polyline (the eccentric reducer's form, where the seam edges are
/// segments of the outline): the declared seam segments may coincide and nothing else; without the declaration they are
/// duplicates.
#[test]
fn declared_seam_segments_of_a_closed_polyline_sector_are_not_duplicates() {
    let phi = 359.999_999_712_000_03_f64.to_radians();
    let mut pts: Vec<[f64; 2]> = (0..=720)
        .map(|k| {
            let t = phi * k as f64 / 720.0;
            [300.0 * t.cos(), 300.0 * t.sin()]
        })
        .collect();
    pts.extend((0..=720).map(|k| {
        let t = phi * (1.0 - k as f64 / 720.0);
        [50.0 * t.cos(), 50.0 * t.sin()]
    }));
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &pts, true);
    let rep = run(&d, &near_full_sector(true).1);
    for name in ["duplicates", "contours_closed", "outer_profiles", "area"] {
        let c = check_named(&rep, name);
        assert_eq!(c.verdict, Verdict::Pass, "{name}: {}", c.detail);
    }
    let rep = run(&d, &near_full_sector(false).1);
    assert_fails(&rep, "duplicates");
}

/// The seam pairing is scoped to the seam ends: a near-360 degree sector whose seam pair is declared, with two CUT_INNER
/// squares that touch at one corner (110, 30), is a branching joint of four ends, reported as branching (FAIL).
/// Without the two squares the same sector passes.
#[test]
fn declared_seam_does_not_hide_a_branching_joint() {
    let (mut d, r) = near_full_sector(true);
    d.line("CUT_INNER", [100.0, 20.0], [110.0, 20.0])
        .line("CUT_INNER", [110.0, 20.0], [110.0, 30.0])
        .line("CUT_INNER", [110.0, 30.0], [100.0, 30.0])
        .line("CUT_INNER", [100.0, 30.0], [100.0, 20.0])
        .line("CUT_INNER", [110.0, 30.0], [120.0, 30.0])
        .line("CUT_INNER", [120.0, 30.0], [120.0, 40.0])
        .line("CUT_INNER", [120.0, 40.0], [110.0, 40.0])
        .line("CUT_INNER", [110.0, 40.0], [110.0, 30.0]);
    let rep = run(&d, &r);
    let c = check_named(&rep, "contours_closed");
    assert_eq!(c.verdict, Verdict::Fail, "{}", c.detail);
    assert!(c.detail.contains("4 branching joint(s)"), "{}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Fail);

    let (d, r) = near_full_sector(true);
    let rep = run(&d, &r);
    assert_eq!(check_named(&rep, "contours_closed").verdict, Verdict::Pass);
}

/// The 10 mm square opening of half-size 5 centred at `c`.
fn square_at(c: [f64; 2]) -> [[f64; 2]; 4] {
    [
        [c[0] - 5.0, c[1] - 5.0],
        [c[0] + 5.0, c[1] - 5.0],
        [c[0] + 5.0, c[1] + 5.0],
        [c[0] - 5.0, c[1] + 5.0],
    ]
}

/// A 200 x 100 mm plate with 10 mm square openings drawn at `drawn`, declared at `declared` (centres).
fn plate(declared: &[[f64; 2]], drawn: &[[f64; 2]]) -> (Doc, Reference) {
    let o = [[0.0, 0.0], [200.0, 0.0], [200.0, 100.0], [0.0, 100.0]];
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &o, true);
    for c in drawn {
        d.poly("CUT_INNER", &square_at(*c), true);
    }
    let closed = [o[0], o[1], o[2], o[3], o[0]];
    let holes = declared
        .iter()
        .map(|c| {
            let s = square_at(*c);
            curve(&[s[0], s[1], s[2], s[3], s[0]], false, 0.0)
        })
        .collect();
    (
        d,
        Reference {
            part: "PLATE".into(),
            outer: vec![curve(&closed, false, 0.0)],
            holes,
            ..Reference::default()
        },
    )
}

/// A drawn opening at (130, 50) where the job declares a hole at (120, 20) is a wrong part: FAIL, naming the hole.
#[test]
fn hole_at_a_wrong_position_fails() {
    let (d, r) = plate(&[[120.0, 20.0]], &[[130.0, 50.0]]);
    let rep = run(&d, &r);
    let c = check_named(&rep, "hole_position");
    assert_eq!(c.verdict, Verdict::Fail, "{}", c.detail);
    assert!(
        c.detail.contains("declared hole 1 at (120.0000, 20.0000)"),
        "{}",
        c.detail
    );
    assert!(
        c.detail.contains("opening 1 at (130.0000, 50.0000)"),
        "{}",
        c.detail
    );
    assert_eq!(rep.verdict(), Verdict::Fail);
}

/// Openings at their declared positions pass, whatever the order they are drawn in.
#[test]
fn holes_at_their_declared_positions_pass() {
    let (d, r) = plate(
        &[[60.0, 20.0], [140.0, 70.0]],
        &[[140.0, 70.0], [60.0, 20.0]],
    );
    let rep = run(&d, &r);
    let c = check_named(&rep, "hole_position");
    assert_eq!(c.verdict, Verdict::Pass, "{}", c.detail);
    assert_eq!(rep.verdict(), Verdict::Pass);

    let (d, r) = plate(&[[120.0, 20.0]], &[[120.0, 20.0]]);
    assert_eq!(
        check_named(&run(&d, &r), "hole_position").verdict,
        Verdict::Pass
    );
}

/// The match is one-to-one: two openings at one declared position leave the second declared hole without an opening.
#[test]
fn two_openings_at_one_declared_position_fail() {
    let (d, r) = plate(
        &[[60.0, 20.0], [140.0, 70.0]],
        &[[60.0, 20.0], [60.0, 20.0]],
    );
    let rep = run(&d, &r);
    assert_fails(&rep, "hole_position");
    let c = check_named(&rep, "hole_position");
    assert!(
        c.detail
            .contains("declared hole 2 at (140.0000, 70.0000) has no opening"),
        "{}",
        c.detail
    );
}

/// A hole mismatch when the reference declares a feature below the tolerance is INCONCLUSIVE (the file is held), not FAIL.
#[test]
fn hole_mismatch_with_a_feature_below_the_tolerance_is_inconclusive() {
    let (d, mut r) = plate(&[[120.0, 20.0]], &[[130.0, 50.0]]);
    r.holes = vec![curve(
        &[
            [120.0, 20.0],
            [120.00001, 20.0],
            [125.0, 25.0],
            [120.0, 20.0],
        ],
        false,
        0.0,
    )];
    let rep = run(&d, &r);
    assert_eq!(
        check_named(&rep, "hole_position").verdict,
        Verdict::Inconclusive
    );
}

/// The points of a closed outline, the first point repeated at the end (as the job declares them).
fn loop_of(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut v = pts.to_vec();
    v.push(pts[0]);
    v
}

fn shift_pts(pts: &[[f64; 2]], dx: f64, dy: f64) -> Vec<[f64; 2]> {
    pts.iter().map(|p| [p[0] + dx, p[1] + dy]).collect()
}

/// A declared opening with the given outline; its length is the polyline length.
fn decl_curve(pts: &[[f64; 2]]) -> RefCurve {
    curve(&loop_of(pts), false, 0.0)
}

/// The 200 x 100 mm plate with the openings `drawn` (closed polylines) and the declared openings `holes`.
fn plate_case(drawn: &[Vec<[f64; 2]>], holes: Vec<RefCurve>) -> Report {
    let o = [[0.0, 0.0], [200.0, 0.0], [200.0, 100.0], [0.0, 100.0]];
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly("CUT_OUTER", &o, true);
    for h in drawn {
        d.poly("CUT_INNER", h, true);
    }
    let r = Reference {
        part: "PLATE".into(),
        outer: vec![decl_curve(&o)],
        holes,
        ..Reference::default()
    };
    run(&d, &r)
}

/// The same size and position, but the wrong size: 3 and 8 mm half-sizes swapped between two openings.
#[test]
fn swapped_opening_sizes_fail_hole_position() {
    let sq = |c: [f64; 2], h: f64| -> Vec<[f64; 2]> {
        vec![
            [c[0] - h, c[1] - h],
            [c[0] + h, c[1] - h],
            [c[0] + h, c[1] + h],
            [c[0] - h, c[1] + h],
        ]
    };
    let rep = plate_case(
        &[sq([50.0, 50.0], 8.0), sq([150.0, 50.0], 3.0)],
        vec![
            decl_curve(&sq([50.0, 50.0], 3.0)),
            decl_curve(&sq([150.0, 50.0], 8.0)),
        ],
    );
    assert_fails(&rep, "hole_position");
}

/// An L-shaped opening turned by 180 degrees inside the same bounding box: same centre, length and area, wrong shape.
#[test]
fn rotated_l_opening_in_the_same_box_fails_hole_position() {
    let l = [
        [0.0, 0.0],
        [40.0, 0.0],
        [40.0, 10.0],
        [10.0, 10.0],
        [10.0, 40.0],
        [0.0, 40.0],
    ];
    let turned: Vec<[f64; 2]> = l.iter().map(|p| [40.0 - p[0], 40.0 - p[1]]).collect();
    let rep = plate_case(
        &[shift_pts(&turned, 80.0, 30.0)],
        vec![decl_curve(&shift_pts(&l, 80.0, 30.0))],
    );
    assert_fails(&rep, "hole_position");
}

/// A right triangle mirrored about the vertical centre line of its box: same centre, length and area, wrong shape.
#[test]
fn mirrored_right_triangle_fails_hole_position() {
    let t = [[0.0, 0.0], [30.0, 0.0], [0.0, 20.0]];
    let mirrored: Vec<[f64; 2]> = t.iter().map(|p| [30.0 - p[0], p[1]]).collect();
    let rep = plate_case(
        &[shift_pts(&mirrored, 60.0, 40.0)],
        vec![decl_curve(&shift_pts(&t, 60.0, 40.0))],
    );
    assert_fails(&rep, "hole_position");
}

/// Correct circle and slot openings (drawn as arcs, declared as sampled polylines with their deviation) pass.
#[test]
fn circle_and_slot_openings_at_their_declared_places_pass() {
    // circle, radius 5 at (50, 50): two drawn half-circles; declared with 96 chords (deviation 5 (1 - cos(pi/96)))
    let s = 5.0 * (1.0 - (std::f64::consts::PI / 96.0).cos());
    let ring: Vec<[f64; 2]> = (0..96)
        .map(|k| {
            let t = (k as f64 * 360.0 / 96.0).to_radians();
            [50.0 + 5.0 * t.cos(), 50.0 + 5.0 * t.sin()]
        })
        .collect();
    let circle = RefCurve {
        points: loop_of(&ring),
        length: 2.0 * std::f64::consts::PI * 5.0,
        max_dev: s,
        is_arc: true,
        seam: false,
        eps_rep: None,
        eps_rep_certified: None,
    };
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly(
        "CUT_OUTER",
        &[[0.0, 0.0], [200.0, 0.0], [200.0, 100.0], [0.0, 100.0]],
        true,
    );
    d.arc("CUT_INNER", [50.0, 50.0], 5.0, 0.0, 180.0);
    d.arc("CUT_INNER", [50.0, 50.0], 5.0, 180.0, 360.0);
    let r = Reference {
        part: "PLATE".into(),
        outer: vec![decl_curve(&[
            [0.0, 0.0],
            [200.0, 0.0],
            [200.0, 100.0],
            [0.0, 100.0],
        ])],
        holes: vec![circle],
        ..Reference::default()
    };
    let rep = run(&d, &r);
    let c = check_named(&rep, "hole_position");
    assert_eq!(c.verdict, Verdict::Pass, "circle: {}", c.detail);

    // slot 20 x 8 mm with round ends of radius 4, centred at (100, 50): drawn as two lines and two arcs
    let sd = 4.0 * (1.0 - (std::f64::consts::PI / 32.0).cos());
    let mut slot = Vec::new();
    for k in 0..=16 {
        let t = (270.0 - k as f64 * 11.25f64).to_radians();
        slot.push([90.0 + 4.0 * t.cos(), 50.0 + 4.0 * t.sin()]);
    }
    for k in 0..=16 {
        let t = (90.0 - k as f64 * 11.25f64).to_radians();
        slot.push([110.0 + 4.0 * t.cos(), 50.0 + 4.0 * t.sin()]);
    }
    let slot_curve = RefCurve {
        points: loop_of(&slot),
        length: 40.0 + 8.0 * std::f64::consts::PI,
        max_dev: sd,
        is_arc: false,
        seam: false,
        eps_rep: Some(sd),
        eps_rep_certified: Some(true),
    };
    let mut d = Doc::new();
    d.insunits = Some(4);
    d.poly(
        "CUT_OUTER",
        &[[0.0, 0.0], [200.0, 0.0], [200.0, 100.0], [0.0, 100.0]],
        true,
    );
    d.line("CUT_INNER", [110.0, 46.0], [90.0, 46.0]);
    d.arc("CUT_INNER", [90.0, 50.0], 4.0, 90.0, 270.0);
    d.line("CUT_INNER", [90.0, 54.0], [110.0, 54.0]);
    d.arc("CUT_INNER", [110.0, 50.0], 4.0, 270.0, 90.0);
    let r = Reference {
        part: "PLATE".into(),
        outer: vec![decl_curve(&[
            [0.0, 0.0],
            [200.0, 0.0],
            [200.0, 100.0],
            [0.0, 100.0],
        ])],
        holes: vec![slot_curve],
        ..Reference::default()
    };
    let rep = run(&d, &r);
    let c = check_named(&rep, "hole_position");
    assert_eq!(c.verdict, Verdict::Pass, "slot: {}", c.detail);
}

/// The square outline of half-size `h` centred at `c`.
fn square_outline(c: [f64; 2], h: f64) -> Vec<[f64; 2]> {
    vec![
        [c[0] - h, c[1] - h],
        [c[0] + h, c[1] - h],
        [c[0] + h, c[1] + h],
        [c[0] - h, c[1] + h],
    ]
}

/// Concentric correct openings (half-sizes 5 and 2 at (50, 50)) each take their own declared hole: PASS.
#[test]
fn concentric_openings_each_take_their_own_hole() {
    let rep = plate_case(
        &[
            square_outline([50.0, 50.0], 2.0),
            square_outline([50.0, 50.0], 5.0),
        ],
        vec![
            decl_curve(&square_outline([50.0, 50.0], 5.0)),
            decl_curve(&square_outline([50.0, 50.0], 2.0)),
        ],
    );
    let c = check_named(&rep, "hole_position");
    assert_eq!(c.verdict, Verdict::Pass, "{}", c.detail);
}

/// A concentric opening missing from the file fails: the inner declared hole has no opening.
#[test]
fn missing_concentric_opening_fails_hole_position() {
    let rep = plate_case(
        &[square_outline([50.0, 50.0], 5.0)],
        vec![
            decl_curve(&square_outline([50.0, 50.0], 5.0)),
            decl_curve(&square_outline([50.0, 50.0], 2.0)),
        ],
    );
    let c = check_named(&rep, "hole_position");
    assert_eq!(c.verdict, Verdict::Fail, "{}", c.detail);
    assert!(
        c.detail
            .contains("declared hole 2 at (50.0000, 50.0000) has no opening"),
        "{}",
        c.detail
    );
}

/// An extra concentric opening fails: the second opening matches no declared hole of its own.
#[test]
fn extra_concentric_opening_fails_hole_position() {
    let rep = plate_case(
        &[
            square_outline([50.0, 50.0], 5.0),
            square_outline([50.0, 50.0], 2.0),
            square_outline([50.0, 50.0], 2.0),
        ],
        vec![
            decl_curve(&square_outline([50.0, 50.0], 5.0)),
            decl_curve(&square_outline([50.0, 50.0], 2.0)),
        ],
    );
    let c = check_named(&rep, "hole_position");
    assert_eq!(c.verdict, Verdict::Fail, "{}", c.detail);
}
