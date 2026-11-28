//! PZ-3 (ZK-PORT P08 + P09): what-if preview of one edit, and the impact of a rule change, computed without saving.
//!
//! Port of zumen-kirizu `pendingPiece` / `pendHtml` / `apply` (design-demos/screen-v2.html:568-590, 878-884,
//! 1035-1067: a pending edit shows the old pattern in red and the new one in green with "outer size a x b -> c x d,
//! mass m -> n kg" before the person applies it) and of the factory-rules panel (screen-v2.html:1278-1317). NF changes
//! on purpose:
//! - ZK applied a rule change at once to the whole job and then un-confirmed the rows whose shape changed (flaw 4: a
//!   silent whole-job effect). Here a rule change is shown first: `rule_impact` names exactly the parts whose pattern
//!   or mass changes and the holds that appear or clear; applying it is a separate, audited write of the order's rules.
//! - Nothing here writes. A preview is `compute_order` twice (before, after) and a comparison; the same parts and
//!   rules give the same bytes, so a preview equals the result that the same edit computes once it is applied
//!   (`after_fingerprint` = `result_fingerprint` of that stored result).
//! - The five edit kinds collapse to two mechanisms. `value` and `split` set one value of one part the way a person
//!   applies it through the values API (CONFIRMED, source `entered`; `null` makes it MISSING). `seam_side`,
//!   `mount_note` and `rule` change keys of the order's rules: their scope is the whole order. ZK's per-part seam
//!   mirror is not wired into the order compute (lane PZ-4) and is no edit here.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value, json};

use crate::order_compute::{
    self as oc, HoldOut, OrderResult, OrderRules, PartIn, PartResult, PatternPiece, Val, ValState,
};
use crate::tee::Mount;

pub const PREVIEW_VERSION: &str = "order-preview 1 (PZ-3)";

/// Largest number of plates a split edit may ask for. The shapes refuse or hold their own limits (cylinder seams
/// 1..=6, cone 2..=12, pad 1..=12); this only stops nonsense before a compute.
pub const MAX_SPLIT: u32 = 99;

/// Largest magnitude of an edited value (the values API's limit).
pub const MAX_ABS_VALUE: f64 = 1.0e7;

// ---------------------------------------------------------------- edits

/// One proposed change, not yet applied.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    /// Set (or clear, `None`) one value of one part, as a person applies a fix: CONFIRMED, source `entered`.
    Value {
        part: String,
        key: String,
        value: Option<f64>,
    },
    /// The number of plates / seams of one part (its `split` value), the ZK split buttons.
    Split { part: String, k: u32 },
    /// Where the seam angle is measured from (rule `seam_from`; the whole order), the ZK 地/天 answer.
    SeamSide { from_bottom: bool },
    /// The branch mount a drawing note asks for (rule `mount`; the whole order), the ZK tee_mount_note answer.
    MountNote { mount: Mount },
    /// Rule keys set on the order's rules object (merged onto it, then checked strictly).
    Rules { set: Map<String, Value> },
}

/// A refused edit or preview: a stable code, the request fields it concerns and a plain message.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PreviewError {
    pub code: &'static str,
    pub fields: Vec<String>,
    pub message: String,
}

fn err(code: &'static str, field: &str, message: impl Into<String>) -> PreviewError {
    PreviewError {
        code,
        fields: if field.is_empty() {
            vec![]
        } else {
            vec![field.to_string()]
        },
        message: message.into(),
    }
}

fn plain_no(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

fn valid_key(s: &str) -> bool {
    s.len() <= 32
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn mount_name(m: Mount) -> &'static str {
    match m {
        Mount::SetOn => "set_on",
        Mount::SetIn => "set_in",
    }
}

impl Edit {
    /// The edit of an API request: `{"kind": "value", "part": no, "key": k, "value": number|null}`,
    /// `{"kind": "split", "part": no, "split": k}`, `{"kind": "seam_side", "seam_from": "bottom"|"top"}`,
    /// `{"kind": "mount_note", "mount": "set_on"|"set_in"}`, `{"kind": "rule", "rules": {..}}`. Strict: an unknown
    /// kind or field, or a value of the wrong type, is refused by name.
    pub fn from_json(v: &Value) -> Result<Edit, PreviewError> {
        let o = v
            .as_object()
            .ok_or_else(|| err("edit_type", "edit", "'edit' must be an object"))?;
        let kind = o.get("kind").and_then(Value::as_str).unwrap_or("");
        let allowed: &[&str] = match kind {
            "value" => &["kind", "part", "key", "value"],
            "split" => &["kind", "part", "split"],
            "seam_side" => &["kind", "seam_from"],
            "mount_note" => &["kind", "mount"],
            "rule" => &["kind", "rules"],
            _ => {
                return Err(err(
                    "edit_kind",
                    "kind",
                    "'kind' must be value, split, seam_side, mount_note or rule",
                ));
            }
        };
        if let Some(extra) = o.keys().find(|k| !allowed.contains(&k.as_str())) {
            return Err(err(
                "edit_field",
                extra,
                format!("'{extra}' is not a field of a {kind} edit"),
            ));
        }
        let part = || -> Result<String, PreviewError> {
            let p = o.get("part").and_then(Value::as_str).unwrap_or("");
            if plain_no(p) {
                Ok(p.to_string())
            } else {
                Err(err(
                    "edit_part",
                    "part",
                    "'part' must be the 'no' of a part of the order",
                ))
            }
        };
        Ok(match kind {
            "value" => {
                let key = o.get("key").and_then(Value::as_str).unwrap_or("");
                if !valid_key(key) {
                    return Err(err(
                        "edit_key",
                        "key",
                        "'key' must be a parameter name such as D, t or branch_x",
                    ));
                }
                let value = match o.get("value") {
                    None => {
                        return Err(err(
                            "edit_value",
                            "value",
                            "'value' is required (a number, or null for a missing value)",
                        ));
                    }
                    Some(Value::Null) => None,
                    Some(x) => Some(
                        x.as_f64()
                            .filter(|f| f.is_finite() && f.abs() <= MAX_ABS_VALUE)
                            .ok_or_else(|| {
                                err(
                                    "edit_value",
                                    "value",
                                    format!(
                                        "'value' must be a finite number within +-{MAX_ABS_VALUE}"
                                    ),
                                )
                            })?,
                    ),
                };
                Edit::Value {
                    part: part()?,
                    key: key.to_string(),
                    value,
                }
            }
            "split" => {
                let k = o
                    .get("split")
                    .and_then(Value::as_u64)
                    .filter(|k| (1..=u64::from(MAX_SPLIT)).contains(k))
                    .ok_or_else(|| {
                        err(
                            "edit_split",
                            "split",
                            format!("'split' must be a whole number from 1 to {MAX_SPLIT}"),
                        )
                    })?;
                Edit::Split {
                    part: part()?,
                    k: k as u32,
                }
            }
            "seam_side" => Edit::SeamSide {
                from_bottom: match o.get("seam_from").and_then(Value::as_str) {
                    Some("bottom") => true,
                    Some("top") => false,
                    _ => {
                        return Err(err(
                            "edit_seam_from",
                            "seam_from",
                            "'seam_from' must be 'bottom' or 'top'",
                        ));
                    }
                },
            },
            "mount_note" => Edit::MountNote {
                mount: match o.get("mount").and_then(Value::as_str) {
                    Some("set_on") => Mount::SetOn,
                    Some("set_in") => Mount::SetIn,
                    _ => {
                        return Err(err(
                            "edit_mount",
                            "mount",
                            "'mount' must be 'set_on' or 'set_in'",
                        ));
                    }
                },
            },
            _ => {
                let set = o
                    .get("rules")
                    .and_then(Value::as_object)
                    .filter(|m| !m.is_empty())
                    .ok_or_else(|| {
                        err(
                            "edit_rules",
                            "rules",
                            "'rules' must be an object with at least one rule",
                        )
                    })?;
                Edit::Rules { set: set.clone() }
            }
        })
    }

    /// The edit as the API names it (the request form, normalised).
    pub fn to_json(&self) -> Value {
        match self {
            Edit::Value { part, key, value } => {
                json!({"kind": "value", "part": part, "key": key, "value": value})
            }
            Edit::Split { part, k } => json!({"kind": "split", "part": part, "split": k}),
            Edit::SeamSide { from_bottom } => {
                json!({"kind": "seam_side", "seam_from": if *from_bottom { "bottom" } else { "top" }})
            }
            Edit::MountNote { mount } => json!({"kind": "mount_note", "mount": mount_name(*mount)}),
            Edit::Rules { set } => json!({"kind": "rule", "rules": set}),
        }
    }

    /// `part` for an edit of one part's value; `order` for a rule edit (it changes every part of the order).
    pub fn scope(&self) -> &'static str {
        match self {
            Edit::Value { .. } | Edit::Split { .. } => "part",
            _ => "order",
        }
    }

    /// The part a value edit names.
    pub fn part(&self) -> Option<&str> {
        match self {
            Edit::Value { part, .. } | Edit::Split { part, .. } => Some(part),
            _ => None,
        }
    }

    /// The value an applied value edit writes: (part, key, value).
    pub fn value_write(&self) -> Option<(&str, &str, Option<f64>)> {
        match self {
            Edit::Value { part, key, value } => Some((part, key, *value)),
            Edit::Split { part, k } => Some((part, "split", Some(f64::from(*k)))),
            _ => None,
        }
    }

    /// The rule keys a rule edit sets on the order's rules object.
    pub fn rule_patch(&self) -> Option<Map<String, Value>> {
        let mut m = Map::new();
        match self {
            Edit::SeamSide { from_bottom } => {
                m.insert(
                    "seam_from".into(),
                    json!(if *from_bottom { "bottom" } else { "top" }),
                );
            }
            Edit::MountNote { mount } => {
                m.insert("mount".into(), json!(mount_name(*mount)));
            }
            Edit::Rules { set } => m.clone_from(set),
            _ => return None,
        }
        Some(m)
    }
}

/// The order's parts and rules object after the edit. Pure. A value edit names a part of `parts` (by `no`); a rule
/// edit is merged onto `rules` (an object, or null for none) and the merged object must pass `OrderRules::from_json`.
pub fn apply_edit(
    parts: &[PartIn],
    rules: &Value,
    edit: &Edit,
) -> Result<(Vec<PartIn>, Value), PreviewError> {
    if let Some((no, key, value)) = edit.value_write() {
        let mut out = parts.to_vec();
        let p = out.iter_mut().find(|p| p.no == no).ok_or_else(|| {
            err(
                "edit_part_unknown",
                "part",
                format!("part '{no}' is not a part of this order"),
            )
        })?;
        p.values.insert(
            key.to_string(),
            Val {
                value,
                state: if value.is_some() {
                    ValState::Confirmed
                } else {
                    ValState::Missing
                },
                source: "entered".into(),
            },
        );
        return Ok((out, rules.clone()));
    }
    let patch = edit.rule_patch().unwrap_or_default();
    let mut merged = match rules {
        Value::Null => Map::new(),
        Value::Object(m) => m.clone(),
        _ => {
            return Err(err(
                "rules_invalid",
                "",
                "the order's rules are not an object",
            ));
        }
    };
    for (k, v) in patch {
        merged.insert(k, v);
    }
    let merged = Value::Object(merged);
    OrderRules::from_json(&merged).map_err(|m| err("edit_rules", "rules", m))?;
    Ok((parts.to_vec(), merged))
}

// ---------------------------------------------------------------- the comparison

/// One part before and after. `pieces_*` are the pattern pieces (outline, holes and circles as point lists, outer
/// size `w` x `h`, mass of one copy); they are empty when `outlines` was not asked for or the part has no pattern.
#[derive(Debug, Clone, Serialize)]
pub struct PartDiff {
    pub no: String,
    pub name: String,
    pub shape: String,
    pub group: String,
    /// The edit names this part.
    pub edited: bool,
    /// The pattern (pieces, outlines, holes, sizes, piece masses) differs.
    pub pattern_changed: bool,
    /// The part's mass (unit, total or gross) differs.
    pub mass_changed: bool,
    pub holds_changed: bool,
    pub state_before: &'static str,
    pub state_after: &'static str,
    pub pieces_count_before: usize,
    pub pieces_count_after: usize,
    /// Outer size of the largest piece (w, h).
    pub size_before: Option<[f64; 2]>,
    pub size_after: Option<[f64; 2]>,
    pub unit_mass_before: Option<f64>,
    pub unit_mass_after: Option<f64>,
    pub mass_before: Option<f64>,
    pub mass_after: Option<f64>,
    pub mass_provisional_before: bool,
    pub mass_provisional_after: bool,
    pub holds_added: Vec<HoldOut>,
    pub holds_removed: Vec<HoldOut>,
    pub questions_added: Vec<String>,
    pub questions_removed: Vec<String>,
    pub refusal_before: Option<String>,
    pub refusal_after: Option<String>,
    pub dxf_blocked_by_before: Vec<String>,
    pub dxf_blocked_by_after: Vec<String>,
    /// The pattern pieces with their outline point lists, before and after (empty when outlines were not asked for).
    pub outline_before: Vec<PatternPiece>,
    pub outline_after: Vec<PatternPiece>,
    pub text_en: String,
    pub text_ja: String,
}

/// A rule whose effective value differs.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RuleChange {
    pub name: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    /// Parts in the order.
    pub parts: usize,
    /// Parts whose pattern or mass changes ("N parts change").
    pub changed: Vec<String>,
    /// Parts whose result changes in any way (pattern, mass, holds, state, refusal, ...), `changed` included.
    pub affected: Vec<String>,
    /// Need holds and questions that appear / clear, over all parts.
    pub holds_added: usize,
    pub holds_removed: usize,
    pub text_en: String,
    pub text_ja: String,
}

/// What an edit (or a rule change) would do, computed without saving.
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub version: &'static str,
    /// `part` (a value edit) or `order` (a rule edit: every part of the order is recomputed).
    pub scope: &'static str,
    pub edit: Value,
    pub label_en: String,
    pub label_ja: String,
    pub summary: Summary,
    /// The edited part and every part whose result changes; parts that do not change are left out.
    pub parts: Vec<PartDiff>,
    pub rules_changed: Vec<RuleChange>,
    /// The order's rules object after the edit (what an applied rule edit stores).
    pub rules_json_after: Value,
    /// The effective rules after the edit, with source and status (`OrderRules::rules_out`).
    pub rules_after: Vec<Value>,
    pub rules_fingerprint_before: String,
    pub rules_fingerprint_after: String,
    /// `result_fingerprint` of the result before and after: an applied edit computes exactly `after_fingerprint`.
    pub before_fingerprint: String,
    pub after_fingerprint: String,
    #[serde(skip)]
    pub before: OrderResult,
    #[serde(skip)]
    pub after: OrderResult,
}

fn json_of<T: Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

fn same_hold(a: &HoldOut, b: &HoldOut) -> bool {
    a.level == b.level && a.code == b.code && a.keys == b.keys && a.scope == b.scope
}

fn largest(ps: &[PatternPiece]) -> Option<[f64; 2]> {
    ps.iter()
        .filter_map(|p| Some([p.w?, p.h?]))
        .max_by(|a, b| (a[0] * a[1]).total_cmp(&(b[0] * b[1])))
}

fn mm(v: f64) -> String {
    format!("{v:.1}")
}

fn kg(v: Option<f64>) -> String {
    match v {
        Some(x) if x.abs() >= 10.0 => format!("{x:.1}"),
        Some(x) => format!("{x:.3}"),
        None => "-".into(),
    }
}

fn state_words(s: &str) -> (&'static str, &'static str) {
    match s {
        "computed" => ("computed", "計算済み"),
        "held" => ("held", "保留"),
        "held_missing" => ("held: a value is missing", "保留（値が足りない）"),
        "refused" => ("refused", "拒否"),
        _ => ("not computed", "未計算"),
    }
}

fn size_words(s: Option<[f64; 2]>) -> (String, String) {
    match s {
        Some([w, h]) => (
            format!("outer size {} x {} mm", mm(w), mm(h)),
            format!("外形 {} × {}", mm(w), mm(h)),
        ),
        None => ("no pattern".into(), "展開図なし".into()),
    }
}

fn part_diff(b: &PartResult, a: &PartResult, edited: bool, outlines: bool) -> PartDiff {
    let pattern_changed = json_of(&b.pattern) != json_of(&a.pattern);
    let mass_changed = json_of(&(b.unit_mass_kg, b.mass_kg, b.gross_mass_kg))
        != json_of(&(a.unit_mass_kg, a.mass_kg, a.gross_mass_kg));
    let holds_added: Vec<HoldOut> = a
        .holds
        .iter()
        .filter(|h| !b.holds.iter().any(|x| same_hold(x, h)))
        .cloned()
        .collect();
    let holds_removed: Vec<HoldOut> = b
        .holds
        .iter()
        .filter(|h| !a.holds.iter().any(|x| same_hold(x, h)))
        .cloned()
        .collect();
    let questions_added: Vec<String> = a
        .questions
        .iter()
        .filter(|q| !b.questions.iter().any(|x| x.code == q.code))
        .map(|q| q.code.to_string())
        .collect();
    let questions_removed: Vec<String> = b
        .questions
        .iter()
        .filter(|q| !a.questions.iter().any(|x| x.code == q.code))
        .map(|q| q.code.to_string())
        .collect();
    let mut d = PartDiff {
        no: a.no.clone(),
        name: a.name.clone(),
        shape: a.shape.clone(),
        group: a.group.clone(),
        edited,
        pattern_changed,
        mass_changed,
        holds_changed: !holds_added.is_empty() || !holds_removed.is_empty(),
        state_before: b.state,
        state_after: a.state,
        pieces_count_before: b.pattern.len(),
        pieces_count_after: a.pattern.len(),
        size_before: largest(&b.pattern),
        size_after: largest(&a.pattern),
        unit_mass_before: b.unit_mass_kg,
        unit_mass_after: a.unit_mass_kg,
        mass_before: b.mass_kg,
        mass_after: a.mass_kg,
        mass_provisional_before: b.mass_provisional,
        mass_provisional_after: a.mass_provisional,
        holds_added,
        holds_removed,
        questions_added,
        questions_removed,
        refusal_before: b.refusal.as_ref().map(|r| r.code.clone()),
        refusal_after: a.refusal.as_ref().map(|r| r.code.clone()),
        dxf_blocked_by_before: b.dxf_blocked_by.clone(),
        dxf_blocked_by_after: a.dxf_blocked_by.clone(),
        outline_before: if outlines { b.pattern.clone() } else { vec![] },
        outline_after: if outlines { a.pattern.clone() } else { vec![] },
        text_en: String::new(),
        text_ja: String::new(),
    };
    let (en, ja) = part_text(&d);
    d.text_en = en;
    d.text_ja = ja;
    d
}

/// Plain words for one part: size, mass, state and the holds that appear or clear.
fn part_text(d: &PartDiff) -> (String, String) {
    let mut en: Vec<String> = Vec::new();
    let mut ja: Vec<String> = Vec::new();
    if d.pattern_changed {
        let (bn, bj) = size_words(d.size_before);
        let (an, aj) = size_words(d.size_after);
        if d.pieces_count_before > 1 || d.pieces_count_after > 1 {
            en.push(format!(
                "pieces {} -> {}; largest piece {bn} -> {an}",
                d.pieces_count_before, d.pieces_count_after
            ));
            ja.push(format!(
                "片の数 {} → {}・いちばん大きい片の{bj} → {aj}",
                d.pieces_count_before, d.pieces_count_after
            ));
        } else {
            en.push(format!("{bn} -> {an}"));
            ja.push(format!("{bj} → {aj}"));
        }
    }
    if d.mass_changed {
        en.push(format!(
            "mass {} -> {} kg",
            kg(d.mass_before),
            kg(d.mass_after)
        ));
        ja.push(format!(
            "重さ {} → {} kg",
            kg(d.mass_before),
            kg(d.mass_after)
        ));
    }
    if d.state_before != d.state_after {
        let (b, bj) = state_words(d.state_before);
        let (a, aj) = state_words(d.state_after);
        en.push(format!("state {b} -> {a}"));
        ja.push(format!("状態 {bj} → {aj}"));
    }
    for h in d.holds_added.iter().filter(|h| h.level == "need") {
        en.push(format!("new hold: {}", h.message_en));
        ja.push(format!("保留が増える：{}", h.message_ja));
    }
    for h in d.holds_removed.iter().filter(|h| h.level == "need") {
        en.push(format!("hold cleared: {}", h.message_en));
        ja.push(format!("保留がなくなる：{}", h.message_ja));
    }
    for q in &d.questions_added {
        en.push(format!("new question: {q}"));
        ja.push(format!("確認が増える：{q}"));
    }
    for q in &d.questions_removed {
        en.push(format!("question cleared: {q}"));
        ja.push(format!("確認がなくなる：{q}"));
    }
    if en.is_empty() {
        return (
            "No change to this part.".into(),
            "この部材は変わりません。".into(),
        );
    }
    (
        format!("{}.", en.join("; ")),
        format!("{}。", ja.join("・")),
    )
}

/// The part result without its list of named rules (that list repeats rule values and is not a result).
fn result_view(p: &PartResult) -> Value {
    let mut v = json_of(p);
    if let Some(o) = v.as_object_mut() {
        o.remove("rules");
    }
    v
}

fn rules_changed(a: &OrderRules, b: &OrderRules) -> Vec<RuleChange> {
    let ra = a.rules_out();
    let rb = b.rules_out();
    let value_of = |list: &[Value], n: &str| {
        list.iter()
            .find(|r| r["name"] == n)
            .and_then(|r| r["value"].as_str().map(str::to_string))
            .unwrap_or_default()
    };
    ra.iter()
        .filter_map(|r| r["name"].as_str())
        .filter_map(|n| {
            let (x, y) = (value_of(&ra, n), value_of(&rb, n));
            (x != y).then(|| RuleChange {
                name: n.to_string(),
                before: x,
                after: y,
            })
        })
        .collect()
}

struct Labels {
    en: String,
    ja: String,
}

#[allow(clippy::too_many_arguments)]
fn build(
    edit: Value,
    scope: &'static str,
    label: Labels,
    rules_a: &OrderRules,
    rules_b: &OrderRules,
    rules_json_after: Value,
    before: OrderResult,
    after: OrderResult,
    focus: Option<&str>,
    outlines: bool,
) -> Preview {
    let by_no: BTreeMap<&str, &PartResult> =
        before.parts.iter().map(|p| (p.no.as_str(), p)).collect();
    let mut parts = Vec::new();
    let mut changed = Vec::new();
    let mut affected = Vec::new();
    let (mut added, mut removed) = (0usize, 0usize);
    for a in &after.parts {
        let Some(b) = by_no.get(a.no.as_str()) else {
            continue;
        };
        let edited = focus == Some(a.no.as_str());
        let any = result_view(b) != result_view(a);
        if !any && !edited {
            continue;
        }
        let d = part_diff(b, a, edited, outlines);
        if d.pattern_changed || d.mass_changed {
            changed.push(d.no.clone());
        }
        if any {
            affected.push(d.no.clone());
        }
        added +=
            d.holds_added.iter().filter(|h| h.level == "need").count() + d.questions_added.len();
        removed += d.holds_removed.iter().filter(|h| h.level == "need").count()
            + d.questions_removed.len();
        parts.push(d);
    }
    let (text_en, text_ja) = summary_text(scope, changed.len(), affected.len(), added, removed);
    Preview {
        version: PREVIEW_VERSION,
        scope,
        edit,
        label_en: label.en,
        label_ja: label.ja,
        summary: Summary {
            parts: after.parts.len(),
            changed,
            affected,
            holds_added: added,
            holds_removed: removed,
            text_en,
            text_ja,
        },
        parts,
        rules_changed: rules_changed(rules_a, rules_b),
        rules_json_after,
        rules_after: rules_b.rules_out(),
        rules_fingerprint_before: rules_a.fingerprint(),
        rules_fingerprint_after: rules_b.fingerprint(),
        before_fingerprint: oc::result_fingerprint(&before),
        after_fingerprint: oc::result_fingerprint(&after),
        before,
        after,
    }
}

fn summary_text(
    scope: &str,
    changed: usize,
    affected: usize,
    added: usize,
    removed: usize,
) -> (String, String) {
    let mut en: Vec<String> = Vec::new();
    let mut ja: Vec<String> = Vec::new();
    if changed > 0 {
        en.push(format!(
            "{changed} part{} change{} shape or mass",
            if changed == 1 { "" } else { "s" },
            if changed == 1 { "s" } else { "" }
        ));
        ja.push(format!("形か重さが変わる部材 {changed}"));
    }
    if affected > changed {
        let n = affected - changed;
        en.push(format!(
            "{n} more part{} change{} holds or state only",
            if n == 1 { "" } else { "s" },
            if n == 1 { "s" } else { "" }
        ));
        ja.push(format!("保留か状態だけが変わる部材 {n}"));
    }
    if added > 0 {
        en.push(format!(
            "{added} new hold{}",
            if added == 1 { "" } else { "s" }
        ));
        ja.push(format!("保留が増える {added} 件"));
    }
    if removed > 0 {
        en.push(format!(
            "{removed} hold{} cleared",
            if removed == 1 { "" } else { "s" }
        ));
        ja.push(format!("保留がなくなる {removed} 件"));
    }
    let (mut e, mut j) = if en.is_empty() {
        (
            "Nothing changes.".to_string(),
            "何も変わりません。".to_string(),
        )
    } else {
        (
            format!("{}.", en.join(", ")),
            format!("{}。", ja.join("・")),
        )
    };
    if scope == "order" {
        e = format!("Whole order: {e}");
        j = format!("工事の全部材：{j}");
    }
    (e, j)
}

fn fmt_val(v: Option<f64>) -> String {
    v.map_or_else(|| "none".to_string(), |x| x.to_string())
}

fn edit_labels(parts: &[PartIn], rules_a: &OrderRules, edit: &Edit) -> Labels {
    let words = |s: &str| -> &'static str {
        match s {
            "set_on" => "set on",
            "set_in" => "set in",
            "bottom" => "the bottom",
            _ => "the top",
        }
    };
    let words_ja = |s: &str| -> &'static str {
        match s {
            "set_on" => "載せる",
            "set_in" => "差し込む",
            "bottom" => "地",
            _ => "天",
        }
    };
    match edit {
        Edit::Value { .. } | Edit::Split { .. } => {
            let (no, key, value) = edit.value_write().unwrap_or(("", "", None));
            let old = parts
                .iter()
                .find(|p| p.no == no)
                .and_then(|p| p.values.get(key))
                .and_then(|v| v.value);
            Labels {
                en: format!(
                    "{no} {}: {} -> {}",
                    oc::label_en(key),
                    fmt_val(old),
                    fmt_val(value)
                ),
                ja: format!(
                    "{no} {}：{} → {}",
                    oc::label_ja(key),
                    fmt_val(old),
                    fmt_val(value)
                ),
            }
        }
        Edit::SeamSide { from_bottom } => {
            let (b, a) = (
                if rules_a.seam_from_bottom {
                    "bottom"
                } else {
                    "top"
                },
                if *from_bottom { "bottom" } else { "top" },
            );
            Labels {
                en: format!(
                    "seam angle measured from {} -> from {} (every part of the order)",
                    words(b),
                    words(a)
                ),
                ja: format!(
                    "縦の継ぎ目の角度 {}から → {}から（工事の全部材）",
                    words_ja(b),
                    words_ja(a)
                ),
            }
        }
        Edit::MountNote { mount } => {
            let b = mount_name(rules_a.mount);
            let a = mount_name(*mount);
            Labels {
                en: format!(
                    "branch mount {} -> {}, as the drawing note says (every part of the order)",
                    words(b),
                    words(a)
                ),
                ja: format!(
                    "枝の付け方 {} → {}（図面の注記どおり、工事の全部材）",
                    words_ja(b),
                    words_ja(a)
                ),
            }
        }
        Edit::Rules { set } => {
            let keys: Vec<&str> = set.keys().map(String::as_str).collect();
            Labels {
                en: format!(
                    "factory rules: {} (every part of the order)",
                    keys.join(", ")
                ),
                ja: format!("工場の決まり：{}（工事の全部材）", keys.join("、")),
            }
        }
    }
}

/// What `edit` would do to the order (`parts`, `rules`), without saving. `outlines`: carry the pattern pieces of
/// every listed part (point lists) for the before/after picture. Refused: an edit of a part the order does not have,
/// a rule edit the strict rules check refuses, rules of the order that are not usable.
pub fn preview(
    parts: &[PartIn],
    rules: &Value,
    edit: &Edit,
    outlines: bool,
) -> Result<Preview, PreviewError> {
    let rules_a = OrderRules::from_json(rules).map_err(|m| {
        err(
            "rules_invalid",
            "",
            format!("the order's rules cannot be used: {m}"),
        )
    })?;
    let (parts_b, rules_json_b) = apply_edit(parts, rules, edit)?;
    let rules_b =
        OrderRules::from_json(&rules_json_b).map_err(|m| err("edit_rules", "rules", m))?;
    let label = edit_labels(parts, &rules_a, edit);
    let before = oc::compute_order(parts, &rules_a);
    let after = oc::compute_order(&parts_b, &rules_b);
    Ok(build(
        edit.to_json(),
        edit.scope(),
        label,
        &rules_a,
        &rules_b,
        rules_json_b,
        before,
        after,
        edit.part(),
        outlines,
    ))
}

/// The impact of replacing the order's rules object `rules_a` by `rules_b` (both checked strictly): the parts whose
/// pattern or mass changes, the holds that appear or clear and the rules whose value differs. Scope: the whole order.
pub fn rule_impact(
    parts: &[PartIn],
    rules_a: &Value,
    rules_b: &Value,
    outlines: bool,
) -> Result<Preview, PreviewError> {
    let ra = OrderRules::from_json(rules_a).map_err(|m| {
        err(
            "rules_invalid",
            "",
            format!("the order's rules cannot be used: {m}"),
        )
    })?;
    let rb = OrderRules::from_json(rules_b).map_err(|m| err("edit_rules", "rules", m))?;
    let names: Vec<String> = rules_changed(&ra, &rb)
        .into_iter()
        .map(|c| c.name)
        .collect();
    let label = Labels {
        en: format!(
            "factory rules: {} (every part of the order)",
            if names.is_empty() {
                "no rule value changes".to_string()
            } else {
                names.join(", ")
            }
        ),
        ja: format!(
            "工場の決まり：{}（工事の全部材）",
            if names.is_empty() {
                "値の変わる決まりはない".to_string()
            } else {
                names.join("、")
            }
        ),
    };
    let before = oc::compute_order(parts, &ra);
    let after = oc::compute_order(parts, &rb);
    Ok(build(
        json!({"kind": "rule", "rules": rules_b}),
        "order",
        label,
        &ra,
        &rb,
        rules_b.clone(),
        before,
        after,
        None,
        outlines,
    ))
}
