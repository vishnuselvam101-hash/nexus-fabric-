#![allow(dead_code)]
use nf_machine::*;
use std::path::PathBuf;

pub const ENGINE: &str = "eng-test-1";

pub fn profile(enc: TextEncoding, ver: DxfVersion) -> MachineProfile {
    let mut layer_map = std::collections::BTreeMap::new();
    layer_map.insert("CUT_OUTER".to_string(), "OUTER".to_string());
    layer_map.insert("CUT_INNER".to_string(), "INNER".to_string());
    layer_map.insert("NF_NOT_RELEASED".to_string(), "NOTE".to_string());
    MachineProfile {
        id: "PLASMA-A".into(),
        version: 1,
        name: "Plasma table A (test fixture)".into(),
        dxf_version: ver,
        text_encoding: enc,
        units: "mm".into(),
        layer_map,
        file_name: "{job}_{part}_{profile}.dxf".into(),
        kerf_mm: None,
        lead_in_mm: None,
        sources: vec!["test fixture, not a machine manual".into()],
        tested_by: None,
    }
}

pub fn dxf(text: &str, layer: &str) -> String {
    format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n0\nLAYER\n2\nCUT_OUTER\n70\n0\n0\nENDTAB\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n{layer}\n10\n0\n20\n0\n11\n10\n21\n0\n\
0\nTEXT\n8\n{layer}\n10\n1\n20\n1\n40\n2\n1\n{text}\n0\nENDSEC\n0\nEOF\n"
    )
}

pub fn cert(part: &str, bytes: &[u8]) -> Certificate {
    Certificate {
        snapshot_id: "snap-1".into(),
        part_id: part.into(),
        engine_id: ENGINE.into(),
        sha256: sha256_hex(bytes),
        verdict: CertVerdict::Pass,
        held: false,
    }
}

/// Fresh empty folder under cargo's per-target tmp dir (not /tmp).
pub fn dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
