//! Closed-form values of generator-owned shapes (v2). A printed derived number names one of these formulas and
//! stores its inputs in the truth line, so the value can be re-derived from the truth alone (see `source::Source::Derived`).
use std::collections::BTreeMap;
use std::f64::consts::PI;

pub type Inputs = BTreeMap<String, f64>;

/// Bend allowance factor K (ISO/JIS sheet metal, neutral-axis position).
pub const K_FACTOR: f64 = 0.44;

pub fn derive(name: &str, i: &Inputs) -> Option<f64> {
    let g = |k: &str| i.get(k).copied();
    let v = match name {
        // chord between adjacent holes on a bolt circle of diameter bcd with n holes
        "bolt_pitch" => g("bcd")? * (PI / g("n")?).sin(),
        // radial width of a ring
        "ring_width" => (g("od")? - g("id")?) / 2.0,
        "rect_perimeter" => 2.0 * (g("a")? + g("b")?),
        // overall length of a stadium slot (centre distance + width)
        "slot_overall" => g("slot_c")? + g("slot_w")?,
        // 2:1 ellipsoidal head: height = D / (2 * ratio) with ratio 2
        "head_height" => g("od")? / (2.0 * g("ratio")?),
        "tank_length" => g("l")? + 2.0 * g("od")? / (2.0 * g("ratio")?),
        // centreline length of a bend of centreline radius r over theta degrees
        "arc_length" => g("r")? * g("theta")?.to_radians(),
        // mitre cut angle from square: the total deflection split over the two pieces of each of the n-1 joints
        "mitre_cut" => g("theta")? / (2.0 * (g("n")? - 1.0)),
        "cone_half_angle" => ((g("r1")? - g("r2")?) / g("h")?).atan().to_degrees(),
        "cone_slant" => (g("h")?.powi(2) + (g("r1")? - g("r2")?).powi(2)).sqrt(),
        // flat blank of a two-flange sheet: straight flat lengths + bend allowance theta * (R + K t)
        "flat_blank" => g("l1")? + g("l2")? + bend_allow(i)?,
        "bend_allow" => bend_allow(i)?,
        // height of two branch tips above the main axis
        "wye_span" => {
            g("l1")? * g("a1")?.to_radians().sin() + g("l2")? * g("a2")?.to_radians().sin()
        }
        // outlet height of a lateral tee measured to the outer pipe surface
        "tee_outlet" => g("hb")? + g("od")? / 2.0,
        _ => return None,
    };
    Some(v)
}

fn bend_allow(i: &Inputs) -> Option<f64> {
    let g = |k: &str| i.get(k).copied();
    Some(g("theta")?.to_radians() * (g("r")? + K_FACTOR * g("t")?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inp(p: &[(&str, f64)]) -> Inputs {
        p.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    }

    #[test]
    fn closed_forms() {
        assert!(
            (derive("bolt_pitch", &inp(&[("bcd", 100.0), ("n", 4.0)])).unwrap() - 70.7107).abs()
                < 1e-3
        );
        assert_eq!(
            derive("ring_width", &inp(&[("od", 200.0), ("id", 120.0)])),
            Some(40.0)
        );
        assert_eq!(
            derive("head_height", &inp(&[("od", 800.0), ("ratio", 2.0)])),
            Some(200.0)
        );
        assert!(
            (derive("arc_length", &inp(&[("r", 100.0), ("theta", 90.0)])).unwrap() - 157.0796)
                .abs()
                < 1e-3
        );
        assert_eq!(
            derive("mitre_cut", &inp(&[("theta", 90.0), ("n", 3.0)])),
            Some(22.5)
        );
        assert_eq!(
            derive("slot_overall", &inp(&[("slot_c", 20.0), ("slot_w", 8.0)])),
            Some(28.0)
        );
        assert_eq!(derive("nope", &inp(&[])), None);
    }

    #[test]
    fn flat_blank_uses_bend_allowance() {
        let v = derive(
            "flat_blank",
            &inp(&[
                ("l1", 50.0),
                ("l2", 40.0),
                ("theta", 90.0),
                ("r", 4.0),
                ("t", 2.0),
            ]),
        )
        .unwrap();
        assert!((v - (90.0 + 90f64.to_radians() * (4.0 + 0.44 * 2.0))).abs() < 1e-9);
    }
}
