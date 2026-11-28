//! PZ-1 review catalogue and the ZK review kinds that the order compute did not raise before this lane.
//!
//! The catalogue is the answer to "what may be done about this hold": every code the compute can emit has one
//! English and one Japanese row, with the answers a person may give. No row approves or releases anything; an answer
//! is a decision with a reason, tied to the value it was made against and undoable.
//!
//! `compute_order_with_context` is additive: `compute_order` stays pure and unchanged for callers that have no
//! page material table. The context only adds review issues (and, for blocking kinds, holds on the named part).

use serde::Serialize;
use serde_json::{Value, json};

use crate::order::TableRow;
use crate::order_compute::{self, OrderResult, OrderRules, PartIn, Val, ValState};

/// Kinds that no decision may ever wave through, including the ZK `FLAT_STOP` and the canonical emitted forms
/// of the ZK aliases (`seam_from` -> `seam_from_unknown`, `unsupported` -> `shape_unsupported`).
pub const NEVER_WAVE: &[&str] = &[
    "FLAT_STOP",
    "split_ring",
    "seam_from",
    "seam_from_unknown",
    "missing",
    "unsupported",
    "shape_unsupported",
    "empty",
    "split_cone",
    "tee_bad",
    "tee_branches",
    "plate_bad",
    "plate_off",
    "plate_C_big",
    "rib_count",
    "rib_n1",
    "bend_bad",
    "wye_bad",
];

/// The review kinds the PZ-1 golden table compares with zumen-kirizu.
pub const LISTED_CODES: &[&str] = &[
    "mass_total",
    "row_unmatched",
    "page_empty",
    "shape_unsupported",
    "seam_from_unknown",
    "tee_angle_obtuse",
    "tee_flange_face",
    "tee_mount_note",
    "pad_qty",
    "tee_qty",
    "split_cone",
    "split_ring",
    "pad_split",
    "size_vs_table",
    "cyl_len",
    "bend_len",
    "mass",
    "stock_rule",
    "unreadable",
];

/// One catalogue row: the code, its two titles, the level and scope of the check, what a person may answer, and
/// whether a flag may be accepted for it. `may_accept` is false for every never-wave kind; no row is an approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueRow {
    pub code: String,
    pub level: &'static str,
    pub scope: &'static str,
    pub actions: Vec<String>,
    pub may_accept: bool,
    pub title_en: String,
    pub title_ja: String,
    pub answers_en: String,
    pub answers_ja: String,
}

/// The kinds whose answers are not the generic "confirm / enter / undo" set.
const SPECIFIC: &[(&str, &str, &str, &str, &str)] = &[
    (
        "mass_total",
        "Material-table mass total",
        "材料表の質量の合計",
        "Correct the total or the table mass, or undo the earlier confirmation; never accept a flag.",
        "合計か表の質量を直すか、以前の確認を取り消す。フラグを受理してはならない。",
    ),
    (
        "row_unmatched",
        "Unmatched material-table row",
        "結び付かない材料表の行",
        "Give the row a part, exclude it deliberately with a reason, or undo that decision.",
        "行に部材を割り当てる、理由を添えて除外する、またはその判断を取り消す。",
    ),
    (
        "page_empty",
        "Empty page",
        "空のページ",
        "Skip the page with a reason or supply its reading again.",
        "理由を添えてページを飛ばすか、読み取りをやり直す。",
    ),
    (
        "shape_unsupported",
        "Unsupported shape",
        "未対応の形",
        "Leave it as a pattern view, or supply a supported shape; never release a cut file.",
        "展開図の表示のままにするか、対応する形を与える。切断ファイルは出さない。",
    ),
    (
        "seam_from_unknown",
        "Unknown seam reference",
        "縦継ぎ目の基準不明",
        "Choose bottom or top and confirm the choice; undo to change it.",
        "管底か天かを選んで確認する。変更するときは取り消す。",
    ),
    (
        "tee_angle_obtuse",
        "Obtuse branch angle",
        "枝の鈍角",
        "Confirm the acute angle, or enter the angle as drawn with a reason.",
        "鋭角を確認するか、理由を添えて図面の角度を入力する。",
    ),
    (
        "tee_flange_face",
        "Length to flange face",
        "フランジ面までの長さ",
        "Confirm that the flange length is not deducted, or enter the corrected length.",
        "フランジ長を引かないことを確認するか、直した長さを入力する。",
    ),
    (
        "tee_mount_note",
        "Branch mounting note",
        "枝の付け方の注記",
        "Confirm the provisional rule or enter the mounting stated in the note.",
        "仮の決まりを確認するか、注記の付け方を入力する。",
    ),
    (
        "pad_qty",
        "Pad split and quantity",
        "補強板の割り方と数量",
        "Choose a supported split (1, 2, 4 or 2..12) with a reason, or undo it.",
        "理由を添えて割り方（1、2、4 または2〜12）を選ぶか、取り消す。",
    ),
    (
        "tee_qty",
        "Tee row quantity",
        "T字管の数量",
        "Confirm the number of branches with a reason, or enter the correct quantity.",
        "理由を添えて枝の数を確認するか、正しい数量を入力する。",
    ),
    (
        "split_cone",
        "Cone split count",
        "円すいの割り枚数",
        "Choose 2..12 pieces with a reason; the two-piece split is only provisional.",
        "理由を添えて2〜12枚を選ぶ。二つ割は仮の値である。",
    ),
    (
        "split_ring",
        "Ring split count",
        "輪の割り枚数",
        "Choose the number of pieces before unfolding; this hold cannot be waved.",
        "展開前に割り枚数を選ぶ。この保留は見過ごせない。",
    ),
    (
        "pad_split",
        "Pad split count",
        "補強板の割り枚数",
        "Choose 1 (unsplit) or 2..12 pieces; the unsplit view is only provisional.",
        "1（割らない）か2〜12枚を選ぶ。割らない表示は仮の値である。",
    ),
    (
        "size_vs_table",
        "Plate size against the table",
        "板の寸法と材料表",
        "Check the drawing and the table; enter the read size or accept the table with a reason.",
        "図面と材料表を照らし、読んだ寸法を入力するか、理由を添えて表を採る。",
    ),
    (
        "cyl_len",
        "Cylinder length against the table",
        "管の長さと材料表",
        "Check the drawing and the table; enter the read length or accept the table with a reason.",
        "図面と材料表を照らし、読んだ長さを入力するか、理由を添えて表を採る。",
    ),
    (
        "bend_len",
        "Bend length against the table",
        "曲管の長さと材料表",
        "Check the piece centres and the table; enter the read length or accept the table with a reason.",
        "片の中心と材料表を照らし、読んだ長さを入力するか、理由を添えて表を採る。",
    ),
    (
        "mass",
        "Computed mass against the table",
        "計算質量と材料表",
        "Check the calculation and the table; enter the read mass or accept the table with a reason.",
        "計算と材料表を照らし、読んだ質量を入力するか、理由を添えて表を採る。",
    ),
    (
        "stock_rule",
        "Provisional stock size",
        "仮の定尺",
        "Check against the factory's stock size when it is known; the default is not confirmed.",
        "工場の定尺が分かり次第照らす。既定値は未確認である。",
    ),
    (
        "unreadable",
        "Unreadable drawing note",
        "読めなかった注記",
        "Read the note again and enter it, or skip it with a reason; nothing is inferred from it.",
        "注記を読み直して入力するか、理由を添えて飛ばす。推定して使わない。",
    ),
    (
        "tee_owner",
        "Tee value owner",
        "T字管の値の持ち主",
        "Name the part that owns the shared value, or enter the value on the part itself.",
        "共有値の持ち主となる部材を指すか、その部材に値を入力する。",
    ),
    (
        "cone_owner",
        "Cone branch value owner",
        "円すい枝の値の持ち主",
        "Name the part that owns the shared value, or enter the value on the part itself.",
        "共有値の持ち主となる部材を指すか、その部材に値を入力する。",
    ),
    (
        "tee_branches",
        "Tee branch dimensions incomplete",
        "T字管の枝の寸法不足",
        "Complete the branch values first; no hole is placed from a partial branch.",
        "先に枝の値を揃える。値が足りない枝から穴は作らない。",
    ),
];

fn generic_answers(en: bool) -> (&'static str, &'static str) {
    if en {
        (
            "Confirm the read value, enter a corrected value with a reason, or undo the decision.",
            "Accept a flag only with a reason; a hold never releases the DXF.",
        )
    } else {
        (
            "読んだ値を確認する、理由を添えて直した値を入力する、または判断を取り消す。",
            "フラグを受理する場合も理由が必要。保留がある限りDXFは出ない。",
        )
    }
}

fn level_of(code: &str) -> &'static str {
    match code {
        "mass_total" | "mass" | "size_vs_table" | "cyl_len" | "bend_len" | "stock_rule" => "review",
        _ => "need",
    }
}

fn scope_of(code: &str) -> &'static str {
    match code {
        "mass_total" | "row_unmatched" | "row" | "page_empty" | "empty" | "shape_unsupported"
        | "unsupported" | "stock_rule" | "unreadable" => "page",
        "tee_owner" | "cone_owner" => "group",
        _ => "part",
    }
}

fn actions_of(code: &str, may_accept: bool) -> Vec<String> {
    let mut out = vec!["view_on_page".to_string()];
    if matches!(
        code,
        "missing"
            | "seam_from"
            | "seam_from_unknown"
            | "split_cone"
            | "split_ring"
            | "pad_split"
            | "tee_branches"
    ) {
        out.push("choose_split_or_side".to_string());
    }
    if !matches!(code, "page_empty" | "empty") {
        out.push("enter_value_with_reason".to_string());
    }
    if matches!(
        code,
        "split_cone" | "split_ring" | "pad_split" | "pad_qty" | "tee_qty"
    ) {
        out.push("follow_note".to_string());
    }
    out.push("hand_drawn_exit".to_string());
    out.push("add_to_issues".to_string());
    if may_accept {
        out.push("accept_with_reason".to_string());
    }
    out
}

/// Case-folded, alias-folded form used for lookups and hashes. `FLAT_STOP`, `flat stop`,
/// `plate_C_big` and `plate-c-big` all name the same review row; ZK's aliases name the emitted code.
pub fn canonical(code: &str) -> String {
    let flat: String = code
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let alias = match flat.as_str() {
        "seam_from" | "seam_from_unknown" => "seam_from_unknown",
        "unsupported" | "shape_unsupported" => "shape_unsupported",
        "empty" | "page_empty" => "page_empty",
        "row" | "row_unmatched" => "row_unmatched",
        _ => flat.as_str(),
    };
    alias.to_string()
}

/// True for a review kind that no reason, identity or decision may wave through. Both the emitted code and the
/// ZK alias are refused, in any case and with any spelling of the separator.
pub fn is_never_wave(code: &str) -> bool {
    let c = canonical(code);
    NEVER_WAVE.iter().any(|n| canonical(n) == c)
}

/// Whether an accept decision may answer this code. Catalogue answers never approve a job.
pub fn may_accept(code: &str) -> bool {
    let c = canonical(code);
    !is_never_wave(code)
        && !matches!(
            c.as_str(),
            "page_empty" | "unreadable" | "row_unmatched" | "mass_total"
        )
}

/// The whole catalogue, in code order. Every code of `HOLD_TEXT` has exactly one row.
pub fn catalogue() -> Vec<CatalogueRow> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for (code, en, ja) in order_compute::HOLD_TEXT {
        let (a_en, note_en) = generic_answers(true);
        let (a_ja, note_ja) = generic_answers(false);
        let specific = SPECIFIC
            .iter()
            .find(|r| r.0 == *code)
            .map(|r| (r.2, r.3, r.4));
        let answers_en =
            specific.map_or_else(|| format!("{a_en} {note_en}"), |(_, a, _)| a.to_string());
        let answers_ja =
            specific.map_or_else(|| format!("{a_ja} {note_ja}"), |(_, _, a)| a.to_string());
        seen.insert(*code);
        let accept = may_accept(code);
        out.push(CatalogueRow {
            code: (*code).to_string(),
            level: level_of(code),
            scope: scope_of(code),
            actions: actions_of(code, accept),
            may_accept: accept,
            title_en: (*en).to_string(),
            title_ja: (*ja).to_string(),
            answers_en,
            answers_ja,
        });
    }
    // question-only and alias codes that are not holds but are still reviewable
    for code in [
        "bend_joints_not_on_drawing",
        "tee_x_not_on_drawing",
        "tee_dir_not_on_drawing",
        "row",
        "empty",
        "unsupported",
        "seam_from",
        "tee_branches",
        "FLAT_STOP",
        "split_ring",
        "missing",
        "split_cone",
        "tee_bad",
        "plate_bad",
        "plate_off",
        "plate_C_big",
        "rib_count",
        "rib_n1",
        "bend_bad",
        "wye_bad",
    ] {
        if !seen.insert(code) {
            continue;
        }
        let (a_en, note_en) = generic_answers(true);
        let (a_ja, note_ja) = generic_answers(false);
        let accept = may_accept(code);
        out.push(CatalogueRow {
            code: code.to_string(),
            level: level_of(code),
            scope: scope_of(code),
            actions: actions_of(code, accept),
            may_accept: accept,
            title_en: format!("{code} (review question)"),
            title_ja: format!("{code}（レビューの確認事項）"),
            answers_en: format!("{a_en} {note_en}"),
            answers_ja: format!("{a_ja} {note_ja}"),
        });
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    out
}

pub fn row(code: &str) -> Option<CatalogueRow> {
    let want = canonical(code);
    catalogue().into_iter().find(|r| canonical(&r.code) == want)
}

pub fn codes() -> Vec<String> {
    catalogue().into_iter().map(|r| r.code).collect()
}

pub fn catalogue_json() -> Value {
    json!(
        catalogue()
            .into_iter()
            .map(|r| {
                json!({
                    "code": r.code,
                    "level": r.level,
                    "scope": r.scope,
                    "actions": r.actions,
                    "may_accept": r.may_accept,
                    "never_wave": !r.may_accept,
                    "title_en": r.title_en,
                    "title_ja": r.title_ja,
                    "answers_en": r.answers_en,
                    "answers_ja": r.answers_ja,
                })
            })
            .collect::<Vec<_>>()
    )
}

/// One page-level review issue. `part` is the same `no` as an order part when the issue belongs to one.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReviewIssue {
    pub code: String,
    pub part: Option<String>,
    pub level: &'static str,
    pub keys: Vec<String>,
    pub message_en: String,
    pub message_ja: String,
}

/// One material-table row as read. `part` is the `no` of the part that owns the row, when known.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialRow {
    pub no: String,
    pub name: String,
    pub size: String,
    pub length: String,
    pub qty: Option<f64>,
    pub mass: Option<f64>,
    /// First part that owns the row (kept for the simple one-to-one case).
    pub part: Option<String>,
    /// Every part whose `table_rows` names this row: a quantity of two may be two sibling parts.
    pub parts: Vec<String>,
}

impl MaterialRow {
    pub fn owns(&self, part: &str) -> bool {
        self.part.as_deref() == Some(part) || self.parts.iter().any(|p| p == part)
    }
}

/// Additive context for the review kinds that need page data that `PartIn` does not carry.
#[derive(Debug, Clone, Default)]
pub struct ReviewContext {
    pub material_rows: Vec<MaterialRow>,
    pub page_notes: Vec<String>,
    pub unit_mass_kg: Option<f64>,
    /// 0 when there is no reading. Only used by the `page_empty` review.
    pub pages: usize,
    /// `seam.from` as written; "不明" is not a reference.
    pub seam_from: Option<String>,
    /// Quantity and split checks are on in the server and in the boundary tests, off when replaying golden readings
    /// whose row quantity is accounted for by a group of sibling parts.
    pub strict_quantity: bool,
    pub ring_split_note: bool,
    pub pad_split_note: bool,
}

/// One number in a material-table cell: full-width digits are not assumed here; the server stores the text as it
/// was read. Used by both the review compute and the material-table API so the two agree on what a row says.
pub fn number(s: &str) -> Option<f64> {
    let s: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_digit() || c == '.' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    for t in s.split_whitespace() {
        if let Ok(v) = t.parse::<f64>()
            && v.is_finite()
        {
            return Some(v);
        }
    }
    None
}

fn size_dims(s: &str) -> Option<(f64, f64)> {
    let parts: Vec<&str> = s.split(['×', 'x', 'X', '＊']).collect();
    let nums: Vec<f64> = parts.iter().filter_map(|p| number(p)).collect();
    if nums.len() >= 2 {
        Some((nums[nums.len() - 2], nums[nums.len() - 1]))
    } else {
        None
    }
}

fn text(code: &str) -> (String, String) {
    row(code).map_or_else(
        || (code.to_string(), code.to_string()),
        |r| (r.title_en, r.title_ja),
    )
}

fn push(
    issues: &mut Vec<ReviewIssue>,
    res: &mut OrderResult,
    code: &str,
    part: Option<&str>,
    level: &'static str,
    keys: &[&str],
    blocking: bool,
) {
    if issues
        .iter()
        .any(|i| (i.code.as_str(), i.part.as_deref()) == (code, part))
    {
        return;
    }
    let (en, ja) = text(code);
    issues.push(ReviewIssue {
        code: code.to_string(),
        part: part.map(str::to_string),
        level,
        keys: keys.iter().map(|k| k.to_string()).collect(),
        message_en: en,
        message_ja: ja,
    });
    if blocking && let Some(p) = part {
        order_compute::add_review_hold(res, p, code, keys);
    }
}

/// Build the order compute inputs from a ZK reading file. Only the reading's own fields are used; expected values
/// and ZK's own answers are never read.
pub fn parts_and_context_from_reading(reading: &Value) -> (Vec<PartIn>, ReviewContext) {
    let mut ctx = ReviewContext {
        pages: 1,
        unit_mass_kg: reading["page"]["unit_mass_kg"].as_f64(),
        seam_from: reading["seam"]["from"].as_str().map(str::to_string),
        ..Default::default()
    };
    if let Some(rows) = reading["material_table"].as_array() {
        for r in rows {
            ctx.material_rows.push(MaterialRow {
                no: r["no"].as_str().unwrap_or_default().to_string(),
                name: r["name"].as_str().unwrap_or_default().to_string(),
                size: r["size"].as_str().unwrap_or_default().to_string(),
                length: r["length"].as_str().unwrap_or_default().to_string(),
                qty: r["qty"].as_str().and_then(number),
                mass: r["mass"].as_str().and_then(number),
                part: None,
                parts: Vec::new(),
            });
        }
    }
    if let Some(notes) = reading["notes"].as_array() {
        for n in notes.iter().filter_map(Value::as_str) {
            ctx.page_notes.push(n.to_string());
        }
    }
    if let Some(u) = reading["unreadable"].as_array() {
        for n in u.iter().filter_map(Value::as_str) {
            ctx.page_notes.push(format!("unreadable: {n}"));
        }
    }
    let mut specs: Vec<(PartIn, Vec<String>)> = Vec::new();
    if let Some(ps) = reading["parts"].as_array() {
        for p in ps {
            let no = p["no"].as_str().unwrap_or_default().to_string();
            let shape = p["shape"].as_str().unwrap_or_default().to_string();
            let mut inp = PartIn::new(&no, &shape);
            inp.name = p["name"].as_str().unwrap_or(&no).to_string();
            inp.attach = p["attach"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            inp.group = p["group"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if let Some(params) = p["params"].as_object() {
                for (k, v) in params {
                    let Some(value) = v["value"].as_f64() else {
                        continue;
                    };
                    let state = match v["state"].as_str().unwrap_or("confirmed") {
                        "proposed" => ValState::Proposed,
                        "missing" => ValState::Missing,
                        "corrected" => ValState::Corrected,
                        _ => ValState::Confirmed,
                    };
                    inp.values.insert(
                        k.clone(),
                        Val {
                            value: Some(value),
                            state,
                            source: v["source"].as_str().unwrap_or("drawing").to_string(),
                            reading_verdict: v["reading_verdict"].as_str().map(str::to_string),
                        },
                    );
                }
            }
            let row_nos: Vec<String> = p["table_rows"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            if let Some(notes) = p["notes"].as_array() {
                for n in notes.iter().filter_map(Value::as_str) {
                    ctx.page_notes.push(format!("{no}: {n}"));
                }
            }
            specs.push((inp, row_nos));
        }
    }
    // A material-table row may be shared by a group of sibling parts; every one of them owns it.
    for (inp, row_nos) in &specs {
        for r in &mut ctx.material_rows {
            if row_nos.iter().any(|n| n == &r.no) {
                if r.part.is_none() {
                    r.part = Some(inp.no.clone());
                }
                if !r.parts.iter().any(|n| n == &inp.no) {
                    r.parts.push(inp.no.clone());
                }
            }
        }
    }
    let mut parts = Vec::new();
    for (mut inp, row_nos) in specs {
        if let Some(first) = row_nos.first()
            && let Some(mr) = ctx.material_rows.iter().find(|r| &r.no == first)
        {
            inp.table = Some(TableRow {
                size: mr.size.clone(),
                length: mr.length.clone(),
                mass: mr.mass,
                mass_decimals: mr
                    .mass
                    .map_or(0, |_| mr.size.split('.').nth(1).map_or(0, str::len)),
                qty: mr.qty,
            });
        }
        parts.push(inp);
    }
    ctx.ring_split_note = ctx
        .page_notes
        .iter()
        .any(|n| n.contains("輪") && n.contains("割"));
    ctx.pad_split_note = ctx
        .page_notes
        .iter()
        .any(|n| n.contains("補強板") && n.contains("割"));
    (parts, ctx)
}

/// Compute an order with the page-level review context. Additive: the ordinary `compute_order` result is unchanged
/// when the context is empty.
pub fn compute_order_with_context(
    parts: &[PartIn],
    rules: &OrderRules,
    ctx: &ReviewContext,
) -> OrderResult {
    let mut res = order_compute::compute_order(parts, rules);
    let mut issues = Vec::new();
    let first = parts.first().map(|p| p.no.as_str());

    // ZK carries a page-level `need` on the first part. Without a part there is no cut file to gate, but the
    // issue is still reported so an empty reading cannot be mistaken for a clean one.
    if ctx.pages == 0 || (parts.is_empty() && ctx.material_rows.is_empty()) {
        push(
            &mut issues,
            &mut res,
            "page_empty",
            first,
            "need",
            &[],
            first.is_some(),
        );
    }
    for n in ctx
        .page_notes
        .iter()
        .filter(|n| n.starts_with("unreadable:"))
    {
        push(
            &mut issues,
            &mut res,
            "unreadable",
            first,
            "need",
            &[n.as_str()],
            first.is_some(),
        );
    }
    if ctx.strict_quantity {
        for r in &ctx.material_rows {
            if !r.parts.iter().any(|no| parts.iter().any(|q| &q.no == no))
                && r.part
                    .as_deref()
                    .is_none_or(|no| !parts.iter().any(|q| q.no == no))
            {
                push(
                    &mut issues,
                    &mut res,
                    "row_unmatched",
                    first,
                    "need",
                    &[r.no.as_str()],
                    first.is_some(),
                );
            }
        }
    }
    if let Some(unit) = ctx.unit_mass_kg
        && unit > 0.0
        && !ctx.material_rows.is_empty()
    {
        let total: f64 = ctx.material_rows.iter().filter_map(|r| r.mass).sum();
        let bad = (total - unit).abs() > 1.0f64.max(unit * 0.002);
        // ZK records the total review whether it agrees (`ok`) or not (`need`). A `need` is carried by the
        // first part so `finish()` and the DXF gate see it; an `ok` row stays an informational issue only.
        push(
            &mut issues,
            &mut res,
            "mass_total",
            if bad { first } else { None },
            if bad { "need" } else { "info" },
            &[&total.to_string(), &unit.to_string()],
            bad && first.is_some(),
        );
    }
    let plate_shape = parts.iter().any(|p| {
        !matches!(p.shape.as_str(), "cylinder" | "purchased" | "other" | "")
            && !rules.stock.is_empty()
    });
    if plate_shape {
        push(
            &mut issues,
            &mut res,
            "stock_rule",
            None,
            "info",
            &[],
            false,
        );
    }
    if ctx
        .seam_from
        .as_deref()
        .is_some_and(|s| !matches!(s, "bottom" | "top" | "管底" | "天"))
    {
        push(
            &mut issues,
            &mut res,
            "seam_from_unknown",
            first,
            "need",
            &[],
            true,
        );
    }

    let mount_note = ctx.page_notes.iter().find(|n| {
        n.contains("枝")
            && (n.contains("差し込")
                || n.contains("差込")
                || n.contains("挿入")
                || n.contains("載せ")
                || n.contains("乗せ"))
    });
    for p in parts {
        // ZK reports a row shared by sibling parts once, on the first part that names it. Match that owner
        // so the golden reading gets one review, not one per sibling.
        let row = ctx
            .material_rows
            .iter()
            .find(|r| r.part.as_deref() == Some(p.no.as_str()));
        if matches!(p.shape.as_str(), "cone" | "eccentric_cone") {
            let split = p.values.get("split").and_then(|v| v.usable());
            if !split.is_some_and(|s| (2.0..=12.0).contains(&s) && s.fract() == 0.0) {
                push(
                    &mut issues,
                    &mut res,
                    "split_cone",
                    Some(&p.no),
                    "need",
                    &["split"],
                    true,
                );
            }
        }
        if p.shape == "ring" {
            let split = p.values.get("split").and_then(|v| v.usable());
            if split.is_none() || ctx.ring_split_note {
                push(
                    &mut issues,
                    &mut res,
                    "split_ring",
                    Some(&p.no),
                    "need",
                    &["split"],
                    true,
                );
            }
        }
        if let Some(a) = p.values.get("branch_angle").and_then(|v| v.usable())
            && a > 90.0
            && a < 180.0
        {
            push(
                &mut issues,
                &mut res,
                "tee_angle_obtuse",
                Some(&p.no),
                "need",
                &["branch_angle"],
                true,
            );
        }
        let flange = ["L", "H"].into_iter().find(|k: &&str| {
            p.values.get(*k).is_some_and(|v| {
                v.source.contains("フランジ面")
                    || ctx.page_notes.iter().any(|n| n.contains("フランジ面"))
            })
        });
        if matches!(p.shape.as_str(), "tee_main" | "tee_branch")
            && let Some(k) = flange
        {
            push(
                &mut issues,
                &mut res,
                "tee_flange_face",
                Some(&p.no),
                "need",
                &[k],
                true,
            );
        }
        if p.shape == "tee_branch"
            && mount_note
                .is_some_and(|n| n.contains("差し込") || n.contains("差込") || n.contains("挿入"))
            && rules.mount == crate::tee::Mount::SetOn
        {
            push(
                &mut issues,
                &mut res,
                "tee_mount_note",
                Some(&p.no),
                "need",
                &["mount"],
                true,
            );
        }
        if let Some(r) = row {
            // ZK does not write the table comparisons for a part it could not unfold (missing values or a
            // refusal): there is no computed number to compare.
            let computed = res.part(&p.no).is_some_and(|x| !x.pattern.is_empty());
            let qty = r.qty.filter(|q| *q > 1.0);
            if ctx.strict_quantity && qty.is_some() {
                if p.shape == "tee_pad" {
                    push(
                        &mut issues,
                        &mut res,
                        "pad_qty",
                        Some(&p.no),
                        "need",
                        &["qty"],
                        true,
                    );
                } else if p.shape.starts_with("tee") {
                    push(
                        &mut issues,
                        &mut res,
                        "tee_qty",
                        Some(&p.no),
                        "need",
                        &["qty"],
                        true,
                    );
                }
            }
            if p.shape == "tee_pad" && ctx.pad_split_note {
                push(
                    &mut issues,
                    &mut res,
                    "pad_split",
                    Some(&p.no),
                    "need",
                    &["split"],
                    true,
                );
            }
            // ZK raises the `mass` review for every table row with a mass; it is an information when the
            // computed mass is within tolerance and a blocking hold when it is not.
            if let Some(tm) = r.mass
                && tm > 0.0
            {
                let calc = res.part(&p.no).and_then(|x| x.mass_kg);
                let bad =
                    calc.is_some_and(|c| (c - tm).abs() / tm > crate::order::MASS_TABLE_REL_TOL);
                push(
                    &mut issues,
                    &mut res,
                    "mass",
                    Some(&p.no),
                    if bad { "need" } else { "info" },
                    &[r.no.as_str()],
                    bad,
                );
            }
            // ZK skips a plate whose size came from the table: there is nothing to compare.
            let from_table = ["plate_W", "plate_H"]
                .iter()
                .any(|k| p.values.get(*k).is_some_and(|v| v.source == "table"));
            if computed
                && p.shape == "plate"
                && !from_table
                && let Some((w, h)) = size_dims(&r.size)
            {
                let pw = p.values.get("plate_W").and_then(|v| v.usable());
                let ph = p.values.get("plate_H").and_then(|v| v.usable());
                if let (Some(pw), Some(ph)) = (pw, ph) {
                    let bad = (pw - w).abs() > crate::order::SIZE_TABLE_TOL_MM
                        || (ph - h).abs() > crate::order::SIZE_TABLE_TOL_MM;
                    push(
                        &mut issues,
                        &mut res,
                        "size_vs_table",
                        Some(&p.no),
                        if bad { "need" } else { "info" },
                        &[r.no.as_str()],
                        bad,
                    );
                }
            }
            // ZK writes the length review for every material row with a length: `ok` when the drawing and the
            // table agree, `need` when they differ by more than 1 mm. The review exists in both cases.
            if computed
                && p.shape == "cylinder"
                && let Some(len) = number(&r.length)
                && let Some(v) = p.values.get("L").and_then(|x| x.usable())
            {
                let bad = (v - len).abs() > crate::order::SIZE_TABLE_TOL_MM;
                push(
                    &mut issues,
                    &mut res,
                    "cyl_len",
                    Some(&p.no),
                    if bad { "need" } else { "info" },
                    &[r.no.as_str()],
                    bad,
                );
            }
            if computed
                && p.shape == "miter_bend"
                && let Some(len) = number(&r.length)
                && p.values.contains_key("R")
                && p.values.contains_key("angle")
                && let Some(got) = res.part(&p.no).and_then(|x| x.review_length_mm)
            {
                let bad = (got - len).abs() > crate::order::SIZE_TABLE_TOL_MM;
                push(
                    &mut issues,
                    &mut res,
                    "bend_len",
                    Some(&p.no),
                    if bad { "need" } else { "info" },
                    &[r.no.as_str()],
                    bad,
                );
            }
        }
    }
    res.review_issues = issues;
    res
}
