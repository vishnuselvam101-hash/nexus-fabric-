//! NFJ-1 S1-L10: order -> calculations compute. The acceptance test (12-part synthetic order), one test per hold
//! SOURCE with a real scenario (nothing injected), the structural rule "a hold blocks the cut file", the conditions of
//! the Chief Architect's "Wiring-lane conditions" (serialized fields, negative offset, rib/flag holds, thickness
//! convention) and the engine identifier.

use nf_core::conebranch::ThicknessConvention;
use nf_core::order::TableRow;
use nf_core::order_compute::{
    HOLD_TEXT, OrderRules, PartIn, PartResult, Val, compute_order, hold_codes, issues,
    material_rows, result_fingerprint, to_json,
};

fn rules() -> OrderRules {
    OrderRules::default()
}

fn run(parts: &[PartIn]) -> Vec<PartResult> {
    compute_order(parts, &rules()).parts
}

fn one(p: PartIn) -> PartResult {
    run(&[p]).remove(0)
}

/// The structural rule of the lane: whatever blocks a part is named in `dxf_blocked_by`, no DXF exists, the mass is
/// provisional while a Need hold, a question or a refusal stands.
fn assert_blocks(r: &PartResult, code: &str) {
    assert!(
        r.dxf_blocked_by.iter().any(|c| c == code),
        "{}: {code} must block the DXF, blocked_by = {:?}",
        r.no,
        r.dxf_blocked_by
    );
    assert!(
        r.dxf.is_none() && !r.dxf_allowed,
        "{}: no DXF while {code} stands",
        r.no
    );
    assert!(
        r.mass_provisional || r.mass_kg.is_none(),
        "{}: mass must be provisional or absent",
        r.no
    );
}

fn assert_hold(r: &PartResult, code: &str) {
    assert!(
        r.has_hold(code) || r.has_question(code),
        "{}: expected hold/question {code}, holds = {:?}, questions = {:?}, refusal = {:?}",
        r.no,
        r.holds
            .iter()
            .map(|h| (h.level, h.code.as_str()))
            .collect::<Vec<_>>(),
        r.questions.iter().map(|q| q.code).collect::<Vec<_>>(),
        r.refusal
    );
    assert_blocks(r, code);
}

fn cyl(no: &str) -> PartIn {
    PartIn::new(no, "cylinder")
        .v("D", 1016.0)
        .v("t", 12.0)
        .v("L", 2000.0)
}

fn plate(no: &str) -> PartIn {
    PartIn::new(no, "plate")
        .v("t", 16.0)
        .v("plate_W", 400.0)
        .v("plate_H", 300.0)
}

fn tee_main(no: &str) -> PartIn {
    PartIn::new(no, "tee_main")
        .v("D", 1016.0)
        .v("t", 12.0)
        .v("L", 2000.0)
        .v("branch_D", 508.0)
        .v("branch_t", 9.0)
        .v("branch_x", 1000.0)
        .v("dir", 0.0)
}

fn tee_branch(no: &str, main: &str) -> PartIn {
    PartIn::new(no, "tee_branch")
        .v("D", 508.0)
        .v("t", 9.0)
        .v("H", 600.0)
        .attached(main)
}

fn twelve() -> Vec<PartIn> {
    vec![
        cyl("C1"),
        cyl("C2").v("split", 2.0).v("L", 3000.0),
        PartIn::new("B1", "miter_bend")
            .v("D", 1016.0)
            .v("t", 12.0)
            .v("R", 1500.0)
            .v("angle", 90.0)
            .v("joints", 2.0)
            .v("start_straight", 300.0)
            .v("end_straight", 300.0),
        PartIn::new("B2", "miter_bend")
            .v("D", 508.0)
            .v("t", 9.0)
            .v("R", 750.0)
            .v("angle", 45.0)
            .v("joints", 2.0)
            .v("start_straight", 200.0)
            .v("end_straight", 200.0),
        tee_main("T1"),
        tee_branch("T1b", "T1"),
        PartIn::new("T1p", "tee_pad")
            .v("t", 9.0)
            .v("pad_B", 100.0)
            .attached("T1"),
        PartIn::new("R1", "cone")
            .v("D1", 1016.0)
            .v("D2", 762.0)
            .v("L", 600.0)
            .v("t", 12.0),
        PartIn::new("R2", "eccentric_cone")
            .v("D1", 1016.0)
            .v("D2", 762.0)
            .v("L", 600.0)
            .v("t", 12.0),
        plate("P1")
            .v("hole_d", 24.0)
            .v("hole_x", 50.0)
            .v("hole_y", 50.0)
            .v("hole_n", 1.0),
        plate("P2")
            .v("t", 9.0)
            .v("plate_W", 300.0)
            .v("plate_H", 200.0),
        PartIn::new("RG1", "ring")
            .v("OD", 800.0)
            .v("ID", 500.0)
            .v("t", 12.0)
            .v("split", 1.0),
    ]
}

// ---------------------------------------------------------------- acceptance

#[test]
fn twelve_part_order_every_part_has_pattern_mass_and_bom_delta() {
    let parts = twelve();
    assert_eq!(parts.len(), 12);
    let first = compute_order(&parts, &rules());
    // the material table mass of every part = what was computed (a different, nearby number: 1e-4 off)
    let with_table: Vec<PartIn> = parts
        .iter()
        .map(|p| {
            let r = first.part(&p.no).expect("part");
            let unit = r
                .unit_mass_kg
                .unwrap_or_else(|| panic!("{} has no mass: {:?}", p.no, r.holds));
            let mut q = p.clone();
            q.table = Some(TableRow {
                mass: Some(unit * 1.0001),
                mass_decimals: 6,
                ..Default::default()
            });
            q
        })
        .collect();
    let res = compute_order(&with_table, &rules());
    assert_eq!(res.parts.len(), 12);
    for r in &res.parts {
        assert!(!r.pattern.is_empty(), "{}: pattern data", r.no);
        assert!(r.unit_mass_kg.is_some_and(|m| m > 0.0), "{}: mass", r.no);
        let b = r
            .bom
            .as_ref()
            .unwrap_or_else(|| panic!("{}: BOM delta", r.no));
        assert!(b.within_tolerance && b.delta_rel < 1e-3, "{}: {b:?}", r.no);
        // L11: only a certified part with no hold gets a DXF. Families whose representation is not
        // yet certified (cone branch, wye) stay withheld with a plain reason.
        assert_eq!(r.dxf.is_some(), r.dxf_allowed, "{}", r.no);
        assert!(!r.area_certified && !r.extents_certified, "{}", r.no);
        assert_eq!(
            r.dxf_allowed,
            r.certified && r.dxf_blocked_by.is_empty(),
            "{}",
            r.no
        );
        if !r.certified {
            assert!(
                r.dxf_blocked_by.contains(&"not_certified".to_string()),
                "{}",
                r.no
            );
        }
        assert!(!r.group.is_empty());
    }
    // the tee group is one assembly
    let g = |n: &str| res.part(n).expect("part").group.clone();
    assert_eq!(g("T1"), g("T1b"));
    assert_eq!(g("T1"), g("T1p"));
    // material list: every part has rows with a mass; issues carry the not-certified entry
    let rows = material_rows(&res);
    assert!(rows.len() >= 12);
    assert!(
        rows.iter().all(|r| r.mass_calc_kg.is_some_and(|m| m > 0.0)),
        "all masses final"
    );
    assert_eq!(
        rows.iter().filter(|r| r.mass_bom_kg.is_some()).count(),
        12,
        "BOM mass on one row per part"
    );
    let iss = issues(&res);
    let uncertified = res.parts.iter().filter(|p| p.pattern_view_only).count();
    assert_eq!(
        iss.iter().filter(|i| i.code == "not_certified").count(),
        uncertified
    );
    assert!(iss.iter().all(|i| !i.message_ja.is_empty()));
}

#[test]
fn missing_value_is_hold_missing_with_no_pattern_and_no_dxf() {
    let mut p = cyl("C1");
    p.values.remove("L");
    let r = one(p);
    assert_eq!(r.state, "held_missing");
    assert!(r.pattern.is_empty() && r.unit_mass_kg.is_none());
    assert_hold(&r, "missing");
    let h = r.holds.iter().find(|h| h.code == "missing").expect("hold");
    assert_eq!(h.keys, vec!["L".to_string()]);
    assert!(h.message_en.contains("length") && h.message_ja.contains("長さ"));
    assert!(!material_rows(&compute_order(&[cyl("C1")], &rules())).is_empty());
}

#[test]
fn a_proposed_value_is_never_computed() {
    let p = cyl("C1").with("L", Val::proposed(2000.0));
    let r = one(p);
    assert_eq!(r.state, "held_missing");
    assert!(r.pattern.is_empty());
    assert_hold(&r, "value_proposed");
    assert_hold(&r, "missing");
    // an optional proposed value is named too, not dropped
    let r = one(cyl("C1").with("split", Val::proposed(2.0)));
    assert_hold(&r, "value_proposed");
}

#[test]
fn an_edit_supersedes_by_a_new_fingerprint_and_an_unchanged_order_does_not() {
    let a = compute_order(&twelve(), &rules());
    let b = compute_order(&twelve(), &rules());
    assert_eq!(
        result_fingerprint(&a),
        result_fingerprint(&b),
        "deterministic"
    );
    let mut edited = twelve();
    edited[0] = cyl("C1").v("L", 2100.0);
    let c = compute_order(&edited, &rules());
    assert_ne!(
        result_fingerprint(&a),
        result_fingerprint(&c),
        "an edit changes the result identity"
    );
    assert_ne!(
        a.part("C1").expect("c1").unit_mass_kg,
        c.part("C1").expect("c1").unit_mass_kg
    );
    assert_eq!(
        a.part("C2").expect("c2").unit_mass_kg,
        c.part("C2").expect("c2").unit_mass_kg
    );
}

// ---------------------------------------------------------------- one real scenario per hold source

#[test]
fn single_part_holds_block_the_dxf() {
    // plate front half
    assert_hold(&one(plate("P").v("plate_W2", 150.0)), "plate_off");
    assert_hold(&one(plate("P").v("plate_C", 10.0)), "plate_C_at");
    assert_hold(
        &one(plate("P")
            .v("plate_H", 15.0)
            .v("plate_C", 10.0)
            .v("plate_C_at", 0.0)),
        "plate_C_big",
    );
    assert_hold(&one(plate("P").v("hole_d", 22.0)), "plate_hole_pos");
    assert_hold(
        &one(plate("P")
            .v("hole_d", 24.0)
            .v("hole_x", 500.0)
            .v("hole_y", 50.0)
            .v("hole_n", 1.0)),
        "plate_hole_out",
    );
    assert_hold(
        &one(plate("P")
            .v("hole_d", 24.0)
            .v("hole_x", 50.0)
            .v("hole_y", 50.0)
            .v("hole_n", 4.0)),
        "plate_hole_grid",
    );
    // cylinder: seam range, stock fit, longer than stock
    assert_hold(&one(cyl("C").v("split", 7.0)), "split_seam");
    assert_hold(&one(cyl("C").v("D", 3000.0)), "stock_fit");
    assert_hold(&one(cyl("C").v("L", 9000.0)), "pipe_longer_than_stock");
    assert_hold(&one(cyl("C").v("split", 2.5)), "split_not_integer");
    // a plate with a mass that disagrees with the table
    let mut p = plate("P");
    p.table = Some(TableRow {
        mass: Some(5.0),
        mass_decimals: 1,
        ..Default::default()
    });
    assert_hold(&one(p), "mass_vs_table_cut");
    // bend: a centre length that disagrees with the drawing
    let bend = twelve().remove(2);
    assert_hold(&one(bend.v("start_center", 1.0)), "bend_center_vs_drawing");
    // a value from an inference, one outside the plausibility band
    assert_hold(
        &one(cyl("C").with("L", Val::confirmed(2000.0, "inferred"))),
        "source_inferred",
    );
    assert_hold(&one(cyl("C").v("D", 10.0).v("t", 2.0)), "out_of_band");
    // shapes not computed in this build
    let r = one(PartIn::new("W1", "wye").v("D", 500.0));
    assert_eq!(r.state, "not_computed");
    assert_hold(&r, "shape_not_computed");
    // purchased: no calculation, no Need hold, but it is listed with its BOM numbers only
    let r = one(PartIn::new("V1", "purchased"));
    assert_eq!(r.state, "not_computed");
    assert!(r.pattern.is_empty());
    assert!(r.dxf.is_none() && !r.dxf_allowed);
}

#[test]
fn engine_refusals_block_the_dxf_and_carry_their_code() {
    let r = one(PartIn::new("RG", "ring")
        .v("OD", 400.0)
        .v("ID", 500.0)
        .v("t", 10.0)
        .v("split", 1.0));
    assert_eq!(
        r.refusal.as_ref().map(|x| x.code.as_str()),
        Some("ring_id_not_below_od")
    );
    assert_eq!(r.state, "refused");
    assert_blocks(&r, "refused");
    let r = one(PartIn::new("K", "cone")
        .v("D1", 800.0)
        .v("D2", 800.0)
        .v("L", 500.0)
        .v("t", 9.0));
    assert_eq!(
        r.refusal.as_ref().map(|x| x.code.as_str()),
        Some("cone_not_conical")
    );
    let r = one(PartIn::new("B", "miter_bend")
        .v("D", 1016.0)
        .v("t", 12.0)
        .v("R", 1500.0)
        .v("angle", 90.0)
        .v("joints", 2.5)
        .v("start_straight", 300.0)
        .v("end_straight", 300.0));
    assert_eq!(
        r.refusal.as_ref().map(|x| x.code.as_str()),
        Some("bend_joints_invalid")
    );
    // a taper piece on a plain bend is refused, never ignored silently (L6b MED-1)
    let r = one(twelve().remove(2).v("taper_piece", 2.0));
    assert_eq!(
        r.refusal.as_ref().map(|x| x.code.as_str()),
        Some("bend_taper_piece_not_reducing")
    );
}

#[test]
fn tee_group_holds_block_the_dxf() {
    // a clean group first: no hold, no question
    let ok = run(&[tee_main("T1"), tee_branch("T1b", "T1")]);
    for r in &ok {
        assert!(
            r.holds.iter().all(|h| h.level != "need") && r.questions.is_empty(),
            "{}: {:?} {:?}",
            r.no,
            r.holds,
            r.questions
        );
        assert_eq!(r.state, "computed", "{}", r.no);
    }
    // main and branch disagree on a shared value
    let r = run(&[tee_main("T1"), tee_branch("T1b", "T1").v("D", 500.0)]);
    assert_hold(&r[0], "tee_mismatch");
    assert_hold(&r[1], "tee_mismatch");
    // a branch without a main
    let r = run(&[tee_branch("T1b", "NONE")
        .v("main_D", 1016.0)
        .v("main_t", 12.0)]);
    assert_hold(&r[0], "tee_group_no_main");
    // branch position and direction not on the drawing: questions, never silent defaults
    let mut m = tee_main("T1");
    m.values.remove("branch_x");
    m.values.remove("dir");
    let r = run(&[m, tee_branch("T1b", "T1")]);
    assert_hold(&r[0], "tee_x_not_on_drawing");
    assert_hold(&r[0], "tee_dir_not_on_drawing");
    assert_hold(&r[1], "tee_dir_not_on_drawing");
    // a lateral branch below 30 degrees is built but held (flag turned into a hold, on the main and the branch)
    let long = tee_main("T1")
        .v("L", 6000.0)
        .v("branch_x", 3000.0)
        .v("branch_angle", 20.0);
    let r = run(&[long, tee_branch("T1b", "T1").v("branch_x", 3000.0)]);
    assert_hold(&r[1], "tee_lateral_angle_below_30");
    assert_hold(&r[0], "tee_lateral_angle_below_30");
    // a branch of the main with a missing value: the main cannot place its hole
    let mut b = tee_branch("T1b", "T1");
    b.values.remove("H");
    let r = run(&[tee_main("T1"), b]);
    assert_hold(&r[1], "missing");
    assert_hold(&r[0], "tee_branch_incomplete");
    // pad: both outlines given
    let r = run(&[
        tee_main("T1"),
        tee_branch("T1b", "T1"),
        PartIn::new("T1p", "tee_pad")
            .v("t", 9.0)
            .v("pad_B", 100.0)
            .v("pad_D", 800.0)
            .attached("T1"),
    ]);
    assert_hold(&r[2], "pad_outline_ambiguous");
    // rib on a lateral branch: the nf-core refusal becomes a blocking hold
    let rib = PartIn::new("T1r", "tee_rib")
        .v("t", 9.0)
        .v("rib_b", 150.0)
        .v("rib_h", 100.0)
        .v("rib_n1", 1.0)
        .attached("T1");
    let r = run(&[
        tee_main("T1").v("branch_angle", 45.0),
        tee_branch("T1b", "T1"),
        rib.clone(),
    ]);
    assert_hold(&r[2], "tee_rib_on_lateral_unsupported");
    // the angle given only on the branch part (not on the main) holds the rib as well
    let r = run(&[
        tee_main("T1"),
        tee_branch("T1b", "T1").v("branch_angle", 45.0),
        rib.clone(),
    ]);
    assert_hold(&r[2], "tee_rib_on_lateral_unsupported");
    // ... and the same rib on a perpendicular branch is not held for that
    let r = run(&[tee_main("T1"), tee_branch("T1b", "T1"), rib]);
    assert!(!r[2].has_hold("tee_rib_on_lateral_unsupported"));
}

#[test]
fn two_large_opposite_branches_with_three_seams_hold_the_seam_crossing() {
    let mut m = PartIn::new("T2", "tee_main")
        .v("D", 1016.0)
        .v("t", 12.0)
        .v("L", 4000.0)
        .v("split", 3.0);
    m = m.v("branch_D", 980.0).v("branch_t", 9.0);
    let b1 = PartIn::new("T2a", "tee_branch")
        .v("D", 980.0)
        .v("t", 9.0)
        .v("H", 700.0)
        .v("branch_x", 1000.0)
        .v("dir", 90.0)
        .attached("T2");
    let b2 = PartIn::new("T2b", "tee_branch")
        .v("D", 980.0)
        .v("t", 9.0)
        .v("H", 700.0)
        .v("branch_x", 3000.0)
        .v("dir", 270.0)
        .attached("T2");
    let r = run(&[m, b1, b2]);
    assert_hold(&r[0], "tee_seam_crosses_hole");
    // the group hold is on every member, so no part of the assembly gets a cut file
    assert_hold(&r[1], "tee_seam_crosses_hole");
    assert_hold(&r[2], "tee_seam_crosses_hole");
}

#[test]
fn negative_offset_without_a_side_is_refused_and_with_a_side_is_used() {
    let r = run(&[tee_main("T1").v("e", -50.0), tee_branch("T1b", "T1")]);
    for x in &r {
        assert_eq!(
            x.refusal.as_ref().map(|f| f.code.as_str()),
            Some("tee_offset_negative_without_side"),
            "{}",
            x.no
        );
        assert_blocks(x, "refused");
    }
    // the same offset with a side perpendicular to the branch direction (dir 0 -> side 90 or 270)
    let r = run(&[
        tee_main("T1").v("e", 50.0).v("e_side", 270.0),
        tee_branch("T1b", "T1"),
    ]);
    assert!(
        r.iter().all(|x| x.refusal.is_none()),
        "{:?}",
        r.iter().map(|x| &x.refusal).collect::<Vec<_>>()
    );
}

#[test]
fn rib_with_a_negative_offset_and_no_side_is_refused() {
    let rib = PartIn::new("T1r", "tee_rib")
        .v("t", 9.0)
        .v("rib_b", 150.0)
        .v("rib_h", 100.0)
        .v("rib_n1", 1.0)
        .attached("T1");
    let r = run(&[tee_main("T1").v("e", -30.0), tee_branch("T1b", "T1"), rib]);
    assert_eq!(
        r[2].refusal.as_ref().map(|f| f.code.as_str()),
        Some("tee_offset_negative_without_side"),
        "{:?}",
        r[2].holds
    );
    assert_blocks(&r[2], "refused");
    assert!(r[2].pattern.is_empty());
}

#[test]
fn rib_front_half_holds_block_the_dxf() {
    let rib = |b: f64, h: f64| {
        PartIn::new("T1r", "tee_rib")
            .v("t", 9.0)
            .v("rib_b", b)
            .v("rib_h", h)
            .v("rib_n1", 1.0)
            .attached("T1")
    };
    let r = run(&[tee_main("T1"), tee_branch("T1b", "T1"), rib(150.0, 100.0)]);
    assert!(
        r[2].holds.iter().all(|h| h.level != "need"),
        "{:?}",
        r[2].holds
    );
    assert!(r[2].unit_mass_kg.is_some());
    let r = run(&[tee_main("T1"), tee_branch("T1b", "T1"), rib(150.0, 1900.0)]);
    assert!(
        r[2].holds
            .iter()
            .any(|h| h.level == "need" && h.code.starts_with("rib_")),
        "{:?}",
        r[2].holds
    );
    assert!(r[2].dxf_blocked_by.iter().any(|c| c.starts_with("rib_")));
}

fn cone_group(ecc: bool) -> Vec<PartIn> {
    let cone = PartIn::new("K1", if ecc { "eccentric_cone" } else { "cone" })
        .v("D1", 1016.0)
        .v("D2", 762.0)
        .v("L", 800.0)
        .v("t", 12.0)
        .v("split", 2.0);
    let br = PartIn::new("K1b", "tee_branch")
        .v("D", 300.0)
        .v("t", 9.0)
        .v("H", 700.0)
        .v("branch_x", 400.0)
        .v("dir", 0.0)
        .attached("K1");
    vec![cone, br]
}

#[test]
fn certified_and_withheld_families_carry_the_representation_status() {
    let tee = run(&[tee_main("T1"), tee_branch("T1b", "T1")]);
    for r in &tee {
        assert!(r.certified, "{}: perpendicular tee must be certified", r.no);
        assert_eq!(r.eps_rep_certified, Some(true), "{}", r.no);
        assert!(r.eps_rep.unwrap_or(f64::INFINITY) <= 0.05, "{}", r.no);
    }

    let bend = one(twelve().remove(2));
    assert!(!bend.certified);
    assert_eq!(bend.eps_rep_certified, Some(false));

    let ecc = run(&cone_group(true));
    assert!(!ecc[0].certified);
    assert_eq!(ecc[0].eps_rep_certified, Some(false));

    let cb = run(&cone_group(false));
    assert!(!cb[0].certified);
    assert_eq!(cb[0].eps_rep_certified, Some(false));
}

#[test]
fn cone_branch_group_serializes_the_wiring_fields_and_defaults_to_normal_thickness() {
    let res = compute_order(&cone_group(false), &rules());
    let j = to_json(&res);
    for r in &j["parts"].as_array().expect("parts").clone() {
        for k in [
            "certified",
            "method",
            "rules",
            "holds",
            "questions",
            "eps_rep",
            "eps_rep_certified",
            "thickness_convention",
            "thickness_delta",
            "dxf",
            "dxf_allowed",
            "dxf_blocked_by",
        ] {
            assert!(
                r.get(k).is_some(),
                "{} missing from the serialized result of {}",
                k,
                r["no"]
            );
        }
        assert_eq!(r["certified"], false);
        assert_eq!(
            r["thickness_convention"], "normal",
            "cone parts use the normal plate thickness by default"
        );
        assert_eq!(r["thickness_delta"]["status"], "available");
        assert!(r["thickness_delta"]["cut_shift_max_mm"].is_number());
        assert!(r["rules"].as_array().is_some_and(|a| !a.is_empty()));
        assert!(r["dxf"].is_null());
    }
    // the radial convention is a named rule, selectable, and visible in the result
    let mut rr = rules();
    rr.cone_thickness = ThicknessConvention::Radial;
    let res = compute_order(&cone_group(false), &rr);
    assert_eq!(res.parts[0].thickness_convention, Some("radial"));
    let a = compute_order(&cone_group(false), &rules());
    let hole = |r: &nf_core::order_compute::OrderResult| {
        to_json(r)["parts"][0]["pattern"][0]["holes"].clone()
    };
    assert!(
        hole(&a) != hole(&res),
        "the two conventions give different holes in the cone"
    );
}

#[test]
fn cone_branch_holds_block_the_dxf() {
    // eccentric cone with no flat side stated: question + hold, the flat side is not guessed silently
    let r = run(&cone_group(true));
    assert!(
        r.iter().all(|x| x.refusal.is_none()),
        "{:?}",
        r.iter().map(|x| &x.refusal).collect::<Vec<_>>()
    );
    assert_hold(&r[0], "conebranch_flat_side_unconfirmed");
    assert_hold(&r[1], "conebranch_flat_side_unconfirmed");
    // position not on the drawing
    let mut g = cone_group(false);
    g[1].values.remove("branch_x");
    let r = run(&g);
    assert_hold(&r[0], "conebranch_x_not_on_drawing");
    assert_hold(&r[1], "conebranch_x_not_on_drawing");
    // a cone with branch keys but no branch part has no H
    let mut g = cone_group(false);
    g.truncate(1);
    g[0] = g[0].clone().v("branch_D", 300.0).v("branch_t", 9.0);
    let r = run(&g);
    assert_hold(&r[0], "conebranch_branch_incomplete");
    // a steep eccentric cone: the neutral surface deviates from the developed cone (mid-surface hold)
    let mut g = cone_group(true);
    g[0] = PartIn::new("K1", "eccentric_cone")
        .v("D1", 1400.0)
        .v("D2", 400.0)
        .v("L", 900.0)
        .v("t", 40.0)
        .v("split", 2.0)
        .v("flat_side", 0.0);
    g[1] = PartIn::new("K1b", "tee_branch")
        .v("D", 250.0)
        .v("t", 9.0)
        .v("H", 800.0)
        .v("branch_x", 450.0)
        .v("dir", 180.0)
        .attached("K1");
    let r = run(&g);
    assert!(r[0].refusal.is_none(), "{:?}", r[0].refusal);
    assert!(
        r[0].mid_surface_dev.is_some_and(|d| d > 1.0),
        "{:?}",
        r[0].mid_surface_dev
    );
    assert_hold(&r[0], "conebranch_mid_surface_deviation");
    assert_hold(&r[1], "conebranch_mid_surface_deviation");
    // a short, steep, thick cone: the seams cannot clear the hole
    g[0] = PartIn::new("K1", "eccentric_cone")
        .v("D1", 1200.0)
        .v("D2", 300.0)
        .v("L", 300.0)
        .v("t", 50.0)
        .v("split", 2.0)
        .v("flat_side", 0.0);
    g[1] = PartIn::new("K1b", "tee_branch")
        .v("D", 250.0)
        .v("t", 9.0)
        .v("H", 800.0)
        .v("branch_x", 150.0)
        .v("dir", 180.0)
        .attached("K1");
    let r = run(&g);
    assert_hold(&r[0], "conebranch_seam_crosses_hole");
    assert_hold(&r[1], "conebranch_seam_crosses_hole");
}

// ---------------------------------------------------------------- structure and text

#[test]
fn every_hold_code_has_english_and_japanese_text_and_is_unique() {
    let codes = hold_codes();
    let mut u = codes.clone();
    u.sort_unstable();
    u.dedup();
    assert_eq!(u.len(), codes.len(), "a hold code is listed twice");
    let has_ja = |s: &str| {
        s.chars().any(|c| {
            ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c)
        })
    };
    for (code, en, ja) in HOLD_TEXT {
        assert!(
            !en.is_empty() && has_ja(ja),
            "{code}: text in both languages"
        );
    }
    // every hold or question code that a scenario above can raise has a text row
    let mut seen: Vec<String> = Vec::new();
    for set in [
        twelve(),
        cone_group(true),
        vec![
            tee_main("T1")
                .v("L", 6000.0)
                .v("branch_x", 3000.0)
                .v("branch_angle", 20.0),
            tee_branch("T1b", "T1"),
        ],
    ] {
        for r in run(&set) {
            for h in r.holds.iter().filter(|h| h.level == "need") {
                seen.push(h.code.clone());
            }
            for q in &r.questions {
                seen.push(q.code.to_string());
            }
        }
    }
    for c in seen {
        assert!(codes.contains(&c.as_str()), "{c} has no text row");
    }
}

#[test]
fn any_need_hold_question_or_refusal_means_no_dxf_and_no_final_mass() {
    // a broad scenario set; the structural rule is checked on every result of every part
    let mut sets: Vec<Vec<PartIn>> = vec![twelve(), cone_group(true), cone_group(false)];
    sets.push(vec![
        tee_main("T1")
            .v("L", 6000.0)
            .v("branch_x", 3000.0)
            .v("branch_angle", 20.0),
        tee_branch("T1b", "T1"),
        PartIn::new("T1p", "tee_pad")
            .v("t", 9.0)
            .v("pad_D", 800.0)
            .attached("T1"),
    ]);
    sets.push(vec![
        plate("P")
            .v("hole_d", 24.0)
            .v("hole_x", 500.0)
            .v("hole_y", 50.0)
            .v("hole_n", 1.0),
        cyl("C").v("split", 7.0),
    ]);
    sets.push(vec![tee_main("T1").v("e", -5.0), tee_branch("T1b", "T1")]);
    let mut n_hold = 0;
    for set in sets {
        let res = compute_order(&set, &rules());
        let rows = material_rows(&res);
        for r in &res.parts {
            let blocking = r.holds.iter().any(|h| h.level == "need")
                || !r.questions.is_empty()
                || r.refusal.is_some();
            assert_eq!(r.dxf.is_some(), r.dxf_allowed, "{}", r.no);
            if blocking || !r.certified {
                assert!(!r.dxf_allowed && r.dxf.is_none(), "{}", r.no);
            }
            if blocking {
                n_hold += 1;
                assert!(r.mass_provisional, "{}: held mass is provisional", r.no);
                for h in r.holds.iter().filter(|h| h.level == "need") {
                    assert!(r.dxf_blocked_by.contains(&h.code), "{}: {}", r.no, h.code);
                }
                for q in &r.questions {
                    assert!(
                        r.dxf_blocked_by.iter().any(|c| c == q.code),
                        "{}: {}",
                        r.no,
                        q.code
                    );
                }
                // a held part's mass never enters the material list
                assert!(
                    rows.iter()
                        .filter(|m| m.no == r.no || m.no.starts_with(&format!("{}#", r.no)))
                        .all(|m| m.mass_calc_kg.is_none()),
                    "{}: provisional mass leaked into the material list",
                    r.no
                );
            }
        }
    }
    assert!(n_hold >= 8, "the scenario set exercises holds ({n_hold})");
}

#[test]
fn engine_version_is_0_6_0_and_older_results_are_recompute() {
    assert!(
        nf_core::ENGINE_VERSION.starts_with("nf-engine 0.6.0+src."),
        "{}",
        nf_core::ENGINE_VERSION
    );
    assert!(!nf_core::engine_is_current(
        "nf-engine 0.4.12+src.0123456789abcdef"
    ));
    assert!(nf_core::engine_is_current(nf_core::ENGINE_VERSION));
    let res = compute_order(&[cyl("C1")], &rules());
    assert_eq!(res.engine, nf_core::ENGINE_VERSION);
}

#[test]
fn the_shape_refusal_codes_of_every_module_are_in_the_build_list() {
    use nf_core::{bend, conebranch, refusal, shapes, tee, tee_pad};
    let all: Vec<&str> = refusal::ALL.to_vec();
    let mut want: Vec<&str> = Vec::new();
    want.extend(shapes::SHAPE_REFUSAL_CODES);
    want.extend(bend::BEND_REFUSAL_CODES);
    want.extend(tee::TEE_REFUSAL_CODES);
    want.extend(tee::TEE_EXTRA_REFUSAL_CODES);
    want.extend(tee_pad::PAD_REFUSAL_CODES);
    want.extend(conebranch::CONEBRANCH_REFUSAL_CODES);
    for c in &want {
        assert!(
            all.contains(c),
            "{c} is a refusal code of a shape module but not in refusal::ALL"
        );
    }
    assert_eq!(want.len(), refusal::SHAPE_CODES.len());
    let mut a = want.clone();
    a.sort_unstable();
    let mut b = refusal::SHAPE_CODES.to_vec();
    b.sort_unstable();
    assert_eq!(a, b);
}

#[test]
fn order_rules_are_strict_and_change_the_result() {
    use serde_json::json;
    let r = OrderRules::from_json(&json!({"allow": 6.0, "seam_from": "top", "mount": "set_in", "cone_thickness": "radial", "stock": [[1500, 3000]]})).expect("rules");
    assert_eq!(r.allow, 6.0);
    assert!(!r.seam_from_bottom);
    assert_eq!(r.cone_thickness, ThicknessConvention::Radial);
    for bad in [
        json!({"bogus": 1}),
        json!({"mount": "on"}),
        json!({"allow": "x"}),
        json!({"stock": [[1]]}),
        json!(5),
    ] {
        assert!(OrderRules::from_json(&bad).is_err(), "{bad}");
    }
    assert!(OrderRules::from_json(&json!({})).is_ok());
    // a rule is not decoration: a smaller stock makes a pipe that fits the default stock not fit
    let small = OrderRules::from_json(&json!({"stock": [[1000, 1500]]})).expect("rules");
    let a = compute_order(&[cyl("C1")], &rules());
    let b = compute_order(&[cyl("C1")], &small);
    assert!(!a.parts[0].has_hold("stock_fit") && b.parts[0].has_hold("stock_fit"));
    assert!(a.rules.iter().any(|x| x["name"] == "order.stock"));
}

#[test]
fn a_main_with_its_own_branch_keys_and_no_branch_part_gets_a_hole() {
    let r = run(&[tee_main("T1")]);
    let m = &r[0];
    assert!(m.refusal.is_none(), "{:?}", m.refusal);
    assert_eq!(m.state, "computed", "{:?} {:?}", m.holds, m.questions);
    assert!(
        m.pattern.iter().any(|p| !p.holes.is_empty()),
        "the branch hole is on the plate"
    );
    assert!(m.unit_mass_kg.is_some_and(|k| k > 0.0));
}

#[test]
fn certified_unheld_tee_gets_a_dxf_and_a_held_or_uncertified_part_does_not() {
    let t = one(tee_main("T1"));
    assert!(t.certified && t.dxf_allowed, "{:?}", t.dxf_blocked_by);
    let text = t.dxf.as_deref().expect("certified unheld tee has a DXF");
    assert!(text.contains("CUT_OUTER") && text.contains("NF_NOT_RELEASED"));
    assert!(nf_dxf::read(text).is_ok());
    assert!(!t.area_certified && !t.extents_certified);

    // a tee with a missing input is held: no DXF
    let held = one(PartIn::new("T2", "tee_main")
        .v("D", 1016.0)
        .v("t", 12.0)
        .v("branch_D", 508.0)
        .v("branch_t", 9.0)
        .v("branch_x", 1000.0)
        .v("dir", 0.0));
    assert!(held.dxf.is_none() && !held.dxf_allowed, "{:?}", held.state);

    // a reducing bend is not certified: no DXF
    let bend = one(PartIn::new("B1", "miter_bend")
        .v("D", 1016.0)
        .v("t", 12.0)
        .v("R", 1500.0)
        .v("angle", 90.0)
        .v("joints", 2.0)
        .v("start_straight", 300.0)
        .v("end_straight", 300.0));
    assert!(!bend.certified && bend.dxf.is_none() && !bend.dxf_allowed);
    assert!(bend.dxf_blocked_by.contains(&"not_certified".to_string()));
}
