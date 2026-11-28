//! Sheet DXF writer. Only a `CheckedNest` reaches this code. Parts go on layer CUT as closed polylines; the
//! marking (NOT RELEASED, part numbers, the UNVERIFIED rules used) goes on layer MARK.

use crate::check::CheckedNest;
use crate::{RULE_EDGE, RULE_GAP, RULE_KERF, RULE_STOCK, utilisation};

pub const NOT_RELEASED: &str = "NOT RELEASED";

fn pair(o: &mut String, code: u32, v: &str) {
    o.push_str(&format!("{code}\n{v}\n"));
}

fn num(v: f64) -> String {
    format!("{v:.4}")
}

/// The DXF text of sheet `idx` (0-based), or None when there is no such sheet.
pub fn sheet_dxf(c: &CheckedNest, idx: usize) -> Option<String> {
    let n = c.nest();
    let sheet = n.sheets.get(idx)?;
    let (per, _) = utilisation(n);
    let mut o = String::new();
    pair(&mut o, 0, "SECTION");
    pair(&mut o, 2, "HEADER");
    pair(&mut o, 9, "$INSUNITS");
    pair(&mut o, 70, "4");
    pair(&mut o, 0, "ENDSEC");
    pair(&mut o, 0, "SECTION");
    pair(&mut o, 2, "TABLES");
    pair(&mut o, 0, "TABLE");
    pair(&mut o, 2, "LAYER");
    pair(&mut o, 70, "2");
    for (name, colour) in [("CUT", "1"), ("MARK", "3")] {
        pair(&mut o, 0, "LAYER");
        pair(&mut o, 2, name);
        pair(&mut o, 70, "0");
        pair(&mut o, 62, colour);
        pair(&mut o, 6, "CONTINUOUS");
    }
    pair(&mut o, 0, "ENDTAB");
    pair(&mut o, 0, "ENDSEC");
    pair(&mut o, 0, "SECTION");
    pair(&mut o, 2, "ENTITIES");
    for p in &sheet.placed {
        // classic POLYLINE with VERTEX records: the form the house reader (nf-dxf) reads back
        pair(&mut o, 0, "POLYLINE");
        pair(&mut o, 8, "CUT");
        pair(&mut o, 66, "1");
        pair(&mut o, 70, "1");
        for &(x, y) in &p.pts {
            pair(&mut o, 0, "VERTEX");
            pair(&mut o, 8, "CUT");
            pair(&mut o, 10, &num(x));
            pair(&mut o, 20, &num(y));
        }
        pair(&mut o, 0, "SEQEND");
        pair(&mut o, 8, "CUT");
    }
    let mark = |o: &mut String, x: f64, y: f64, h: f64, t: &str| {
        pair(o, 0, "TEXT");
        pair(o, 8, "MARK");
        pair(o, 10, &num(x));
        pair(o, 20, &num(y));
        pair(o, 40, &num(h));
        pair(o, 1, t);
    };
    mark(&mut o, 0.0, -20.0, 10.0, NOT_RELEASED);
    let s = &n.rules;
    mark(
        &mut o,
        0.0,
        -35.0,
        5.0,
        &format!(
            "SHEET {} OF {} | {} x {} mm | {} {} | {} {} | {} {} | {} {} (UNVERIFIED) | UTIL {:.1} %",
            idx + 1,
            n.sheets.len(),
            s.sheet_w,
            s.sheet_h,
            RULE_KERF,
            s.kerf_mm,
            RULE_GAP,
            s.part_gap_mm,
            RULE_EDGE,
            s.edge_mm,
            RULE_STOCK,
            s.sheet_w,
            per[idx]
        ),
    );
    for p in &sheet.placed {
        let label = format!("{} #{}", p.id, p.copy + 1);
        let (x, y) = p
            .pts
            .iter()
            .fold((f64::MAX, f64::MAX), |a, q| (a.0.min(q.0), a.1.min(q.1)));
        mark(&mut o, x, y - 8.0, 4.0, &label);
    }
    pair(&mut o, 0, "ENDSEC");
    pair(&mut o, 0, "EOF");
    Some(o)
}
