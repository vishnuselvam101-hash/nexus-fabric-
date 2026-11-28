//! Development aid: run the reader over PNG files, one JSON line per page (tokens, seconds).
//! Parameters can be overridden with NUMREAD_<NAME> environment variables to measure a change.
use nf_numread::{image_in, to_json, Params, Reader};
use std::time::Instant;

fn env_f(name: &str, d: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(d)
}

fn main() {
    let mut p = Params::default();
    p.det_side = env_f("NUMREAD_DET_SIDE", p.det_side as f32) as usize;
    p.bin_thresh = env_f("NUMREAD_BIN", p.bin_thresh);
    p.box_thresh = env_f("NUMREAD_BOX", p.box_thresh);
    p.unclip = env_f("NUMREAD_UNCLIP", p.unclip);
    p.min_pixels = env_f("NUMREAD_MINPX", p.min_pixels as f32) as u32;
    p.phi_thresh = env_f("NUMREAD_PHI", p.phi_thresh);
    p.vertical = env_f("NUMREAD_VERT", 1.0) > 0.5;
    p.vertical_margin = env_f("NUMREAD_VMARGIN", p.vertical_margin);
    let reader = Reader::with_params(p).expect("models");
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("read");
        let t = Instant::now();
        let g = image_in::decode_png(&bytes).expect("png");
        let items = reader.read_gray(&g).expect("read");
        let secs = t.elapsed().as_secs_f64();
        let doc: serde_json::Value = serde_json::from_str(&to_json(&items, g.w, g.h)).unwrap();
        let toks: Vec<_> = doc["items"].as_array().unwrap().iter().map(|i| serde_json::json!({"text": i["text"], "box": i["box"], "conf": i["confidence"], "kind": i["kind_hint"]})).collect();
        println!(
            "{}",
            serde_json::json!({"png": path, "secs": secs, "tokens": toks})
        );
    }
}
