//! Property-based fuzz tests of the PDF parser (work plan rule A6, task 3.1), with proptest.
//!
//! Each property runs the whole converter on generated input and checks the contract that must
//! hold for ANY input: it returns (no panic, no hang past its own time limit); a refusal or a
//! limit carries a code and a message and no DXF; a success is a DXF that the nf-dxf reader
//! accepts and that carries the NOT RELEASED label. Inputs: raw bytes; structurally valid
//! pages with generated content streams; streams through every supported filter with generated
//! data and parameters; and generated nested objects in the document structure.
//!
//! The number of cases per property is `PROPTEST_CASES` (proptest's own variable), default 256
//! here. Failures are shrunk by proptest and printed with the minimal input.

mod common;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};

use common::*;
use nf_pdf::{convert, Options, Scale};
use proptest::prelude::*;
use proptest::test_runner::Config;

/// The converter's own budget for one case, and the bound a case may take in total.
const CASE_BUDGET: Duration = Duration::from_secs(5);
const CASE_BOUND: Duration = Duration::from_secs(20);

fn cases() -> u32 {
    std::env::var("PROPTEST_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(256)
}

fn config() -> Config {
    Config {
        cases: cases(),
        // No files are written for failures: the minimal input is printed instead.
        failure_persistence: None,
        ..Config::default()
    }
}

/// Runs the converter and checks the contract for any input.
fn check_contract(input: &[u8]) -> Result<(), TestCaseError> {
    let mut opts = Options::new(Scale::MmPerPoint(1.0));
    opts.time_limit = CASE_BUDGET;
    let started = Instant::now();
    let outcome = catch_unwind(AssertUnwindSafe(|| convert(input, &opts)));
    let took = started.elapsed();
    prop_assert!(took < CASE_BOUND, "took {took:?}");
    match outcome {
        Err(_) => prop_assert!(false, "the converter panicked"),
        Ok(Ok(done)) => {
            let text = String::from_utf8_lossy(&done.dxf);
            prop_assert!(
                text.contains("CONVERTED FROM PDF - NOT VERIFIED - NOT RELEASED"),
                "a DXF without the NOT RELEASED label"
            );
            prop_assert!(nf_dxf::read(&text).is_ok(), "a DXF the reader refuses");
            prop_assert_eq!(done.report.status, "converted");
        }
        Ok(Err(failure)) => {
            prop_assert!(!failure.code.is_empty(), "a failure without a code");
            prop_assert!(!failure.message.is_empty(), "a failure without a message");
        }
    }
    Ok(())
}

/// One token of a content stream: operators, numbers (huge, tiny, negative), names, strings,
/// and the delimiters of arrays, dictionaries and inline images.
fn token() -> impl Strategy<Value = String> {
    prop_oneof![
        prop::sample::select(vec![
            "m",
            "l",
            "c",
            "v",
            "y",
            "h",
            "re",
            "S",
            "s",
            "f",
            "F",
            "f*",
            "B",
            "B*",
            "b",
            "b*",
            "n",
            "q",
            "Q",
            "cm",
            "w",
            "d",
            "BT",
            "ET",
            "Tf",
            "Td",
            "TD",
            "Tm",
            "T*",
            "Tj",
            "TJ",
            "'",
            "\"",
            "Tc",
            "Tw",
            "Tz",
            "TL",
            "Ts",
            "Tr",
            "Do",
            "BI",
            "ID",
            "EI",
            "sh",
            "gs",
            "W",
            "W*",
            "[",
            "]",
            "<<",
            ">>",
            "true",
            "false",
            "null",
            "/F1",
            "/Fm0",
            "(a)",
            "<41>",
            "1e308",
            "-1e308",
            "3.4e38",
            "0.0000001",
            "99999999999",
        ])
        .prop_map(String::from),
        any::<i32>().prop_map(|n| n.to_string()),
        (-1.0e6f64..1.0e6).prop_map(|x| format!("{x:.3}")),
    ]
}

fn content() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(token(), 0..400).prop_map(|t| t.join(" ").into_bytes())
}

/// A generated object: numbers, names, strings, references, arrays and dictionaries nested to
/// a generated depth.
fn object() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        any::<i16>().prop_map(|n| n.to_string()),
        "[A-Za-z]{1,6}".prop_map(|n| format!("/{n}")),
        "[ -~]{0,8}".prop_map(|s| format!("({})", s.replace(['(', ')', '\\'], ""))),
        (1u32..20, 0u32..3).prop_map(|(a, b)| format!("{a} {b} R")),
        Just("true".to_string()),
        Just("null".to_string()),
    ];
    leaf.prop_recursive(8, 256, 16, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..16).prop_map(|v| format!("[{}]", v.join(" "))),
            prop::collection::vec(("[A-Za-z]{1,6}", inner), 0..16).prop_map(|v| {
                let body: Vec<String> = v.into_iter().map(|(k, o)| format!("/{k} {o}")).collect();
                format!("<<{}>>", body.join(" "))
            }),
        ]
    })
}

fn ascii85(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in data.chunks(4) {
        let mut g = [0u8; 4];
        g[..chunk.len()].copy_from_slice(chunk);
        let mut v = u32::from_be_bytes(g);
        if chunk.len() == 4 && v == 0 {
            out.push(b'z');
            continue;
        }
        let mut d = [0u8; 5];
        for x in d.iter_mut().rev() {
            *x = (v % 85) as u8 + b'!';
            v /= 85;
        }
        out.extend_from_slice(&d[..chunk.len() + 1]);
    }
    out.extend_from_slice(b"~>");
    out
}

proptest! {
    #![proptest_config(config())]

    /// Any bytes at all, with and without a PDF header in front.
    #[test]
    fn any_bytes(data in prop::collection::vec(any::<u8>(), 0..4096), header in any::<bool>()) {
        let mut input = if header { b"%PDF-1.7\n".to_vec() } else { Vec::new() };
        input.extend_from_slice(&data);
        check_contract(&input)?;
    }

    /// A valid page around a generated content stream, plain or Flate-compressed.
    #[test]
    fn generated_content_streams(c in content(), flate in any::<bool>()) {
        let mut spec = PageSpec::new(&c);
        spec.flate = flate;
        check_contract(&page_pdf(&spec))?;
    }

    /// A valid page whose content stream goes through a generated chain of filters with
    /// generated parameters (predictors included) and generated or damaged data.
    #[test]
    fn generated_filter_chains(
        c in content(),
        chain in prop::sample::select(vec![
            "/ASCIIHexDecode", "/ASCII85Decode", "/RunLengthDecode", "/FlateDecode",
            "[/ASCII85Decode /FlateDecode]", "[/ASCIIHexDecode /FlateDecode]",
            "[/ASCII85Decode /ASCII85Decode]", "/LZWDecode", "/DCTDecode",
        ]),
        predictor in prop::sample::select(vec![1i64, 2, 10, 12, 15, 99]),
        columns in 0i64..64,
        damage in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..4),
    ) {
        let mut data = match chain {
            "/ASCIIHexDecode" => {
                let mut h: Vec<u8> = c.iter().flat_map(|b| format!("{b:02x}").into_bytes()).collect();
                h.push(b'>');
                h
            }
            "/ASCII85Decode" => ascii85(&c),
            "/FlateDecode" => zlib(&c),
            "[/ASCII85Decode /FlateDecode]" => ascii85(&zlib(&c)),
            "[/ASCIIHexDecode /FlateDecode]" => {
                let mut h: Vec<u8> = zlib(&c).iter().flat_map(|b| format!("{b:02x}").into_bytes()).collect();
                h.push(b'>');
                h
            }
            "[/ASCII85Decode /ASCII85Decode]" => ascii85(&ascii85(&c)),
            _ => c.clone(),
        };
        for (at, byte) in damage {
            if !data.is_empty() {
                let i = at.index(data.len());
                data[i] = byte;
            }
        }
        let mut pdf = Pdf::new();
        let catalog = pdf.reserve();
        let pages = pdf.reserve();
        let stream = pdf.stream(
            &format!("/Filter {chain} /DecodeParms << /Predictor {predictor} /Columns {columns} >>"),
            &data,
            false,
        );
        let page = pdf.add(format!(
            "<< /Type /Page /Parent {pages} 0 R /MediaBox [0 0 200 200] /Contents {stream} 0 R >>"
        ));
        pdf.set(pages, format!("<< /Type /Pages /Kids [{page} 0 R] /Count 1 >>"));
        pdf.set(catalog, format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
        check_contract(&pdf.classic(catalog))?;
    }

    /// Generated nested objects in the page dictionary, the resources and the trailer.
    #[test]
    fn generated_document_objects(a in object(), b in object(), c in content()) {
        let mut spec = PageSpec::new(&c);
        spec.extra = format!("/NfFuzzA {a}");
        spec.resources = format!("/NfFuzzB {b}");
        check_contract(&page_pdf(&spec))?;
    }
}
