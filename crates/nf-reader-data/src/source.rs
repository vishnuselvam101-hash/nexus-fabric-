//! Where a printed number comes from. Every number on a page carries a `Source` that can be re-queried
//! against the engine (`resolve`): the generator never invents a drawing value.

use nf_core::job::{JobOutput, Request};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// A parameter of the request the engine accepted (echoed in `JobOutput::request`), times `factor`.
    Request { field: String, factor: f64 },
    /// A named quantity of a part's pattern, times `factor`.
    Quantity {
        part: String,
        name: String,
        factor: f64,
    },
    /// The length the engine reports for one outline or opening curve.
    CurveLength { part: String, curve: String },
    /// Extent of the engine's pattern points along one axis ("x" or "y"), outer outline.
    Bbox { part: String, axis: String },
    /// Printed number that is not an engine measure: a list position, a quantity of parts.
    Structure { what: String },
    /// Tolerance table of the stated general-tolerance standard (not an engine value).
    ToleranceTable { class: String },
    /// v2: a parameter of the generator-owned shape spec stored in the truth line (`spec.params`), times `factor`.
    Spec { field: String, factor: f64 },
    /// v2: closed-form value of the stored inputs (see `derive::derive`); re-derivable from the truth line alone.
    Derived {
        name: String,
        inputs: std::collections::BTreeMap<String, f64>,
    },
}

pub fn request_field(req: &Request, field: &str) -> Option<f64> {
    serde_json::to_value(req).ok()?.get(field)?.as_f64()
}

/// The engine value a source names, re-read from a job output. `None` when the source is not an engine value.
pub fn resolve(out: &JobOutput, src: &Source) -> Option<f64> {
    match src {
        Source::Request { field, factor } => Some(request_field(&out.request, field)? * factor),
        Source::Quantity { part, name, factor } => {
            let p = out.patterns.iter().find(|p| &p.part == part)?;
            Some(*p.quantities.get(name)? * factor)
        }
        Source::CurveLength { part, curve } => {
            let p = out.patterns.iter().find(|p| &p.part == part)?;
            p.curves
                .iter()
                .chain(p.holes.iter())
                .find(|c| &c.name == curve)
                .map(|c| c.length)
        }
        Source::Bbox { part, axis } => {
            let p = out.patterns.iter().find(|p| &p.part == part)?;
            let i = if axis == "x" { 0 } else { 1 };
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for c in p.curves.iter().filter(|c| c.layer == "CUT_OUTER") {
                for q in &c.points {
                    lo = lo.min(q[i]);
                    hi = hi.max(q[i]);
                }
            }
            (hi >= lo).then_some(hi - lo)
        }
        Source::Structure { .. } | Source::ToleranceTable { .. } => None,
        Source::Spec { .. } | Source::Derived { .. } => None,
    }
}

/// Round half away from zero to `dec` decimals through the decimal text, so the printed text and the truth agree.
pub fn round_to(v: f64, dec: usize) -> f64 {
    format!("{v:.dec$}").parse().unwrap_or(v)
}

/// ISO 2768-1 / JIS B 0405 permitted linear deviation (mm, +/-) for a nominal size and class f, m, c.
/// `None` where the standard gives no value: class f stops at 2000 mm.
pub fn linear_tolerance(class: &str, nominal: f64) -> Option<f64> {
    if class == "f" && nominal > 2000.0 {
        return None;
    }
    let rows: [(f64, [f64; 3]); 8] = [
        (3.0, [0.05, 0.1, 0.2]),
        (6.0, [0.05, 0.1, 0.3]),
        (30.0, [0.1, 0.2, 0.5]),
        (120.0, [0.15, 0.3, 0.8]),
        (400.0, [0.2, 0.5, 1.2]),
        (1000.0, [0.3, 0.8, 2.0]),
        (2000.0, [0.5, 1.2, 3.0]),
        (f64::INFINITY, [0.5, 2.0, 4.0]),
    ];
    let col = match class {
        "f" => 0,
        "c" => 2,
        _ => 1,
    };
    rows.iter()
        .find(|(lim, _)| nominal <= *lim)
        .map(|(_, v)| v[col])
}
