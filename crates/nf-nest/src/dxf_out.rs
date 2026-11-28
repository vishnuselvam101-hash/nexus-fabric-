//! Sheet DXF writer. Only a `CheckedNest` reaches this code. Parts go on layer CUT as closed polylines; the
//! marking (NOT RELEASED / 未リリース, part ids, the UNVERIFIED rules used) goes on layer MARK.
//!
//! Two formats: `R12Sjis` (AC1009, TEXT entities, Shift_JIS bytes, the hand-off format for older CAM) and
//! `R2000Mtext` (AC1015, MTEXT with \U+XXXX Unicode escapes, so the file stays ASCII and is exact in UTF-8).

use crate::check::CheckedNest;
use crate::{RULE_EDGE, RULE_GAP, RULE_KERF, RULE_ROT, RULE_STOCK, utilisation};
use encoding_rs::SHIFT_JIS;

pub const NOT_RELEASED_EN: &str = "NOT RELEASED";
pub const NOT_RELEASED_JA: &str = "未リリース";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DxfFormat {
    R12Sjis,
    R2000Mtext,
}

fn pair(o: &mut String, code: u32, v: &str) {
    o.push_str(&format!("{code}\n{v}\n"));
}

fn num(v: f64) -> String {
    format!("{v:.4}")
}

/// Unicode text as MTEXT inline codes: ASCII stays, everything else becomes \U+XXXX.
pub fn mtext_escape(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        if ch.is_ascii() {
            o.push(ch);
        } else {
            let mut b = [0u16; 2];
            for u in ch.encode_utf16(&mut b) {
                o.push_str(&format!("\\U+{u:04X}"));
            }
        }
    }
    o
}

/// The DXF bytes of sheet `idx` (0-based), or None when there is no such sheet.
pub fn sheet_dxf(c: &CheckedNest, idx: usize, fmt: DxfFormat) -> Option<Vec<u8>> {
    let n = c.nest();
    let sheet = n.sheets.get(idx)?;
    let (per, _) = utilisation(n);
    let r12 = fmt == DxfFormat::R12Sjis;
    let mut o = String::new();
    pair(&mut o, 0, "SECTION");
    pair(&mut o, 2, "HEADER");
    pair(&mut o, 9, "$ACADVER");
    pair(&mut o, 1, if r12 { "AC1009" } else { "AC1015" });
    pair(&mut o, 9, "$DWGCODEPAGE");
    pair(&mut o, 3, if r12 { "ANSI_932" } else { "UTF-8" });
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
    // marking: each text is written in the chosen format
    let mark = |o: &mut String, x: f64, y: f64, h: f64, t: &str| {
        if r12 {
            pair(o, 0, "TEXT");
            pair(o, 8, "MARK");
            pair(o, 10, &num(x));
            pair(o, 20, &num(y));
            pair(o, 40, &num(h));
            pair(o, 1, t);
        } else {
            pair(o, 0, "MTEXT");
            pair(o, 8, "MARK");
            pair(o, 10, &num(x));
            pair(o, 20, &num(y));
            pair(o, 40, &num(h));
            pair(o, 1, &mtext_escape(t));
        }
    };
    let s = &n.rules;
    mark(
        &mut o,
        0.0,
        -20.0,
        10.0,
        &format!("{NOT_RELEASED_EN} / {NOT_RELEASED_JA}"),
    );
    mark(
        &mut o,
        0.0,
        -35.0,
        5.0,
        &format!(
            "SHEET {} OF {} / シート {}/{} | {} x {} mm | {} {} / {} {} / {} {} / {} {} (UNVERIFIED / 未確認) | {} {} | UTIL {:.1} %",
            idx + 1,
            n.sheets.len(),
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
            RULE_ROT,
            s.rotations
                .iter()
                .map(|r| r.to_string())
                .collect::<Vec<_>>()
                .join(","),
            per[idx]
        ),
    );
    for p in &sheet.placed {
        let label = format!("{} #{} / 部品 {}", p.id, p.copy + 1, p.id);
        let (x, y) = p
            .pts
            .iter()
            .fold((f64::MAX, f64::MAX), |a, q| (a.0.min(q.0), a.1.min(q.1)));
        mark(&mut o, x, y - 8.0, 4.0, &label);
    }
    pair(&mut o, 0, "ENDSEC");
    pair(&mut o, 0, "EOF");
    if r12 {
        // the whole file is Shift_JIS; only the MARK text is non-ASCII, and it was encoded above
        let (bytes, _, _) = SHIFT_JIS.encode(&o);
        Some(bytes.into_owned())
    } else {
        Some(o.into_bytes())
    }
}
