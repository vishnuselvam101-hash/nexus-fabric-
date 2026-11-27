//! Part parameters: random requests per family, accepted only when NF's own engine accepts them.
//! Values are multiples of 0.5 mm (angles whole degrees) so that the printed number is exact.

use crate::rng::Rng;
use nf_core::job::{JobOutput, Request, run};
use serde_json::{Value, json};

pub const FAMILIES: [&str; 5] = [
    "cylinder_shell",
    "cone_frustum",
    "eccentric_reducer",
    "branch_stub",
    "header_with_branch",
];

fn draw(fam: &str, r: &mut Rng) -> Value {
    match fam {
        "cylinder_shell" => json!({"family": fam,
            "r_dev": r.stepped(40.0, 600.0, 0.5), "length": r.stepped(100.0, 2400.0, 0.5)}),
        "cone_frustum" => {
            let r1 = r.stepped(100.0, 600.0, 0.5);
            let r2 = r.stepped(30.0, (r1 * 0.8).floor().max(40.0), 0.5);
            json!({"family": fam, "r1": r1, "r2": r2, "h": r.stepped(120.0, 900.0, 0.5)})
        }
        "eccentric_reducer" => {
            let r1 = r.stepped(100.0, 500.0, 0.5);
            let r2 = r.stepped(30.0, (r1 * 0.8).floor().max(40.0), 0.5);
            let h = r.stepped(150.0, 900.0, 0.5);
            json!({"family": fam, "r1": r1, "r2": r2, "h": h, "e": r.stepped(0.0, (h * 0.25).floor(), 0.5)})
        }
        "branch_stub" => {
            let big = r.stepped(150.0, 600.0, 0.5);
            let small = r.stepped(40.0, (big * 0.7).floor(), 0.5);
            json!({"family": fam, "header_radius": big, "branch_radius": small, "branch_dev_radius": small,
                "beta_deg": r.int(30, 150), "delta": 0, "s_end": r.stepped(big * 1.6, big * 4.0, 0.5).floor()})
        }
        _ => {
            let big = r.stepped(200.0, 600.0, 0.5);
            let small = r.stepped(40.0, (big * 0.5).floor(), 0.5);
            let len = r.stepped(1000.0, 2400.0, 0.5);
            json!({"family": fam, "header_radius": big, "header_dev_radius": big, "header_length": len,
                "branch_station": r.stepped((len * 0.35).ceil(), (len * 0.65).floor(), 0.5),
                "branch_radius": small, "branch_dev_radius": small,
                "beta_deg": r.int(45, 135), "delta": 0, "s_end": r.stepped(big * 2.0, big * 4.0, 0.5).floor(),
                "surface_reference": "outside"})
        }
    }
}

/// Draw requests until the engine accepts one. Returns the accepted request and its output.
pub fn sample(fam: &str, r: &mut Rng) -> (Request, JobOutput) {
    for _ in 0..200 {
        let v = draw(fam, r);
        let Ok(req) = Request::parse_strict(&v) else {
            continue;
        };
        let out = run(&req, "reader-data");
        if out.outcome.is_none() && !out.patterns.is_empty() {
            return (req, out);
        }
    }
    panic!("no accepted request for family {fam}");
}

/// Re-run a stored request through the engine (used by the truth re-query test).
pub fn rerun(req_json: &Value) -> Option<JobOutput> {
    let req = Request::parse_strict(req_json).ok()?;
    Some(run(&req, "reader-data"))
}
