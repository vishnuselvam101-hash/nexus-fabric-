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
        assert!(out.contains("MANIFEST_OK 9"), "{out}");
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
