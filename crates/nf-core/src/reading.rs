//! ZK-PORT P11 (lane PZ-2): reading validation and provenance checks. A number read from a drawing page by the reader
//! or by AMOEBA must not reach a part unless (1) the value is a number written in its text, (2) its box on the page is
//! sane and (3) a second reading of that box, when one was made, agrees. Pure: no I/O, no clock, no model; the second
//! reading itself is done by `nf-numread` (`Reader::verify_box`) and comes in here as an [`OcrCheck`].
//!
//! Port of zumen-kirizu (ZK) `reader/schema.mjs` (READING_SCHEMA :70-134, normalizeReading :137-149, validate
//! :152-174), its number helpers (`build-core.mjs:13` num, `screen-v2.html:1095-1098` numIn) and the comma rule of
//! nf-server `orders.rs` (`reading()`, which this module now owns). ZK flaw 8 (`read_page.mjs:113-122` rescales every
//! box blindly) is answered by [`check_box`]: a box is checked against the page it claims to be on.
//!
//! Rules (NF law): nothing here confirms a value. A verdict that [`Verdict::blocks`] keeps the value out of every
//! computation (`order_compute` treats it like a PROPOSED value and raises the hold `reading_conflict`); the
//! non-blocking verdicts (`ocr_unavailable`, `unchecked`) raise the information hold `reading_unverified`. Every limit
//! below is a named rule marked UNVERIFIED: an implementation choice, not a factory or engineering value.

use std::sync::LazyLock;

use serde_json::{Value, json};

// ---------------------------------------------------------------- the reading schema (ZK reader/schema.mjs)

/// Part shapes a reading may name (ZK `SHAPES`).
pub const SHAPES: [&str; 13] = [
    "cylinder",
    "cone",
    "eccentric_cone",
    "miter_bend",
    "ring",
    "tee_main",
    "tee_branch",
    "tee_pad",
    "tee_rib",
    "plate",
    "wye",
    "purchased",
    "other",
];

/// Every parameter key of a reading, in ZK's `PARAM_LABEL` order.
pub const PARAM_KEYS: [&str; 57] = [
    "D",
    "D1",
    "D2",
    "L",
    "t",
    "R",
    "angle",
    "joints",
    "start_straight",
    "end_straight",
    "start_center",
    "mid_center",
    "end_center",
    "D_end",
    "taper_piece",
    "OD",
    "ID",
    "split",
    "main_D",
    "main_t",
    "branch_D",
    "branch_t",
    "H",
    "branch_x",
    "branch_angle",
    "e",
    "dir",
    "e_side",
    "flat_side",
    "pad_D",
    "pad_B",
    "plate_W",
    "plate_H",
    "plate_W2",
    "plate_off",
    "plate_C",
    "plate_C_at",
    "hole_d",
    "hole_n",
    "hole_x",
    "hole_y",
    "hole_nx",
    "hole_ny",
    "hole_px",
    "hole_py",
    "rib_b",
    "rib_h",
    "rib_top",
    "rib_n1",
    "rib_n2",
    "wye_angle",
    "leg_angle",
    "H2",
    "cone_s",
    "nom",
    "nom2",
    "pclass",
];

/// Where a value of a reading comes from (ZK `source` enum). `inferred` and `standard` are not read from the page:
/// their text is an expression or a table reference, and `order_compute` holds them with `source_inferred`.
pub const SOURCES: [&str; 6] = ["drawing", "table", "note", "count", "inferred", "standard"];
/// Standard items a part may name (ZK `STD_ITEMS`).
pub const STD_ITEMS: [&str; 6] = ["pipe", "bend", "tee", "reducer", "bellmouth", "drain_tee"];
pub const PAGE_KINDS: [&str; 3] = ["drawing", "cut_sheet", "other"];
/// The side a seam angle is measured from: bottom, top, unknown (ZK `seam.from`).
pub const SEAM_FROM: [&str; 3] = ["地", "天", "不明"];

/// Sources that claim "this number is written on the page" (nf-server `READ_SOURCES`): their text must hold the value.
pub const READ_SOURCES: [&str; 5] = ["read", "drawing", "table", "note", "count"];

/// Need and check keys per shape (ZK `SHAPE_PARAMS`, the form `review_read_validate.mjs` uses for its `off`
/// list). The `nom`, `nom2`, `pclass` keys the READING schema appends for the standard-table step are not part
/// of this table: a value of those keys on a shape that does not take a standard item is off-shape in ZK too.
pub fn shape_keys(shape: &str) -> Option<(&'static [&'static str], Vec<&'static str>)> {
    let (need, check): (&'static [&'static str], &'static [&'static str]) = match shape {
        "cylinder" => (&["D", "t", "L"], &["split"]),
        "cone" => (
            &["D1", "D2", "L", "t"],
            &[
                "split", "branch_D", "branch_t", "branch_x", "dir", "e", "e_side",
            ],
        ),
        "eccentric_cone" => (
            &["D1", "D2", "L", "t"],
            &[
                "split",
                "flat_side",
                "branch_D",
                "branch_t",
                "branch_x",
                "dir",
                "e",
                "e_side",
            ],
        ),
        "miter_bend" => (
            &[
                "D",
                "t",
                "R",
                "angle",
                "joints",
                "start_straight",
                "end_straight",
            ],
            &[
                "start_center",
                "mid_center",
                "end_center",
                "split",
                "D_end",
                "taper_piece",
            ],
        ),
        "ring" => (&["OD", "ID", "t", "split"], &[]),
        "tee_main" => (
            &["D", "t", "L", "branch_D", "branch_t"],
            &["branch_x", "e", "dir", "e_side", "branch_angle", "split"],
        ),
        "tee_branch" => (
            &["D", "t", "main_D", "main_t", "H"],
            &["branch_x", "dir", "e", "e_side", "branch_angle"],
        ),
        "tee_pad" => (
            &["t", "main_D", "branch_D"],
            &[
                "pad_D",
                "pad_B",
                "e",
                "dir",
                "e_side",
                "branch_angle",
                "split",
            ],
        ),
        "tee_rib" => (
            &["t", "main_D", "branch_D", "rib_b", "rib_h"],
            &["e", "rib_top", "rib_n1", "rib_n2"],
        ),
        "plate" => (
            &["t", "plate_W", "plate_H"],
            &[
                "plate_W2",
                "plate_off",
                "plate_C",
                "plate_C_at",
                "hole_d",
                "hole_n",
                "hole_x",
                "hole_y",
                "hole_nx",
                "hole_ny",
                "hole_px",
                "hole_py",
            ],
        ),
        "wye" => (
            &["D", "t", "wye_angle", "L", "H"],
            &["leg_angle", "H2", "branch_D", "branch_t", "cone_s", "split"],
        ),
        "purchased" | "other" => (&[], &[]),
        _ => return None,
    };
    let mut check = check.to_vec();
    if matches!(
        shape,
        "cylinder"
            | "cone"
            | "eccentric_cone"
            | "miter_bend"
            | "tee_main"
            | "tee_branch"
            | "tee_pad"
            | "tee_rib"
    ) {
        check.extend(["nom", "nom2", "pclass"]);
    }
    Some((need, check))
}

/// The subset of JSON Schema that ZK's `validate` reads: type, required, properties, additionalProperties, items,
/// enum, anyOf.
enum Schema {
    Node {
        types: &'static [&'static str],
        allowed: Option<&'static [&'static str]>,
        props: Option<Props>,
        items: Option<Box<Schema>>,
    },
    AnyOf(Vec<Schema>),
}

struct Props {
    list: Vec<(&'static str, Schema)>,
    required: Vec<&'static str>,
    /// additionalProperties: false
    closed: bool,
}

fn node(types: &'static [&'static str]) -> Schema {
    Schema::Node {
        types,
        allowed: None,
        props: None,
        items: None,
    }
}

fn string() -> Schema {
    node(&["string"])
}

fn one_of(allowed: &'static [&'static str]) -> Schema {
    Schema::Node {
        types: &["string"],
        allowed: Some(allowed),
        props: None,
        items: None,
    }
}

fn array_of(items: Schema) -> Schema {
    Schema::Node {
        types: &["array"],
        allowed: None,
        props: None,
        items: Some(Box::new(items)),
    }
}

/// A closed object whose every listed property is required (every object of the reading schema is).
fn object(list: Vec<(&'static str, Schema)>) -> Schema {
    let required = list.iter().map(|(k, _)| *k).collect();
    Schema::Node {
        types: &["object"],
        allowed: None,
        props: Some(Props {
            list,
            required,
            closed: true,
        }),
        items: None,
    }
}

fn bbox_schema() -> Schema {
    Schema::Node {
        types: &["array", "null"],
        allowed: None,
        props: None,
        items: Some(Box::new(node(&["number"]))),
    }
}

fn param_schema() -> Schema {
    Schema::AnyOf(vec![
        object(vec![
            ("value", node(&["number", "null"])),
            ("text", string()),
            ("bbox", bbox_schema()),
            ("source", one_of(&SOURCES)),
        ]),
        node(&["null"]),
    ])
}

static READING_SCHEMA: LazyLock<Schema> = LazyLock::new(|| {
    let strings = |keys: &[&'static str]| -> Vec<(&'static str, Schema)> {
        keys.iter().map(|k| (*k, string())).collect()
    };
    let page = object(vec![
        ("kind", one_of(&PAGE_KINDS)),
        ("title", string()),
        ("drawing_no", string()),
        ("job_no", string()),
        ("scale", string()),
        ("company", string()),
        ("qty", node(&["number", "null"])),
        ("unit_mass_kg", node(&["number", "null"])),
    ]);
    let mut row = strings(&[
        "no",
        "name",
        "material",
        "size",
        "length",
        "qty",
        "unit_mass",
        "mass",
        "note",
    ]);
    row.push(("bbox", bbox_schema()));
    let params = object(PARAM_KEYS.iter().map(|k| (*k, param_schema())).collect());
    let std = Schema::AnyOf(vec![
        node(&["null"]),
        object(vec![
            ("item", one_of(&STD_ITEMS)),
            ("text", string()),
            ("bbox", bbox_schema()),
        ]),
    ]);
    let dim = object(vec![
        ("label", string()),
        ("text", string()),
        ("bbox", bbox_schema()),
    ]);
    let part = object(vec![
        ("no", string()),
        ("name", string()),
        ("attach", string()),
        ("shape", one_of(&SHAPES)),
        ("table_rows", array_of(string())),
        ("params", params),
        ("notes", array_of(string())),
        ("std", std),
        ("dims", array_of(dim)),
    ]);
    let seam = Schema::AnyOf(vec![
        node(&["null"]),
        object(vec![
            ("angle", node(&["number", "null"])),
            ("from", one_of(&SEAM_FROM)),
            ("text", string()),
            ("bbox", bbox_schema()),
        ]),
    ]);
    object(vec![
        ("page", page),
        ("material_table", array_of(object(row))),
        ("parts", array_of(part)),
        ("seam", seam),
        ("notes", array_of(string())),
        ("unreadable", array_of(string())),
    ])
});

/// One finding of [`validate`]. `path` is ZK's (`.parts[0].params.D`; the whole reading is `（全体）`).
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaError {
    pub path: String,
    pub kind: SchemaErrorKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaErrorKind {
    /// None of the allowed forms fits (a param, the seam or the std block).
    NoForm,
    /// Wrong JSON type: (allowed types joined by `|`, the type found).
    Type(String, String),
    /// Not one of the allowed words: (the value as JSON, the allowed words).
    NotAllowed(String, &'static [&'static str]),
    /// A required field is absent.
    Missing,
    /// A field the reading does not have.
    Unknown,
}

impl SchemaError {
    fn at(path: &str) -> String {
        if path.is_empty() {
            "（全体）".to_string()
        } else {
            path.to_string()
        }
    }

    /// ZK's own Japanese text (the golden cases compare it byte for byte).
    pub fn ja(&self) -> String {
        match &self.kind {
            SchemaErrorKind::NoForm => format!("{}: 形が合わない", Self::at(&self.path)),
            SchemaErrorKind::Type(want, got) => {
                format!("{}: {want} のはずが {got}", Self::at(&self.path))
            }
            SchemaErrorKind::NotAllowed(v, list) => {
                format!("{}: {v} は {} のどれでもない", self.path, list.join("・"))
            }
            SchemaErrorKind::Missing => format!("{}: ない", self.path),
            SchemaErrorKind::Unknown => format!("{}: 形に無い項目", self.path),
        }
    }

    pub fn en(&self) -> String {
        let at = if self.path.is_empty() {
            "(whole reading)".to_string()
        } else {
            self.path.clone()
        };
        match &self.kind {
            SchemaErrorKind::NoForm => format!("{at}: does not have an allowed form"),
            SchemaErrorKind::Type(want, got) => format!("{at}: should be {want}, is {got}"),
            SchemaErrorKind::NotAllowed(v, list) => {
                format!("{at}: {v} is not one of {}", list.join(", "))
            }
            SchemaErrorKind::Missing => format!("{at}: missing"),
            SchemaErrorKind::Unknown => format!("{at}: not a field of a reading"),
        }
    }
}

fn type_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// ZK `validate(value, schema, at)`, step for step: an `anyOf` reports one finding for the whole node; a wrong type
/// stops the node; then the allowed words, the required fields, the fields themselves and the array items.
/// One deliberate difference: ZK looks properties up on a plain JS object, so a field named like an Object prototype
/// member (`constructor`, `toString`) passes unchecked there; here it is an unknown field.
fn validate_node(v: &Value, s: &Schema, at: &str, errs: &mut Vec<SchemaError>) {
    match s {
        Schema::AnyOf(alts) => {
            let fits = alts.iter().any(|alt| {
                let mut e = Vec::new();
                validate_node(v, alt, at, &mut e);
                e.is_empty()
            });
            if !fits {
                errs.push(SchemaError {
                    path: at.to_string(),
                    kind: SchemaErrorKind::NoForm,
                });
            }
        }
        Schema::Node {
            types,
            allowed,
            props,
            items,
        } => {
            let t = type_of(v);
            if !types.contains(&t) {
                errs.push(SchemaError {
                    path: at.to_string(),
                    kind: SchemaErrorKind::Type(types.join("|"), t.to_string()),
                });
                return;
            }
            if let Some(list) = allowed
                && !v.as_str().is_some_and(|s| list.contains(&s))
            {
                errs.push(SchemaError {
                    path: at.to_string(),
                    kind: SchemaErrorKind::NotAllowed(v.to_string(), list),
                });
            }
            if let (Value::Object(map), Some(p)) = (v, props) {
                for k in &p.required {
                    if !map.contains_key(*k) {
                        errs.push(SchemaError {
                            path: format!("{at}.{k}"),
                            kind: SchemaErrorKind::Missing,
                        });
                    }
                }
                for (k, x) in map {
                    match p.list.iter().find(|(name, _)| name == k) {
                        Some((_, sub)) => validate_node(x, sub, &format!("{at}.{k}"), errs),
                        None if p.closed => errs.push(SchemaError {
                            path: format!("{at}.{k}"),
                            kind: SchemaErrorKind::Unknown,
                        }),
                        None => {}
                    }
                }
            }
            if let (Value::Array(list), Some(sub)) = (v, items) {
                for (i, x) in list.iter().enumerate() {
                    validate_node(x, sub, &format!("{at}[{i}]"), errs);
                }
            }
        }
    }
}

/// Every way `reading` differs from the reading schema (ZK `validate`). Empty = a well-formed reading.
pub fn validate(reading: &Value) -> Vec<SchemaError> {
    let mut errs = Vec::new();
    validate_node(reading, &READING_SCHEMA, "", &mut errs);
    errs
}

/// JS truthiness of a JSON value (`x || default` in ZK).
fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// ZK `normalizeReading`: the fields a hand-made or older reading left out are filled (params with null, attach "",
/// table_rows / notes / dims / unreadable with [], std and seam with null). Where ZK throws (a reading that is not an
/// object, a part that is not an object, parts that are not a list) this leaves the value as it is: `validate` then
/// reports it.
pub fn normalize(reading: &mut Value) {
    let Value::Object(r) = reading else { return };
    if let Some(Value::Array(parts)) = r.get_mut("parts") {
        for p in parts.iter_mut() {
            let Value::Object(p) = p else { continue };
            if !truthy(p.get("params")) {
                p.insert("params".into(), json!({}));
            }
            if let Some(Value::Object(params)) = p.get_mut("params") {
                for k in PARAM_KEYS {
                    if !params.contains_key(k) {
                        params.insert(k.into(), Value::Null);
                    }
                }
            }
            if p.get("attach").is_none_or(Value::is_null) {
                p.insert("attach".into(), json!(""));
            }
            for k in ["table_rows", "notes", "dims"] {
                if !truthy(p.get(k)) {
                    p.insert(k.into(), json!([]));
                }
            }
            if !p.contains_key("std") {
                p.insert("std".into(), Value::Null);
            }
        }
    }
    for k in ["notes", "unreadable"] {
        if !truthy(r.get(k)) {
            r.insert(k.into(), json!([]));
        }
    }
    if r.get("seam").is_none_or(Value::is_null) {
        r.insert("seam".into(), Value::Null);
    }
}

/// Param keys of the reading that ZK does not know, as `no:key` (review_read_validate.mjs `unk`).
pub fn unknown_keys(reading: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for p in parts_of(reading) {
        if let Some(Value::Object(params)) = p.get("params") {
            for k in params.keys() {
                if !PARAM_KEYS.contains(&k.as_str()) {
                    out.push(format!("{}:{k}", js_str(p.get("no"))));
                }
            }
        }
    }
    out
}

/// Params given (not null) that the part's shape neither needs nor checks, as `no(shape):key`; an unknown shape is
/// `no: shape <shape>` (review_read_validate.mjs `off`).
pub fn off_shape_params(reading: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for p in parts_of(reading) {
        let no = js_str(p.get("no"));
        let shape = js_str(p.get("shape"));
        let Some((need, check)) = shape_keys(&shape) else {
            out.push(format!("{no}: shape {shape}"));
            continue;
        };
        if let Some(Value::Object(params)) = p.get("params") {
            for (k, q) in params {
                if truthy(Some(q)) && !need.contains(&k.as_str()) && !check.contains(&k.as_str()) {
                    out.push(format!("{no}({shape}):{k}"));
                }
            }
        }
    }
    out
}

fn parts_of(reading: &Value) -> impl Iterator<Item = &serde_json::Map<String, Value>> {
    reading
        .get("parts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

/// A JSON value as JS string concatenation shows it (`undefined` for an absent one).
fn js_str(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "null".into(),
        Some(x) => x.to_string(),
    }
}

// ---------------------------------------------------------------- numbers written in a text

/// The part of NFKC that drawing text needs, and one form for each mark: full-width ASCII (digits, letters,
/// punctuation) and the ideographic space become ASCII; every minus and dash becomes '-' (the katakana long-vowel mark
/// only directly before a digit, as ZK `numIn` reads it); the diameter signs become 'φ'; ring and ordinal marks
/// become '°'; prime marks become ' and ".
pub fn fold(text: &str) -> String {
    let src: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in src.iter().enumerate() {
        let f = match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            '\u{3000}' => ' ',
            '\u{2212}' | '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}'
            | '\u{2796}' | '\u{FE63}' => '-',
            'ー' if src.get(i + 1).is_some_and(|n| is_digit_like(*n)) => '-',
            'Φ' | 'Ø' | 'ø' | '⌀' | '∅' | 'Ф' | 'ϕ' => 'φ',
            '˚' | 'º' => '°',
            '′' => '\'',
            '″' => '"',
            c => c,
        };
        out.push(f);
    }
    out
}

fn is_digit_like(c: char) -> bool {
    c.is_ascii_digit() || ('\u{FF10}'..='\u{FF19}').contains(&c)
}

/// Value of one written number token, and whether it is ambiguous. A comma is a thousands separator only when the
/// first group has 1-3 digits without a leading 0 and every later group has exactly 3 ("1,500", "12,345.6"). Any
/// other comma gives one reading, a decimal comma, flagged ambiguous ("1,5", "0,500"): an ambiguous reading never
/// counts as the number written on the page. (nf-server orders.rs `reading()`, moved here unchanged.)
pub fn token_reading(tok: &str) -> Option<(f64, bool)> {
    if !tok.contains(',') {
        return tok.parse::<f64>().ok().map(|f| (f, false));
    }
    let (int_part, frac) = match tok.split_once('.') {
        Some((a, b)) => (a, Some(b)),
        None => (tok, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let groups: Vec<&str> = int_part.split(',').collect();
    let grouped = groups.len() > 1
        && digits(groups[0])
        && groups[0].len() <= 3
        && !groups[0].starts_with('0')
        && groups[1..].iter().all(|g| g.len() == 3 && digits(g))
        && !frac.is_some_and(|f| f.contains(','));
    if grouped {
        tok.replace(',', "").parse::<f64>().ok().map(|f| (f, false))
    } else {
        tok.replace(',', ".").parse::<f64>().ok().map(|f| (f, true))
    }
}

/// One number written in a text.
#[derive(Debug, Clone, PartialEq)]
pub struct DimNum {
    pub value: f64,
    /// More than one reading of the written form (decimal comma, or a leading point or comma the writer may have
    /// lost a digit in front of): never counts as the number on the page.
    pub ambiguous: bool,
    /// The mark directly in front (spaces and one '=' between allowed): φ R C t T M L H W B F e D d h b × x *.
    pub prefix: Option<char>,
    /// The unit or counter directly after (spaces allowed): mm ° 度 A t 枚 本 個 か所 つ 割 隅 % kg.
    pub unit: Option<&'static str>,
    /// Written in kanji numerals (二つ割 = 2).
    pub kanji: bool,
    /// Char range in the folded text.
    pub start: usize,
    pub end: usize,
}

const PREFIX_MARKS: &str = "φRCtTMLHWBFeDdhb×xX*";
const UNITS: [&str; 18] = [
    "mm", "°", "度", "kg", "%", "か所", "ヶ所", "カ所", "ケ所", "箇所", "枚", "本", "個", "つ",
    "割", "隅", "A", "t",
];
const KANJI_DIGITS: [(char, u32); 11] = [
    ('〇', 0),
    ('一', 1),
    ('二', 2),
    ('三', 3),
    ('四', 4),
    ('五', 5),
    ('六', 6),
    ('七', 7),
    ('八', 8),
    ('九', 9),
    ('十', 10),
];
/// Counters after which a kanji numeral is read as a number (二つ割, 三枚, 四隅, 二箇所). Without one, kanji numerals
/// are words (一般, 十分) and not read.
const KANJI_COUNTERS: &str = "つ枚本個箇ヶケカか所割等隅分列段穴";

fn kanji_value(run: &[char]) -> Option<u32> {
    let d = |c: char| KANJI_DIGITS.iter().find(|(k, _)| *k == c).map(|(_, v)| *v);
    if let Some(pos) = run.iter().position(|c| *c == '十') {
        let tens = match &run[..pos] {
            [] => 1,
            [c] => d(*c).filter(|v| (1..10).contains(v))?,
            _ => return None,
        };
        let ones = match &run[pos + 1..] {
            [] => 0,
            [c] => d(*c).filter(|v| *v < 10)?,
            _ => return None,
        };
        return Some(tens * 10 + ones);
    }
    let mut v: u32 = 0;
    for c in run {
        v = v.checked_mul(10)?.checked_add(d(*c).filter(|x| *x < 10)?)?;
    }
    Some(v)
}

fn unit_after(t: &[char], mut i: usize) -> Option<&'static str> {
    while t.get(i) == Some(&' ') {
        i += 1;
    }
    UNITS.iter().copied().find(|u| {
        let uc: Vec<char> = u.chars().collect();
        let fits = t.len() >= i + uc.len() && t[i..i + uc.len()] == uc[..];
        // a one-letter unit (A for a nominal size, t for a thickness) only when no letter follows it
        fits && (uc.len() > 1
            || !uc[0].is_ascii_alphabetic()
            || !t.get(i + 1).is_some_and(|c| c.is_ascii_alphanumeric()))
    })
}

fn prefix_before(t: &[char], start: usize) -> Option<char> {
    let mut j = start;
    while j > 0 && t[j - 1] == ' ' {
        j -= 1;
    }
    if j > 0 && t[j - 1] == '=' {
        j -= 1;
        while j > 0 && t[j - 1] == ' ' {
            j -= 1;
        }
    }
    let c = *t.get(j.checked_sub(1)?)?;
    // a mark, not the last letter of a word ("SS400" has no prefix, "M16" and "φ24" have)
    let word_before = j >= 2 && t[j - 2].is_ascii_alphabetic() && c.is_ascii_alphabetic();
    (PREFIX_MARKS.contains(c) && !word_before).then_some(c)
}

/// Every number written in `text`, with its sign, marks and unit, in order (decimal numbers first, then the kanji
/// numerals that stand before a counter, then degree-minute-second angles as one more number). A '-' is a sign only
/// right before the digits and not right after a letter or digit ("PL-9" and "100-200" hold the unsigned 9, 100, 200).
/// A degree value followed by minutes (45°30') also yields the decimal angle (45.5).
pub fn parse_dim(text: &str) -> Vec<DimNum> {
    let t: Vec<char> = fold(text).chars().collect();
    let mut out: Vec<DimNum> = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let lead = t[i].is_ascii_digit()
            || (matches!(t[i], '.' | ',') && t.get(i + 1).is_some_and(char::is_ascii_digit));
        if !lead {
            i += 1;
            continue;
        }
        let start = i;
        while i < t.len() && (t[i].is_ascii_digit() || t[i] == '.' || t[i] == ',') {
            i += 1;
        }
        let raw: String = t[start..i].iter().collect();
        // a lost leading digit (".5", ",500") is put back as 0 but never trusted
        let lead_cut = raw.starts_with(['.', ',']);
        let tok = raw.trim_end_matches(['.', ',']);
        let tok = tok.trim_start_matches(['.', ',']);
        let tok = if lead_cut {
            format!("0.{tok}")
        } else {
            tok.to_string()
        };
        let neg =
            start > 0 && t[start - 1] == '-' && (start < 2 || !t[start - 2].is_alphanumeric());
        if let Some((f, ambiguous)) = token_reading(&tok) {
            let end = start + raw.trim_end_matches(['.', ',']).chars().count();
            out.push(DimNum {
                value: if neg { -f } else { f },
                ambiguous: ambiguous || lead_cut,
                prefix: if neg { None } else { prefix_before(&t, start) },
                unit: unit_after(&t, end),
                kanji: false,
                start: if neg { start - 1 } else { start },
                end,
            });
        }
    }
    // kanji numerals before a counter
    let mut i = 0;
    while i < t.len() {
        if !KANJI_DIGITS.iter().any(|(k, _)| *k == t[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < t.len() && KANJI_DIGITS.iter().any(|(k, _)| *k == t[i]) {
            i += 1;
        }
        if t.get(i).is_some_and(|c| KANJI_COUNTERS.contains(*c))
            && let Some(v) = kanji_value(&t[start..i])
        {
            out.push(DimNum {
                value: f64::from(v),
                ambiguous: false,
                prefix: None,
                unit: unit_after(&t, i),
                kanji: true,
                start,
                end: i,
            });
        }
    }
    // degrees and minutes (and seconds) as one angle
    let mut dms = Vec::new();
    for (k, n) in out.iter().enumerate() {
        if n.unit != Some("°") || n.kanji || n.ambiguous {
            continue;
        }
        let deg_end = t[n.end..]
            .iter()
            .position(|c| *c == '°')
            .map(|p| n.end + p + 1);
        let Some(after) = deg_end else { continue };
        let next = out
            .get(k + 1)
            .filter(|m| m.start == after && !m.kanji && !m.ambiguous);
        if let Some(m) = next
            && t.get(m.end) == Some(&'\'')
        {
            let mut v = n.value.abs() + m.value / 60.0;
            if let Some(s) = out
                .get(k + 2)
                .filter(|s| s.start == m.end + 1 && t.get(s.end) == Some(&'"'))
            {
                v += s.value / 3600.0;
            }
            dms.push(DimNum {
                value: if n.value < 0.0 { -v } else { v },
                ambiguous: false,
                prefix: n.prefix,
                unit: Some("°"),
                kanji: false,
                start: n.start,
                end: m.end + 1,
            });
        }
    }
    out.extend(dms);
    out
}

/// The same number, within rounding: |a - b| <= 1e-9 * max(|b|, 1).
pub fn same_number(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

/// True if some number written in `text` equals `value`, sign included, read without ambiguity (nf-server's rule for
/// a value that claims to come from the page; key-free: use [`check_text`] when the key is known).
pub fn text_contains_number(text: &str, value: f64) -> bool {
    parse_dim(text)
        .iter()
        .any(|n| !n.ambiguous && same_number(n.value, value))
}

/// True if one of the strings a second reading returned names `value`. An `agrees` label without
/// such a string is not evidence: the value must be among the numbers the model actually read.
pub fn ocr_read_has_value(read: &[String], value: f64) -> bool {
    read.iter().any(|s| text_contains_number(s, value))
}

/// True if `value` is in `text` only as an ambiguous reading (a decimal comma).
pub fn text_has_only_ambiguous(text: &str, value: f64) -> bool {
    let nums = parse_dim(text);
    !nums
        .iter()
        .any(|n| !n.ambiguous && same_number(n.value, value))
        && nums
            .iter()
            .any(|n| n.ambiguous && same_number(n.value, value))
}

/// ZK `numIn` (screen-v2.html:1096): the number typed into one value field, or None. Full-width forms, minus signs, a
/// leading φ/R/C, thousands commas and a trailing mm, ミリ, ° or 度 are accepted; anything else is not a number.
pub fn value_in(text: &str) -> Option<f64> {
    let mut s: String = fold(text)
        .chars()
        .map(|c| if c == 'ー' { '-' } else { c })
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut sign = "";
    if let Some(rest) = s.strip_prefix('-') {
        sign = "-";
        s = rest.to_string();
    }
    if s.starts_with(['φ', 'R', 'C', 'c']) {
        s = s.chars().skip(1).collect();
    }
    // thousands commas: a comma between a digit and exactly three digits that end the token or meet a non-digit
    let c: Vec<char> = s.chars().collect();
    let mut kept = String::new();
    for (i, ch) in c.iter().enumerate() {
        let thousands = *ch == ','
            && i > 0
            && c[i - 1].is_ascii_digit()
            && c.len() >= i + 4
            && c[i + 1..i + 4].iter().all(char::is_ascii_digit)
            && !c.get(i + 4).is_some_and(char::is_ascii_digit);
        if !thousands {
            kept.push(*ch);
        }
    }
    let lower = kept.to_lowercase();
    let body = ["mm", "ミリ", "°", "度"]
        .iter()
        .find_map(|u| lower.strip_suffix(u).map(|b| &kept[..b.len()]))
        .unwrap_or(&kept);
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    let ok = match body.split_once('.') {
        Some((a, b)) => digits(a) && digits(b),
        None => digits(body),
    };
    if !ok {
        return None;
    }
    format!("{sign}{body}").parse().ok()
}

/// ZK `num` (build-core.mjs:13): the first decimal number of a text with its sign (`-?\d+(\.\d+)?` after NFKC), or
/// None. Material-table cells are read this way.
pub fn first_number(text: &str) -> Option<f64> {
    let t: Vec<char> = fold(text).chars().collect();
    let mut i = 0;
    while i < t.len() {
        if t[i].is_ascii_digit() {
            let neg = i > 0 && t[i - 1] == '-';
            let st = i;
            while i < t.len() && t[i].is_ascii_digit() {
                i += 1;
            }
            if i + 1 < t.len() && t[i] == '.' && t[i + 1].is_ascii_digit() {
                i += 1;
                while i < t.len() && t[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let s: String = t[st..i].iter().collect();
            return s.parse::<f64>().ok().map(|v| if neg { -v } else { v });
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------- word values per key

/// Direction keys: degrees with 天 (top) 0, 右 (right) 90, 地 (bottom) 180, 左 (left) 270 (ZK PARAM_LABEL dir/e_side/
/// flat_side).
const DIRECTION_KEYS: [&str; 3] = ["dir", "e_side", "flat_side"];
/// Keys whose value is often written as a word on the drawing. For them a text with no number and no word this check
/// knows is `unchecked` (a person checks it), not a conflict.
pub const WORD_KEYS: [&str; 7] = [
    "dir",
    "e_side",
    "flat_side",
    "plate_C_at",
    "plate_off",
    "plate_W2",
    "split",
];

/// (values the words of `t` mean for `key`, char ranges of `t` the words take up). The digits inside a word ("4隅")
/// are part of the word and do not count as numbers of the text.
fn word_values(key: &str, t: &[char]) -> (Vec<f64>, Vec<(usize, usize)>) {
    let s: String = t.iter().collect();
    let lower = s.to_lowercase();
    let mut vals = Vec::new();
    let mut spans = Vec::new();
    let find_all = |hay: &str, needle: &str, out: &mut Vec<(usize, usize)>| -> bool {
        let mut found = false;
        for (b, _) in hay.match_indices(needle) {
            let start = hay[..b].chars().count();
            out.push((start, start + needle.chars().count()));
            found = true;
        }
        found
    };
    let words = |table: &[(&str, f64)], spans: &mut Vec<(usize, usize)>, vals: &mut Vec<f64>| {
        for (w, v) in table {
            if find_all(&lower, &w.to_lowercase(), spans) {
                vals.push(*v);
            }
        }
    };
    if DIRECTION_KEYS.contains(&key) {
        words(
            &[
                ("天", 0.0),
                ("上", 0.0),
                ("top", 0.0),
                ("右", 90.0),
                ("right", 90.0),
                ("地", 180.0),
                ("下", 180.0),
                ("底", 180.0),
                ("bottom", 180.0),
                ("左", 270.0),
                ("left", 270.0),
            ],
            &mut spans,
            &mut vals,
        );
    }
    match key {
        // 0 = every corner, 1 = bottom left, 2 = bottom right, 3 = top right, 4 = top left (ZK plate_C_at)
        "plate_C_at" => words(
            &[
                ("4隅", 0.0),
                ("四隅", 0.0),
                ("全部", 0.0),
                ("全て", 0.0),
                ("すべて", 0.0),
                ("全周", 0.0),
                ("all", 0.0),
                ("左下", 1.0),
                ("lower left", 1.0),
                ("bottom left", 1.0),
                ("右下", 2.0),
                ("lower right", 2.0),
                ("bottom right", 2.0),
                ("右上", 3.0),
                ("upper right", 3.0),
                ("top right", 3.0),
                ("左上", 4.0),
                ("upper left", 4.0),
                ("top left", 4.0),
            ],
            &mut spans,
            &mut vals,
        ),
        // a triangle has no top edge: its width is 0 (ZK plate_W2)
        "plate_W2" => words(&[("三角", 0.0), ("triangle", 0.0)], &mut spans, &mut vals),
        // a right angle on the left: the top edge starts above the bottom edge's left end (ZK plate_off). A right
        // angle on the right side is not 0 and is not read here.
        "plate_off" if !s.contains('右') && !lower.contains("right side") => words(
            &[("直角", 0.0), ("right angle", 0.0), ("square", 0.0)],
            &mut spans,
            &mut vals,
        ),
        // pieces: not split = 1, split in half = 2
        "split" => words(
            &[
                ("割らない", 1.0),
                ("割りなし", 1.0),
                ("割無し", 1.0),
                ("割なし", 1.0),
                ("半割", 2.0),
                ("not split", 1.0),
            ],
            &mut spans,
            &mut vals,
        ),
        _ => {}
    }
    (vals, spans)
}

// ---------------------------------------------------------------- per-item checks

/// What the text of a value says about the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextCheck {
    /// Not a reading: the source is an expression, a table, a standard or a person's entry, or there is no value.
    NotRead,
    /// The value is one of the numbers written in the text.
    Matches,
    /// The value is what a word of the text means for this key (天 = 0 for a direction, 二つ割 counts as a number).
    Word,
    /// The value is in the text only in a form with two readings (decimal comma, lost leading digit).
    Ambiguous,
    /// The text holds numbers (or words this key reads), none of them is the value.
    Mismatch,
    /// The text holds no number and no word this key reads.
    NoNumber,
}

impl TextCheck {
    pub fn code(self) -> &'static str {
        match self {
            TextCheck::NotRead => "not_read",
            TextCheck::Matches => "matches",
            TextCheck::Word => "word",
            TextCheck::Ambiguous => "ambiguous",
            TextCheck::Mismatch => "mismatch",
            TextCheck::NoNumber => "no_number",
        }
    }

    /// The stored form; unknown codes are refused.
    pub fn from_code(code: &str) -> Option<TextCheck> {
        Some(match code {
            "not_read" => TextCheck::NotRead,
            "matches" => TextCheck::Matches,
            "word" => TextCheck::Word,
            "ambiguous" => TextCheck::Ambiguous,
            "mismatch" => TextCheck::Mismatch,
            "no_number" => TextCheck::NoNumber,
            _ => return None,
        })
    }
}

/// Is `value` written in `text` for parameter `key`? The value must be one of the numbers of the text (any of them:
/// "4×φ24" holds hole_n 4 and hole_d 24, "H2 800" holds 800), or a word this key reads.
pub fn check_text(key: &str, text: &str, value: f64) -> TextCheck {
    let t: Vec<char> = fold(text).chars().collect();
    let (words, spans) = word_values(key, &t);
    if words.iter().any(|w| same_number(*w, value)) {
        return TextCheck::Word;
    }
    let nums: Vec<DimNum> = parse_dim(text)
        .into_iter()
        .filter(|n| !spans.iter().any(|(a, b)| n.start < *b && *a < n.end))
        .collect();
    if nums
        .iter()
        .any(|n| !n.ambiguous && same_number(n.value, value))
    {
        return if nums.iter().any(|n| n.kanji && same_number(n.value, value))
            && !nums
                .iter()
                .any(|n| !n.kanji && !n.ambiguous && same_number(n.value, value))
        {
            TextCheck::Word
        } else {
            TextCheck::Matches
        };
    }
    if nums
        .iter()
        .any(|n| n.ambiguous && same_number(n.value, value))
    {
        return TextCheck::Ambiguous;
    }
    if nums.is_empty() && words.is_empty() {
        return TextCheck::NoNumber;
    }
    TextCheck::Mismatch
}

/// UNVERIFIED rule (PZ-2): a box narrower or lower than this many page pixels cannot hold a legible number.
pub const BOX_MIN_SIDE_PX: f64 = 2.0;
/// UNVERIFIED rule (PZ-2): the box of one value covers at most this share of the page area. A dimension text covers
/// far less (a 60 x 20 px number on an A3 page at 200 dpi is 0.01 %); a box this large points at a region, not at
/// one number. ZK sets no limit.
pub const BOX_MAX_PAGE_FRACTION: f64 = 0.05;
/// UNVERIFIED rule (PZ-2): a box may stick out of the page by this many pixels (rounding of a rescaled box).
pub const BOX_EDGE_TOL_PX: f64 = 1.0;

/// What the box of a value says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoxCheck {
    /// No box given (hand-made readings and typed values have none).
    Absent,
    Ok,
    /// Not usable; the reason code says why.
    Invalid(String),
}

/// Every reason a box can be refused. A stored check is read back strictly: only these codes are
/// accepted, so a hand-written flag cannot invent a box reason.
pub const BOX_REASONS: [&str; 6] = [
    "box_not_four_numbers",
    "box_inverted",
    "box_too_small",
    "box_outside_page",
    "box_too_large",
    "box_missing",
];

impl BoxCheck {
    pub fn code(&self) -> &'static str {
        match self {
            BoxCheck::Absent => "absent",
            BoxCheck::Ok => "ok",
            BoxCheck::Invalid(_) => "invalid",
        }
    }

    pub fn reason(&self) -> &str {
        match self {
            BoxCheck::Invalid(r) => r,
            _ => "",
        }
    }

    /// The stored form; unknown codes are refused.
    pub fn from_json(code: &str, reason: &str) -> Option<BoxCheck> {
        match code {
            "absent" if reason.is_empty() => Some(BoxCheck::Absent),
            "ok" if reason.is_empty() => Some(BoxCheck::Ok),
            "invalid" if BOX_REASONS.contains(&reason) => Some(BoxCheck::Invalid(reason.into())),
            _ => None,
        }
    }
}

/// Sanity of a box `[x0, y0, x1, y1]` in pixels of the page image `page = (width, height)` (None = page size not
/// known: only the box itself is checked). Refused: not four finite numbers, no area, a side under
/// [`BOX_MIN_SIDE_PX`], outside the page by more than [`BOX_EDGE_TOL_PX`], larger than [`BOX_MAX_PAGE_FRACTION`] of
/// the page.
pub fn check_box(bbox: Option<&[f64]>, page: Option<(f64, f64)>) -> BoxCheck {
    let Some(b) = bbox else {
        return BoxCheck::Absent;
    };
    let &[x0, y0, x1, y1] = b else {
        return BoxCheck::Invalid("box_not_four_numbers".into());
    };
    if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
        return BoxCheck::Invalid("box_not_four_numbers".into());
    }
    if x1 <= x0 || y1 <= y0 {
        return BoxCheck::Invalid("box_inverted".into());
    }
    if x1 - x0 < BOX_MIN_SIDE_PX || y1 - y0 < BOX_MIN_SIDE_PX {
        return BoxCheck::Invalid("box_too_small".into());
    }
    if let Some((w, h)) = page.filter(|(w, h)| *w > 0.0 && *h > 0.0) {
        let tol = BOX_EDGE_TOL_PX;
        if x0 < -tol || y0 < -tol || x1 > w + tol || y1 > h + tol {
            return BoxCheck::Invalid("box_outside_page".into());
        }
        if (x1 - x0) * (y1 - y0) > BOX_MAX_PAGE_FRACTION * w * h {
            return BoxCheck::Invalid("box_too_large".into());
        }
    }
    BoxCheck::Ok
}

/// The second reading of a value's box (nf-numread `Reader::verify_box`, or a reader that reports its own).
#[derive(Debug, Clone, PartialEq)]
pub enum OcrCheck {
    /// No second reading was made. Contributes nothing to the verdict (the item says so in `ocr`).
    NotRun,
    /// The second reading found the value; `read` = the numbers it read.
    Agrees { read: Vec<String> },
    /// The second reading found numbers, none of them the value (or ink-free paper where the box points).
    Disagrees { read: Vec<String> },
    /// A second reading was tried and could not be made; the reason code says why.
    Unavailable(String),
}

impl OcrCheck {
    pub fn code(&self) -> &'static str {
        match self {
            OcrCheck::NotRun => "not_run",
            OcrCheck::Agrees { .. } => "agrees",
            OcrCheck::Disagrees { .. } => "disagrees",
            OcrCheck::Unavailable(_) => "unavailable",
        }
    }

    /// The stored form. A second read that claims to agree but names no numbers, or numbers that do
    /// not contain the value, is a disagreement — an `agrees` label alone carries no authority.
    pub fn from_json(code: &str, read: &[String], reason: &str) -> Option<OcrCheck> {
        Some(match code {
            "not_run" if read.is_empty() && reason.is_empty() => OcrCheck::NotRun,
            "agrees" if !read.is_empty() && reason.is_empty() => OcrCheck::Agrees {
                read: read.to_vec(),
            },
            "disagrees" if reason.is_empty() => OcrCheck::Disagrees {
                read: read.to_vec(),
            },
            "unavailable" if !reason.is_empty() && read.is_empty() => {
                OcrCheck::Unavailable(reason.into())
            }
            _ => return None,
        })
    }
}

/// The verdict of one read value, worst first in [`Verdict::rank`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    Ok,
    /// Written as a word this check cannot read: a person checks it (does not block).
    Unchecked,
    /// The second reading could not be made: not cross-checked (does not block).
    OcrUnavailable,
    /// The second reading found another number: blocks.
    OcrDisagrees,
    /// The box is not usable: blocks.
    BoxInvalid,
    /// The value is not a number of its text: blocks.
    Conflicting,
}

impl Verdict {
    pub const ALL: [Verdict; 6] = [
        Verdict::Ok,
        Verdict::Unchecked,
        Verdict::OcrUnavailable,
        Verdict::OcrDisagrees,
        Verdict::BoxInvalid,
        Verdict::Conflicting,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Verdict::Ok => "ok",
            Verdict::Unchecked => "unchecked",
            Verdict::OcrUnavailable => "ocr_unavailable",
            Verdict::OcrDisagrees => "ocr_disagrees",
            Verdict::BoxInvalid => "box_invalid",
            Verdict::Conflicting => "conflicting",
        }
    }

    pub fn from_code(code: &str) -> Option<Verdict> {
        Verdict::ALL.into_iter().find(|v| v.code() == code)
    }

    /// The value must not be used: it stays out of every computation until it is fixed (hold `reading_conflict`).
    pub fn blocks(self) -> bool {
        matches!(
            self,
            Verdict::Conflicting | Verdict::BoxInvalid | Verdict::OcrDisagrees
        )
    }

    /// Not checked in full; a person must look (information hold `reading_unverified`).
    pub fn unverified(self) -> bool {
        matches!(self, Verdict::Unchecked | Verdict::OcrUnavailable)
    }

    /// (English, Japanese) in plain words for the person at the screen.
    pub fn text(self) -> (&'static str, &'static str) {
        verdict_text(self.code())
    }
}

/// (code, English, Japanese) of every verdict and every reason code (a code without a row is a test failure).
pub const VERDICT_TEXT: &[(&str, &str, &str)] = &[
    (
        "ok",
        "The value agrees with the drawing.",
        "値は図面と合っています。",
    ),
    (
        "unchecked",
        "The value is written as words this check cannot read; a person must check it.",
        "値がこの確認では読めない言葉で書かれています。人が確かめてください。",
    ),
    (
        "ocr_unavailable",
        "The place on the drawing could not be read a second time; a person must check the value.",
        "図面のその場所をもう一度読めませんでした。人が値を確かめてください。",
    ),
    (
        "ocr_disagrees",
        "A second reading of the place on the drawing found a different number.",
        "図面のその場所をもう一度読むと、違う数でした。",
    ),
    (
        "box_invalid",
        "The place marked on the drawing cannot be used.",
        "図面に付けた場所が使えません。",
    ),
    (
        "conflicting",
        "The value is not the number written on the drawing.",
        "値が図面に書いてある数と違います。",
    ),
    (
        "text_mismatch",
        "The text read from the drawing holds other numbers, not this value.",
        "図面から読んだ文字には別の数があり、この値はありません。",
    ),
    (
        "text_ambiguous",
        "The number on the drawing can be read two ways (a comma or a missing digit).",
        "図面の数は2通りに読めます（コンマ、または桁の抜け）。",
    ),
    (
        "text_has_no_number",
        "The text read from the drawing has no number.",
        "図面から読んだ文字に数がありません。",
    ),
    (
        "text_word_not_read",
        "The text is a word this check does not read.",
        "この確認では読まない言葉です。",
    ),
    (
        "box_missing",
        "The place on the drawing is not marked.",
        "図面上の場所が付いていません。",
    ),
    (
        "box_not_four_numbers",
        "The place must be given as four numbers.",
        "場所は4つの数で付けてください。",
    ),
    (
        "box_inverted",
        "The marked place has no area.",
        "付けた場所に広さがありません。",
    ),
    (
        "box_too_small",
        "The marked place is too small to hold a number.",
        "付けた場所が小さすぎて数が入りません。",
    ),
    (
        "box_outside_page",
        "The marked place is outside the page.",
        "付けた場所がページの外にあります。",
    ),
    (
        "box_too_large",
        "The marked place covers too much of the page to point at one number.",
        "付けた場所がページの広い範囲にかかり、1つの数を指していません。",
    ),
    (
        "ocr_other_number",
        "The second reading found another number there.",
        "もう一度読むと、そこには別の数がありました。",
    ),
    (
        "ocr_box_blank",
        "There is no writing where the place is marked.",
        "付けた場所に文字がありません。",
    ),
    (
        "ocr_read_mismatch",
        "The second reading did not read the value it is said to have read.",
        "もう一度の読み取りは、報告された値を読んでいません。",
    ),
    (
        "ocr_bad_box",
        "The marked place could not be read a second time because it is not usable.",
        "付けた場所が使えないので、もう一度読めませんでした。",
    ),
    (
        "ocr_no_models",
        "The number reader is not installed on this computer.",
        "このコンピューターには数の読み取り機能が入っていません。",
    ),
    (
        "ocr_nothing_read",
        "Nothing could be read at the marked place.",
        "付けた場所から何も読めませんでした。",
    ),
    (
        "ocr_not_a_number",
        "Only words, no number, were read at the marked place.",
        "付けた場所からは数ではなく言葉だけが読めました。",
    ),
    (
        "ocr_not_this_page",
        "The second reading was made on another picture than this page.",
        "もう一度の読み取りは、このページとは別の画像で行われました。",
    ),
    (
        "ocr_failed",
        "The number reader stopped with an error.",
        "数の読み取りがエラーで止まりました。",
    ),
];

fn verdict_text(code: &str) -> (&'static str, &'static str) {
    VERDICT_TEXT
        .iter()
        .find(|r| r.0 == code)
        .map_or(("", ""), |r| (r.1, r.2))
}

/// (English, Japanese) of a reason code ("" when the code has no row; a test checks every code has one).
pub fn reason_text(code: &str) -> (&'static str, &'static str) {
    verdict_text(code)
}

/// One value to check, as the reader proposed it.
#[derive(Debug, Clone, Copy)]
pub struct ItemIn<'a> {
    pub key: &'a str,
    pub value: Option<f64>,
    pub text: &'a str,
    pub bbox: Option<&'a [f64]>,
    pub source: &'a str,
}

/// What is known about the page and who proposed the value.
#[derive(Debug, Clone, Copy, Default)]
pub struct CheckCtx {
    /// Page image size in pixels (None = not known).
    pub page: Option<(f64, f64)>,
    /// The value was proposed by the reader or AMOEBA (agent identity): a read value then needs its box.
    pub require_box: bool,
}

/// The result for one value: the verdict and every part of the check, with reason codes.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemCheck {
    pub verdict: Verdict,
    pub text: TextCheck,
    pub bbox: BoxCheck,
    pub ocr: OcrCheck,
    pub reasons: Vec<String>,
}

impl ItemCheck {
    /// Stored form (order_values.reading_check) and API form; `provenance` adds model id and input sha256.
    pub fn to_json(&self) -> Value {
        let (read, ocr_reason) = match &self.ocr {
            OcrCheck::Agrees { read } | OcrCheck::Disagrees { read } => {
                (read.clone(), String::new())
            }
            OcrCheck::Unavailable(why) => (Vec::new(), why.clone()),
            OcrCheck::NotRun => (Vec::new(), String::new()),
        };
        let (en, ja) = self.verdict.text();
        json!({
            "verdict": self.verdict.code(),
            "blocks": self.verdict.blocks(),
            "text": self.text.code(),
            "box": self.bbox.code(),
            "box_reason": self.bbox.reason(),
            "ocr": self.ocr.code(),
            "ocr_read": read,
            "ocr_reason": ocr_reason,
            "reasons": self.reasons,
            "message_en": en,
            "message_ja": ja,
        })
    }
}

/// Who proposed a value and what was handed to that model. Stored with the verdict, so a later person can see
/// which model and which exact input produced the check. No model id ever grants authority: nothing here
/// confirms a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    pub model_id: String,
    /// SHA-256 of the exact crop pixels handed to the second read.
    pub input_sha256: String,
    /// SHA-256 of the exact page image the crop came from.
    pub page_sha256: String,
}

/// One checked value with its provenance, as stored on the proposal (an additive column in `order_values`).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredCheck {
    pub item: ItemRef,
    pub check: ItemCheck,
    pub provenance: Provenance,
}

impl StoredCheck {
    pub fn to_json(&self) -> Value {
        let mut v = self.check.to_json();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("part".into(), json!(self.item.part));
            obj.insert("no".into(), json!(self.item.no));
            obj.insert("shape".into(), json!(self.item.shape));
            obj.insert("key".into(), json!(self.item.key));
            obj.insert("model_id".into(), json!(self.provenance.model_id));
            obj.insert("input_sha256".into(), json!(self.provenance.input_sha256));
            obj.insert("page_sha256".into(), json!(self.provenance.page_sha256));
        }
        v
    }

    /// Read a stored check back strictly. A missing, unknown or inconsistent field refuses the
    /// flag: a hand-edited row cannot name a verdict or a provenance that was never produced here.
    pub fn from_json(v: &Value, value: Option<f64>) -> Option<StoredCheck> {
        fn sha(s: &str, allow_empty: bool) -> bool {
            (allow_empty && s.is_empty())
                || (s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        }
        let obj = v.as_object()?;
        let str_ = |k: &str| obj.get(k).and_then(Value::as_str);
        let verdict = Verdict::from_code(str_("verdict")?)?;
        if obj.get("blocks")?.as_bool()? != verdict.blocks() {
            return None;
        }
        let (en, ja) = verdict.text();
        if str_("message_en")? != en || str_("message_ja")? != ja {
            return None;
        }
        let text = TextCheck::from_code(str_("text")?)?;
        let bbox = BoxCheck::from_json(str_("box")?, str_("box_reason")?)?;
        let read: Vec<String> = serde_json::from_value(obj.get("ocr_read")?.clone()).ok()?;
        let ocr = OcrCheck::from_json(str_("ocr")?, &read, str_("ocr_reason")?)?;
        if let OcrCheck::Agrees { read } = &ocr
            && !value.is_some_and(|v| ocr_read_has_value(read, v))
        {
            return None;
        }
        let reasons: Vec<String> = serde_json::from_value(obj.get("reasons")?.clone()).ok()?;
        let item = ItemRef {
            part: usize::try_from(obj.get("part")?.as_u64()?).ok()?,
            no: str_("no")?.to_string(),
            shape: str_("shape")?.to_string(),
            key: str_("key")?.to_string(),
        };
        let input_sha256 = str_("input_sha256")?.to_string();
        let page_sha256 = str_("page_sha256")?.to_string();
        if !sha(&input_sha256, true) || !sha(&page_sha256, false) {
            return None;
        }
        Some(StoredCheck {
            item,
            check: ItemCheck {
                verdict,
                text,
                bbox,
                ocr,
                reasons,
            },
            provenance: Provenance {
                model_id: str_("model_id")?.to_string(),
                input_sha256,
                page_sha256,
            },
        })
    }
}

/// Check one read value. Sources that do not claim to be on the page (inferred, standard, entered, ...) and missing
/// values are not readings: their verdict is `ok` (the holds `source_inferred` and `missing` cover them). For a read
/// value: the text must hold the value ([`check_text`]), the box must be sane ([`check_box`]; a value proposed by the
/// reader or AMOEBA must have one), and the second reading, when one was made, must agree.
pub fn check_item(item: &ItemIn<'_>, ctx: &CheckCtx, ocr: OcrCheck) -> ItemCheck {
    let mut reasons: Vec<String> = Vec::new();
    let read = READ_SOURCES.contains(&item.source);
    let Some(value) = item.value.filter(|_| read) else {
        return ItemCheck {
            verdict: Verdict::Ok,
            text: TextCheck::NotRead,
            bbox: check_box(item.bbox, ctx.page),
            ocr: OcrCheck::NotRun,
            reasons,
        };
    };
    let mut verdict = Verdict::Ok;
    let mut worst = |v: Verdict, reason: &str, reasons: &mut Vec<String>| {
        verdict = verdict.max(v);
        reasons.push(reason.to_string());
    };
    let text = check_text(item.key, item.text, value);
    match text {
        TextCheck::Mismatch => worst(Verdict::Conflicting, "text_mismatch", &mut reasons),
        TextCheck::Ambiguous => worst(Verdict::Conflicting, "text_ambiguous", &mut reasons),
        TextCheck::NoNumber if WORD_KEYS.contains(&item.key) => {
            worst(Verdict::Unchecked, "text_word_not_read", &mut reasons)
        }
        TextCheck::NoNumber => worst(Verdict::Conflicting, "text_has_no_number", &mut reasons),
        TextCheck::NotRead | TextCheck::Matches | TextCheck::Word => {}
    }
    let bbox = match check_box(item.bbox, ctx.page) {
        BoxCheck::Absent if ctx.require_box => BoxCheck::Invalid("box_missing".into()),
        b => b,
    };
    if let BoxCheck::Invalid(why) = &bbox {
        worst(Verdict::BoxInvalid, why, &mut reasons);
    }
    let ocr = match ocr {
        // A claimed agreement must be backed by the numbers the second read returned.
        OcrCheck::Agrees { read } if !ocr_read_has_value(&read, value) => {
            worst(Verdict::OcrDisagrees, "ocr_read_mismatch", &mut reasons);
            OcrCheck::Disagrees { read }
        }
        other => other,
    };
    match &ocr {
        OcrCheck::Disagrees { read } if read.is_empty() => {
            worst(Verdict::OcrDisagrees, "ocr_box_blank", &mut reasons)
        }
        OcrCheck::Disagrees { .. } => {
            worst(Verdict::OcrDisagrees, "ocr_other_number", &mut reasons)
        }
        OcrCheck::Unavailable(why) => worst(Verdict::OcrUnavailable, why, &mut reasons),
        OcrCheck::NotRun | OcrCheck::Agrees { .. } => {}
    }
    ItemCheck {
        verdict,
        text,
        bbox,
        ocr,
        reasons,
    }
}

/// Where a value sits in a reading: part index, part no, shape and key.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemRef {
    pub part: usize,
    pub no: String,
    pub shape: String,
    pub key: String,
}

/// The check of a whole reading (ZK `submit`: normalize, validate; here also every value).
#[derive(Debug, Clone)]
pub struct ReadingReport {
    /// Findings of the schema check; a reading with any is refused as a whole and its values are not checked.
    pub schema_errors: Vec<SchemaError>,
    pub unknown_keys: Vec<String>,
    pub off_shape: Vec<String>,
    pub items: Vec<(ItemRef, ItemCheck)>,
}

impl ReadingReport {
    pub fn accepted(&self) -> bool {
        self.schema_errors.is_empty()
    }
    pub fn count(&self, v: Verdict) -> usize {
        self.items.iter().filter(|(_, c)| c.verdict == v).count()
    }
}

/// Check a reading as the reader returns it: normalize a copy (ZK normalizeReading), validate it, and check every
/// value that is given. `ocr` makes (or looks up) the second reading of one value.
pub fn check_reading(
    raw: &Value,
    ctx: &CheckCtx,
    ocr: &mut dyn FnMut(&ItemRef, &ItemIn<'_>) -> OcrCheck,
) -> ReadingReport {
    let mut r = raw.clone();
    normalize(&mut r);
    let schema_errors = validate(&r);
    let mut items = Vec::new();
    if schema_errors.is_empty() {
        for (i, p) in parts_of(&r).enumerate() {
            let no = js_str(p.get("no"));
            let shape = js_str(p.get("shape"));
            let Some(Value::Object(params)) = p.get("params") else {
                continue;
            };
            for k in PARAM_KEYS {
                let Some(Value::Object(q)) = params.get(k) else {
                    continue;
                };
                let bbox: Option<Vec<f64>> = q
                    .get("bbox")
                    .and_then(Value::as_array)
                    .map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect());
                let item = ItemIn {
                    key: k,
                    value: q.get("value").and_then(Value::as_f64),
                    text: q.get("text").and_then(Value::as_str).unwrap_or(""),
                    bbox: bbox.as_deref(),
                    source: q.get("source").and_then(Value::as_str).unwrap_or(""),
                };
                let at = ItemRef {
                    part: i,
                    no: no.clone(),
                    shape: shape.clone(),
                    key: k.to_string(),
                };
                let o = if item.value.is_some() && READ_SOURCES.contains(&item.source) {
                    ocr(&at, &item)
                } else {
                    OcrCheck::NotRun
                };
                let c = check_item(&item, ctx, o);
                items.push((at, c));
            }
        }
    }
    ReadingReport {
        schema_errors,
        unknown_keys: unknown_keys(&r),
        off_shape: off_shape_params(&r),
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn kanji_numerals() {
        let k = |s: &str| kanji_value(&s.chars().collect::<Vec<_>>());
        assert_eq!(k("二"), Some(2));
        assert_eq!(k("十"), Some(10));
        assert_eq!(k("十二"), Some(12));
        assert_eq!(k("二十"), Some(20));
        assert_eq!(k("二十四"), Some(24));
        assert_eq!(k("二〇"), Some(20));
        assert_eq!(k("十十"), None);
    }

    #[test]
    fn every_verdict_and_reason_has_text() {
        for v in Verdict::ALL {
            let (en, ja) = v.text();
            assert!(!en.is_empty() && !ja.is_empty(), "{v:?}");
            assert_eq!(Verdict::from_code(v.code()), Some(v));
        }
        for code in [
            "text_mismatch",
            "text_ambiguous",
            "text_has_no_number",
            "text_word_not_read",
            "box_missing",
            "box_not_four_numbers",
            "box_inverted",
            "box_too_small",
            "box_outside_page",
            "box_too_large",
            "ocr_other_number",
            "ocr_box_blank",
            "ocr_read_mismatch",
            "ocr_bad_box",
            "ocr_no_models",
            "ocr_nothing_read",
            "ocr_not_a_number",
            "ocr_not_this_page",
            "ocr_failed",
        ] {
            let (en, ja) = reason_text(code);
            assert!(!en.is_empty() && !ja.is_empty(), "{code}");
        }
    }

    /// The 60-case JA/EN parse table (BC:13, SC:1095-1098): every number the check must find, in order.
    #[test]
    fn parse_table_60_ja_en() {
        let table: &[(&str, &[f64])] = &[
            ("1500", &[1500.0]),
            ("1,500", &[1500.0]),
            ("１２，３４５", &[12345.0]),
            ("φ301.0", &[301.0]),
            ("Φ301", &[301.0]),
            ("ø42", &[42.0]),
            ("⌀42", &[42.0]),
            ("Ф42", &[42.0]),
            ("ϕ42", &[42.0]),
            ("R25", &[25.0]),
            ("C 45", &[45.0]),
            ("45°", &[45.0]),
            ("４５°", &[45.0]),
            ("45度", &[45.0]),
            ("45˚", &[45.0]),
            ("45º", &[45.0]),
            ("300×400", &[300.0, 400.0]),
            ("300 x 400", &[300.0, 400.0]),
            ("３００＊４００", &[300.0, 400.0]),
            ("300mm", &[300.0]),
            ("300 mm", &[300.0]),
            ("300ミリ", &[300.0]),
            ("-15", &[-15.0]),
            ("−15", &[-15.0]),
            ("－１５", &[-15.0]),
            ("ー15", &[-15.0]),
            ("PL-9", &[9.0]),
            ("100-200", &[100.0, 200.0]),
            ("-1,500", &[-1500.0]),
            ("0,500", &[0.5]),
            ("1,5", &[1.5]),
            (".5", &[0.5]),
            (",500", &[0.5]),
            ("12,345.6", &[12345.6]),
            ("01,500", &[1.5]),
            ("1234,567", &[1234.567]),
            ("1,5000", &[1.5]),
            ("二つ割", &[2.0]),
            ("三枚", &[3.0]),
            ("四隅", &[4.0]),
            ("十枚", &[10.0]),
            ("二十枚", &[20.0]),
            ("二十四枚", &[24.0]),
            ("一般", &[]),
            ("十時", &[]),
            ("45°30'", &[45.0, 30.0, 45.5]),
            ("-45°30'", &[-45.0, 30.0, -45.5]),
            ("45°30'15\"", &[45.0, 30.0, 15.0, 45.504_166_666_666_67]),
            ("t12", &[12.0]),
            ("L=2000", &[2000.0]),
            ("SS400", &[400.0]),
            ("M16", &[16.0]),
            ("穴４個", &[4.0]),
            ("6か所", &[6.0]),
            ("１，５００", &[1500.0]),
            ("１２３．４５", &[123.45]),
            ("A", &[]),
            ("", &[]),
            ("1e3", &[1.0, 3.0]),
            ("φ1,500 mm", &[1500.0]),
        ];
        assert_eq!(table.len(), 60, "the table must hold 60 cases");
        for (text, want) in table {
            let got: Vec<f64> = parse_dim(text).into_iter().map(|n| n.value).collect();
            assert_eq!(got.as_slice(), *want, "parse_dim({text:?})");
        }
        // The typed form of the same JA/EN values (ZK numIn) gives one number or none.
        for (text, want) in [
            ("１，５００", Some(1500.0)),
            ("-1,500", Some(-1500.0)),
            ("φ42mm", Some(42.0)),
            ("R25ミリ", Some(25.0)),
            ("45度", Some(45.0)),
            ("45°", Some(45.0)),
            ("1,5mm", None),
            ("abc", None),
        ] {
            assert_eq!(value_in(text), want, "value_in({text:?})");
        }
    }

    /// reader/research/review_read_validate.mjs frozen by extract_validate.mjs: 23 real readings and 42 malformed
    /// variants, compared field for field with ZK's own sorted Japanese error strings.
    #[test]
    fn zk_review_read_validate_cases() {
        let doc: Value = serde_json::from_str(include_str!(
            "../../../tests/golden/zumen/validate_cases.json"
        ))
        .unwrap();
        let cases = doc["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 65, "23 readings + 42 variants");
        let mut readings = 0;
        for c in cases {
            let name = c["name"].as_str().unwrap();
            let input = match c.get("input") {
                Some(v) => v.clone(),
                None => {
                    readings += 1;
                    let file = c["file"].as_str().unwrap();
                    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../../tests/golden/zumen/readings")
                        .join(file);
                    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
                }
            };
            let mut raw: Vec<String> = validate(&input).into_iter().map(|e| e.ja()).collect();
            raw.sort();
            let want: Vec<String> = serde_json::from_value(c["raw_errors"].clone()).unwrap();
            assert_eq!(raw, want, "{name}: raw validate()");

            let mut normalized = input.clone();
            normalize(&mut normalized);
            if c["norm_throws"].is_null() {
                let mut got: Vec<String> =
                    validate(&normalized).into_iter().map(|e| e.ja()).collect();
                got.sort();
                let want: Vec<String> = serde_json::from_value(c["norm_errors"].clone()).unwrap();
                assert_eq!(got, want, "{name}: validate(normalize())");
            }
            // Where ZK throws, normalize must leave the input alone rather than panic; validate then refuses it.
            let mut unknown = unknown_keys(&input);
            unknown.sort();
            assert_eq!(
                unknown,
                serde_json::from_value::<Vec<String>>(c["unknown_keys"].clone()).unwrap(),
                "{name}: unknown keys"
            );
            let mut off = off_shape_params(&input);
            off.sort();
            assert_eq!(
                off,
                serde_json::from_value::<Vec<String>>(c["off_shape"].clone()).unwrap(),
                "{name}: off-shape params"
            );
        }
        assert_eq!(readings, 23);
    }

    /// reader-synth v1b held-out pages with known truth. Every numeric truth item is checked once with its
    /// printed value and once with a deliberately injected wrong value. Rates are REPORTED, not tuned: the
    /// brief's 95 % / <3 % are estimates, not a test set to fit.
    #[test]
    fn reader_synth_v1b_injected_wrong_report() {
        let path = std::env::var_os("NF_READER_SYNTH_TRUTH")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/root/nf-data/reader-synth/v1b/test/truth.jsonl"));
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("SKIPPED: no reader-synth truth at {}", path.display());
            return;
        };
        let ctx = CheckCtx::default();
        let (mut injected, mut caught) = (0usize, 0usize);
        let (mut correct, mut right_accepted) = (0usize, 0usize);
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let page: Value = serde_json::from_str(line).unwrap();
            for it in page["items"].as_array().unwrap() {
                let printed = it["text"].as_str().unwrap_or("");
                let Some(value) = it["value"]
                    .as_str()
                    .and_then(|s| s.trim().parse::<f64>().ok())
                    .or_else(|| it["value"].as_f64())
                else {
                    continue;
                };
                if !value.is_finite() {
                    continue;
                }
                let key = it["type"].as_str().unwrap_or("D");
                let bbox = it["box_page"]
                    .as_array()
                    .and_then(|a| {
                        let v: Option<Vec<f64>> = a.iter().map(Value::as_f64).collect();
                        v
                    })
                    .unwrap_or_default();
                let item = ItemIn {
                    key,
                    value: Some(value),
                    text: printed,
                    bbox: Some(&bbox),
                    source: "read",
                };
                let ok = check_item(&item, &ctx, OcrCheck::NotRun);
                correct += 1;
                if !ok.verdict.blocks() {
                    right_accepted += 1;
                }
                let wrong = value + 1.0;
                let item = ItemIn {
                    value: Some(wrong),
                    ..item
                };
                let bad = check_item(&item, &ctx, OcrCheck::NotRun);
                injected += 1;
                if bad.verdict.blocks() {
                    caught += 1;
                }
            }
        }
        assert!(
            injected > 0 && correct > 0,
            "truth file must have numeric items"
        );
        eprintln!(
            "RESULT PZ-2 v1b injected-wrong: flag_rate={:.3}% ({}/{}), false_flag_rate={:.3}% ({}/{}) [EST targets 95% / <3%, not tuned]",
            100.0 * caught as f64 / injected as f64,
            caught,
            injected,
            100.0 * right_accepted as f64 / correct as f64,
            right_accepted,
            correct
        );
    }

    /// Deterministic fuzz of all text/number entry points: garbage never panics, never returns a NaN or an
    /// infinity, and a verdict is always one of the named codes.
    #[test]
    fn fuzz_parser_no_panic_no_nan() {
        let chars: Vec<char> =
            "0123456789.,-+−－０１２３４５６７８９φΦø⌀ФϕRCTtxX*°度ミリ枚個つ割隅abcSSPL=× \t\nー"
                .chars()
                .collect();
        let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..20_000 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let len = ((seed >> 33) % 48) as usize;
            let mut text = String::new();
            for _ in 0..len {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                text.push(chars[(seed >> 33) as usize % chars.len()]);
            }
            let nums = parse_dim(&text);
            for n in &nums {
                assert!(n.value.is_finite(), "{text:?}: {n:?}");
                assert!(n.start <= n.end && n.end <= fold(&text).chars().count());
            }
            if let Some(v) = value_in(&text) {
                assert!(v.is_finite(), "{text:?}");
            }
            if let Some(v) = first_number(&text) {
                assert!(v.is_finite(), "{text:?}");
            }
            let key = if seed.is_multiple_of(2) { "dir" } else { "D" };
            let check = check_text(key, &text, nums.first().map_or(1.0, |n| n.value));
            assert!(Verdict::from_code(check.code()).is_none() || !check.code().is_empty());
            let bbox = [0.0, 0.0, 10.0, 10.0];
            let got = check_item(
                &ItemIn {
                    key,
                    value: Some(nums.first().map_or(0.0, |n| n.value)),
                    text: &text,
                    bbox: Some(&bbox),
                    source: "read",
                },
                &CheckCtx {
                    page: Some((100.0, 100.0)),
                    require_box: true,
                },
                OcrCheck::NotRun,
            );
            assert!(Verdict::from_code(got.verdict.code()) == Some(got.verdict));
            for r in &got.reasons {
                assert!(!r.is_empty());
            }
        }
    }

    #[test]
    fn provenance_is_stored_with_the_verdict_and_confirms_nothing() {
        let check = ItemCheck {
            verdict: Verdict::Conflicting,
            text: TextCheck::Mismatch,
            bbox: BoxCheck::Ok,
            ocr: OcrCheck::NotRun,
            reasons: vec!["text_mismatch".into()],
        };
        let stored = StoredCheck {
            item: ItemRef {
                part: 1,
                no: "1".into(),
                shape: "cylinder".into(),
                key: "D".into(),
            },
            check,
            provenance: Provenance {
                model_id: "nf-numread R0".into(),
                input_sha256: "abc123".into(),
                page_sha256: "def456".into(),
            },
        };
        let json = stored.to_json();
        assert_eq!(json["verdict"], "conflicting");
        assert_eq!(json["blocks"], true);
        assert_eq!(json["model_id"], "nf-numread R0");
        assert_eq!(json["input_sha256"], "abc123");
        assert!(Verdict::Conflicting.blocks());
        assert!(!Verdict::Ok.blocks() && !Verdict::OcrUnavailable.blocks());
    }

    #[test]
    fn a_claimed_agreement_must_contain_the_value() {
        let item = ItemIn {
            key: "D",
            value: Some(216.3),
            text: "φ216.3",
            bbox: Some(&[10.0, 10.0, 60.0, 40.0]),
            source: "read",
        };
        let agrees = check_item(
            &item,
            &CheckCtx {
                page: Some((200.0, 200.0)),
                require_box: true,
            },
            OcrCheck::Agrees {
                read: vec!["999".into()],
            },
        );
        assert_eq!(agrees.verdict, Verdict::OcrDisagrees);
        assert_eq!(agrees.ocr.code(), "disagrees");
        assert!(agrees.reasons.iter().any(|r| r == "ocr_read_mismatch"));
    }

    #[test]
    fn stored_flags_are_parsed_strictly() {
        let sha = "a".repeat(64);
        let check = ItemCheck {
            verdict: Verdict::Ok,
            text: TextCheck::Matches,
            bbox: BoxCheck::Ok,
            ocr: OcrCheck::Agrees {
                read: vec!["216.3".into()],
            },
            reasons: Vec::new(),
        };
        let stored = StoredCheck {
            item: ItemRef {
                part: 0,
                no: "1".into(),
                shape: String::new(),
                key: "D".into(),
            },
            check,
            provenance: Provenance {
                model_id: "nf-numread R0 / PP-OCRv4 (det+rec)".into(),
                input_sha256: sha.clone(),
                page_sha256: sha.clone(),
            },
        };
        let good = stored.to_json();
        assert!(StoredCheck::from_json(&good, Some(216.3)).is_some());
        let mut tampered = good.clone();
        tampered["ocr_read"] = json!(["999"]);
        assert!(StoredCheck::from_json(&tampered, Some(216.3)).is_none());
        let mut tampered = good;
        tampered["verdict"] = json!("ok");
        tampered["blocks"] = json!(true);
        assert!(StoredCheck::from_json(&tampered, Some(216.3)).is_none());
    }
}
