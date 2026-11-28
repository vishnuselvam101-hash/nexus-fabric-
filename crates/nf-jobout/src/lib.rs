//! Pure job output writers for NEXUS FABRIC: one stored ZIP with the DXF pieces, the material list
//! (CSV EN/JA and XLSX), the issue list, the change record and an `inputs.json` manifest.
//!
//! No server types, no clock, no randomness: the caller passes every value (including the date
//! strings), so the same input always gives the same bytes.
//!
//! Every output carries the NOT RELEASED / 未リリース marking and either "Checked by <name> <date>" or
//! "Not checked by a person". Nothing here approves or releases anything.

mod json;
pub mod xlsx;
pub mod zip;

use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use xlsx::{Cell, Sheet};

pub use zip::{crc32, ZipError, ZipWriter};

pub const NOT_RELEASED_EN: &str = "NOT RELEASED";
pub const NOT_RELEASED_JA: &str = "未リリース";

/// A person's check record (not a release).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub name: String,
    pub date: String,
}

#[derive(Clone, Debug, Default)]
pub struct Part {
    pub drawing: String,
    pub no: String,
    pub name: String,
    pub shape: String,
    pub qty: u32,
    pub material: String,
    pub thickness_mm: Option<f64>,
    /// Hold code when no DXF was made for this part.
    pub held: Option<String>,
}

/// A DXF already made by the engine. File name: `<drawing>-<no>_<name>[_<label>].dxf`.
#[derive(Clone, Debug, Default)]
pub struct Piece {
    pub drawing: String,
    pub no: String,
    pub name: String,
    pub label: Option<String>,
    pub dxf: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RowKind {
    #[default]
    Piece,
    Purchased,
}

/// One material list row. Numbers are typed (empty when `None`); notes are separate columns.
#[derive(Clone, Debug, Default)]
pub struct MaterialRow {
    pub drawing: String,
    pub no: String,
    pub name: String,
    pub shape: String,
    pub kind: RowKind,
    pub material: String,
    pub thickness_mm: Option<f64>,
    pub qty: Option<u32>,
    pub width_mm: Option<f64>,
    pub height_mm: Option<f64>,
    pub length_mm: Option<f64>,
    pub unit_mass_kg: Option<f64>,
    pub mass_calc_kg: Option<f64>,
    /// Mass from the drawing's material table; put it on ONE row of a multi-piece part.
    pub mass_bom_kg: Option<f64>,
    pub note_en: String,
    pub note_ja: String,
}

#[derive(Clone, Debug, Default)]
pub struct Issue {
    pub drawing: String,
    pub no: String,
    pub name: String,
    pub code: String,
    pub title_en: String,
    pub title_ja: String,
    pub message_en: String,
    pub message_ja: String,
}

/// One line of the change record.
/// One question for the designer (a review code and the two-language text of the question and any claim).
#[derive(Clone, Debug, Default)]
pub struct DesignerNote {
    pub drawing: String,
    pub no: String,
    pub code: String,
    pub question_en: String,
    pub question_ja: String,
    pub answer_en: String,
    pub answer_ja: String,
}

#[derive(Clone, Debug, Default)]
pub struct RecordEntry {
    pub date: String,
    pub user: String,
    pub action: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, Default)]
pub struct ValueEntry {
    pub part: String,
    pub key: String,
    pub value: Option<f64>,
    pub text: String,
    pub source: String,
    pub state: String,
}

#[derive(Clone, Debug, Default)]
pub struct Provenance {
    pub engine_version: String,
    /// How the values were read (reader id, model hash, page image sha256, ...), name/value pairs.
    pub reading: Vec<(String, String)>,
    /// Factory rules and conventions applied, name/value pairs.
    pub rules: Vec<(String, String)>,
    pub values: Vec<ValueEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct JobInput {
    pub job_name: String,
    /// Export time as text, given by the caller (e.g. `2026-11-27 10:30`).
    pub generated: String,
    pub checked: Option<Checked>,
    pub parts: Vec<Part>,
    pub pieces: Vec<Piece>,
    pub material: Vec<MaterialRow>,
    pub issues: Vec<Issue>,
    /// Review questions, designer claims and hand-drawn exits that go to the designer, not to the cutter.
    pub designer: Vec<DesignerNote>,
    pub record: Vec<RecordEntry>,
    pub provenance: Provenance,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JoboutError {
    Zip(ZipError),
}

impl std::fmt::Display for JoboutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoboutError::Zip(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for JoboutError {}

impl From<ZipError> for JoboutError {
    fn from(e: ZipError) -> Self {
        JoboutError::Zip(e)
    }
}

pub fn sha256_hex(data: &[u8]) -> String {
    let d = Sha256::digest(data);
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// "NOT RELEASED; Checked by X date" / "Not checked by a person" in both languages.
pub fn marking(checked: &Option<Checked>, ja: bool) -> String {
    match (checked, ja) {
        (Some(c), false) => format!("{NOT_RELEASED_EN}; Checked by {} {}", c.name, c.date),
        (None, false) => format!("{NOT_RELEASED_EN}; Not checked by a person"),
        (Some(c), true) => format!("{NOT_RELEASED_JA}；{} {} 確認済み", c.name, c.date),
        (None, true) => format!("{NOT_RELEASED_JA}；担当者の確認なし"),
    }
}

/// Shortest-ish decimal text, at most 3 decimals; empty for non-finite.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return String::new();
    }
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn num(v: Option<f64>) -> Cell {
    match v.map(fmt_num) {
        Some(s) if !s.is_empty() => Cell::Num(s),
        _ => Cell::Empty,
    }
}

fn text(s: &str) -> Cell {
    if s.is_empty() {
        Cell::Empty
    } else {
        Cell::Text(s.to_string())
    }
}

/// Material list as typed cells. Row 0 is the header.
pub fn material_cells(inp: &JobInput, ja: bool) -> Vec<Vec<Cell>> {
    let h: [&str; 16] = if ja {
        [
            "図面",
            "番号",
            "部材",
            "形",
            "区分",
            "材質",
            "板厚mm",
            "枚数",
            "外形の幅mm",
            "外形の高さmm",
            "長さmm",
            "単位質量kg",
            "重さkg（計算）",
            "材料表の重さkg",
            "備考",
            "状態",
        ]
    } else {
        [
            "Drawing",
            "No",
            "Part",
            "Shape",
            "Kind",
            "Material",
            "Thickness mm",
            "Qty",
            "Blank width mm",
            "Blank height mm",
            "Length mm",
            "Unit mass kg",
            "Mass calculated kg",
            "Mass in material table kg",
            "Note",
            "Status",
        ]
    };
    let mut rows = vec![h
        .iter()
        .map(|s| Cell::Text((*s).to_string()))
        .collect::<Vec<_>>()];
    let status = marking(&inp.checked, ja);
    if inp.material.is_empty() {
        // Empty list: still one line carrying the marking and the check record.
        let mut row = vec![Cell::Empty; 15];
        row.push(Cell::Text(status.clone()));
        rows.push(row);
    }
    for r in &inp.material {
        let kind = match (r.kind, ja) {
            (RowKind::Piece, false) => "Piece",
            (RowKind::Piece, true) => "切図",
            (RowKind::Purchased, false) => "Purchased",
            (RowKind::Purchased, true) => "購入品",
        };
        rows.push(vec![
            text(&r.drawing),
            text(&r.no),
            text(&r.name),
            text(&r.shape),
            text(kind),
            text(&r.material),
            num(r.thickness_mm),
            r.qty.map_or(Cell::Empty, |q| Cell::Num(q.to_string())),
            num(r.width_mm),
            num(r.height_mm),
            num(r.length_mm),
            num(r.unit_mass_kg),
            num(r.mass_calc_kg),
            num(r.mass_bom_kg),
            text(if ja { &r.note_ja } else { &r.note_en }),
            text(&status),
        ]);
    }
    rows
}

fn csv_field(c: &Cell) -> String {
    let (s, is_text) = match c {
        Cell::Empty => return String::new(),
        Cell::Num(n) => (n.as_str(), false),
        Cell::Text(t) => (t.as_str(), true),
    };
    // Spreadsheet formula injection: text that would be read as a formula gets a leading quote.
    let guarded;
    let formula = matches!(
        s.chars().next(),
        Some('=' | '+' | '-' | '@' | '\t' | '\r' | '＝' | '＋' | '－' | '＠')
    );
    let s = if is_text && formula {
        guarded = format!("'{s}");
        guarded.as_str()
    } else {
        s
    };
    if s.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// UTF-8 with BOM, CRLF lines.
pub fn cells_to_csv(rows: &[Vec<Cell>]) -> Vec<u8> {
    let mut out = String::from("\u{feff}");
    for r in rows {
        let line: Vec<String> = r.iter().map(csv_field).collect();
        out.push_str(&line.join(","));
        out.push_str("\r\n");
    }
    out.into_bytes()
}

pub fn material_csv(inp: &JobInput, ja: bool) -> Vec<u8> {
    cells_to_csv(&material_cells(inp, ja))
}

pub fn material_xlsx(inp: &JobInput) -> Result<Vec<u8>, ZipError> {
    xlsx::write_xlsx(&[
        Sheet {
            name: "材料表".into(),
            rows: material_cells(inp, true),
        },
        Sheet {
            name: "Materials".into(),
            rows: material_cells(inp, false),
        },
    ])
}

fn head(inp: &JobInput, ja: bool) -> String {
    let mut s = String::new();
    s.push_str(&marking(&inp.checked, ja));
    s.push('\n');
    if ja {
        s.push_str(&format!(
            "工事: {}\n作成: {}\n",
            inp.job_name, inp.generated
        ));
    } else {
        s.push_str(&format!(
            "Job: {}\nGenerated: {}\n",
            inp.job_name, inp.generated
        ));
    }
    s
}

/// Issue list, English then Japanese in one UTF-8 text file.
pub fn issues_txt(inp: &JobInput) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&head(inp, false));
    s.push_str("\n== Issues for the designer ==\n");
    if inp.issues.is_empty() {
        s.push_str("No issues.\n");
    }
    for i in &inp.issues {
        s.push_str(&format!(
            "\n{} {} {}: {} [{}]\n{}\n",
            i.drawing, i.no, i.name, i.title_en, i.code, i.message_en
        ));
    }
    s.push_str("\n\n");
    s.push_str(&head(inp, true));
    s.push_str("\n== 設計者への指摘 ==\n");
    if inp.issues.is_empty() {
        s.push_str("指摘なし。\n");
    }
    for i in &inp.issues {
        s.push_str(&format!(
            "\n{} {} {}：{} [{}]\n{}\n",
            i.drawing, i.no, i.name, i.title_ja, i.code, i.message_ja
        ));
    }
    s.into_bytes()
}

/// Designer section: the review questions, claims and hand-drawn exits. It is a question list, never an approval;
/// the NOT RELEASED marking of the header applies to it too.
pub fn designer_txt(inp: &JobInput) -> Vec<u8> {
    let mut s = String::new();
    s.push_str(&head(inp, false));
    s.push_str("\n== Questions for the designer ==\n");
    s.push_str("These entries ask for a decision; none of them releases a cut file.\n");
    if inp.designer.is_empty() {
        s.push_str("No questions.\n");
    }
    for n in &inp.designer {
        s.push_str(&format!(
            "\n{} {} [{}] {}\nAnswer: {}\n",
            n.drawing, n.no, n.code, n.question_en, n.answer_en
        ));
    }
    s.push_str("\n\n");
    s.push_str(&head(inp, true));
    s.push_str("\n== 設計者への確認事項 ==\n");
    s.push_str("以下は判断を求めるものです。切断ファイルを許可するものではありません。\n");
    if inp.designer.is_empty() {
        s.push_str("確認事項なし。\n");
    }
    for n in &inp.designer {
        s.push_str(&format!(
            "\n{} {} [{}] {}\n回答：{}\n",
            n.drawing, n.no, n.code, n.question_ja, n.answer_ja
        ));
    }
    s.into_bytes()
}

/// Change record: date and user as separate columns, before/after separate.
pub fn record_csv(inp: &JobInput) -> Vec<u8> {
    let t = |s: &str| Cell::Text(s.to_string());
    let mut rows = vec![vec![
        t("Date 日時"),
        t("User 担当者"),
        t("Action 内容"),
        t("Before 前"),
        t("After 後"),
        t("Status 状態"),
    ]];
    let status = format!(
        "{} / {}",
        marking(&inp.checked, false),
        marking(&inp.checked, true)
    );
    if inp.record.is_empty() {
        let mut row = vec![Cell::Empty; 5];
        row.push(Cell::Text(status.clone()));
        rows.push(row);
    }
    for r in &inp.record {
        rows.push(vec![
            text(&r.date),
            text(&r.user),
            text(&r.action),
            text(&r.before),
            text(&r.after),
            text(&status),
        ]);
    }
    cells_to_csv(&rows)
}

fn safe_name(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
            o.push('_');
        } else {
            o.push(c);
        }
    }
    o
}

fn piece_stem(p: &Piece) -> String {
    let mut stem = String::new();
    if !p.drawing.is_empty() {
        stem.push_str(&safe_name(&p.drawing));
        stem.push('-');
    }
    stem.push_str(&safe_name(&p.no));
    if !p.name.is_empty() {
        stem.push('_');
        stem.push_str(&safe_name(&p.name));
    }
    if let Some(l) = p.label.as_deref().filter(|l| !l.is_empty()) {
        stem.push('_');
        stem.push_str(&safe_name(l));
    }
    let mut stem = cap_bytes(stem.trim_matches(['.', ' ']), NAME_CAP);
    if stem.is_empty() {
        stem = "part".to_string();
    }
    if is_reserved(&stem) {
        stem.insert(0, '_');
    }
    stem
}

/// Longest stem kept, in bytes: a part name is at most 255 bytes, and a `_NNNNN.dxf` suffix must fit.
const NAME_CAP: usize = 200;

/// Truncate to at most `max` bytes on a char boundary.
fn cap_bytes(s: &str, max: usize) -> String {
    let mut end = 0;
    for (i, c) in s.char_indices() {
        if i + c.len_utf8() > max {
            break;
        }
        end = i + c.len_utf8();
    }
    s[..end].trim_matches(['.', ' ']).to_string()
}

/// Windows device names (CON, PRN, AUX, NUL, COM1-9, LPT1-9), with or without an extension, any case.
fn is_reserved(stem: &str) -> bool {
    let base = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
    match base.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        b if b.len() == 4 && (b.starts_with("COM") || b.starts_with("LPT")) => {
            matches!(b.as_bytes()[3], b'1'..=b'9')
        }
        _ => false,
    }
}

/// Manifest: values with source and state, rules, reading provenance, sha256 of every other file.
pub fn inputs_json(inp: &JobInput, files: &[(String, Vec<u8>)]) -> Vec<u8> {
    use json::J;
    let kv = |v: &[(String, String)]| {
        J::Obj(
            v.iter()
                .map(|(k, x)| (k.clone(), J::Str(x.clone())))
                .collect(),
        )
    };
    let opt = |v: Option<f64>| {
        v.filter(|x| x.is_finite())
            .map_or(J::Null, |x| J::Raw(fmt_num(x)))
    };
    let doc = J::Obj(vec![
        ("schema".into(), J::Str("nf-jobout/1".into())),
        ("job".into(), J::Str(inp.job_name.clone())),
        ("generated".into(), J::Str(inp.generated.clone())),
        (
            "release_state".into(),
            J::Str(format!("{NOT_RELEASED_EN} / {NOT_RELEASED_JA}")),
        ),
        (
            "checked".into(),
            match &inp.checked {
                Some(c) => J::Obj(vec![
                    ("by".into(), J::Str(c.name.clone())),
                    ("date".into(), J::Str(c.date.clone())),
                ]),
                None => J::Str("Not checked by a person / 担当者の確認なし".into()),
            },
        ),
        (
            "engine_version".into(),
            J::Str(inp.provenance.engine_version.clone()),
        ),
        ("reading".into(), kv(&inp.provenance.reading)),
        ("rules".into(), kv(&inp.provenance.rules)),
        (
            "parts".into(),
            J::Arr(
                inp.parts
                    .iter()
                    .map(|p| {
                        J::Obj(vec![
                            ("drawing".into(), J::Str(p.drawing.clone())),
                            ("no".into(), J::Str(p.no.clone())),
                            ("name".into(), J::Str(p.name.clone())),
                            ("shape".into(), J::Str(p.shape.clone())),
                            ("qty".into(), J::Raw(p.qty.to_string())),
                            ("material".into(), J::Str(p.material.clone())),
                            ("thickness_mm".into(), opt(p.thickness_mm)),
                            ("held".into(), p.held.clone().map_or(J::Null, J::Str)),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "designer".into(),
            J::Arr(
                inp.designer
                    .iter()
                    .map(|n| {
                        J::Obj(vec![
                            ("drawing".into(), J::Str(n.drawing.clone())),
                            ("no".into(), J::Str(n.no.clone())),
                            ("code".into(), J::Str(n.code.clone())),
                            ("question_en".into(), J::Str(n.question_en.clone())),
                            ("question_ja".into(), J::Str(n.question_ja.clone())),
                            ("answer_en".into(), J::Str(n.answer_en.clone())),
                            ("answer_ja".into(), J::Str(n.answer_ja.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "values".into(),
            J::Arr(
                inp.provenance
                    .values
                    .iter()
                    .map(|v| {
                        J::Obj(vec![
                            ("part".into(), J::Str(v.part.clone())),
                            ("key".into(), J::Str(v.key.clone())),
                            ("value".into(), opt(v.value)),
                            ("text".into(), J::Str(v.text.clone())),
                            ("source".into(), J::Str(v.source.clone())),
                            ("state".into(), J::Str(v.state.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "files".into(),
            J::Arr(
                files
                    .iter()
                    .map(|(n, d)| {
                        J::Obj(vec![
                            ("name".into(), J::Str(n.clone())),
                            ("bytes".into(), J::Raw(d.len().to_string())),
                            ("sha256".into(), J::Str(sha256_hex(d))),
                        ])
                    })
                    .collect(),
            ),
        ),
        (
            "files_note".into(),
            J::Str("inputs.json cannot list its own hash".into()),
        ),
    ]);
    let mut s = String::new();
    doc.write(&mut s, 0);
    s.push('\n');
    s.into_bytes()
}

/// Build the export zip. Order: dxf/*, materials_en.csv, materials_ja.csv, materials.xlsx,
/// issues.txt, designer.txt, record.csv, inputs.json. Never panics on an empty job.
pub fn build_job_zip(inp: &JobInput) -> Result<Vec<u8>, JoboutError> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    // Names compare case-insensitively (Windows and macOS file systems). `next` remembers the next
    // suffix per stem so duplicates cost O(1) each.
    let mut used: HashSet<String> = HashSet::new();
    let mut next: HashMap<String, u32> = HashMap::new();
    for p in &inp.pieces {
        let stem = piece_stem(p);
        let key = stem.to_lowercase();
        let mut k = next.get(&key).copied().unwrap_or(1);
        let name = loop {
            let cand = if k == 1 {
                format!("dxf/{stem}.dxf")
            } else {
                format!("dxf/{stem}_{k}.dxf")
            };
            k += 1;
            if used.insert(cand.to_lowercase()) {
                break cand;
            }
        };
        next.insert(key, k);
        files.push((name, p.dxf.clone()));
    }
    files.push(("materials_en.csv".into(), material_csv(inp, false)));
    files.push(("materials_ja.csv".into(), material_csv(inp, true)));
    files.push(("materials.xlsx".into(), material_xlsx(inp)?));
    files.push(("issues.txt".into(), issues_txt(inp)));
    files.push(("designer.txt".into(), designer_txt(inp)));
    files.push(("record.csv".into(), record_csv(inp)));
    let manifest = inputs_json(inp, &files);
    files.push(("inputs.json".into(), manifest));
    let mut z = ZipWriter::new();
    for (n, d) in &files {
        z.add(n, d)?;
    }
    Ok(z.finish()?)
}
