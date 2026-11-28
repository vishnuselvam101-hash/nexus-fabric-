//! Quote geometry checked against closed forms, on results produced by the real engine.
use nf_core::order_compute::{compute_order, to_json, OrderRules, PartIn};
use nf_estimate::{csv, estimate, Rates, CutBand};
use serde_json::Value;

const RHO: f64 = 7850.0 * 1e-9; // kg per mm3

fn result(parts: &[PartIn]) -> Value {
    to_json(&compute_order(parts, &OrderRules::default()))
}

fn set_rates() -> Rates {
    Rates {
        currency: "JPY".into(),
        material_price_per_kg: 2.0,
        cut_cost_per_m: vec![CutBand { max_t_mm: 25.0, per_m: 100.0 }],
        weld_cost_per_m: 300.0,
        labour_rate_per_h: 1000.0,
        labour_min_per_piece: 1.0,
        margin_pct: 10.0,
        ..Rates::default()
    }
}

fn close(a: f64, b: f64, rel: f64) -> bool {
    (a - b).abs() <= rel * b.abs().max(1e-12)
}

#[test]
fn plate_mass_and_cut_match_closed_form() {
    let v = result(&[PartIn::new("P1", "plate").v("t", 16.0).v("plate_W", 400.0).v("plate_H", 300.0)]);
    let e = estimate(&v, &Rates::default()).unwrap();
    assert_eq!(e.parts.len(), 1, "plate must be computed: {:?}", e.not_priced);
    let p = &e.parts[0];
    assert!(close(p.mass_kg, 400.0 * 300.0 * 16.0 * RHO, 1e-9));
    assert!(close(p.cut_m, 2.0 * (400.0 + 300.0) / 1000.0, 1e-9));
    assert_eq!(p.pierces, 1);
    assert_eq!(p.weld_m, 0.0);
}

#[test]
fn pipe_mass_cut_and_seam_match_closed_form() {
    let v = result(&[PartIn::new("C1", "cylinder").v("D", 200.0).v("t", 4.0).v("L", 1000.0).v("split", 1.0)]);
    let e = estimate(&v, &Rates::default()).unwrap();
    assert_eq!(e.parts.len(), 1, "cylinder must be computed: {:?}", e.not_priced);
    let p = &e.parts[0];
    // engine convention (shapes::cylinder): development on the mid-surface, circumference PI * (D - t)
    let circ = std::f64::consts::PI * (200.0 - 4.0);
    assert!(close(p.mass_kg, 1000.0 * circ * 4.0 * RHO, 1e-6), "mass {}", p.mass_kg);
    assert!(close(p.cut_m, 2.0 * (1000.0 + circ) / 1000.0, 1e-6), "cut {}", p.cut_m);
    assert!(close(p.weld_m, 1.0, 1e-9));
}

#[test]
fn ring_mass_matches_closed_form() {
    let v = result(&[PartIn::new("RG1", "ring").v("OD", 800.0).v("ID", 500.0).v("t", 6.0).v("split", 1.0)]);
    let e = estimate(&v, &Rates::default()).unwrap();
    assert_eq!(e.parts.len(), 1, "ring must be computed: {:?}", e.not_priced);
    let area = std::f64::consts::PI * (400.0_f64.powi(2) - 250.0_f64.powi(2));
    assert!(close(e.parts[0].mass_kg, area * 6.0 * RHO, 1e-6), "mass {}", e.parts[0].mass_kg);
}

#[test]
fn held_part_is_not_priced() {
    let v = result(&[
        PartIn::new("OK1", "plate").v("t", 16.0).v("plate_W", 400.0).v("plate_H", 300.0),
        PartIn::new("BAD", "plate").v("plate_W", 400.0).v("plate_H", 300.0),
    ]);
    let e = estimate(&v, &set_rates()).unwrap();
    assert!(e.not_priced.iter().any(|h| h.no == "BAD"), "held part must be listed: {:?}", e.not_priced);
    assert!(e.parts.iter().all(|p| p.no != "BAD"));
    assert_eq!(e.status, "incomplete");
}

#[test]
fn zero_rates_hold_every_part() {
    let v = result(&[PartIn::new("P1", "plate").v("t", 16.0).v("plate_W", 400.0).v("plate_H", 300.0)]);
    let e = estimate(&v, &Rates::default()).unwrap();
    assert!(e.parts[0].costs.is_none());
    assert!(e.parts[0].reason.as_ref().unwrap().en.starts_with("set your rates"));
    assert!(e.rates_unset.contains(&"material_price_per_kg"));
    assert_eq!(e.totals.price, 0.0);
    assert_eq!(e.status, "incomplete");
}

#[test]
fn priced_draft_matches_hand_calculation() {
    let v = result(&[PartIn::new("P1", "plate").v("t", 16.0).v("plate_W", 400.0).v("plate_H", 300.0)]);
    let mut r = set_rates();
    r.currency = "JPY".into();
    let e = estimate(&v, &r).unwrap();
    let material = 15.072 * 2.0;
    let cut = 1.4 * 100.0;
    let labour = 1.0 / 60.0 * 1000.0;
    let price = (material + cut + labour) * 1.1;
    assert_eq!(e.status, "draft");
    assert!(close(e.totals.price, price, 1e-9), "price {}", e.totals.price);
}

#[test]
fn csv_is_formula_safe() {
    let mut v = result(&[PartIn::new("P1", "plate").v("t", 16.0).v("plate_W", 400.0).v("plate_H", 300.0)]);
    v["parts"][0]["name"] = Value::String("=HYPERLINK(\"http://x\")".into());
    let e = estimate(&v, &set_rates()).unwrap();
    let text = String::from_utf8(csv(&e, false)).unwrap();
    assert!(text.contains("'=HYPERLINK"), "{text}");
    assert!(!text.lines().any(|l| l.split(',').nth(1).is_some_and(|c| c.starts_with('='))));
}
