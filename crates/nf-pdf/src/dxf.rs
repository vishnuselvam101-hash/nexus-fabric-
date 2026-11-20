//! DXF R12 (AC1009) output.
//!
//! The entity layout is the one `nf-dxf` writes and reads: POLYLINE with VERTEX
//! and SEQEND, and TEXT. Numbers are written with six decimals (micrometre
//! resolution in millimetres). Entities are appended to a body that is bounded
//! by the output-size limit; the header and tables are added at the end, when
//! the counts they carry are known.

use std::fmt::Write as _;

use crate::limits::{limit, Fault};

pub const LAYER_STROKE: &str = "PDF_STROKE";
pub const LAYER_DASHED: &str = "PDF_DASHED";
pub const LAYER_FILL: &str = "PDF_FILL";
pub const LAYER_TEXT: &str = "PDF_TEXT";
pub const LAYER_NOTE: &str = "NF_NOT_RELEASED";

/// Layers in table order, with their colour numbers.
pub const LAYERS: [(&str, u8); 5] = [
    (LAYER_STROKE, 7),
    (LAYER_DASHED, 3),
    (LAYER_FILL, 5),
    (LAYER_TEXT, 2),
    (LAYER_NOTE, 1),
];

/// A number as written in the file: six decimals, never "-0.000000".
pub fn num(v: f64) -> String {
    let s = format!("{v:.6}");
    if s == "-0.000000" {
        "0.000000".to_string()
    } else {
        s
    }
}

/// Entities written so far, with the drawing extents they cover.
pub struct Body {
    pub text: String,
    limit: usize,
    pub min: (f64, f64),
    pub max: (f64, f64),
    pub entities: u64,
    /// Polylines written (path geometry, as opposed to text).
    pub polylines: u64,
}

impl Body {
    pub fn new(limit: usize) -> Self {
        Body {
            text: String::new(),
            limit,
            min: (f64::MAX, f64::MAX),
            max: (f64::MIN, f64::MIN),
            entities: 0,
            polylines: 0,
        }
    }

    fn extend_box(&mut self, p: (f64, f64)) {
        self.min = (self.min.0.min(p.0), self.min.1.min(p.1));
        self.max = (self.max.0.max(p.0), self.max.1.max(p.1));
    }

    fn push(&mut self, s: &str) -> Result<(), Fault> {
        if self.text.len().saturating_add(s.len()) > self.limit {
            return limit("output_bytes", "the DXF output exceeds the size limit");
        }
        self.text.push_str(s);
        Ok(())
    }

    /// One POLYLINE. `closed` sets the closing flag; the closing edge is not repeated.
    pub fn polyline(&mut self, layer: &str, pts: &[(f64, f64)], closed: bool) -> Result<(), Fault> {
        let mut s = String::new();
        let _ = write!(
            s,
            "  0\nPOLYLINE\n  8\n{layer}\n 66\n     1\n 70\n{}\n",
            if closed { 1 } else { 0 }
        );
        for p in pts {
            let _ = write!(
                s,
                "  0\nVERTEX\n  8\n{layer}\n 10\n{}\n 20\n{}\n",
                num(p.0),
                num(p.1)
            );
            self.extend_box(*p);
        }
        s.push_str(&format!("  0\nSEQEND\n  8\n{layer}\n"));
        self.entities += 1;
        self.polylines += 1;
        self.push(&s)
    }

    /// One TEXT annotation. `width` is the width factor (41), `rotation` in degrees (50).
    pub fn text(
        &mut self,
        layer: &str,
        at: (f64, f64),
        height: f64,
        rotation: f64,
        width: f64,
        text: &str,
    ) -> Result<(), Fault> {
        let mut s = String::new();
        let _ = write!(
            s,
            "  0\nTEXT\n  8\n{layer}\n 10\n{}\n 20\n{}\n 40\n{}\n  1\n{text}\n",
            num(at.0),
            num(at.1),
            num(height)
        );
        if rotation != 0.0 {
            let _ = writeln!(s, " 50\n{}", num(rotation));
        }
        if width != 1.0 {
            let _ = writeln!(s, " 41\n{}", num(width));
        }
        s.push_str("  7\nSTANDARD\n");
        self.extend_box(at);
        self.extend_box((
            at.0 + height * text.chars().count() as f64 * 0.6,
            at.1 + height,
        ));
        self.entities += 1;
        self.push(&s)
    }
}

/// Header facts: the leading comment lines and the extents.
pub struct Header {
    pub comments: Vec<(String, String)>,
}

/// Complete DXF text: comments, HEADER, TABLES, ENTITIES and EOF around the body.
pub fn assemble(body: &Body, header: &Header) -> String {
    let (minx, miny, maxx, maxy) = if body.entities == 0 {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        (body.min.0, body.min.1, body.max.0, body.max.1)
    };
    let mut o = String::new();
    for (k, v) in &header.comments {
        let _ = write!(o, "999\n{k}: {v}\n");
    }
    let _ = write!(
        o,
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1009\n  9\n$LUNITS\n 70\n2\n  9\n$AUNITS\n 70\n0\n  9\n$ANGBASE\n 50\n0.0\n  9\n$ANGDIR\n 70\n0\n  9\n$EXTMIN\n 10\n{}\n 20\n{}\n  9\n$EXTMAX\n 10\n{}\n 20\n{}\n  0\nENDSEC\n",
        num(minx),
        num(miny),
        num(maxx),
        num(maxy)
    );
    o.push_str("  0\nSECTION\n  2\nTABLES\n  0\nTABLE\n  2\nLTYPE\n 70\n1\n  0\nLTYPE\n  2\nCONTINUOUS\n 70\n0\n  3\nSolid line\n 72\n65\n 73\n0\n 40\n0.0\n  0\nENDTAB\n");
    let _ = write!(o, "  0\nTABLE\n  2\nLAYER\n 70\n{}\n", LAYERS.len());
    for (name, colour) in LAYERS {
        let _ = write!(
            o,
            "  0\nLAYER\n  2\n{name}\n 70\n0\n 62\n{colour}\n  6\nCONTINUOUS\n"
        );
    }
    o.push_str("  0\nENDTAB\n  0\nTABLE\n  2\nSTYLE\n 70\n1\n  0\nSTYLE\n  2\nSTANDARD\n 70\n0\n 40\n0.0\n 41\n1.0\n 50\n0.0\n 71\n0\n 42\n1.0\n  3\ntxt\n  4\n\n  0\nENDTAB\n  0\nENDSEC\n");
    o.push_str("  0\nSECTION\n  2\nENTITIES\n");
    o.push_str(&body.text);
    o.push_str("  0\nENDSEC\n  0\nEOF\n");
    o
}
