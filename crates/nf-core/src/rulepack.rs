//! PAR-2 factory rule packs: a versioned, importable set of every named factory rule the engine uses or will use.
//!
//! Each rule has a value, a unit, a source (EN/JA) and a status: `UNVERIFIED` until a person confirms it, then
//! `CONFIRMED` with the person's name and date. Pure: no I/O, no clock (the server passes the time in).
//!
//! Law (PARITY-PLAN, DECISIONS): an AI may propose a value through a question, never confirm one. Import never keeps a
//! confirmation (a file cannot confirm itself); confirmations come only from the onboarding interview. A rule with no
//! value (`null`) is not guessed: the engine keeps its provisional default for rules it reads, and a rule it does not
//! read yet is only a question.
//!
//! Rules the engine reads (`read_by_engine`) keep their values in `OrderRules` (see `order_compute`). A pack whose
//! values equal the defaults produces no change in the engine (`order_overlay` is empty).
use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use crate::order_compute::OrderRules;

pub const PACK_SCHEMA: &str = "nf.rulepack.v1";
pub const UNVERIFIED: &str = "UNVERIFIED";
pub const CONFIRMED: &str = "CONFIRMED";
const MAX_NAME_CHARS: usize = 120;
const MAX_VERSION_CHARS: usize = 32;

/// One named rule of the registry. `default` is JSON text (parsed on use) so that numbers keep the spelling the engine
/// reads. `choices` is empty for numbers and lists.
pub struct Entry {
    pub key: &'static str,
    pub unit: &'static str,
    pub choices: &'static [&'static str],
    pub default: &'static str,
    pub read_by_engine: bool,
    pub source_en: &'static str,
    pub source_ja: &'static str,
    pub question_en: &'static str,
    pub question_ja: &'static str,
    pub picture: &'static str,
}

const NOT_SET_EN: &str = "no value yet; the engine does not read this rule";
const NOT_SET_JA: &str = "まだ値なし。エンジンはこの決まりをまだ読まない";

/// Every named rule. Order is the order of the interview. Keys the engine reads match `OrderRules::from_json` keys.
pub const REGISTRY: &[Entry] = &[
    Entry {
        key: "basis",
        unit: "",
        choices: &["outer", "mid", "inner"],
        default: "\"mid\"",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (developed at the plate mid surface); no factory has confirmed it",
        source_ja: "zumen-kirizu の仮の既定（板厚の中心で展開）。工場の確認なし",
        question_en: "Are your sizes measured on the inside (内寸), the outside (外寸), or the middle of the plate (中立)?",
        question_ja: "寸法は内寸、外寸、それとも板厚の中立（中心）で測っていますか？",
        picture: "pic_basis",
    },
    Entry {
        key: "allow",
        unit: "mm",
        choices: &[],
        default: "4.0",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (4 mm per bend piece); no public source",
        source_ja: "zumen-kirizu の仮の既定（えび曲げの片ごとに 4 mm）。公の出典なし",
        question_en: "How much length do you add per bend piece (mm)?",
        question_ja: "曲げの一片ごとに何 mm 足しますか？",
        picture: "pic_bend_allow",
    },
    Entry {
        key: "seam_from",
        unit: "",
        choices: &["bottom", "top"],
        default: "\"bottom\"",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (seam measured from the bottom); not confirmed",
        source_ja: "zumen-kirizu の仮の既定（継ぎ目は下から測る）。未確認",
        question_en: "Do you measure the seam angle from the bottom (heel) or from the top?",
        question_ja: "継ぎ目の角度は、下（地）から測りますか、上から測りますか？",
        picture: "pic_seam_from",
    },
    Entry {
        key: "seam_deg",
        unit: "deg",
        choices: &[],
        default: "45.0",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (seam 45 deg); not confirmed",
        source_ja: "zumen-kirizu の仮の既定（継ぎ目 45°）。未確認",
        question_en: "At what angle do you put the vertical seam?",
        question_ja: "縦の継ぎ目は何度の位置に入れますか？",
        picture: "pic_seam_angle",
    },
    Entry {
        key: "mount",
        unit: "",
        choices: &["set_on", "set_in"],
        default: "\"set_on\"",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (the branch is set on the main); not checked against a factory cut sheet",
        source_ja: "zumen-kirizu の仮の既定（枝を本管に載せる）。工場の切図で未確認",
        question_en: "On a branch joint, does the branch sit on the outside of the main, or go into it?",
        question_ja: "枝管の取り付けは、本管の外に載せますか、中に差し込みますか？",
        picture: "pic_mount",
    },
    Entry {
        key: "stock",
        unit: "mm",
        choices: &[],
        default: "[[2438.0,6096.0]]",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (one stock plate 2438 x 6096); a temporary value",
        source_ja: "zumen-kirizu の仮の既定（板 2438×6096）。仮の値",
        question_en: "Which plate sizes do you buy (width x length, mm)?",
        question_ja: "買う板の寸法は（幅 x 長さ、mm）？",
        picture: "pic_stock",
    },
    Entry {
        key: "stock_margin",
        unit: "mm",
        choices: &[],
        default: "0.0",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (0 mm free at the plate edge); a temporary value",
        source_ja: "zumen-kirizu の仮の既定（板の縁の空き 0 mm）。仮の値",
        question_en: "How much must stay free at the plate edge (mm)?",
        question_ja: "板の縁に残す空き（mm）は？",
        picture: "pic_margin",
    },
    Entry {
        key: "seam_gap",
        unit: "mm",
        choices: &[],
        default: "null",
        read_by_engine: true,
        source_en: "engine default: 5 x plate thickness between seam and hole; not confirmed",
        source_ja: "エンジンの既定：継ぎ目と穴の間は板厚の 5 倍。未確認",
        question_en: "How far must a seam be from a hole (mm)? Leave empty to use 5 times the plate thickness.",
        question_ja: "継ぎ目と穴の間は何 mm 空けますか？空欄なら板厚の 5 倍。",
        picture: "pic_seam_gap",
    },
    Entry {
        key: "pad_cut",
        unit: "",
        choices: &["cross", "along"],
        default: "\"cross\"",
        read_by_engine: true,
        source_en: "zumen-kirizu provisional default (a reinforcing pad is split across the main axis); not confirmed",
        source_ja: "zumen-kirizu の仮の既定（補強板は本管の軸に直角に二つ割）。未確認",
        question_en: "How do you split a reinforcing pad in two: across the main axis, or along it?",
        question_ja: "補強板の二つ割は、本管の軸に直角ですか、軸に沿ってですか？",
        picture: "pic_pad_cut",
    },
    Entry {
        key: "cone_thickness",
        unit: "",
        choices: &["normal", "radial"],
        default: "\"normal\"",
        read_by_engine: true,
        source_en: "NF decision of 28 Nov 2026 (plate thickness normal to the cone wall); the radial reading is the named alternative",
        source_ja: "NF の決定（2026-11-28、板厚は円すいの面に直角）。半径方向は別の決まり",
        question_en: "On a cone branch, is the plate thickness measured normal to the cone wall, or radially?",
        question_ja: "円すいの枝管で、板厚は円すいの面に直角に測りますか、半径方向ですか？",
        picture: "pic_cone_thickness",
    },
    Entry {
        key: "swing_radius",
        unit: "mm",
        choices: &[],
        default: "null",
        read_by_engine: false,
        source_en: NOT_SET_EN,
        source_ja: NOT_SET_JA,
        question_en: "What swing radius does your cut curve use (mm)?",
        question_ja: "切り込みの曲線の振り半径は何 mm ですか？",
        picture: "pic_swing_radius",
    },
    Entry {
        key: "tee_cut_surface",
        unit: "",
        choices: &["highest_contact", "stub_inner"],
        default: "null",
        read_by_engine: false,
        source_en: NOT_SET_EN,
        source_ja: NOT_SET_JA,
        question_en: "For a tee, is the set-on cut taken at the highest contact across the wall, or at the inner surface of the stub?",
        question_ja: "チーズ（T 字）の載せ切りは、壁の最も高い接点か、枝の内面か？",
        picture: "pic_tee_cut",
    },
    Entry {
        key: "wye_joint",
        unit: "",
        choices: &["common_line", "separate"],
        default: "null",
        read_by_engine: false,
        source_en: NOT_SET_EN,
        source_ja: NOT_SET_JA,
        question_en: "For a wye (Y) joint, do the two branches share one cut line, or are they cut separately?",
        question_ja: "Y 字継手は、二本の枝が一本の線を共有しますか、別々に切りますか？",
        picture: "pic_wye",
    },
    Entry {
        key: "density",
        unit: "kg/mm3",
        choices: &[],
        default: "7.85e-6",
        read_by_engine: false,
        source_en: "steel density 7.85e-6 kg/mm3 (common value); not read from the pack yet",
        source_ja: "鋼の密度 7.85e-6 kg/mm3（一般の値）。パックからはまだ読まない",
        question_en: "Which steel density do you use for weights (kg/mm3)?",
        question_ja: "重さの計算に使う鋼の密度は（kg/mm3）？",
        picture: "pic_density",
    },
    Entry {
        key: "kerf",
        unit: "mm",
        choices: &[],
        default: "null",
        read_by_engine: false,
        source_en: NOT_SET_EN,
        source_ja: NOT_SET_JA,
        question_en: "How wide is the cut (kerf) of your machine (mm)?",
        question_ja: "お使いの機械の切り代（カーフ）は何 mm ですか？",
        picture: "pic_kerf",
    },
    Entry {
        key: "part_gap",
        unit: "mm",
        choices: &[],
        default: "null",
        read_by_engine: false,
        source_en: NOT_SET_EN,
        source_ja: NOT_SET_JA,
        question_en: "How far apart must parts be placed on the sheet (mm)?",
        question_ja: "板の上で部品同士は何 mm 離しますか？",
        picture: "pic_part_gap",
    },
    Entry {
        key: "tolerance_mm",
        unit: "mm",
        choices: &[],
        default: "null",
        read_by_engine: false,
        source_en: NOT_SET_EN,
        source_ja: NOT_SET_JA,
        question_en: "What tolerance do you accept on finished dimensions (mm)?",
        question_ja: "仕上がり寸法の許容差は何 mm ですか？",
        picture: "pic_tolerance",
    },
    Entry {
        key: "k_factor",
        unit: "",
        choices: &[],
        default: "null",
        read_by_engine: false,
        source_en: NOT_SET_EN,
        source_ja: NOT_SET_JA,
        question_en: "Which K-factor does your press brake use? Leave empty if you do not know.",
        question_ja: "お使いのプレスブレーキの K 係数は？分からなければ空欄。",
        picture: "pic_k_factor",
    },
];

pub fn entry(key: &str) -> Option<&'static Entry> {
    REGISTRY.iter().find(|e| e.key == key)
}

fn default_value(e: &Entry) -> Value {
    serde_json::from_str(e.default).expect("registry default is JSON")
}

/// A rule's state in a pack: its value (`null` = not set) and who confirmed it, when.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleState {
    pub value: Value,
    pub source_en: String,
    pub source_ja: String,
    /// `(name, YYYY-MM-DD)` of the person who confirmed it; `None` while UNVERIFIED.
    pub confirmed: Option<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub name: String,
    pub version: String,
    pub rules: BTreeMap<String, RuleState>,
}

/// Outcome of an import: the pack, and the rules whose confirmation in the file was dropped.
#[derive(Debug)]
pub struct Imported {
    pub pack: Pack,
    pub downgraded: Vec<String>,
}

impl Pack {
    /// The built-in pack: every registry rule at its provisional default, all UNVERIFIED.
    pub fn defaults(name: &str, version: &str) -> Pack {
        let rules = REGISTRY
            .iter()
            .map(|e| {
                (
                    e.key.to_string(),
                    RuleState {
                        value: default_value(e),
                        source_en: e.source_en.to_string(),
                        source_ja: e.source_ja.to_string(),
                        confirmed: None,
                    },
                )
            })
            .collect();
        Pack {
            name: name.to_string(),
            version: version.to_string(),
            rules,
        }
    }

    /// Strict import. Unknown rules and bad values are errors naming the rule. Missing registry rules are filled with
    /// their defaults (UNVERIFIED). A `CONFIRMED` status in the file is dropped and listed in `downgraded`.
    pub fn from_json(v: &Value) -> Result<Imported, String> {
        parse(v, false)
    }

    /// The stored form (`to_json` of a pack the server kept): the same checks as `from_json`, but confirmations stay.
    pub fn from_stored(v: &Value) -> Result<Pack, String> {
        parse(v, true).map(|i| i.pack)
    }

    /// Canonical JSON: every registry rule, with value, unit, source and status. `from_json(to_json())` gives the same
    /// values and sources; confirmations come back as UNVERIFIED (see `Imported::downgraded`).
    pub fn to_json(&self) -> Value {
        let mut rules = Map::new();
        for e in REGISTRY {
            let st = &self.rules[e.key];
            rules.insert(e.key.to_string(), rule_json(e, st));
        }
        json!({
            "schema": PACK_SCHEMA,
            "name": self.name,
            "version": self.version,
            "rules": rules,
        })
    }

    /// SHA-256 of the canonical JSON (identity of this exact pack state).
    pub fn sha256(&self) -> String {
        crate::sha256_hex(self.to_json().to_string().as_bytes())
    }

    /// A rule as JSON, the form `rule_set` takes back (used to store the state before a confirmation, for undo).
    pub fn rule_state_json(&self, key: &str) -> Option<Value> {
        let e = entry(key)?;
        Some(rule_json(e, self.rules.get(key)?))
    }

    /// Put a rule back from `rule_state_json` (undo of a confirmation).
    pub fn rule_set(&mut self, key: &str, v: &Value) -> Result<(), String> {
        let e = entry(key).ok_or_else(|| format!("unknown rule '{key}'"))?;
        let value = v.get("value").cloned().unwrap_or(Value::Null);
        validate_value(e.key, &value)?;
        let st = self.rules.get_mut(key).expect("registry key");
        st.value = value;
        st.confirmed = match v.get("status").and_then(Value::as_str) {
            Some(CONFIRMED) => Some((
                v.get("confirmed_by")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                v.get("confirmed_on")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            )),
            _ => None,
        };
        Ok(())
    }

    /// A person's confirmation: the value (given, or the current one when `None`), status CONFIRMED, name and date.
    /// Only the interview calls this; the caller checks who the person is. A rule with no value (`null`) cannot be
    /// confirmed: accepting nothing is not a confirmation, the person must give a value.
    pub fn confirm(
        &mut self,
        key: &str,
        value: Option<Value>,
        by: &str,
        on: &str,
    ) -> Result<(), String> {
        let e = entry(key).ok_or_else(|| format!("unknown rule '{key}'"))?;
        let st = self.rules.get_mut(key).expect("registry key");
        let next = value.unwrap_or_else(|| st.value.clone());
        if next.is_null() {
            return Err(format!(
                "rule '{key}' has no value yet; give a value before confirming it"
            ));
        }
        validate_value(e.key, &next)?;
        st.value = next;
        st.confirmed = Some((by.to_string(), on.to_string()));
        Ok(())
    }

    /// Engine-read rules the engine must not use silently: each is `null`, or not CONFIRMED (UNVERIFIED or skipped).
    /// The order's parts hold while this list is not empty. Sorted by registry order.
    pub fn unconfirmed_engine_rules(&self) -> Vec<String> {
        REGISTRY
            .iter()
            .filter(|e| e.read_by_engine)
            .filter(|e| {
                let st = &self.rules[e.key];
                st.value.is_null() || st.confirmed.is_none()
            })
            .map(|e| e.key.to_string())
            .collect()
    }

    /// Identity of this pack state for a decision: pack name, version, canonical hash and each rule's status.
    pub fn identity(&self) -> String {
        let status: Vec<String> = REGISTRY
            .iter()
            .map(|e| format!("{}={}", e.key, status_of(&self.rules[e.key])))
            .collect();
        format!(
            "pack[{}|{}|{}|{}]",
            self.name,
            self.version,
            self.sha256(),
            status.join(",")
        )
    }

    /// The engine's view: `OrderRules` keys whose pack value differs from the engine default. Empty for a pack of the
    /// defaults, so the engine's output does not change (compat). Not-set (`null`) rules are not sent.
    pub fn order_overlay(&self) -> Value {
        let mut o = Map::new();
        for e in REGISTRY.iter().filter(|e| e.read_by_engine) {
            let v = &self.rules[e.key].value;
            if !v.is_null() && *v != default_value(e) {
                o.insert(e.key.to_string(), v.clone());
            }
        }
        Value::Object(o)
    }

    /// The interview (EN/JA plain questions) with each rule's current state, in registry order.
    pub fn interview(&self) -> Value {
        Value::Array(
            REGISTRY
                .iter()
                .map(|e| {
                    let st = &self.rules[e.key];
                    json!({
                        "key": e.key,
                        "question_en": e.question_en,
                        "question_ja": e.question_ja,
                        "picture": e.picture,
                        "unit": e.unit,
                        "choices": e.choices,
                        "default": default_value(e),
                        "value": st.value,
                        "status": status_of(st),
                        "read_by_engine": e.read_by_engine,
                    })
                })
                .collect(),
        )
    }

    /// Whether every registry rule is confirmed with a real value (a confirmed `null` does not count).
    pub fn complete(&self) -> bool {
        REGISTRY.iter().all(|e| {
            let st = &self.rules[e.key];
            st.confirmed.is_some() && !st.value.is_null()
        })
    }
}

/// Check a value for a rule. Engine rules go through `OrderRules::from_json`, so the engine and the pack agree. `null`
/// is "not set" and always allowed. Unread rules check the kind only.
pub fn validate_value(key: &str, v: &Value) -> Result<(), String> {
    let e = entry(key).ok_or_else(|| format!("unknown rule '{key}'"))?;
    if v.is_null() {
        return Ok(());
    }
    if e.read_by_engine {
        let mut m = Map::new();
        m.insert(key.to_string(), v.clone());
        return OrderRules::from_json(&Value::Object(m))
            .map(|_| ())
            .map_err(|m| format!("{key}: {m}"));
    }
    if !e.choices.is_empty() {
        return match v.as_str() {
            Some(s) if e.choices.contains(&s) => Ok(()),
            _ => Err(format!(
                "rule '{key}' must be one of {}",
                e.choices.join(", ")
            )),
        };
    }
    match v.as_f64() {
        Some(n) if n.is_finite() && n >= 0.0 => Ok(()),
        _ => Err(format!("rule '{key}' must be a finite number of 0 or more")),
    }
}

/// Shared import: `keep` = the stored form, where a CONFIRMED status keeps its name and date.
fn parse(v: &Value, keep: bool) -> Result<Imported, String> {
    let o = v.as_object().ok_or("the pack must be an object")?;
    if o.get("schema").and_then(Value::as_str) != Some(PACK_SCHEMA) {
        return Err(format!("'schema' must be '{PACK_SCHEMA}'"));
    }
    let name = text(o, "name", MAX_NAME_CHARS)?;
    let version = text(o, "version", MAX_VERSION_CHARS)?;
    let given = o
        .get("rules")
        .and_then(Value::as_object)
        .ok_or("'rules' must be an object")?;
    let mut pack = Pack::defaults(&name, &version);
    let mut downgraded = Vec::new();
    for (key, item) in given {
        let e = entry(key).ok_or_else(|| format!("unknown rule '{key}'"))?;
        let item = item
            .as_object()
            .ok_or_else(|| format!("rule '{key}' must be an object"))?;
        let value = item.get("value").cloned().unwrap_or(Value::Null);
        validate_value(e.key, &value)?;
        let st = pack.rules.get_mut(e.key).expect("registry key");
        st.value = value;
        if let Some(s) = item.get("source").and_then(Value::as_str) {
            st.source_en = clip(s, 400);
        }
        if let Some(s) = item.get("source_ja").and_then(Value::as_str) {
            st.source_ja = clip(s, 400);
        }
        if item.get("status").and_then(Value::as_str) == Some(CONFIRMED) {
            if keep {
                let by = item
                    .get("confirmed_by")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let on = item
                    .get("confirmed_on")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if by.is_empty() || on.is_empty() {
                    return Err(format!("rule '{key}' is CONFIRMED without a name and date"));
                }
                st.confirmed = Some((by.to_string(), on.to_string()));
            } else {
                downgraded.push(key.clone());
            }
        }
    }
    Ok(Imported { pack, downgraded })
}

fn status_of(st: &RuleState) -> &'static str {
    if st.confirmed.is_some() {
        CONFIRMED
    } else {
        UNVERIFIED
    }
}

fn rule_json(e: &Entry, st: &RuleState) -> Value {
    let mut o = json!({
        "value": st.value,
        "unit": e.unit,
        "source": st.source_en,
        "source_ja": st.source_ja,
        "status": status_of(st),
        "read_by_engine": e.read_by_engine,
    });
    if let Some((by, on)) = &st.confirmed {
        o["confirmed_by"] = json!(by);
        o["confirmed_on"] = json!(on);
    }
    o
}

fn text(o: &Map<String, Value>, key: &str, max: usize) -> Result<String, String> {
    let s = o
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("'{key}' is required"))?;
    if s.chars().count() > max {
        return Err(format!("'{key}' is longer than {max} characters"));
    }
    Ok(s.to_string())
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// Civil date (UTC) of a Unix time in seconds, as YYYY-MM-DD. Days since 1970-01-01, proleptic Gregorian.
pub fn ymd(epoch_secs: i64) -> String {
    let z = epoch_secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_engine_so_the_overlay_is_empty() {
        let p = Pack::defaults("d", "1");
        assert!(p.order_overlay().as_object().unwrap().is_empty());
        let r = OrderRules::from_json(&p.order_overlay()).unwrap();
        assert_eq!(r.fingerprint(), OrderRules::default().fingerprint());
    }

    #[test]
    fn export_import_round_trip_keeps_values_and_sources() {
        let mut p = Pack::defaults("Shop A", "2");
        p.rules.get_mut("allow").unwrap().value = json!(5.0);
        p.rules.get_mut("seam_gap").unwrap().value = json!(12.5);
        let imp = Pack::from_json(&p.to_json()).unwrap();
        assert!(imp.downgraded.is_empty());
        assert_eq!(imp.pack, p);
        assert_eq!(imp.pack.sha256(), p.sha256());
    }

    #[test]
    fn import_drops_a_confirmation_in_the_file() {
        let mut p = Pack::defaults("Shop A", "2");
        p.confirm("basis", None, "Vishnu S.", "2026-11-28").unwrap();
        let imp = Pack::from_json(&p.to_json()).unwrap();
        assert_eq!(imp.downgraded, vec!["basis".to_string()]);
        assert!(imp.pack.rules["basis"].confirmed.is_none());
    }

    #[test]
    fn import_refuses_unknown_rules_and_bad_values() {
        let mut v = Pack::defaults("a", "1").to_json();
        v["rules"]["bogus"] = json!({"value": 1});
        assert!(
            Pack::from_json(&v)
                .unwrap_err()
                .contains("unknown rule 'bogus'")
        );
        let mut v = Pack::defaults("a", "1").to_json();
        v["rules"]["basis"]["value"] = json!("middle");
        assert!(Pack::from_json(&v).unwrap_err().contains("basis"));
        let mut v = Pack::defaults("a", "1").to_json();
        v["rules"]["kerf"]["value"] = json!(-1.0);
        assert!(Pack::from_json(&v).unwrap_err().contains("kerf"));
        let mut v = Pack::defaults("a", "1").to_json();
        v["schema"] = json!("other");
        assert!(Pack::from_json(&v).is_err());
    }

    #[test]
    fn overlay_carries_only_changed_engine_rules() {
        let mut p = Pack::defaults("a", "1");
        p.rules.get_mut("seam_from").unwrap().value = json!("top");
        p.rules.get_mut("kerf").unwrap().value = json!(0.2);
        let o = p.order_overlay();
        assert_eq!(o, json!({"seam_from": "top"}));
        let r = OrderRules::from_json(&o).unwrap();
        assert!(!r.seam_from_bottom);
    }

    #[test]
    fn confirm_then_restore_keeps_the_previous_state() {
        let mut p = Pack::defaults("a", "1");
        let before = p.rule_state_json("allow").unwrap();
        p.confirm("allow", Some(json!(4.5)), "Vishnu S.", "2026-11-28")
            .unwrap();
        assert_eq!(p.rules["allow"].value, json!(4.5));
        assert_eq!(p.rules["allow"].confirmed.as_ref().unwrap().0, "Vishnu S.");
        p.rule_set("allow", &before).unwrap();
        assert_eq!(p.rules["allow"].value, json!(4.0));
        assert!(p.rules["allow"].confirmed.is_none());
    }

    #[test]
    fn interview_covers_every_rule_once_and_completes_on_confirm() {
        let mut p = Pack::defaults("a", "1");
        let q = p.interview();
        assert_eq!(q.as_array().unwrap().len(), REGISTRY.len());
        let keys: std::collections::BTreeSet<_> = REGISTRY.iter().map(|e| e.key).collect();
        assert_eq!(keys.len(), REGISTRY.len());
        assert!(!p.complete());
        for e in REGISTRY {
            p.confirm(e.key, Some(sample_for(e)), "Vishnu S.", "2026-11-28")
                .unwrap();
        }
        assert!(p.complete());
        assert_eq!(p.interview()[0]["status"], CONFIRMED);
    }

    #[test]
    fn null_is_never_a_confirmation_and_the_engine_holds_on_open_rules() {
        let mut p = Pack::defaults("a", "1");
        // seam_gap is read by the engine and has no value: accepting it is refused
        assert!(
            p.confirm("seam_gap", None, "Vishnu S.", "2026-11-28")
                .is_err()
        );
        assert!(p.rules["seam_gap"].confirmed.is_none());
        // a set null is refused too
        assert!(
            p.confirm("seam_gap", Some(Value::Null), "Vishnu S.", "2026-11-28")
                .is_err()
        );
        // every engine rule is open on a fresh pack, and so the pack is not complete
        let open = p.unconfirmed_engine_rules();
        assert!(open.contains(&"seam_gap".to_string()));
        assert!(open.contains(&"basis".to_string()));
        assert!(!p.complete());
        // a confirmed value on every engine rule closes them; the others are still open
        for k in open.clone() {
            let e = entry(&k).unwrap();
            p.confirm(&k, Some(sample_for(e)), "Vishnu S.", "2026-11-28")
                .unwrap();
        }
        assert!(p.unconfirmed_engine_rules().is_empty());
        assert!(!p.complete());
    }

    /// A value the rule accepts: a gap for seam_gap, the first choice, or the registry default when it has one.
    fn sample_for(e: &Entry) -> Value {
        if e.key == "seam_gap" {
            return json!(12.5);
        }
        if let Some(c) = e.choices.first() {
            return json!(c);
        }
        match serde_json::from_str::<Value>(e.default) {
            Ok(v) if !v.is_null() => v,
            _ => json!(1.0),
        }
    }

    #[test]
    fn identity_changes_with_status_and_value() {
        let mut p = Pack::defaults("a", "1");
        let before = p.identity();
        p.confirm("allow", Some(json!(4.5)), "Vishnu S.", "2026-11-28")
            .unwrap();
        assert_ne!(p.identity(), before);
        assert!(p.identity().contains("allow=CONFIRMED"));
    }

    #[test]
    fn ymd_is_correct() {
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(1_764_288_000), "2025-11-28");
        assert_eq!(ymd(1_709_164_800), "2024-02-29");
    }
}
