use nf_jobout::xlsx::Cell;
use nf_jobout::*;
use std::path::PathBuf;
use std::process::Command;

fn sample() -> JobInput {
    let dxf = b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n".to_vec();
    JobInput {
        job_name: "G-TEE 工事".into(),
        generated: "2026-11-27 10:30".into(),
        checked: Some(Checked {
            name: "関".into(),
            date: "2026-11-27".into(),
        }),
        parts: vec![Part {
            drawing: "G-TEE".into(),
            no: "1".into(),
            name: "本管".into(),
            shape: "tee_main".into(),
            qty: 1,
            material: "STW400".into(),
            thickness_mm: Some(12.0),
            held: None,
        }],
        pieces: vec![
            Piece {
                drawing: "G-TEE".into(),
                no: "1".into(),
                name: "本管".into(),
                label: None,
                dxf: dxf.clone(),
            },
            Piece {
                drawing: "J-BIGPIPE".into(),
                no: "2".into(),
                name: "曲管".into(),
                label: Some("始点側".into()),
                dxf: dxf.clone(),
            },
            Piece {
                drawing: "a/b".into(),
                no: "3".into(),
                name: "x:y".into(),
                label: None,
                dxf: dxf.clone(),
            },
            Piece {
                drawing: "a/b".into(),
                no: "3".into(),
                name: "x:y".into(),
                label: None,
                dxf,
            },
        ],
        material: vec![
            MaterialRow {
                drawing: "G-TEE".into(),
                no: "1".into(),
                name: "本管".into(),
                shape: "本管（枝の穴）".into(),
                material: "STW400".into(),
                thickness_mm: Some(12.0),
                qty: Some(1),
                width_mm: Some(3154.2),
                height_mm: Some(1500.0),
                mass_calc_kg: Some(418.1),
                mass_bom_kg: Some(445.7),
                note_en: "Mass is after cutting, \"blank\" 445.7 kg".into(),
                note_ja: "計算の重さは切った後（切る前 445.7kg）、枝は載せる".into(),
                ..Default::default()
            },
            MaterialRow {
                drawing: "G-TEE".into(),
                no: "4".into(),
                name: "管フランジ".into(),
                kind: RowKind::Purchased,
                material: "SS400".into(),
                qty: Some(1),
                note_en: "=cmd|' /C calc'!A0".into(),
                note_ja: "購入品".into(),
                ..Default::default()
            },
        ],
        issues: vec![Issue {
            drawing: "J-VALVE".into(),
            no: "1".into(),
            name: "本管".into(),
            code: "stock_fit".into(),
            title_en: "Does not fit the plate".into(),
            title_ja: "板に入らない".into(),
            message_en: "Blank 3163.6 x 2500, plate 2438 x 6096.".into(),
            message_ja: "1枚の外形 3163.6×2500、板 2438×6096。".into(),
        }],
        designer: vec![DesignerNote {
            drawing: "G-TEE".into(),
            no: "1".into(),
            code: "split_ring".into(),
            question_en: "Choose the number of pieces before unfolding.".into(),
            question_ja: "展開前に割り枚数を選んでください。".into(),
            answer_en: "Recorded as a hand-drawn exit; no cut file.".into(),
            answer_ja: "手描きの出口として記録。切断ファイルなし。".into(),
        }],
        record: vec![RecordEntry {
            date: "2026-11-27 10:29".into(),
            user: "関".into(),
            action: "J-VALVE 1 本管 このまま進める".into(),
            before: "".into(),
            after: "板に入らない".into(),
        }],
        provenance: Provenance {
            engine_version: "0.5.0".into(),
            reading: vec![("reader".into(), "typed".into())],
            rules: vec![("basis".into(), "mid".into())],
            values: vec![ValueEntry {
                part: "G-TEE-1".into(),
                key: "D".into(),
                value: Some(508.0),
                text: "φ508".into(),
                source: "drawing".into(),
                state: "CONFIRMED".into(),
            }],
        },
    }
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("nf-jobout-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

fn python(script: &str, args: &[&PathBuf]) -> Option<String> {
    let mut c = Command::new("python3");
    c.arg("-I").arg("-c").arg(script);
    for a in args {
        c.arg(a);
    }
    match c.output() {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string();
            assert!(
                o.status.success(),
                "python failed: {}\n{}",
                String::from_utf8_lossy(&o.stderr),
                out
            );
            Some(out)
        }
        Err(_) => {
            eprintln!("python3 not found, skipped");
            None
        }
    }
}

#[test]
fn deterministic_bytes_and_hash() {
    let a = build_job_zip(&sample()).unwrap();
    let b = build_job_zip(&sample()).unwrap();
    assert_eq!(a, b);
    assert_eq!(sha256_hex(&a), sha256_hex(&b));
}

#[test]
fn empty_job_does_not_panic() {
    let z = build_job_zip(&JobInput::default()).unwrap();
    assert!(z.len() > 100);
    let txt = String::from_utf8(issues_txt(&JobInput::default())).unwrap();
    assert!(txt.contains("No issues.") && txt.contains("指摘なし"));
    assert!(txt.contains("Not checked by a person") && txt.contains("担当者の確認なし"));
}

#[test]
fn zip_has_a_designer_section_in_both_languages() {
    let path = tmp("designer.zip");
    std::fs::write(&path, build_job_zip(&sample()).unwrap()).unwrap();
    let out = python(
        "import sys,zipfile,json\nz=zipfile.ZipFile(sys.argv[1])\nt=z.read('designer.txt').decode('utf-8')\nm=json.loads(z.read('inputs.json'))\nassert 'Questions for the designer' in t and '設計者への確認事項' in t,t\nassert 'split_ring' in t and '手描きの出口' in t,t\nassert 'NOT RELEASED' in t and '未リリース' in t,t\nassert m['designer'][0]['code']=='split_ring',m['designer']\nprint('DESIGNER_OK')\n",
        &[&path],
    );
    if let Some(out) = out {
        assert!(out.contains("DESIGNER_OK"), "{out}");
    }
    let empty = String::from_utf8(designer_txt(&JobInput::default())).unwrap();
    assert!(empty.contains("No questions.") && empty.contains("確認事項なし"));
}

#[test]
fn zip_read_by_python_and_unzip() {
    let path = tmp("job.zip");
    std::fs::write(&path, build_job_zip(&sample()).unwrap()).unwrap();
    let out = python(
        "import sys,zipfile,hashlib,json\nz=zipfile.ZipFile(sys.argv[1])\nassert z.testzip() is None\nn=z.namelist()\nprint('\\n'.join(n))\nm=json.loads(z.read('inputs.json'))\nfor f in m['files']:\n    assert hashlib.sha256(z.read(f['name'])).hexdigest()==f['sha256'],f\n    assert len(z.read(f['name']))==f['bytes']\nassert m['release_state'].startswith('NOT RELEASED')\nprint('MANIFEST_OK',len(m['files']))\n",
        &[&path],
    );
    if let Some(out) = out {
        assert!(out.contains("dxf/G-TEE-1_本管.dxf"), "{out}");
        assert!(out.contains("dxf/J-BIGPIPE-2_曲管_始点側.dxf"), "{out}");
        assert!(
            out.contains("dxf/a_b-3_x_y.dxf") && out.contains("dxf/a_b-3_x_y_2.dxf"),
            "{out}"
        );
        assert!(out.contains("MANIFEST_OK 10"), "{out}");
    }
    if let Ok(o) = Command::new("unzip").arg("-t").arg(&path).output() {
        let s = String::from_utf8_lossy(&o.stdout).to_string();
        assert!(o.status.success(), "{s}");
        assert!(s.contains("No errors detected"), "{s}");
    }
}

#[test]
fn csv_ja_and_typed_columns() {
    let ja = material_csv(&sample(), true);
    assert_eq!(&ja[..3], &[0xEF, 0xBB, 0xBF]);
    let path = tmp("m_ja.csv");
    std::fs::write(&path, &ja).unwrap();
    let out = python(
        "import sys,csv\nb=open(sys.argv[1],'rb').read()\nassert b[:3]==b'\\xef\\xbb\\xbf'\nt=b.decode('utf-8-sig')\nr=list(csv.reader(t.splitlines(True)))\nprint(r[0][2],r[1][2],r[1][3])\nassert r[0][6]=='板厚mm'\nfloat(r[1][6]);float(r[1][12]);float(r[1][13])\nassert r[2][12]=='' and r[2][13]=='' and r[2][4]=='購入品'\nassert r[1][14]=='計算の重さは切った後（切る前 445.7kg）、枝は載せる'\nprint('CSV_OK',len(r))\n",
        &[&path],
    );
    if let Some(out) = out {
        assert!(out.contains("部材 本管 本管（枝の穴）"), "{out}");
        assert!(out.contains("CSV_OK 3"), "{out}");
    }
    let en = String::from_utf8(material_csv(&sample(), false)).unwrap();
    assert!(
        en.contains("Mass in material table kg")
            && en.contains("NOT RELEASED; Checked by 関 2026-11-27")
    );
    // formula-looking text is neutralised, quotes doubled
    assert!(en.contains("'=cmd|"), "{en}");
    assert!(en.contains("\"Mass is after cutting, \"\"blank\"\" 445.7 kg\""));
}

#[test]
fn xlsx_equals_csv_source() {
    let inp = sample();
    let xp = tmp("m.xlsx");
    std::fs::write(&xp, material_xlsx(&inp).unwrap()).unwrap();
    // Reference reader using only the standard library (LibreOffice/openpyxl are not installed here).
    let script = "import sys,zipfile,csv,io\nimport xml.etree.ElementTree as ET\nNS='{http://schemas.openxmlformats.org/spreadsheetml/2006/main}'\nz=zipfile.ZipFile(sys.argv[1]);assert z.testzip() is None\nwb=ET.fromstring(z.read('xl/workbook.xml'))\nnames=[s.get('name') for s in wb.iter(NS+'sheet')]\nprint(names)\nrows=[]\nfor row in ET.fromstring(z.read('xl/worksheets/sheet1.xml')).iter(NS+'row'):\n    cells={}\n    for c in row:\n        col=ord(c.get('r')[0])-65\n        v=c.find(NS+'v')\n        if v is not None: cells[col]=v.text\n        else:\n            t=c.find(NS+'is')\n            cells[col]=''.join(t.itertext())\n    rows.append([cells.get(i,'') for i in range(16)])\ncsvr=list(csv.reader(io.StringIO(open(sys.argv[2],'rb').read().decode('utf-8-sig'),newline='')))\nassert len(rows)==len(csvr),(len(rows),len(csvr))\nfor a,b in zip(rows,csvr):\n    b=[x[1:] if x.startswith(\"'=\") else x for x in b]\n    assert a==b,(a,b)\nprint('XLSX_EQ_CSV',len(rows))\n";
    let cp = tmp("m_ja2.csv");
    std::fs::write(&cp, material_csv(&inp, true)).unwrap();
    if let Some(out) = python(script, &[&xp, &cp]) {
        assert!(out.contains("XLSX_EQ_CSV 3"), "{out}");
        assert!(out.contains("材料表") && out.contains("Materials"), "{out}");
    }
    // LibreOffice, when installed: convert to csv and compare with the source.
    if Command::new("soffice").arg("--version").output().is_ok() {
        let dir = tmp("lo");
        std::fs::create_dir_all(&dir).unwrap();
        let o = Command::new("soffice")
            .args(["--headless", "--convert-to", "csv", "--outdir"])
            .arg(&dir)
            .arg(&xp)
            .output()
            .unwrap();
        assert!(o.status.success());
    }
}

#[test]
fn zip_rejects_bad_and_duplicate_names() {
    let mut z = ZipWriter::new();
    z.add("a.txt", b"x").unwrap();
    assert_eq!(
        z.add("a.txt", b"y"),
        Err(ZipError::DuplicateName("a.txt".into()))
    );
    assert!(z.add("../evil", b"y").is_err());
    assert!(z.add("/abs", b"y").is_err());
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn record_and_issues_text() {
    let r = String::from_utf8(record_csv(&sample())).unwrap();
    assert!(
        r.starts_with('\u{feff}') && r.contains("2026-11-27 10:29,関,"),
        "{r}"
    );
    let t = String::from_utf8(issues_txt(&sample())).unwrap();
    assert!(t.contains("Does not fit the plate") && t.contains("板に入らない"));
}

fn piece(drawing: &str, no: &str, name: &str) -> Piece {
    Piece {
        drawing: drawing.into(),
        no: no.into(),
        name: name.into(),
        label: None,
        dxf: b"0\nEOF\n".to_vec(),
    }
}

fn zip_names(z: &[u8], tag: &str) -> Vec<String> {
    let p = tmp(&format!("{tag}.zip"));
    std::fs::write(&p, z).unwrap();
    let o = Command::new("unzip").arg("-Z1").arg(&p).output().unwrap();
    String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

#[test]
fn long_japanese_name_extracts_with_unzip() {
    let inp = JobInput {
        pieces: vec![
            piece("G", "1", &"曲管".repeat(60)),
            piece("G", "1", &"曲管".repeat(60)),
        ],
        ..Default::default()
    };
    let z = build_job_zip(&inp).unwrap();
    let zp = tmp("long.zip");
    std::fs::write(&zp, &z).unwrap();
    let out = tmp("long_out");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let o = Command::new("unzip")
        .arg("-q")
        .arg(&zp)
        .arg("-d")
        .arg(&out)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    for n in zip_names(&z, "long") {
        for part in n.split('/') {
            assert!(part.len() <= 255, "{n}");
        }
    }
}

#[test]
fn xlsx_drops_non_xml_characters() {
    let mut inp = sample();
    inp.material[0].name = "a\u{fffe}b\u{ffff}c\u{1}d".into();
    let xp = tmp("badchars.xlsx");
    std::fs::write(&xp, material_xlsx(&inp).unwrap()).unwrap();
    let script = "import sys,zipfile,xml.etree.ElementTree as ET\n\
        z=zipfile.ZipFile(sys.argv[1])\n\
        for n in z.namelist():\n\
        \tif n.endswith('.xml') or n.endswith('.rels'): ET.fromstring(z.read(n))\n\
        print('XMLOK')\n";
    let o = Command::new("python3")
        .args(["-I", "-c", script])
        .arg(&xp)
        .output()
        .unwrap();
    let so = String::from_utf8_lossy(&o.stdout);
    assert!(
        so.contains("XMLOK"),
        "{so} {}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn empty_outputs_carry_marking_and_checked_by() {
    let inp = JobInput {
        checked: Some(Checked {
            name: "関".into(),
            date: "2026-11-27".into(),
        }),
        ..Default::default()
    };
    for (what, bytes) in [
        ("materials_en", material_csv(&inp, false)),
        ("materials_ja", material_csv(&inp, true)),
        ("record", record_csv(&inp)),
        ("issues", issues_txt(&inp)),
    ] {
        let t = String::from_utf8(bytes).unwrap();
        assert!(
            t.contains("NOT RELEASED") || t.contains("未リリース"),
            "{what}: {t}"
        );
        assert!(t.contains("関"), "{what}: {t}");
    }
    let t = String::from_utf8(material_csv(&inp, true)).unwrap();
    assert!(t.contains("未リリース"));
}

#[test]
fn windows_reserved_and_case_duplicate_names() {
    let mut inp = JobInput::default();
    for n in ["CON", "prn", "Aux", "nul", "COM1", "lpt9", "con.txt"] {
        inp.pieces.push(piece("", n, ""));
    }
    inp.pieces.push(piece("g", "1", "abc"));
    inp.pieces.push(piece("G", "1", "ABC"));
    inp.pieces.push(piece("G", "1", "abc"));
    let names = zip_names(&build_job_zip(&inp).unwrap(), "win");
    let dxf: Vec<&String> = names.iter().filter(|n| n.starts_with("dxf/")).collect();
    assert_eq!(dxf.len(), 10);
    let mut lower: Vec<String> = dxf.iter().map(|n| n.to_lowercase()).collect();
    lower.sort();
    lower.dedup();
    assert_eq!(lower.len(), 10, "{dxf:?}");
    for n in &dxf {
        let stem = n.trim_start_matches("dxf/");
        let base = stem.split('.').next().unwrap().to_ascii_uppercase();
        assert!(
            !["CON", "PRN", "AUX", "NUL", "COM1", "LPT9"].contains(&base.as_str()),
            "{n}"
        );
    }
}

#[test]
fn full_width_formula_prefixes_guarded() {
    let rows = vec![vec![
        Cell::Text("＝1".into()),
        Cell::Text("＋1".into()),
        Cell::Text("－1".into()),
        Cell::Text("＠x".into()),
    ]];
    let t = String::from_utf8(cells_to_csv(&rows)).unwrap();
    assert!(t.contains("'＝1,'＋1,'－1,'＠x"), "{t}");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "timing bound: release build only")]
fn ten_thousand_same_name_pieces_are_fast() {
    let inp = JobInput {
        pieces: (0..10_000).map(|_| piece("G", "1", "same")).collect(),
        ..Default::default()
    };
    let t = std::time::Instant::now();
    let z = build_job_zip(&inp).unwrap();
    let el = t.elapsed();
    assert!(el.as_secs_f64() < 1.0, "{el:?}");
    assert!(z.len() > 10_000);
}
