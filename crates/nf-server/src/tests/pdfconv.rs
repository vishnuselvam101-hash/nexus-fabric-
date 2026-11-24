//! Task 3.1: PDF to DXF through the server (API, refusals, limits, roles, isolation, time-out,
//! the fuzz corpus through the server path). The real converter program `nf-pdf2dxf` is used:
//! `NF_PDF2DXF_TEST`, or the program beside this test executable's directory
//! (`target/<profile>/nf-pdf2dxf`, built by `cargo test --workspace`). Without it these tests
//! FAIL with that reason; they are never skipped.

use super::*;

const LINES_PDF: &[u8] = include_bytes!("../../../nf-pdf/tests/fixtures/reportlab_lines.pdf");

fn converter() -> PathBuf {
    if let Some(p) = std::env::var_os("NF_PDF2DXF_TEST") {
        return PathBuf::from(p);
    }
    // The test executable lies below `target/<profile>/` (in `deps/`, or deeper in cargo's newer build
    // directory layout); the converter is in `target/<profile>/`.
    let exe = std::env::current_exe().unwrap();
    let found = exe
        .ancestors()
        .skip(1)
        .map(|d| d.join(crate::pdfconv::program_name()))
        .find(|p| p.is_file());
    match found {
        Some(p) => p,
        None => panic!(
            "the converter {} was not found beside any directory above {}: run `cargo test --workspace` (it builds nf-pdf2dxf) or set NF_PDF2DXF_TEST",
            crate::pdfconv::program_name(),
            exe.display()
        ),
    }
}

/// A test server whose conversions use `exe` (the real converter unless a test says otherwise).
fn boot_with(exe: Option<PathBuf>, timeout: Duration) -> T {
    let mut t = boot();
    let app = Arc::get_mut(&mut t.app).unwrap();
    app.pdf2dxf_exe = exe;
    app.convert_timeout = timeout;
    t
}

fn boot_real() -> T {
    boot_with(Some(converter()), Duration::from_secs(110))
}

/// A one-page PDF with a correct cross-reference table. `objects[0]` is the catalogue.
fn pdf_of(objects: &[Vec<u8>], trailer_extra: &str) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(o);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend_from_slice(
        format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
    );
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R {trailer_extra} >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn stream(dict: &str, data: &[u8]) -> Vec<u8> {
    let mut o = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
    o.extend_from_slice(data);
    o.extend_from_slice(b"\nendstream");
    o
}

/// Catalogue, pages, page with `resources`, content stream (with `filter` entries).
fn page(
    content: &[u8],
    filter: &str,
    resources: &str,
    extra_objects: &[Vec<u8>],
    trailer: &str,
) -> Vec<u8> {
    let mut objs = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Resources << {resources} >> /Contents 4 0 R >>").into_bytes(),
        stream(filter, content),
    ];
    objs.extend_from_slice(extra_objects);
    pdf_of(&objs, trailer)
}

fn simple(content: &[u8]) -> Vec<u8> {
    page(content, "", "", &[], "")
}

fn zlib(data: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    e.write_all(data).unwrap();
    e.finish().unwrap()
}

impl T {
    async fn upload_bytes(&self, name: &str, bytes: &[u8]) -> (StatusCode, i64) {
        let req = HttpRequest::builder()
            .method("POST")
            .uri("/api/projects/1/documents")
            .header("x-nf-request", "1")
            .header("host", "127.0.0.1:8787")
            .header("cookie", &self.cookie)
            .header("x-filename", name)
            .body(Body::from(bytes.to_vec()))
            .unwrap();
        let st = router(self.app.clone())
            .oneshot(req)
            .await
            .unwrap()
            .status();
        let id = self.count("SELECT COALESCE(MAX(id), 0) FROM documents");
        (st, id)
    }
    async fn upload_pdf(&self, bytes: &[u8]) -> i64 {
        let (st, id) = self.upload_bytes("drawing.pdf", bytes).await;
        assert_eq!(st, StatusCode::OK, "upload");
        id
    }
    async fn convert(&self, doc: i64, body: Value) -> (StatusCode, Value) {
        self.call(
            "POST",
            &format!("/api/documents/{doc}/pdf-to-dxf"),
            Some(body),
        )
        .await
    }
}

fn ratio(n: f64) -> Value {
    json!({ "page": 1, "scale_kind": "ratio", "scale_value": n })
}

#[tokio::test]
async fn a_vector_pdf_is_converted_stored_recorded_and_labelled_not_released() {
    let t = boot_real();
    let doc = t.upload_pdf(LINES_PDF).await;
    let docs_before = t.count("SELECT COUNT(*) FROM documents");
    let snaps_before = t.count("SELECT COUNT(*) FROM records WHERE kind = 'snapshot'");
    let (st, v) = t.convert(doc, ratio(1.0)).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let c = &v["conversion"];
    assert_eq!(c["status"], "converted");
    assert_eq!(
        c["label"],
        "Converted from PDF - not verified - not released"
    );
    assert_eq!(c["release_state"], "NOT RELEASED");
    assert_eq!(c["flat_pattern"], false);
    assert_eq!(c["scale"], json!({ "kind": "ratio", "value": 1.0 }));
    assert_eq!(v["report"]["status"], "converted");
    // The isolation the platform requires is stated in the report (the server checks it too).
    let iso = &v["report"]["isolation"];
    if cfg!(target_os = "linux") {
        assert!(
            iso["seccomp"]
                .as_str()
                .unwrap()
                .starts_with("allow-list enforced"),
            "{iso}"
        );
    } else if cfg!(windows) {
        assert!(iso["job_object"].is_string(), "{iso}");
    } else {
        assert!(iso["resource_limits"].is_string(), "{iso}");
    }
    // Two documents (the DXF and the report), a new snapshot, an append-only record, an audit entry.
    assert_eq!(t.count("SELECT COUNT(*) FROM documents"), docs_before + 2);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM records WHERE kind = 'snapshot'"),
        snaps_before + 1
    );
    let id = c["id"].as_i64().unwrap();
    assert_eq!(
        t.count(&format!(
            "SELECT COUNT(*) FROM documents WHERE conversion_id = {id}"
        )),
        2
    );
    let rec: String = t
        .app
        .db()
        .query_row(
            "SELECT json FROM records WHERE kind = 'pdf_conversion'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let rec: Value = serde_json::from_str(&rec).unwrap();
    assert_eq!(
        rec["payload"]["source_sha256"],
        nf_core::sha256_hex(LINES_PDF)
    );
    assert_eq!(rec["payload"]["page"], 1);
    assert_eq!(rec["payload"]["release_state"], "NOT RELEASED");
    assert_eq!(rec["relations"][0]["rel"], "derived_from");
    assert!(rec["payload"]["converter_version"].is_string());
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'pdf_converted'"),
        1
    );
    // The stored files are encrypted like every document: the plaintext is not in the blob store.
    let dxf_sha = c["dxf_sha256"].as_str().unwrap().to_string();
    let sealed = std::fs::read(storage::blob_path(&t.dir, &dxf_sha)).unwrap();
    assert!(!sealed.windows(7).any(|w| w == b"SECTION" || w == b"AC1009"));
    // Download: the DXF with its hash and the label inside, the report, the conversion.
    let (st, h, body) = t
        .send("GET", &format!("/api/pdf-conversions/{id}/dxf"), None, &[])
        .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(nf_core::sha256_hex(&body), dxf_sha);
    assert!(
        String::from_utf8_lossy(&body).contains("CONVERTED FROM PDF - NOT VERIFIED - NOT RELEASED")
    );
    assert!(
        h[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("NOT-RELEASED_from-pdf_")
    );
    let (st, _, rep) = t
        .send(
            "GET",
            &format!("/api/pdf-conversions/{id}/report"),
            None,
            &[],
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        serde_json::from_slice::<Value>(&rep).unwrap()["status"],
        "converted"
    );
    let g = t.get(&format!("/api/pdf-conversions/{id}")).await;
    assert_eq!(g["conversion"]["dxf_sha256"], dxf_sha.as_str());
    let p = t.get("/api/projects/1").await;
    assert_eq!(p["conversions"][0]["id"], id);
    // Never attached to a calculation, never a calculation of its own.
    assert_eq!(t.count("SELECT COUNT(*) FROM calculations"), 0);
    // The working directory is gone.
    assert_eq!(
        std::fs::read_dir(t.dir.join(crate::pdfconv::WORK_DIR))
            .unwrap()
            .count(),
        0
    );
    let audit = t.get("/api/audit/verify").await;
    assert_eq!(audit["intact"], true, "{audit}");
}

#[tokio::test]
async fn every_refusal_code_reaches_the_client_unchanged_and_stores_no_dxf() {
    let t = boot_real();
    let vector = t.upload_pdf(LINES_PDF).await;
    // Server-side checks of the request.
    let cases: Vec<(Value, &str)> = vec![
        (json!({ "page": 1 }), "scale_required"),
        (
            json!({ "page": 1, "scale_kind": "ratio" }),
            "scale_required",
        ),
        (
            json!({ "page": 1, "scale_kind": "inch", "scale_value": 1 }),
            "invalid_scale",
        ),
        (
            json!({ "page": 1, "scale_kind": "ratio", "scale_value": "ten" }),
            "invalid_scale",
        ),
        (
            json!({ "page": 0, "scale_kind": "ratio", "scale_value": 1 }),
            "invalid_page",
        ),
        (
            json!({ "page": 1.5, "scale_kind": "ratio", "scale_value": 1 }),
            "invalid_page",
        ),
        (
            json!({ "page": 1, "scale_kind": "ratio", "scale_value": 1, "tolerance_mm": "x" }),
            "invalid_tolerance",
        ),
        (
            json!({ "page": 1, "scale_kind": "ratio", "scale_value": 1, "include_fill": "no" }),
            "invalid_arguments",
        ),
        // The converter's own checks of the options.
        (
            json!({ "page": 1, "scale_kind": "ratio", "scale_value": -10 }),
            "invalid_scale",
        ),
        (
            json!({ "page": 1, "scale_kind": "mm_per_point", "scale_value": 0 }),
            "invalid_scale",
        ),
        (
            json!({ "page": 1, "scale_kind": "ratio", "scale_value": 1, "tolerance_mm": 0 }),
            "invalid_tolerance",
        ),
        (
            json!({ "page": 9, "scale_kind": "ratio", "scale_value": 1 }),
            "page_out_of_range",
        ),
    ];
    for (body, code) in cases {
        let (st, v) = t.convert(vector, body.clone()).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{body}: {v}");
        assert_eq!(v["code"], code, "{body}: {v}");
        assert!(!v["message"].as_str().unwrap().is_empty());
    }
    // The converter's refusals of documents.
    let image = page(
        b"q 200 0 0 200 0 0 cm /Im1 Do Q",
        "",
        "/XObject << /Im1 5 0 R >>",
        &[stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            &[0],
        )],
        "",
    );
    let docs: Vec<(Vec<u8>, &str)> = vec![
        (image, "raster_page"),
        (simple(b"BT ET"), "no_vector_paths"),
        (
            page(
                b"0 0 m 1 1 l S",
                "",
                "",
                &[],
                "/Encrypt << /Filter /Standard /V 1 >>",
            ),
            "encrypted",
        ),
        (
            page(b"0 0 m 1 1 l S", "/Filter /LZWDecode", "", &[], ""),
            "unsupported_filter",
        ),
        (
            b"%PDF-1.7\nthis is not a PDF body at all\n%%EOF\n".to_vec(),
            "malformed",
        ),
    ];
    for (bytes, code) in docs {
        let doc = t.upload_pdf(&bytes).await;
        let (st, v) = t.convert(doc, ratio(1.0)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{code}: {v}");
        assert_eq!(v["code"], code, "{v}");
        assert_eq!(v["status"], "refused");
        assert!(!v["message"].as_str().unwrap().is_empty());
        // Recorded (append-only, with the converter's report), but no DXF document.
        assert_eq!(v["conversion"]["status"], "refused");
        assert!(v["conversion"]["dxf_sha256"].is_null());
        let id = v["conversion"]["id"].as_i64().unwrap();
        let (st, _, _) = t
            .send("GET", &format!("/api/pdf-conversions/{id}/dxf"), None, &[])
            .await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }
    // A scanned drawing is refused with the reason in words.
    assert_eq!(
        t.count("SELECT COUNT(*) FROM pdf_conversions WHERE code = 'raster_page'"),
        1
    );
    // A document that is not a PDF.
    let (st, png) = t
        .upload_bytes(
            "x.png",
            &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0],
        )
        .await;
    assert_eq!(st, StatusCode::OK);
    let (st, v) = t.convert(png, ratio(1.0)).await;
    assert_eq!(
        (st, v["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("not_a_pdf"))
    );
    // A document that does not exist.
    let (st, _) = t.convert(9999, ratio(1.0)).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM documents WHERE conversion_id IS NOT NULL"),
        0
    );
    // Refusals by the converter are recorded (4 of the options, 5 of the documents); refusals
    // of the request by the server itself are answered before anything runs and are not.
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'pdf_conversion_refused'"),
        9
    );
    assert_eq!(t.count("SELECT COUNT(*) FROM pdf_conversions"), 9);
}

#[tokio::test]
async fn the_converters_limits_reach_the_client_with_their_names() {
    let t = boot_real();
    let mut cases: Vec<(Vec<u8>, &str)> = Vec::new();
    // 8 MiB of zeros behind Flate: the compression-ratio guard (100:1 above 4 MiB).
    cases.push((
        page(
            &zlib(&vec![0u8; 8 << 20]),
            "/Filter /FlateDecode",
            "",
            &[],
            "",
        ),
        "compression_ratio",
    ));
    // Nested arrays deeper than 64.
    cases.push((
        simple(format!("{}{} 0 0 m 1 1 l S", "[".repeat(70), "]".repeat(70)).as_bytes()),
        "nesting",
    ));
    // 1,025 operands waiting for one operator.
    cases.push((
        simple(format!("{}0 0 m 1 1 l S", "1 ".repeat(1025)).as_bytes()),
        "operands",
    ));
    // 257 nested q.
    cases.push((
        simple(format!("{}0 0 m 1 1 l S", "q ".repeat(257)).as_bytes()),
        "graphics_depth",
    ));
    // An array of 1,000,001 items (Flate-compressed so that it fits the upload limit easily).
    let big = format!("[{}] pop 0 0 m 1 1 l S", "0 ".repeat(1_000_001));
    cases.push((
        page(&zlib(big.as_bytes()), "/Filter /FlateDecode", "", &[], ""),
        "collection_items",
    ));
    // 4,000,001 items in the operands waiting for one operator (not compressed: 8 MB of content
    // behind Flate would meet the compression-ratio guard first).
    let items = format!(
        "{} [0] 0 0 m 1 1 l S",
        format!("[{}] ", "0 ".repeat(1_000_000)).repeat(4)
    );
    cases.push((simple(items.as_bytes()), "total_items"));
    // 2,000,001 operators.
    cases.push((
        page(
            &zlib("n ".repeat(2_000_001).as_bytes()),
            "/Filter /FlateDecode",
            "",
            &[],
            "",
        ),
        "operators",
    ));
    // A Form XObject that draws itself: deeper than 16.
    cases.push((
        page(
            b"/F Do 0 0 m 1 1 l S",
            "",
            "/XObject << /F 5 0 R >>",
            &[stream(
                "/Type /XObject /Subtype /Form /BBox [0 0 10 10] /Resources << /XObject << /F 5 0 R >> >>",
                b"/F Do",
            )],
            "",
        ),
        "form_depth",
    ));
    for (bytes, limit) in cases {
        let doc = t.upload_pdf(&bytes).await;
        let (st, v) = t.convert(doc, ratio(1.0)).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{limit}: {v}");
        assert_eq!(v["code"], "limit_exceeded", "{v}");
        assert_eq!(v["status"], "limit_exceeded", "{v}");
        assert_eq!(v["limit"], limit, "{v}");
        assert_eq!(v["conversion"]["limit"], limit);
    }
    // Not reached through the server here: objects, xref_sections, path_segments, text_items,
    // decoded_bytes, output_bytes, wall_clock (tested on the converter itself).
    // The converter's input limit (64 MiB) cannot be reached through the server: the upload
    // limit is lower, and a larger file never becomes a document.
    const { assert!(membrane::MAX_UPLOAD_BYTES < 64 << 20) };
}

#[tokio::test]
async fn only_engineers_and_administrators_of_the_project_convert() {
    let t = boot_real();
    let doc = t.upload_pdf(LINES_PDF).await;
    let viewer = add_session(&t.app, "vera", "viewer");
    t.sql("INSERT INTO project_members(project_id, user_id) SELECT 1, id FROM users WHERE username = 'vera'");
    let outsider = add_session(&t.app, "otto", "engineer");
    let body = ratio(1.0).to_string();
    for (cookie, want) in [
        (viewer.as_str(), StatusCode::FORBIDDEN),
        (outsider.as_str(), StatusCode::NOT_FOUND),
        ("nf_session=nobody", StatusCode::UNAUTHORIZED),
    ] {
        let req = HttpRequest::builder()
            .method("POST")
            .uri(format!("/api/documents/{doc}/pdf-to-dxf"))
            .header("x-nf-request", "1")
            .header("host", "127.0.0.1:8787")
            .header("cookie", cookie)
            .header("content-type", "application/json")
            .body(Body::from(body.clone()))
            .unwrap();
        let st = router(t.app.clone()).oneshot(req).await.unwrap().status();
        assert_eq!(st, want, "{cookie}");
    }
    assert_eq!(t.count("SELECT COUNT(*) FROM pdf_conversions"), 0);
    // A viewer member may read a conversion; an outsider may not even learn it exists.
    let (st, v) = t.convert(doc, ratio(1.0)).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let id = v["conversion"]["id"].as_i64().unwrap();
    for (cookie, want) in [
        (viewer.as_str(), StatusCode::OK),
        (outsider.as_str(), StatusCode::NOT_FOUND),
    ] {
        let (st, _, _) = t
            .send(
                "GET",
                &format!("/api/pdf-conversions/{id}/dxf"),
                None,
                &[("cookie", cookie)],
            )
            .await;
        assert_eq!(st, want, "{cookie}");
    }
}

/// A wrapper that runs the real converter with `--isolation-probe` and keeps its messages.
#[cfg(target_os = "linux")]
fn probe_wrapper(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("probe-converter.sh");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nexec '{}' \"$@\" --isolation-probe 2>'{}'\n",
            converter().display(),
            dir.join("probe.txt").display()
        ),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn the_converter_started_by_the_server_has_no_network_no_files_and_no_environment() {
    let mut t = boot_real();
    let wrapper = probe_wrapper(&t.dir);
    Arc::get_mut(&mut t.app).unwrap().pdf2dxf_exe = Some(wrapper);
    let doc = t.upload_pdf(LINES_PDF).await;
    let (st, v) = t.convert(doc, ratio(1.0)).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let probe = std::fs::read_to_string(t.dir.join("probe.txt")).unwrap();
    for line in [
        "landlock: tcp connect 127.0.0.1:9: refused (EACCES)",
        "landlock: read /etc/hostname: refused (EACCES)",
        "landlock: read the input by name: refused (EACCES)",
        "landlock: run /bin/true: refused (EACCES)",
        "seccomp: socket inet tcp: refused (EPERM)",
        "seccomp: socket inet6 udp: refused (EPERM)",
        "seccomp: socket unix: refused (EPERM)",
        "seccomp: socketpair unix: refused (EPERM)",
        "seccomp: read the input by name: refused (EPERM)",
        "seccomp: run /bin/true: refused (EPERM)",
        "limits: rlimit AS: 2147483648/2147483648",
        "limits: rlimit CPU: 60/60",
    ] {
        assert!(
            probe.contains(&format!("nf-pdf2dxf probe: {line}")),
            "missing '{line}' in:\n{probe}"
        );
    }
    assert!(!probe.contains("ALLOWED"), "{probe}");
}

#[cfg(unix)]
fn script(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(unix)]
#[tokio::test]
async fn a_converter_that_runs_too_long_is_killed_and_nothing_is_stored() {
    let sdir = temp_dir("slow");
    std::fs::create_dir_all(&sdir).unwrap();
    let pidfile = sdir.join("child.pid");
    let envfile = sdir.join("child.env");
    let exe = script(
        &sdir,
        "slow.sh",
        &format!(
            "env > '{}'\necho $$ > '{}'\nexec sleep 60",
            envfile.display(),
            pidfile.display()
        ),
    );
    let t = boot_with(Some(exe), Duration::from_secs(1));
    let doc = t.upload_pdf(LINES_PDF).await;
    let started = std::time::Instant::now();
    let (st, v) = t.convert(doc, ratio(1.0)).await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "{v}");
    assert_eq!(v["infrastructure"], true);
    assert!(v["error"].as_str().unwrap().contains("was stopped"), "{v}");
    assert!(started.elapsed() < Duration::from_secs(10));
    let pid = std::fs::read_to_string(&pidfile)
        .unwrap()
        .trim()
        .to_string();
    assert!(
        !std::path::Path::new(&format!("/proc/{pid}")).exists(),
        "the converter outlived its time-out"
    );
    // No inherited environment: only what the shell itself sets.
    let env = std::fs::read_to_string(&envfile).unwrap();
    for name in env
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, _)| k))
    {
        assert!(
            ["PWD", "SHLVL", "_", "OLDPWD"].contains(&name),
            "inherited variable {name}"
        );
    }
    assert_eq!(t.count("SELECT COUNT(*) FROM pdf_conversions"), 0);
    assert_eq!(
        t.count("SELECT COUNT(*) FROM audit WHERE action = 'pdf_conversion_unavailable'"),
        1
    );
    assert_eq!(
        std::fs::read_dir(t.dir.join(crate::pdfconv::WORK_DIR))
            .unwrap()
            .count(),
        0
    );
    let _ = std::fs::remove_dir_all(&sdir);
}

#[cfg(unix)]
#[tokio::test]
async fn a_dxf_is_accepted_only_with_a_matching_hash_and_confirmed_isolation() {
    let t = boot();
    // A fake converter: exit 0 with a DXF and a report whose hash or isolation is wrong.
    let fake = |name: &str, iso: &str, hash: &str| {
        script(
            &t.dir,
            name,
            &format!(
                r#"while [ $# -gt 0 ]; do case "$1" in --out) out="$2"; shift;; --report) rep="$2"; shift;; esac; shift; done
printf '0\nSECTION\n0\nEOF\n' > "$out"
h={hash}
printf '{{"status": "converted", "isolation": {iso}, "output": {{"dxf_sha256": "%s"}}}}' "$h" > "$rep""#
            ),
        )
    };
    let real_hash = "$(sha256sum \"$out\" | cut -d' ' -f1)";
    let iso_ok =
        r#"{"seccomp": "allow-list enforced: x", "resource_limits": "x", "job_object": "x"}"#;
    let cases = [
        (
            fake("wrong-hash.sh", iso_ok, "0000"),
            "does not match the hash",
        ),
        (
            fake("no-isolation.sh", "null", real_hash),
            "does not confirm",
        ),
        (fake("ok.sh", iso_ok, real_hash), ""),
    ];
    for (exe, why) in cases {
        let mut t2 = boot();
        let doc2 = t2.upload_pdf(LINES_PDF).await;
        Arc::get_mut(&mut t2.app).unwrap().pdf2dxf_exe = Some(exe);
        let (st, v) = t2.convert(doc2, ratio(1.0)).await;
        if why.is_empty() {
            assert_eq!(st, StatusCode::OK, "{v}");
        } else {
            assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE, "{v}");
            assert!(v["error"].as_str().unwrap().contains(why), "{v}");
            assert_eq!(
                t2.count("SELECT COUNT(*) FROM documents WHERE conversion_id IS NOT NULL"),
                0
            );
        }
    }
}

#[tokio::test]
async fn a_missing_converter_or_a_busy_slot_is_an_infrastructure_answer() {
    let t = boot_with(
        Some(PathBuf::from("/nonexistent/nf-pdf2dxf")),
        Duration::from_secs(5),
    );
    let doc = t.upload_pdf(LINES_PDF).await;
    let (st, v) = t.convert(doc, ratio(1.0)).await;
    assert_eq!(st, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(v["infrastructure"], true);
    assert!(
        v["error"].as_str().unwrap().contains("not installed"),
        "{v}"
    );
    let t = boot_real();
    let doc = t.upload_pdf(LINES_PDF).await;
    let held = t.app.convert_slots.clone().try_acquire_owned().unwrap();
    let (st, v) = t.convert(doc, ratio(1.0)).await;
    assert_eq!(
        (st, v["retry"].as_bool()),
        (StatusCode::SERVICE_UNAVAILABLE, Some(true))
    );
    drop(held);
    let (st, _) = t.convert(doc, ratio(1.0)).await;
    assert_eq!(st, StatusCode::OK);
}

#[tokio::test]
async fn a_stale_working_directory_is_removed_at_start_up() {
    let t = boot();
    let stale = t.dir.join(crate::pdfconv::WORK_DIR).join("c-stale");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("in.pdf"), LINES_PDF).unwrap();
    drop(build_app(config(&t.dir)).unwrap());
    assert!(!t.dir.join(crate::pdfconv::WORK_DIR).exists());
}

/// The fuzz corpus through the server path: mutated PDFs are uploaded and converted. Every
/// answer is a conversion (200) or a refusal (422); never an error of the server, never a DXF
/// document after a refusal. `NF_PDF_SERVER_FUZZ_CASES` sets the count (default 150).
#[tokio::test]
async fn mutated_pdfs_through_the_server_end_in_a_conversion_or_a_refusal() {
    let t = boot_real();
    let cases: usize = std::env::var("NF_PDF_SERVER_FUZZ_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(150);
    let seeds: Vec<Vec<u8>> = vec![
        LINES_PDF.to_vec(),
        include_bytes!("../../../nf-pdf/tests/fixtures/reportlab_curves.pdf").to_vec(),
        include_bytes!("../../../nf-pdf/tests/fixtures/reportlab_text.pdf").to_vec(),
        simple(b"0 0 m 10 10 l S q 1 0 0 1 5 5 cm 0 0 10 10 re f Q"),
        page(
            &zlib(b"0 0 m 100 0 l 100 100 l h S"),
            "/Filter /FlateDecode",
            "",
            &[],
            "",
        ),
    ];
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    let (mut converted, mut refused) = (0, 0);
    for case in 0..cases {
        let mut b = seeds[case % seeds.len()].clone();
        let edits = 1 + (next() % 8) as usize;
        for _ in 0..edits {
            let n = b.len().max(1);
            let at = 9 + (next() as usize) % n.saturating_sub(9).max(1);
            match next() % 4 {
                0 => {
                    if at < b.len() {
                        b[at] = next() as u8;
                    }
                }
                1 => {
                    if at < b.len() {
                        b.remove(at);
                    }
                }
                2 => b.insert(at.min(b.len()), (*b"[]<>(0/\n")[(next() % 8) as usize]),
                _ => {
                    let cut = at.min(b.len());
                    b.truncate(cut.max(16));
                }
            }
        }
        let (st, doc) = t.upload_bytes("fuzz.pdf", &b).await;
        if st != StatusCode::OK {
            continue; // the membrane refused it (no %PDF- header left): nothing to convert
        }
        let before = t.count("SELECT COUNT(*) FROM documents WHERE conversion_id IS NOT NULL");
        let (st, v) = t.convert(doc, ratio(1.0)).await;
        match st {
            StatusCode::OK => {
                converted += 1;
                assert_eq!(v["conversion"]["status"], "converted");
            }
            StatusCode::UNPROCESSABLE_ENTITY => {
                refused += 1;
                assert!(
                    v["code"].as_str().is_some_and(|c| !c.is_empty()),
                    "case {case}: {v}"
                );
                assert_eq!(
                    t.count("SELECT COUNT(*) FROM documents WHERE conversion_id IS NOT NULL"),
                    before,
                    "case {case}"
                );
            }
            other => panic!("case {case}: {other} {v}"),
        }
    }
    eprintln!("server-path fuzz: {cases} cases, {converted} converted, {refused} refused");
    assert!(
        converted > 0 && refused > 0,
        "the corpus must reach both outcomes"
    );
}
