use crate::check::{check, check_pipes};
use crate::dxf_out::sheet_dxf;
use crate::outline::outline_from_dxf;
use crate::pipes::{PipeIn, cut_pipes};
use crate::{Nest, PartIn, Rules, nest, utilisation};

fn rect(w: f64, h: f64) -> Vec<(f64, f64)> {
    vec![(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)]
}

fn part(id: &str, pts: Vec<(f64, f64)>, qty: u32) -> PartIn {
    PartIn {
        id: id.into(),
        outline: pts,
        qty,
    }
}

fn flat_rules(w: f64, h: f64) -> Rules {
    Rules {
        sheet_w: w,
        sheet_h: h,
        kerf_mm: 0.2,
        part_gap_mm: 0.0,
        edge_mm: 0.0,
        rotations: vec![0, 90],
    }
}

/// xorshift64: a fixed seed keeps the property test repeatable.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (self.next() % 1_000_000) as f64 / 1_000_000.0 * (hi - lo)
    }
}

fn valid_nest() -> Nest {
    let parts = vec![
        part("A", rect(400.0, 300.0), 2),
        part("B", vec![(0.0, 0.0), (250.0, 0.0), (0.0, 250.0)], 3),
    ];
    nest(&parts, &flat_rules(1000.0, 1000.0)).expect("nest")
}

#[test]
fn valid_nest_passes_check() {
    assert!(check(valid_nest()).is_ok());
}

#[test]
fn checker_catches_injected_overlap() {
    let mut n = valid_nest();
    let target = n.sheets[0].placed[0].pts.clone();
    // move the second placed part onto the first
    let moved = crate::geom::place(
        &crate::geom::normalise(&n.parts[1].outline),
        0,
        (target[0].0 + 10.0, target[0].1 + 10.0),
    );
    n.sheets[0].placed[1].pts = moved;
    n.sheets[0].placed[1].at = (target[0].0 + 10.0, target[0].1 + 10.0);
    let errs = check(n).err().expect("must fail");
    assert!(
        errs.iter()
            .any(|e| e.contains("overlap") || e.contains("does not match")),
        "{errs:?}"
    );
}

#[test]
fn checker_catches_kerf_violation() {
    // 0.1 mm apart: inside the 0.2 mm kerf
    let parts = vec![part("A", rect(100.0, 100.0), 2)];
    let mut n = nest(&parts, &flat_rules(1000.0, 1000.0)).expect("nest");
    let x1 = n.sheets[0].placed[0]
        .pts
        .iter()
        .map(|p| p.0)
        .fold(f64::MIN, f64::max);
    let shape = crate::geom::place(
        &crate::geom::normalise(&parts[0].outline),
        0,
        (x1 + 0.1, 0.0),
    );
    n.sheets[0].placed[1].pts = shape;
    n.sheets[0].placed[1].at = (x1 + 0.1, 0.0);
    let errs = check(n).err().expect("must fail");
    assert!(errs.iter().any(|e| e.contains("apart")), "{errs:?}");
}

#[test]
fn checker_catches_off_sheet_duplicate_drop_and_rotation() {
    let base = valid_nest();

    let mut off = base.clone();
    let shape = crate::geom::place(
        &crate::geom::normalise(&off.parts[0].outline),
        0,
        (900.0, 900.0),
    );
    off.sheets[0].placed[0].pts = shape;
    off.sheets[0].placed[0].at = (900.0, 900.0);
    assert!(
        check(off)
            .err()
            .expect("off sheet")
            .iter()
            .any(|e| e.contains("outside"))
    );

    let mut dup = base.clone();
    let copy = dup.sheets[0].placed[0].clone();
    dup.sheets[0].placed.push(copy);
    assert!(
        check(dup)
            .err()
            .expect("dup")
            .iter()
            .any(|e| e.contains("placed"))
    );

    let mut drop = base.clone();
    drop.sheets[0].placed.pop();
    assert!(
        check(drop)
            .err()
            .expect("drop")
            .iter()
            .any(|e| e.contains("placed"))
    );

    let mut rot = base.clone();
    rot.sheets[0].placed[0].rot = 180;
    rot.rules.rotations = vec![0, 90];
    assert!(check(rot).is_err());
}

#[test]
fn random_nests_pass_check_and_place_every_copy() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for trial in 0..40 {
        let n_kinds = 1 + (rng.next() % 4) as usize;
        let mut parts = Vec::new();
        for k in 0..n_kinds {
            let w = rng.range(20.0, 220.0);
            let h = rng.range(20.0, 220.0);
            let pts = if rng.next() % 2 == 0 {
                rect(w, h)
            } else {
                vec![(0.0, 0.0), (w, 0.0), (w * 0.4, h), (0.0, h * 0.7)]
            };
            parts.push(part(
                &format!("P{trial}-{k}"),
                pts,
                1 + (rng.next() % 6) as u32,
            ));
        }
        let rules = Rules {
            sheet_w: 600.0,
            sheet_h: 600.0,
            kerf_mm: 0.2,
            part_gap_mm: 1.0,
            edge_mm: 3.0,
            rotations: vec![0, 90, 180, 270],
        };
        let n = nest(&parts, &rules).expect("random parts fit");
        let checked = check(n).unwrap_or_else(|e| panic!("trial {trial}: {e:?}"));
        let (per, all) = utilisation(checked.nest());
        assert!(per.iter().all(|u| *u <= 100.0 + 1e-6) && all <= 100.0 + 1e-6);
    }
}

#[test]
fn fixed_set_utilisation() {
    // 400 x 300 mm rectangles, 0.2 mm kerf, no edge: three rows of two on a 1000 x 1000 sheet.
    let parts = vec![part("R", rect(400.0, 300.0), 6)];
    let n = nest(&parts, &flat_rules(1000.0, 1000.0)).expect("nest");
    assert_eq!(n.sheets.len(), 1);
    let checked = check(n).expect("passes");
    let (per, all) = utilisation(checked.nest());
    assert!((per[0] - 72.0).abs() < 1e-9, "got {per:?}");
    assert!((all - 72.0).abs() < 1e-9);
}

#[test]
fn oversize_part_is_refused() {
    let parts = vec![part("BIG", rect(1200.0, 1200.0), 1)];
    assert!(nest(&parts, &flat_rules(1000.0, 1000.0)).is_err());
}

#[test]
fn outline_from_dxf_closed_loop_and_hole_refusal() {
    let square = "0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nCUT\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nLINE\n8\nCUT\n10\n10\n20\n0\n11\n10\n21\n10\n0\nLINE\n8\nCUT\n10\n10\n20\n10\n11\n0\n21\n10\n\
0\nLINE\n8\nCUT\n10\n0\n20\n10\n11\n0\n21\n0\n0\nENDSEC\n0\nEOF\n";
    let poly = outline_from_dxf(square).expect("square");
    assert!((crate::geom::area(&poly) - 100.0).abs() < 1e-9);
    let with_hole = format!("{square}");
    let hole = "0\nLINE\n8\nCUT\n10\n2\n20\n2\n11\n3\n21\n2\n0\nLINE\n8\nCUT\n10\n3\n20\n2\n11\n3\n21\n3\n0\nLINE\n8\nCUT\n10\n3\n20\n3\n11\n2\n21\n3\n0\nLINE\n8\nCUT\n10\n2\n20\n3\n11\n2\n21\n2\n";
    let text = with_hole.replace("0\nENDSEC", &format!("{hole}0\nENDSEC"));
    assert!(outline_from_dxf(&text).is_err());
}

#[test]
fn sheet_dxf_only_from_checked_nest() {
    let c = check(valid_nest()).expect("passes");
    let text = sheet_dxf(&c, 0).expect("sheet 1");
    assert!(text.contains("NOT RELEASED"));
    assert!(text.contains("UNVERIFIED"));
    let (ents, unknown) = nf_dxf::read(&text).expect("reads back");
    assert!(unknown.is_empty());
    assert!(ents.iter().any(|e| e.layer() == "CUT"));
    assert!(sheet_dxf(&c, 5).is_none());
}

#[test]
fn pipe_cut_list_with_kerf_and_checker() {
    let items = vec![PipeIn {
        id: "T1".into(),
        len_mm: 2000.0,
        qty: 3,
    }];
    let one = cut_pipes(&items, 6000.0, 0.0).expect("cut");
    assert_eq!(one.bars.len(), 1);
    assert!(check_pipes(one, &items).is_ok());

    let kerfed = cut_pipes(&items, 6000.0, 2.0).expect("cut");
    assert_eq!(
        kerfed.bars.len(),
        2,
        "2 x 2000 + one kerf = 4002 mm; a third cut would need 6004 mm"
    );
    assert!(check_pipes(kerfed, &items).is_ok());

    let mut bad = cut_pipes(&items, 6000.0, 2.0).expect("cut");
    bad.bars[0].cuts.push(("T1".into(), 2000.0));
    assert!(check_pipes(bad, &items).is_err());
    assert!(
        cut_pipes(
            &[PipeIn {
                id: "L".into(),
                len_mm: 7000.0,
                qty: 1
            }],
            6000.0,
            0.0
        )
        .is_err()
    );
}
