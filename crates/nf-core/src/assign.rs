//! AIOP-1 R1: assign numbers read from a page to the parameter slots of a unit, with the engine as the verifier.
//!
//! Pure and deterministic: the same readings (in any order) give the same answer. A number is never trusted
//! because of where it stands or what its label says; an assignment is accepted only when the engine, run on
//! the assigned values, reproduces what the material list (BOM) of the page states. When more than one
//! assignment agrees with the list, or none, the answer is a question for a person, never a guess.
//!
//! - A reading that carries a reader flag (repaired, ambiguous, phi inferred, low confidence) can only ever be
//!   PROPOSED; it is never offered as a confirmed value.
//! - A unit with no BOM facts has nothing to verify against: no assignment is made (question).
//! - A slot only accepts readings written in a form it can take (a diameter sign, an R, a plain number).
//!
//! Nothing here changes engine maths: `FamilyVerifier` runs `job::run` unchanged.

use std::collections::BTreeMap;

use crate::job::{self, Request};

/// Most candidate assignments the engine is run on for one unit; above it the answer is a question.
pub const MAX_COMBINATIONS: usize = 4000;

/// One number found on a page (the reader's item, reduced to what assignment needs).
#[derive(Debug, Clone, PartialEq)]
pub struct Reading {
    pub id: usize,
    pub text: String,
    pub value: f64,
    pub page: i64,
    pub bbox: [f32; 4],
    pub confidence: f32,
    /// Names of the reader flags that are set (`repaired`, `ambiguous`, `phi_inferred`, `low_confidence`).
    pub flags: Vec<String>,
    /// The number stands in the material list. It is kept as evidence, and when the same value is also written on
    /// the drawing the drawing's reading is the one pointed at.
    pub in_table: bool,
}

impl Reading {
    pub fn flagged(&self) -> bool {
        !self.flags.is_empty()
    }
}

/// The written form of a number, from its text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    /// digits only (a tolerance suffix and a degree sign do not change this)
    Plain,
    /// diameter sign in front
    Phi,
    /// a letter and digits: `R161`, `e26.5`, `L515.2`
    Prefix(char),
    /// a word in front (`Smax 227.09`) or anything else
    Other,
}

/// Classify the written form of a reading's text.
pub fn mark_of(text: &str) -> Mark {
    let t = text.trim();
    let mut it = t.chars();
    let Some(c) = it.next() else { return Mark::Other };
    if "φΦØø∅⌀".contains(c) {
        return Mark::Phi;
    }
    if c.is_ascii_digit() || c == '.' {
        return Mark::Plain;
    }
    if c.is_ascii_alphabetic() {
        let rest: String = it.collect();
        let next = rest.chars().next();
        if next.is_some_and(|n| n.is_ascii_digit() || n == '.') {
            return Mark::Prefix(c);
        }
    }
    Mark::Other
}

/// One parameter of a unit and the written forms it takes. `accept` pairs a form with the factor that turns the
/// printed number into the parameter (a diameter sign with 0.5 for a radius).
#[derive(Debug, Clone)]
pub struct Slot {
    pub key: String,
    pub accept: Vec<(Mark, f64)>,
    /// plausible range of the parameter after the factor (inclusive)
    pub range: (f64, f64),
}

impl Slot {
    pub fn new(key: &str, accept: &[(Mark, f64)], range: (f64, f64)) -> Slot {
        Slot {
            key: key.into(),
            accept: accept.to_vec(),
            range,
        }
    }
}

/// What the page states about one part of a unit: the blank (pattern bounding box) as printed in the list.
#[derive(Debug, Clone)]
pub struct BomRow {
    pub part: String,
    pub width: f64,
    pub height: f64,
    /// decimals the numbers were printed with (sets how closely the engine has to agree)
    pub decimals: u32,
}

/// A group of slots that one engine request reads together, with the facts of the list it must reproduce.
#[derive(Debug, Clone)]
pub struct Unit {
    pub id: String,
    pub family: String,
    pub slots: Vec<Slot>,
    /// Parameters that are not on the page and are set by rule (value, note). They are not assigned from readings.
    pub fixed: BTreeMap<String, f64>,
    /// `(key, from, factor)`: parameter `key` is `from` times `factor` (a development radius equal to the radius).
    pub derived: Vec<(String, String, f64)>,
    pub bom: Vec<BomRow>,
    /// smaller requests that narrow some slots before the full check
    pub pre: Vec<Pre>,
}

/// A smaller request of another family whose blank depends on some slots alone.
#[derive(Debug, Clone)]
pub struct Pre {
    pub family: String,
    /// (slot of the unit, slot of the smaller request)
    pub map: Vec<(String, String)>,
    /// (part of the unit's list, part of the smaller request)
    pub row: (String, String),
    /// the unit's list prints width and height the other way round
    pub swap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Match,
    Mismatch,
    Refused,
}

/// Runs the engine for one candidate and says whether it reproduces the list.
pub trait Verifier {
    fn check(&self, unit: &Unit, values: &BTreeMap<String, f64>) -> Verdict;
}

/// The legacy calculation families (`job::run`), checked against the blank size of each listed part.
pub struct FamilyVerifier;

fn outline_extent(p: &job::PatternView) -> Option<(f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    for c in p.curves.iter().filter(|c| c.layer == "CUT_OUTER") {
        for q in &c.points {
            b = Some(match b {
                None => (q[0], q[1], q[0], q[1]),
                Some((a, b2, c2, d)) => (a.min(q[0]), b2.min(q[1]), c2.max(q[0]), d.max(q[1])),
            });
        }
    }
    b.map(|(x0, y0, x1, y1)| (x1 - x0, y1 - y0))
}

impl Verifier for FamilyVerifier {
    fn check(&self, unit: &Unit, values: &BTreeMap<String, f64>) -> Verdict {
        let mut obj = serde_json::Map::new();
        obj.insert("family".into(), serde_json::json!(unit.family));
        for (k, v) in values {
            obj.insert(k.clone(), serde_json::json!(v));
        }
        // the surface a drawing's radii refer to is not on a page and a blank does not depend on it: the engine's
        // own word for 'not declared' is used, never a guess
        let needs_surface = matches!(unit.family.as_str(), "branch_stub" | "header_with_branch");
        if needs_surface && !obj.contains_key("surface_reference") {
            obj.insert("surface_reference".into(), serde_json::json!("unknown"));
        }
        let req = match Request::parse_strict(&serde_json::Value::Object(obj)) {
            Ok(r) => r,
            Err(_) => return Verdict::Refused,
        };
        let out = job::run(&req, "assign-verify");
        if out.outcome.is_some() || out.patterns.is_empty() {
            return Verdict::Refused;
        }
        for row in &unit.bom {
            let Some(p) = out.patterns.iter().find(|p| p.part == row.part) else {
                return Verdict::Refused;
            };
            let Some((w, h)) = outline_extent(p) else {
                return Verdict::Refused;
            };
            let tol = 0.5 * 10f64.powi(-(row.decimals as i32)) + 1e-6;
            if (w - row.width).abs() > tol || (h - row.height).abs() > tol {
                return Verdict::Mismatch;
            }
        }
        Verdict::Match
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// every reading used is clean and the assignment is the only one the list accepts
    Confirmed,
    /// the only assignment the list accepts uses a flagged reading: a person must confirm it
    Proposed,
}

#[derive(Debug, Clone)]
pub struct Fill {
    pub key: String,
    pub value: f64,
    pub reading: usize,
    pub text: String,
    pub page: i64,
    pub bbox: [f32; 4],
    pub confidence: f32,
    pub flags: Vec<String>,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    /// `no_bom`, `no_consistent_assignment`, `ambiguous_assignment`, `too_many_candidates`, `slot_without_candidate`
    pub code: &'static str,
    pub en: String,
    pub ja: String,
}

#[derive(Debug, Clone)]
pub struct Assignment {
    pub unit: String,
    pub fills: Vec<Fill>,
    /// parameters set by rule (not read), with their values
    pub fixed: BTreeMap<String, f64>,
    pub question: Option<Question>,
    /// how many candidate assignments the engine was run on
    pub tried: usize,
}

impl Assignment {
    pub fn is_filled(&self) -> bool {
        self.question.is_none() || !self.fills.is_empty()
    }
}

fn q(code: &'static str, en: String, ja: String) -> Question {
    Question { code, en, ja }
}

fn key6(v: f64) -> i64 {
    (v * 1e6).round() as i64
}

/// Deterministic order of readings that does not depend on how the page was scanned.
fn better(a: &Reading, b: &Reading) -> std::cmp::Ordering {
    a.flagged()
        .cmp(&b.flagged())
        .then(a.in_table.cmp(&b.in_table))
        .then(b.confidence.total_cmp(&a.confidence))
        .then(a.page.cmp(&b.page))
        .then(a.bbox[1].total_cmp(&b.bbox[1]))
        .then(a.bbox[0].total_cmp(&b.bbox[0]))
        .then(a.id.cmp(&b.id))
}

/// Candidates of one slot: one reading per distinct parameter value (the best one), in value order.
fn candidates(slot: &Slot, readings: &[Reading]) -> Vec<(f64, Reading)> {
    let mut by_value: BTreeMap<i64, (f64, Reading)> = BTreeMap::new();
    for r in readings {
        let m = mark_of(&r.text);
        let Some((_, f)) = slot.accept.iter().find(|(mk, _)| *mk == m) else {
            continue;
        };
        let v = r.value * f;
        if !(v.is_finite() && v >= slot.range.0 && v <= slot.range.1) {
            continue;
        }
        let k = key6(v);
        match by_value.get(&k) {
            Some((_, old)) if better(old, r) != std::cmp::Ordering::Greater => {}
            _ => {
                by_value.insert(k, (v, r.clone()));
            }
        }
    }
    by_value.into_values().collect()
}

/// Every assignment of one candidate per slot that the verifier accepts, in a fixed order, and how many were tried.
fn enumerate(
    unit: &Unit,
    cands: &[Vec<(f64, Reading)>],
    verifier: &dyn Verifier,
) -> (Vec<Vec<usize>>, usize) {
    let mut tried = 0usize;
    let mut idx = vec![0usize; cands.len()];
    let mut hits: Vec<Vec<usize>> = Vec::new();
    loop {
        let mut values: BTreeMap<String, f64> = unit.fixed.clone();
        for (s, (c, i)) in unit.slots.iter().zip(cands.iter().zip(&idx)) {
            values.insert(s.key.clone(), c[*i].0);
        }
        for (k, from, f) in &unit.derived {
            if let Some(v) = values.get(from).copied() {
                values.insert(k.clone(), v * f);
            }
        }
        tried += 1;
        if verifier.check(unit, &values) == Verdict::Match {
            hits.push(idx.clone());
        }
        let mut p = cands.len();
        let mut done = true;
        while p > 0 {
            p -= 1;
            idx[p] += 1;
            if idx[p] < cands[p].len() {
                done = false;
                break;
            }
            idx[p] = 0;
        }
        if done {
            break;
        }
    }
    (hits, tried)
}

/// Narrow the candidates of some slots with a smaller engine request that depends on them alone (a header's blank
/// is that of a cylinder shell of the same radius and length). Only candidates are removed here: the answer is
/// still verified by the full request, so a wrong pre-check can cost an answer but never produce a wrong one.
fn narrow(unit: &Unit, cands: &mut [Vec<(f64, Reading)>], verifier: &dyn Verifier, tried: &mut usize) {
    for pre in &unit.pre {
        let Some(mut sub) = family_unit(&unit.id, &pre.family, vec![]) else { continue };
        let Some(row) = unit.bom.iter().find(|b| b.part == pre.row.0) else { continue };
        let (width, height) = if pre.swap { (row.height, row.width) } else { (row.width, row.height) };
        sub.bom = vec![BomRow { part: pre.row.1.clone(), width, height, decimals: row.decimals }];
        sub.slots.clear();
        let mut sub_cands = Vec::new();
        let mut positions = Vec::new();
        for (mk, sk) in &pre.map {
            let Some(pos) = unit.slots.iter().position(|s| &s.key == mk) else { continue };
            let mut slot = unit.slots[pos].clone();
            slot.key = sk.clone();
            sub.slots.push(slot);
            sub_cands.push(cands[pos].clone());
            positions.push(pos);
        }
        if sub.slots.len() != pre.map.len()
            || sub_cands.iter().any(Vec::is_empty)
            || sub_cands.iter().fold(1usize, |a, c| a.saturating_mul(c.len())) > MAX_COMBINATIONS
        {
            continue;
        }
        let (hits, t) = enumerate(&sub, &sub_cands, verifier);
        *tried += t;
        if hits.is_empty() {
            continue;
        }
        for (j, pos) in positions.iter().enumerate() {
            let keep: Vec<usize> = hits.iter().map(|h| h[j]).collect();
            let kept: Vec<(f64, Reading)> = sub_cands[j]
                .iter()
                .enumerate()
                .filter(|(i, _)| keep.contains(i))
                .map(|(_, c)| c.clone())
                .collect();
            cands[*pos] = kept;
        }
    }
}

/// Assign the readings of one unit's page to its slots. See the module comment for the rules.
pub fn assign(unit: &Unit, readings: &[Reading], verifier: &dyn Verifier) -> Assignment {
    let mut out = Assignment {
        unit: unit.id.clone(),
        fills: vec![],
        fixed: unit.fixed.clone(),
        question: None,
        tried: 0,
    };
    if unit.bom.is_empty() {
        out.question = Some(q(
            "no_bom",
            format!("Unit {}: the material list gives nothing to check the numbers against.", unit.id),
            format!("ユニット {}: 数値を照合できる部品表の記載がありません。", unit.id),
        ));
        return out;
    }
    let mut cands: Vec<Vec<(f64, Reading)>> = unit.slots.iter().map(|s| candidates(s, readings)).collect();
    if let Some(i) = cands.iter().position(Vec::is_empty) {
        let k = &unit.slots[i].key;
        out.question = Some(q(
            "slot_without_candidate",
            format!("Unit {}: no number on the page can be '{k}'; please enter it.", unit.id),
            format!("ユニット {}: ページに '{k}' となり得る数値がありません。入力してください。", unit.id),
        ));
        return out;
    }
    narrow(unit, &mut cands, verifier, &mut out.tried);
    let total = cands.iter().fold(1usize, |a, c| a.saturating_mul(c.len()));
    if total > MAX_COMBINATIONS {
        out.question = Some(q(
            "too_many_candidates",
            format!(
                "Unit {}: {total} possible assignments ({}); too many to check.",
                unit.id,
                unit.slots.iter().zip(&cands).map(|(s, c)| format!("{} {}", s.key, c.len())).collect::<Vec<_>>().join(", ")
            ),
            format!("ユニット {}: 組み合わせが {total} 通りあり、確認できません。", unit.id),
        ));
        return out;
    }
    let (hits, tried) = enumerate(unit, &cands, verifier);
    out.tried += tried;
    match hits.len() {
        0 => {
            out.question = Some(q(
                "no_consistent_assignment",
                format!(
                    "Unit {}: no assignment of the numbers on the page reproduces the material list ({} tried).",
                    unit.id, out.tried
                ),
                format!(
                    "ユニット {}: ページの数値のどの割り当てでも部品表と一致しません({} 通りを確認)。",
                    unit.id, out.tried
                ),
            ));
        }
        n => {
            // a slot that every accepted assignment gives the same value is determined by the list; the others are asked
            let mut open: Vec<String> = Vec::new();
            for (si, s) in unit.slots.iter().enumerate() {
                let first = hits[0][si];
                if hits.iter().all(|h| h[si] == first) {
                    let (v, r) = &cands[si][first];
                    out.fills.push(Fill {
                        key: s.key.clone(),
                        value: *v,
                        reading: r.id,
                        text: r.text.clone(),
                        page: r.page,
                        bbox: r.bbox,
                        confidence: r.confidence,
                        flags: r.flags.clone(),
                        status: if r.flagged() { Status::Proposed } else { Status::Confirmed },
                    });
                } else {
                    open.push(s.key.clone());
                }
            }
            if n > 1 {
                let list = open.join(", ");
                out.question = Some(q(
                    "ambiguous_assignment",
                    format!(
                        "Unit {}: {n} different assignments reproduce the material list; which numbers are right for {list}?",
                        unit.id
                    ),
                    format!(
                        "ユニット {}: {n} 通りの割り当てが部品表と一致します。{list} はどの数値が正しいですか?",
                        unit.id
                    ),
                ));
            }
        }
    }
    out
}

// ---------------------------------------------------------------- slot sets of the legacy families

const RADIUS: [(Mark, f64); 2] = [(Mark::Phi, 0.5), (Mark::Prefix('R'), 1.0)];
const RANGE_R: (f64, f64) = (1.0, 5000.0);
const RANGE_L: (f64, f64) = (1.0, 20000.0);

fn plain(key: &str, range: (f64, f64)) -> Slot {
    Slot::new(key, &[(Mark::Plain, 1.0)], range)
}
fn radius(key: &str) -> Slot {
    Slot::new(key, &RADIUS, RANGE_R)
}

/// The slots, fixed values and derivations of a legacy family as a drawing prints them. `bom` is read by the
/// caller from the page's list.
pub fn family_unit(id: &str, family: &str, bom: Vec<BomRow>) -> Option<Unit> {
    let mut u = Unit {
        id: id.into(),
        family: family.into(),
        slots: vec![],
        fixed: BTreeMap::new(),
        derived: vec![],
        bom,
        pre: vec![],
    };
    match family {
        "cylinder_shell" => {
            u.slots = vec![radius("r_dev"), plain("length", RANGE_L)];
        }
        "cone_frustum" => {
            u.slots = vec![radius("r1"), radius("r2"), plain("h", RANGE_L)];
        }
        "eccentric_reducer" => {
            u.slots = vec![
                radius("r1"),
                radius("r2"),
                plain("h", RANGE_L),
                Slot::new("e", &[(Mark::Prefix('e'), 1.0)], (0.0, 5000.0)),
            ];
        }
        "branch_stub" => {
            u.slots = vec![
                radius("header_radius"),
                radius("branch_radius"),
                Slot::new("beta_deg", &[(Mark::Plain, 1.0)], (1.0, 179.0)),
                plain("s_end", RANGE_L),
            ];
            u.derived = vec![("branch_dev_radius".into(), "branch_radius".into(), 1.0)];
            u.fixed.insert("delta".into(), 0.0);
        }
        "header_with_branch" => {
            u.slots = vec![
                radius("header_radius"),
                radius("branch_radius"),
                plain("header_length", RANGE_L),
                plain("branch_station", RANGE_L),
                Slot::new("beta_deg", &[(Mark::Plain, 1.0)], (1.0, 179.0)),
                plain("s_end", RANGE_L),
            ];
            u.derived = vec![
                ("header_dev_radius".into(), "header_radius".into(), 1.0),
                ("branch_dev_radius".into(), "branch_radius".into(), 1.0),
            ];
            u.fixed.insert("delta".into(), 0.0);
            let m = |a: &str, b: &str| (a.to_string(), b.to_string());
            u.pre = vec![
                Pre { family: "cylinder_shell".into(), map: vec![m("header_radius", "r_dev"), m("header_length", "length")], row: m("header", "shell"), swap: true },
                Pre {
                    family: "branch_stub".into(),
                    map: vec![m("header_radius", "header_radius"), m("branch_radius", "branch_radius"), m("beta_deg", "beta_deg"), m("s_end", "s_end")],
                    row: m("branch", "branch"),
                    swap: false,
                },
            ];
        }
        _ => return None,
    }
    Some(u)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn marks() {
        assert_eq!(mark_of("φ301.0"), Mark::Phi);
        assert_eq!(mark_of("Ø12"), Mark::Phi);
        assert_eq!(mark_of("945.6 ±0.8"), Mark::Plain);
        assert_eq!(mark_of("106.00°"), Mark::Plain);
        assert_eq!(mark_of("R161.00"), Mark::Prefix('R'));
        assert_eq!(mark_of("L515.22"), Mark::Prefix('L'));
        assert_eq!(mark_of("Smax 227.09"), Mark::Other);
        assert_eq!(mark_of(""), Mark::Other);
    }

    /// deterministic pseudo-random numbers (no dependency)
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn range(&mut self, lo: f64, hi: f64) -> f64 {
            lo + (hi - lo) * (self.next() % 100000) as f64 / 100000.0
        }
    }

    fn rd(id: usize, text: &str) -> Reading {
        let num: String = text
            .chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        Reading {
            id,
            text: text.into(),
            value: num.parse().unwrap(),
            page: 1,
            bbox: [id as f32 * 10.0, 0.0, id as f32 * 10.0 + 8.0, 5.0],
            confidence: 0.99,
            flags: vec![],
            in_table: false,
        }
    }

    /// The BOM rows the engine states for a request, printed with 2 decimals.
    fn bom_for(req: &serde_json::Value) -> Vec<BomRow> {
        let r = Request::parse_strict(req).unwrap();
        let out = job::run(&r, "test");
        assert!(out.outcome.is_none(), "{:?}", out.outcome);
        out.patterns
            .iter()
            .map(|p| {
                let (w, h) = outline_extent(p).unwrap();
                BomRow {
                    part: p.part.clone(),
                    width: (w * 100.0).round() / 100.0,
                    height: (h * 100.0).round() / 100.0,
                    decimals: 2,
                }
            })
            .collect()
    }

    fn cyl(r_dev: f64, length: f64) -> (Unit, Vec<Reading>) {
        let bom = bom_for(&json!({"family":"cylinder_shell","r_dev":r_dev,"length":length}));
        let mut texts = vec![
            format!("{length:.1}"),
            format!("φ{:.1}", 2.0 * r_dev),
            format!("{:.2}", bom[0].width),
            format!("{:.2}", bom[0].height),
        ];
        texts.push("1/1".replace('/', "."));
        let rs = texts.iter().enumerate().map(|(i, t)| rd(i, t)).collect();
        (family_unit("shell", "cylinder_shell", bom).unwrap(), rs)
    }

    fn val(a: &Assignment, k: &str) -> f64 {
        a.fills.iter().find(|f| f.key == k).unwrap().value
    }

    #[test]
    fn cylinder_is_assigned_and_confirmed() {
        let (u, rs) = cyl(150.5, 2372.5);
        let a = assign(&u, &rs, &FamilyVerifier);
        assert!(a.question.is_none(), "{:?}", a.question);
        assert_eq!(val(&a, "r_dev"), 150.5);
        assert_eq!(val(&a, "length"), 2372.5);
        assert!(a.fills.iter().all(|f| f.status == Status::Confirmed));
    }

    #[test]
    fn distractor_numbers_do_not_change_the_answer() {
        let (u, mut rs) = cyl(150.5, 2372.5);
        // distractors in the same written form as the true numbers
        for (i, t) in ["φ410.0", "φ88.0", "515.22", "172.37", "269.84", "R40.00", "Smax 227.09", "L515.22", "2372.4"]
            .iter()
            .enumerate()
        {
            rs.push(rd(100 + i, t));
        }
        let a = assign(&u, &rs, &FamilyVerifier);
        assert!(a.question.is_none(), "{:?}", a.question);
        assert_eq!(val(&a, "r_dev"), 150.5);
        assert_eq!(val(&a, "length"), 2372.5);
    }

    #[test]
    fn no_bom_means_a_question_and_no_fill() {
        let (mut u, rs) = cyl(150.5, 2372.5);
        u.bom.clear();
        let a = assign(&u, &rs, &FamilyVerifier);
        assert!(a.fills.is_empty());
        assert_eq!(a.question.unwrap().code, "no_bom");
    }

    #[test]
    fn flagged_true_reading_is_only_proposed() {
        let (u, mut rs) = cyl(150.5, 2372.5);
        rs[1].flags.push("phi_inferred".into());
        let a = assign(&u, &rs, &FamilyVerifier);
        assert!(a.question.is_none());
        let f = a.fills.iter().find(|f| f.key == "r_dev").unwrap();
        assert_eq!(f.status, Status::Proposed);
        assert_eq!(a.fills.iter().find(|f| f.key == "length").unwrap().status, Status::Confirmed);
    }

    #[test]
    fn clean_duplicate_of_a_flagged_value_is_confirmed_from_the_clean_one() {
        let (u, mut rs) = cyl(150.5, 2372.5);
        rs[1].flags.push("low_confidence".into());
        rs.push(rd(50, "φ301.0"));
        let a = assign(&u, &rs, &FamilyVerifier);
        let f = a.fills.iter().find(|f| f.key == "r_dev").unwrap();
        assert_eq!((f.status, f.reading), (Status::Confirmed, 50));
    }

    #[test]
    fn wrong_list_gives_no_consistent_assignment() {
        let (mut u, rs) = cyl(150.5, 2372.5);
        u.bom[0].width += 5.0;
        let a = assign(&u, &rs, &FamilyVerifier);
        assert!(a.fills.is_empty());
        assert_eq!(a.question.unwrap().code, "no_consistent_assignment");
    }

    #[test]
    fn two_assignments_that_both_fit_are_a_question() {
        // a stand-in verifier that accepts a product: 6 = 2 x 3 = 3 x 2
        struct Prod;
        impl Verifier for Prod {
            fn check(&self, _u: &Unit, v: &BTreeMap<String, f64>) -> Verdict {
                if (v["a"] * v["b"] - 6.0).abs() < 1e-9 { Verdict::Match } else { Verdict::Mismatch }
            }
        }
        let u = Unit {
            id: "x".into(),
            family: "x".into(),
            slots: vec![plain("a", (1.0, 10.0)), plain("b", (1.0, 10.0))],
            fixed: BTreeMap::new(),
            derived: vec![],
            bom: vec![BomRow { part: "p".into(), width: 1.0, height: 1.0, decimals: 0 }],
            pre: vec![],
        };
        let rs: Vec<Reading> = ["2", "3", "7"].iter().enumerate().map(|(i, t)| rd(i, t)).collect();
        let a = assign(&u, &rs, &Prod);
        assert!(a.fills.is_empty());
        assert_eq!(a.question.unwrap().code, "ambiguous_assignment");
        // the same unit with a symmetric duplicate removed is unique: 2 x 3 only if b is restricted
        let rs2: Vec<Reading> = ["6", "1", "7"].iter().enumerate().map(|(i, t)| rd(i, t)).collect();
        let a2 = assign(&u, &rs2, &Prod);
        assert_eq!(a2.question.unwrap().code, "ambiguous_assignment"); // 6x1 and 1x6
    }

    #[test]
    fn too_many_candidates_is_a_question() {
        let (mut u, _) = cyl(150.5, 2372.5);
        u.slots = vec![plain("a", (0.0, 1e9)), plain("b", (0.0, 1e9)), plain("c", (0.0, 1e9))];
        let rs: Vec<Reading> = (0..40).map(|i| rd(i, &format!("{}", 10 + i))).collect();
        let a = assign(&u, &rs, &FamilyVerifier);
        assert_eq!(a.question.unwrap().code, "too_many_candidates");
    }

    #[test]
    fn cone_and_eccentric_and_branch_families() {
        let cases = [
            json!({"family":"cone_frustum","r1":277.5,"r2":82.0,"h":186.0}),
            json!({"family":"eccentric_reducer","r1":305.0,"r2":205.0,"h":579.5,"e":26.5}),
            json!({"family":"header_with_branch","beta_deg":80.0,"branch_dev_radius":161.0,"branch_radius":161.0,"branch_station":956.0,"delta":0.0,"header_dev_radius":500.0,"header_length":1996.0,"header_radius":500.0,"s_end":1007.0,"surface_reference":"unknown"}),
            json!({"family":"branch_stub","header_radius":189.0,"branch_radius":79.0,"branch_dev_radius":79.0,"beta_deg":71.0,"delta":0.0,"s_end":679.0,"surface_reference":"unknown"}),
        ];
        for req in cases {
            let fam = req["family"].as_str().unwrap().to_string();
            let bom = bom_for(&req);
            let mut texts: Vec<String> = vec![];
            for b in &bom {
                texts.push(format!("{:.2}", b.width));
                texts.push(format!("{:.2}", b.height));
            }
            let u = family_unit("u", &fam, bom).unwrap();
            for s in &u.slots {
                let v = req[s.key.as_str()].as_f64().unwrap();
                texts.push(match s.key.as_str() {
                    "r1" | "r2" | "header_radius" | "branch_radius" => format!("φ{:.1}", 2.0 * v),
                    "e" => format!("e{v}"),
                    _ => format!("{v}"),
                });
            }
            texts.extend(["269.84", "172.37", "Smax 227.09", "L515.22"].map(String::from));
            let rs: Vec<Reading> = texts.iter().enumerate().map(|(i, t)| rd(i, t)).collect();
            let a = assign(&u, &rs, &FamilyVerifier);
            // the blank of a header with branch does not depend on where the branch stands: that number is asked
            let asked = if fam == "header_with_branch" { vec!["branch_station"] } else { vec![] };
            assert_eq!(a.question.as_ref().map(|q| q.code), asked.first().map(|_| "ambiguous_assignment"), "{fam}: {:?}", a.question);
            for s in &u.slots {
                if asked.contains(&s.key.as_str()) {
                    assert!(a.fills.iter().all(|f| f.key != s.key), "{fam} {} must not be filled", s.key);
                } else {
                    assert!((val(&a, &s.key) - req[s.key.as_str()].as_f64().unwrap()).abs() < 1e-9, "{fam} {}", s.key);
                }
            }
        }
    }

    /// Property: over random cylinder and cone units with random distractors, a confirmed fill is never wrong,
    /// and the answer does not depend on the order of the readings.
    #[test]
    fn random_units_never_confirm_a_wrong_value_and_ignore_order() {
        let mut g = Lcg(7);
        let (mut confirmed, mut questions) = (0, 0);
        for n in 0..40 {
            let r = (g.range(20.0, 600.0) * 2.0).round() / 2.0;
            let l = (g.range(100.0, 3000.0) * 2.0).round() / 2.0;
            let (u, mut rs) = cyl(r, l);
            for i in 0..8 {
                let d = (g.range(5.0, 3000.0) * 100.0).round() / 100.0;
                let t = match g.next() % 3 {
                    0 => format!("{d:.2}"),
                    1 => format!("φ{d:.1}"),
                    _ => format!("R{d:.1}"),
                };
                let mut x = rd(100 + i, &t);
                if g.next() % 4 == 0 {
                    x.flags.push("ambiguous".into());
                }
                rs.push(x);
            }
            let a = assign(&u, &rs, &FamilyVerifier);
            for f in &a.fills {
                let want = if f.key == "r_dev" { r } else { l };
                if f.status == Status::Confirmed {
                    assert!((f.value - want).abs() < 1e-9, "case {n}: wrong confirmed {} {}", f.key, f.value);
                    confirmed += 1;
                }
            }
            if a.question.is_some() {
                questions += 1;
            }
            let mut rev = rs.clone();
            rev.reverse();
            let b = assign(&u, &rev, &FamilyVerifier);
            assert_eq!(
                a.fills.iter().map(|f| (f.key.clone(), f.value.to_bits(), f.reading, f.status)).collect::<Vec<_>>(),
                b.fills.iter().map(|f| (f.key.clone(), f.value.to_bits(), f.reading, f.status)).collect::<Vec<_>>(),
                "case {n}: order changed the answer"
            );
            assert_eq!(a.question.map(|q| q.code), b.question.map(|q| q.code));
        }
        assert!(confirmed > 40, "confirmed {confirmed}, questions {questions}");
    }
}
