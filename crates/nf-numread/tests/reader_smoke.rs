//! End-to-end check on one synthetic truth page (generator output, not a partner drawing).
//! The reader is slow in debug builds: run with `cargo test --release -p nf-numread`.
use nf_numread::{image_in, Kind, Reader, DET_FILE};
use std::path::PathBuf;

const PAGE: &[u8] = include_bytes!("data/synth-test-00000.png");

/// The model directory, or None (with a message) when the models were not fetched, e.g. on CI.
fn model_dir() -> Option<PathBuf> {
    let d = nf_numread::default_model_dir();
    if d.join(DET_FILE).exists() {
        Some(d)
    } else {
        eprintln!(
            "SKIPPED: models not found in {}; run crates/nf-numread/scripts/fetch-models.sh",
            d.display()
        );
        None
    }
}

#[test]
fn finds_known_numbers_on_synthetic_page() {
    if model_dir().is_none() {
        return;
    }
    let reader = Reader::new().expect("models load and match their SHA-256");
    let g = image_in::decode_png(PAGE).expect("png");
    let items = reader.read_gray(&g).expect("read");
    assert!(!items.is_empty());
    for it in &items {
        assert!(
            it.bbox[0] >= 0.0 && it.bbox[1] >= 0.0,
            "box inside page: {:?}",
            it.bbox
        );
        assert!(
            it.bbox[2] <= g.w as f32 + 1.0 && it.bbox[3] <= g.h as f32 + 1.0,
            "box inside page: {:?}",
            it.bbox
        );
        assert!((0.0..=1.0).contains(&it.confidence));
    }
    // truth items of this page (ground truth from the generator, exact text as printed)
    let dia = items
        .iter()
        .find(|i| i.text == "φ301.0")
        .expect("φ301.0 read");
    assert_eq!(dia.kind, Kind::Diameter);
    assert_eq!(dia.value, Some(301.0));
    // truth: 2372.5 is printed three times (two dimensions and the Blank H cell of the BOM)
    assert_eq!(items.iter().filter(|i| i.text == "2372.5").count(), 3);
    assert!(items
        .iter()
        .any(|i| i.text == "2026-10-25" && i.value.is_none()));
}

/// Truth boxes of the page (generator ground truth): each must be found with IoU >= 0.5.
#[test]
fn truth_boxes_are_found() {
    if model_dir().is_none() {
        return;
    }
    let reader = Reader::new().expect("models");
    let g = image_in::decode_png(PAGE).expect("png");
    let items = reader.read_gray(&g).expect("read");
    let truth: [(&str, [f32; 4]); 5] = [
        ("φ301.0", [127.1, 268.8, 204.9, 289.7]),
        ("2372.5", [455.7, 333.2, 533.5, 350.0]),
        ("2372.5", [526.1, 869.4, 603.9, 886.2]),
        ("945.6", [1523.2, 901.5, 1577.4, 915.6]),
        ("2372.5", [1630.9, 901.5, 1696.3, 915.6]),
    ];
    for (text, b) in truth {
        let hit = items
            .iter()
            .any(|i| i.text == text && iou(i.bbox, b) >= 0.5);
        assert!(hit, "{text} at {b:?} not read");
    }
}

fn iou(a: [f32; 4], b: [f32; 4]) -> f32 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let inter = w * h;
    let area = |r: [f32; 4]| (r[2] - r[0]) * (r[3] - r[1]);
    inter / (area(a) + area(b) - inter)
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nf-numread-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("scratch dir");
    d
}

fn expect_refused(r: Result<Reader, nf_numread::Error>, what: &str) {
    match r {
        Ok(_) => panic!("{what}: reader accepted a model that does not match its SHA-256"),
        Err(e) => assert!(
            e.to_string().contains("pinned SHA-256"),
            "{what}: wrong error: {e}"
        ),
    }
}

/// A file whose hash differs from the pinned one is refused before it is parsed
/// (needs no real models, so it also runs on CI).
#[test]
fn refuses_wrong_sha256() {
    let d = scratch("wrongsha");
    for f in [
        nf_numread::DET_FILE,
        nf_numread::REC_FILE,
        nf_numread::DICT_FILE,
    ] {
        std::fs::write(d.join(f), b"not the model").expect("write");
    }
    expect_refused(
        Reader::from_dir(&d, nf_numread::Params::default()),
        "garbage files",
    );
    let _ = std::fs::remove_dir_all(&d);
}

/// One flipped byte in the real detector is refused.
#[test]
fn refuses_flipped_byte_in_real_model() {
    let Some(src) = model_dir() else { return };
    let d = scratch("flip");
    for f in [nf_numread::REC_FILE, nf_numread::DICT_FILE] {
        std::fs::copy(src.join(f), d.join(f)).expect("copy");
    }
    let mut det = std::fs::read(src.join(DET_FILE)).expect("read det");
    let mid = det.len() / 2;
    det[mid] ^= 1;
    std::fs::write(d.join(DET_FILE), det).expect("write det");
    expect_refused(
        Reader::from_dir(&d, nf_numread::Params::default()),
        "flipped byte",
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn missing_model_dir_is_an_error_not_a_panic() {
    let d = scratch("missing").join("nothing-here");
    let e = Reader::from_dir(&d, nf_numread::Params::default())
        .err()
        .expect("must fail");
    assert!(e.to_string().contains("fetch-models.sh"), "{e}");
}

/// A model file above the size cap is refused before it is read or hashed.
#[test]
fn oversized_model_file_is_refused_before_hashing() {
    let d = scratch("big");
    for f in [nf_numread::REC_FILE, nf_numread::DICT_FILE] {
        std::fs::write(d.join(f), b"x").expect("write");
    }
    let f = std::fs::File::create(d.join(DET_FILE)).expect("create");
    f.set_len(nf_numread::MAX_MODEL_BYTES + 1).expect("sparse");
    drop(f);
    let e = Reader::from_dir(&d, nf_numread::Params::default())
        .err()
        .expect("must fail");
    assert!(e.to_string().contains("too large"), "{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// PZ-2: the second read of one box returns a stored verdict with its provenance. A correct value
/// over a box that holds another number must be rejected; a matching one must agree.
#[test]
fn verify_box_reports_a_check_with_provenance() {
    if model_dir().is_none() {
        return;
    }
    let reader = Reader::new().expect("models");
    let v = reader
        .verify_box(
            PAGE,
            [127.1, 268.8, 204.9, 289.7],
            "D",
            "φ301.0",
            301.0,
            "read",
        )
        .expect("verify");
    let json: serde_json::Value = serde_json::from_str(&v.to_json()).expect("json");
    assert_eq!(json["model_id"], nf_numread::MODEL_ID);
    assert_eq!(json["input_sha256"].as_str().unwrap().len(), 64);
    assert_eq!(json["verdict"], "ok", "{json}");
    assert_eq!(json["ocr"], "agrees", "{json}");
    assert!(json["ocr_read"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s.as_str() == Some("φ301.0")));
    // a wrong value over the same box is a blocking verdict; the reader never confirms it
    let v = reader
        .verify_box(
            PAGE,
            [127.1, 268.8, 204.9, 289.7],
            "D",
            "φ999.0",
            999.0,
            "read",
        )
        .expect("verify");
    let json: serde_json::Value = serde_json::from_str(&v.to_json()).expect("json");
    assert_eq!(json["verdict"], "ocr_disagrees", "{json}");
    assert!(json["reasons"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("ocr_other_number")));
    assert_eq!(json["blocks"], true);
}

#[test]
fn verify_box_cli_reports_a_verdict_or_a_usage_error() {
    let exe = env!("CARGO_BIN_EXE_nf-numread");
    let bad = std::process::Command::new(exe)
        .args([
            "verify_box",
            "page.png",
            "--bbox",
            "1,2",
            "--text",
            "x",
            "--value",
            "1",
        ])
        .output()
        .expect("run");
    assert_eq!(bad.status.code(), Some(2));
    if model_dir().is_none() {
        return;
    }
    let path = std::env::temp_dir().join(format!("nf-numread-verify-{}.png", std::process::id()));
    std::fs::write(&path, PAGE).expect("write");
    let out = std::process::Command::new(exe)
        .arg("verify_box")
        .arg(&path)
        .args([
            "--bbox",
            "127.1,268.8,204.9,289.7",
            "--text",
            "φ301.0",
            "--value",
            "301.0",
        ])
        .output()
        .expect("run");
    let _ = std::fs::remove_file(&path);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(json["verdict"], "ok", "{json}");
    assert_eq!(json["model_id"], nf_numread::MODEL_ID);
}
