mod common;
use common::*;
use nf_machine::*;

#[test]
fn gate_refuses_held_uncertified_and_mismatched() {
    let bytes = b"0\nEOF\n";
    let ok = cert("P1", bytes);
    assert!(gate::check(Some(&ok), "P1", ENGINE, bytes).is_ok());
    assert_eq!(
        gate::check(None, "P1", ENGINE, bytes),
        Err(Refusal::NoCertificate)
    );

    let mut c = ok.clone();
    c.verdict = CertVerdict::Fail;
    assert_eq!(
        gate::check(Some(&c), "P1", ENGINE, bytes),
        Err(Refusal::NotCertified(CertVerdict::Fail))
    );
    c.verdict = CertVerdict::Inconclusive;
    assert_eq!(
        gate::check(Some(&c), "P1", ENGINE, bytes),
        Err(Refusal::NotCertified(CertVerdict::Inconclusive))
    );

    let mut h = ok.clone();
    h.held = true;
    assert_eq!(
        gate::check(Some(&h), "P1", ENGINE, bytes),
        Err(Refusal::Held)
    );

    assert!(matches!(
        gate::check(Some(&ok), "P2", ENGINE, bytes),
        Err(Refusal::PartMismatch { .. })
    ));
    assert!(matches!(
        gate::check(Some(&ok), "P1", "other-engine", bytes),
        Err(Refusal::EngineStale { .. })
    ));
}

#[test]
fn gate_refuses_one_byte_change() {
    let bytes = b"0\nEOF\n".to_vec();
    let c = cert("P1", &bytes);
    let mut tampered = bytes.clone();
    tampered[0] = b'1';
    assert!(matches!(
        gate::check(Some(&c), "P1", ENGINE, &tampered),
        Err(Refusal::HashMismatch { .. })
    ));
}

#[test]
fn profile_round_trip_and_rules() {
    let p = profile(TextEncoding::ShiftJis, DxfVersion::R12);
    let json = p.to_json();
    assert_eq!(MachineProfile::from_json(&json).unwrap(), p);
    assert!(json.contains("\"shift_jis\"") && json.contains("\"R12\""));

    // a missing required field is rejected (no defaults)
    let no_id = json.replacen("\"id\":", "\"ident\":", 1);
    assert!(matches!(
        MachineProfile::from_json(&no_id),
        Err(ProfileError::Parse(_))
    ));
    // unknown field rejected
    let extra = json.replacen('{', "{\"surprise\": 1,", 1);
    assert!(matches!(
        MachineProfile::from_json(&extra),
        Err(ProfileError::Parse(_))
    ));

    let mut bad = p.clone();
    bad.units = "inch".into();
    assert!(matches!(bad.validate(), Err(ProfileError::BadUnits(_))));
    let mut bad = p.clone();
    bad.layer_map.clear();
    assert_eq!(bad.validate(), Err(ProfileError::NoLayers));
    let mut bad = p.clone();
    bad.sources.clear();
    assert_eq!(bad.validate(), Err(ProfileError::NoSources));
    let mut bad = p.clone();
    bad.file_name = "../{part}.dxf".into();
    assert_eq!(bad.validate(), Err(ProfileError::BadFileName));
}

#[test]
fn shift_jis_text_option() {
    let p = profile(TextEncoding::ShiftJis, DxfVersion::R12);
    let src = dxf("東京", "CUT_OUTER");
    let out = nf_machine::dxf::convert(src.as_bytes(), &p).unwrap();
    // 東京 in Shift_JIS (93 8C 8B 9E, checked with Python codecs)
    assert!(out.windows(4).any(|w| w == [0x93, 0x8c, 0x8b, 0x9e]));
    assert!(!out.windows(3).any(|w| w == "東".as_bytes()));
    // header codepage set, layer renamed, version set
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("ANSI_932") && s.contains("AC1009") && s.contains("OUTER"));

    // a character Shift_JIS cannot hold is refused, not replaced
    let bad = dxf("😀", "CUT_OUTER");
    assert!(matches!(
        nf_machine::dxf::convert(bad.as_bytes(), &p),
        Err(nf_machine::dxf::DxfError::Unencodable(_))
    ));

    // unmapped layer refused
    let un = dxf("x", "CUT_OTHER");
    assert!(matches!(
        nf_machine::dxf::convert(un.as_bytes(), &p),
        Err(nf_machine::dxf::DxfError::UnmappedLayer(_))
    ));

    // R12 refuses entities it cannot hold
    let p12 = profile(TextEncoding::Utf8, DxfVersion::R12);
    let pl = dxf("x", "CUT_OUTER").replace("0\nLINE\n", "0\nLWPOLYLINE\n");
    assert!(matches!(
        nf_machine::dxf::convert(pl.as_bytes(), &p12),
        Err(nf_machine::dxf::DxfError::UnsupportedEntity(_))
    ));
}
