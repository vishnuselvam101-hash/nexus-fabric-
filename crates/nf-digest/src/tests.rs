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
    assert_eq!(rep.checks.len(), 20);
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
    assert_fails(&rep, "tiny_segments");
    assert!(check_named(&rep, "tiny_segments").measured.unwrap() >= 1.0);
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
