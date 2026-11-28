//! NFJ-1 S1-L10: order -> calculations compute. The wiring layer between the confirmed values of an order's parts
//! and the shape maths (`shapes`, `bend`, `tee`, `tee_pad`, `conebranch`, front halves in `order`). Pure: no server
//! types, no I/O, no clock. Port of the front half of zumen-kirizu `build()` (applyOver, teeGroups, fillTee, fillCone,
//! piecesOf, build-core.mjs:912) with the hold rules of DESIGN 2.6.
//!
//! What this layer guarantees (Chief Architect conditions of 27 Nov 2026, "Wiring-lane conditions"):
//! - Every result states `certified`, `method`, the named `rules` it applied, its `holds`, its `questions`, `eps_rep`
//!   (an estimate unless `eps_rep_certified`), and for a cone branch the plate-thickness convention and the delta the
//!   other convention would make.
//! - Nothing here writes a DXF. Every shape built here is `certified: false` (sampled polylines, quadrature without a
//!   bound), so under the owner's rule of 24 Nov no cut file is served for it: the part gets a pattern view and an
//!   issue entry. `dxf_blocked_by` lists why; `dxf_allowed` is true only for a certified result with no hold, no
//!   question and no refusal (no shape of this layer can be, until the certification lane).
//! - A hold of level `Need`, a question, a flag of a result (`tee_lateral_angle_below_30`), the rib-on-lateral refusal
//!   and the mid-surface deviation of a cone branch are all holds that block the DXF. A held part's mass is reported
//!   `mass_provisional` and does not enter the material list (L5b follow-up: `order::build_piece` computes masses for
//!   invalid holes).
//! - A PROPOSED value is never computed. A negative offset without a side is refused, never `abs()`'d.
//!
//! Not here: DXF/AMOEBA wiring and `eps_rep` certification (lane L11), the JIS standard-table fill, the wye and the
//! `Request`/`job::run` path (the five engine families keep their own route, unchanged).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

use crate::bend::{self, BendSpec, SeamFrom};
use crate::conebranch::{self, ConeBranchSpec, ThicknessConvention};
use crate::order::{self, CylParams, Hold, Level, PlateParams, RibGeo, RibParams, TableRow};
use crate::shapes::{self, Basis, ConeSpec, RingSpec, ShapeRefusal, Stock};
use crate::tee::{
    self, BranchOnMain, BranchPieceSpec, Mount, TeeMainSpec, TeeSeamFrom, signed_offset,
};
use crate::tee_pad::{self, PadOutline, PadSpec, SplitCut};

pub const COMPUTE_VERSION: &str = "order-compute 1 (NFJ-1 L10)";

// ---------------------------------------------------------------- inputs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValState {
    Proposed,
    Confirmed,
    Corrected,
    Missing,
}

#[derive(Debug, Clone)]
pub struct Val {
    pub value: Option<f64>,
    pub state: ValState,
    /// drawing, table, note, count, inferred, standard, entered, fixed (or `group` for a value filled from the group).
    pub source: String,
}

impl Val {
    pub fn confirmed(v: f64, source: &str) -> Val {
        Val {
            value: Some(v),
            state: ValState::Confirmed,
            source: source.to_string(),
        }
    }
    pub fn proposed(v: f64) -> Val {
        Val {
            value: Some(v),
            state: ValState::Proposed,
            source: "drawing".into(),
        }
    }
    fn usable(&self) -> Option<f64> {
        match self.state {
            ValState::Confirmed | ValState::Corrected => self.value,
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PartIn {
    pub no: String,
    pub name: String,
    pub shape: String,
    pub qty: u32,
    /// `no` of the part this one stands on (the tee main for a branch, pad or rib; the cone for a branch on a cone).
    pub attach: Option<String>,
    pub group: Option<String>,
    pub values: BTreeMap<String, Val>,
    pub table: Option<TableRow>,
}

impl PartIn {
    pub fn new(no: &str, shape: &str) -> PartIn {
        PartIn {
            no: no.into(),
            name: no.into(),
            shape: shape.into(),
            qty: 1,
            ..Default::default()
        }
    }
    /// Confirmed value from the drawing (builder style).
    pub fn v(mut self, key: &str, value: f64) -> PartIn {
        self.values
            .insert(key.into(), Val::confirmed(value, "drawing"));
        self
    }
    pub fn with(mut self, key: &str, v: Val) -> PartIn {
        self.values.insert(key.into(), v);
        self
    }
    pub fn attached(mut self, to: &str) -> PartIn {
        self.attach = Some(to.into());
        self
    }
    pub fn qty(mut self, q: u32) -> PartIn {
        self.qty = q;
        self
    }
}

/// Factory rules of one order (DESIGN 2.4). ALL are provisional ZK defaults, none confirmed by a factory.
#[derive(Debug, Clone)]
pub struct OrderRules {
    /// Allowance per bend piece (mm).
    pub allow: f64,
    /// Seam angle is measured from the bottom (heel) when true, from the top when false.
    pub seam_from_bottom: bool,
    pub seam_deg: f64,
    pub mount: Mount,
    pub stock: Vec<Stock>,
    pub stock_margin: f64,
    /// Clear distance seam to hole (mm); `None`: 5 x t.
    pub seam_gap: Option<f64>,
    pub pad_cut: SplitCut,
    pub basis: Basis,
    /// Plate thickness convention of the cone-branch cone (default normal, DECISIONS 28 Nov).
    pub cone_thickness: ThicknessConvention,
}

impl Default for OrderRules {
    fn default() -> Self {
        OrderRules {
            allow: bend::ALLOW_PER_PIECE_MM,
            seam_from_bottom: true,
            seam_deg: 45.0,
            mount: Mount::SetOn,
            stock: vec![Stock {
                w: 2438.0,
                l: 6096.0,
            }],
            stock_margin: 0.0,
            seam_gap: None,
            pad_cut: SplitCut::Cross,
            basis: Basis::Mid,
            cone_thickness: ThicknessConvention::Normal,
        }
    }
}

/// Plausibility band (DESIGN 2.6, D4: proposal, needs factory sign-off). Out of band = hold "check value".
pub const BAND_D_MM: (f64, f64) = (20.0, 4000.0);
pub const BAND_T_MM: (f64, f64) = (1.5, 50.0);
pub const BAND_T_OVER_D_MAX: f64 = 0.25;

impl OrderRules {
    /// Rules from the order's `rules_json` (an object; missing keys keep the provisional defaults). Strict: an unknown
    /// key or a value of the wrong kind is an error naming it, never ignored. Keys: `allow`, `seam_deg`, `seam_from`
    /// (`bottom`|`top`), `mount` (`set_on`|`set_in`), `stock` (list of `[width, length]`), `stock_margin`, `seam_gap`
    /// (number or null), `pad_cut` (`cross`|`along`), `basis` (`outer`|`mid`|`inner`), `cone_thickness`
    /// (`normal`|`radial`).
    pub fn from_json(v: &Value) -> Result<OrderRules, String> {
        let mut r = OrderRules::default();
        let Some(o) = v.as_object() else {
            return if v.is_null() {
                Ok(r)
            } else {
                Err("rules must be an object".into())
            };
        };
        let num = |k: &str, x: &Value| -> Result<f64, String> {
            x.as_f64()
                .filter(|n| n.is_finite())
                .ok_or_else(|| format!("rule '{k}' must be a finite number"))
        };
        for (k, x) in o {
            let text = x.as_str().unwrap_or("");
            match k.as_str() {
                "allow" => r.allow = num(k, x)?,
                "seam_deg" => r.seam_deg = num(k, x)?,
                "stock_margin" => r.stock_margin = num(k, x)?,
                "seam_gap" => r.seam_gap = if x.is_null() { None } else { Some(num(k, x)?) },
                "seam_from" => {
                    r.seam_from_bottom = match text {
                        "bottom" => true,
                        "top" => false,
                        _ => return Err("rule 'seam_from' must be 'bottom' or 'top'".into()),
                    }
                }
                "mount" => {
                    r.mount = match text {
                        "set_on" => Mount::SetOn,
                        "set_in" => Mount::SetIn,
                        _ => return Err("rule 'mount' must be 'set_on' or 'set_in'".into()),
                    }
                }
                "pad_cut" => {
                    r.pad_cut = match text {
                        "cross" => SplitCut::Cross,
                        "along" => SplitCut::Along,
                        _ => return Err("rule 'pad_cut' must be 'cross' or 'along'".into()),
                    }
                }
                "basis" => {
                    r.basis = match text {
                        "outer" => Basis::Outer,
                        "mid" => Basis::Mid,
                        "inner" => Basis::Inner,
                        _ => return Err("rule 'basis' must be 'outer', 'mid' or 'inner'".into()),
                    }
                }
                "cone_thickness" => {
                    r.cone_thickness = match text {
                        "normal" => ThicknessConvention::Normal,
                        "radial" => ThicknessConvention::Radial,
                        _ => {
                            return Err("rule 'cone_thickness' must be 'normal' or 'radial'".into());
                        }
                    }
                }
                "stock" => {
                    let a = x.as_array().ok_or_else(|| {
                        "rule 'stock' must be a list of [width, length]".to_string()
                    })?;
                    r.stock = a
                        .iter()
                        .map(|e| {
                            let p = e.as_array().filter(|p| p.len() == 2);
                            match p.map(|p| (p[0].as_f64(), p[1].as_f64())) {
                                Some((Some(w), Some(l))) if w > 0.0 && l > 0.0 => {
                                    Ok(Stock { w, l })
                                }
                                _ => Err("rule 'stock' must be a list of [width, length] above 0"
                                    .to_string()),
                            }
                        })
                        .collect::<Result<_, _>>()?;
                }
                other => return Err(format!("unknown rule '{other}'")),
            }
        }
        Ok(r)
    }

    pub fn rules_out(&self) -> Vec<Value> {
        let r = |n: &str, v: String| json!({"name": n, "value": v, "kind": "rule", "status": "UNVERIFIED"});
        vec![
            r("order.basis", format!("{:?}", self.basis).to_lowercase()),
            r("order.allow_mm", self.allow.to_string()),
            r(
                "order.seam",
                format!(
                    "{} deg from the {}",
                    self.seam_deg,
                    if self.seam_from_bottom {
                        "bottom"
                    } else {
                        "top"
                    }
                ),
            ),
            r(
                "order.mount",
                match self.mount {
                    Mount::SetOn => "set_on".into(),
                    Mount::SetIn => "set_in".into(),
                },
            ),
            r(
                "order.stock",
                self.stock
                    .iter()
                    .map(|s| format!("{}x{}", s.w, s.l))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            r("order.stock_margin_mm", self.stock_margin.to_string()),
            r(
                "order.seam_gap_mm",
                self.seam_gap.map_or("5 x t".to_string(), |g| g.to_string()),
            ),
            r(
                "order.pad_cut",
                match self.pad_cut {
                    SplitCut::Cross => "cross".into(),
                    SplitCut::Along => "along".into(),
                },
            ),
            r(
                "order.cone_thickness",
                self.cone_thickness.name().to_string(),
            ),
            r(
                "order.plausibility_band",
                format!(
                    "D {}..{} mm, t {}..{} mm, t/D < {}",
                    BAND_D_MM.0, BAND_D_MM.1, BAND_T_MM.0, BAND_T_MM.1, BAND_T_OVER_D_MAX
                ),
            ),
            r(
                "order.mass",
                format!(
                    "density {} kg/mm3; table mass within {} (UNVERIFIED rule)",
                    shapes::STEEL_DENSITY_KG_MM3,
                    order::MASS_TABLE_REL_TOL
                ),
            ),
        ]
    }
}

// ---------------------------------------------------------------- outputs

#[derive(Debug, Clone, Serialize)]
pub struct HoldOut {
    pub level: &'static str,
    pub code: String,
    pub keys: Vec<String>,
    /// `part` or `group` (a hold of the whole assembly, repeated on every part of the group).
    pub scope: &'static str,
    pub message_en: String,
    pub message_ja: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuestionOut {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RefusalOut {
    pub code: String,
    pub fields: Vec<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PatternPiece {
    pub label: String,
    pub kind: &'static str,
    pub w: Option<f64>,
    pub h: Option<f64>,
    pub area_mm2: Option<f64>,
    /// Mass of ONE copy (kg).
    pub kg: Option<f64>,
    pub copies: u32,
    pub outline: Vec<[f64; 2]>,
    pub holes: Vec<Vec<[f64; 2]>>,
    /// Circular holes: centre x, centre y, radius.
    pub circles: Vec<[f64; 3]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BomDelta {
    pub table_mass_kg: f64,
    pub calc_mass_kg: f64,
    pub gross_mass_kg: Option<f64>,
    pub used: &'static str,
    pub delta_kg: f64,
    pub delta_rel: f64,
    pub within_tolerance: bool,
    pub tolerance_rel: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PartResult {
    pub no: String,
    pub name: String,
    pub shape: String,
    pub qty: u32,
    pub group: String,
    /// computed | held | held_missing | refused | not_computed
    pub state: &'static str,
    pub holds: Vec<HoldOut>,
    pub questions: Vec<QuestionOut>,
    pub refusal: Option<RefusalOut>,
    pub certified: bool,
    pub method: Option<String>,
    pub rules: Vec<Value>,
    pub eps_rep: Option<f64>,
    pub eps_rep_certified: Option<bool>,
    pub thickness_convention: Option<&'static str>,
    pub thickness_delta: Option<Value>,
    pub mid_surface_dev: Option<f64>,
    pub taper_flags: Vec<String>,
    pub pattern: Vec<PatternPiece>,
    pub t_mm: Option<f64>,
    pub length_mm: Option<f64>,
    pub unit_mass_kg: Option<f64>,
    pub mass_kg: Option<f64>,
    pub gross_mass_kg: Option<f64>,
    /// True while a Need hold, a question or a refusal stands: the number is a view, not a result.
    pub mass_provisional: bool,
    pub bom: Option<BomDelta>,
    /// Always null in this lane: no shape built here is certified (owner rule of 24 Nov).
    pub dxf: Option<String>,
    pub dxf_allowed: bool,
    pub dxf_blocked_by: Vec<String>,
    pub pattern_view_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderResult {
    pub engine: &'static str,
    pub compute: &'static str,
    pub rules: Vec<Value>,
    pub parts: Vec<PartResult>,
}

impl PartResult {
    pub fn has_hold(&self, code: &str) -> bool {
        self.holds
            .iter()
            .any(|h| h.code == code && h.level == "need")
    }
    pub fn has_question(&self, code: &str) -> bool {
        self.questions.iter().any(|q| q.code == code)
    }
}

impl OrderResult {
    pub fn part(&self, no: &str) -> Option<&PartResult> {
        self.parts.iter().find(|p| p.no == no)
    }
}

// ---------------------------------------------------------------- text

/// Parameter labels (ZK PARAM_LABEL, shortened): (key, English, Japanese).
pub const LABELS: [(&str, &str, &str); 53] = [
    ("D", "outer diameter", "外径"),
    (
        "D1",
        "mouth outer diameter (branch side)",
        "口の外径（枝の位置を測る側）",
    ),
    ("D2", "mouth outer diameter (other)", "口の外径（もう片方）"),
    ("L", "length", "長さ"),
    ("t", "plate thickness", "板厚"),
    ("R", "bend radius", "曲げ半径"),
    ("angle", "bend angle", "曲がり角度"),
    ("joints", "number of cuts", "切り口の数"),
    (
        "start_straight",
        "start straight length",
        "始点側の直管の長さ",
    ),
    ("end_straight", "end straight length", "終点側の直管の長さ"),
    (
        "start_center",
        "start piece centre length",
        "始点側の片の中心の長さ",
    ),
    (
        "mid_center",
        "middle piece centre length",
        "中間の片の中心の長さ",
    ),
    (
        "end_center",
        "end piece centre length",
        "終点側の片の中心の長さ",
    ),
    ("D_end", "end mouth outer diameter", "終点側の口の外径"),
    ("taper_piece", "cone piece number", "円すいにする片の番号"),
    ("OD", "outer diameter", "外径"),
    ("ID", "inner diameter", "内径"),
    ("split", "number of plates", "割り方（枚数）"),
    ("main_D", "main outer diameter", "本管の外径"),
    ("main_t", "main plate thickness", "本管の板厚"),
    ("branch_D", "branch outer diameter", "枝の外径"),
    ("branch_t", "branch plate thickness", "枝の板厚"),
    (
        "H",
        "length from the axis crossing to the branch end",
        "交点から枝の管端まで",
    ),
    (
        "branch_x",
        "position of the branch along the main",
        "枝の位置（管端から）",
    ),
    ("branch_angle", "branch angle", "枝の角度"),
    ("e", "branch offset", "枝の片寄せ"),
    ("dir", "branch direction", "枝の向き"),
    ("e_side", "side of the offset", "片寄せの向き"),
    ("flat_side", "flat side azimuth", "平らな側の方位"),
    ("pad_D", "pad outer diameter (plan view)", "補強板の外径"),
    ("pad_B", "pad width round the branch", "補強板の幅"),
    ("plate_W", "plate width", "板の幅"),
    ("plate_H", "plate height", "板の高さ"),
    ("plate_W2", "top edge width", "上の辺の幅"),
    ("plate_off", "top edge offset", "上の辺の位置"),
    ("plate_C", "corner chamfer", "角の面取り"),
    ("plate_C_at", "chamfered corner", "面取りする角"),
    ("hole_d", "hole diameter", "穴の径"),
    ("hole_n", "number of holes", "穴の数"),
    ("hole_x", "hole position x", "穴の位置（横）"),
    ("hole_y", "hole position y", "穴の位置（縦）"),
    ("hole_nx", "holes across", "横に並ぶ穴の数"),
    ("hole_ny", "holes up", "縦に並ぶ穴の数"),
    ("hole_px", "hole pitch across", "横の穴の間隔"),
    ("hole_py", "hole pitch up", "縦の穴の間隔"),
    ("rib_b", "rib length", "リブの長さ"),
    ("rib_h", "rib height", "リブの高さ"),
    ("rib_top", "rib flat top width", "リブの先の幅"),
    ("rib_n1", "ribs round the main", "本管の円に沿うリブの数"),
    ("rib_n2", "triangular ribs", "三角のリブの数"),
    ("nom", "nominal size", "呼び径"),
    ("nom2", "second nominal size", "もう片方の呼び径"),
    ("pclass", "class", "種類"),
];

fn label_en(k: &str) -> String {
    LABELS
        .iter()
        .find(|l| l.0 == k)
        .map_or_else(|| k.to_string(), |l| format!("{k} ({})", l.1))
}
fn label_ja(k: &str) -> String {
    LABELS
        .iter()
        .find(|l| l.0 == k)
        .map_or_else(|| k.to_string(), |l| format!("{k}（{}）", l.2))
}

/// Every hold code of this layer and of the front halves it uses: (code, English, Japanese). A code without a row is a
/// test failure (`every_hold_code_has_text`). The keys of a hold are appended by the caller.
pub const HOLD_TEXT: &[(&str, &str, &str)] = &[
    (
        "missing",
        "A needed value is missing or not yet confirmed; the part is not computed.",
        "必要な値がないか未確認のため、この部材は計算していません。",
    ),
    (
        "value_proposed",
        "A proposed value has not been confirmed by a person; it is not used.",
        "提案された値が人に確認されていないため、使っていません。",
    ),
    (
        "source_inferred",
        "A value comes from an inference or a standard table; a person must confirm it.",
        "推定または規格表から得た値です。人の確認が必要です。",
    ),
    (
        "out_of_band",
        "A value is outside the plausible range; check it.",
        "値が想定範囲外です。確認してください。",
    ),
    (
        "shape_not_computed",
        "This shape is not computed by this build.",
        "この形はこの版では計算しません。",
    ),
    (
        "purchased_not_computed",
        "Purchased or other part: no calculation, the material list keeps the table numbers.",
        "購入品・その他：計算せず、材料表の数値のままです。",
    ),
    (
        "tee_mismatch",
        "The same value differs between the main and the branch of one tee.",
        "同じT字管の本管と枝で同じ値が食い違っています。",
    ),
    (
        "tee_group_no_main",
        "A branch, pad or rib has no tee main in its group.",
        "枝・補強板・リブの組に本管がありません。",
    ),
    (
        "tee_group_many_main",
        "More than one tee main in one group.",
        "1つの組に本管が複数あります。",
    ),
    (
        "tee_branch_incomplete",
        "A branch of this main is missing a needed value; the main hole cannot be placed.",
        "この本管の枝に必要な値が足りず、穴の位置を決められません。",
    ),
    (
        "tee_seam_crosses_hole",
        "No seam position keeps every hole off the longitudinal seams; choose a seam count or side.",
        "どの継ぎ目位置でも穴が縦継ぎ目にかかります。継ぎ目の数か側を選んでください。",
    ),
    (
        "tee_lateral_angle_below_30",
        "A branch at less than 30 degrees: built, but a person must confirm the angle.",
        "30度未満の枝です。計算はしましたが、角度の確認が必要です。",
    ),
    (
        "tee_rib_on_lateral_unsupported",
        "A rib on a lateral branch is not built.",
        "斜めの枝のリブは作りません。",
    ),
    (
        "tee_x_not_on_drawing",
        "Branch position along the main not on the drawing: taken at half the length; confirm.",
        "枝の位置が図面にないため、長さの半分としました。確認してください。",
    ),
    (
        "tee_dir_not_on_drawing",
        "Branch direction not on the drawing: taken as the top (0 deg); confirm.",
        "枝の向きが図面にないため、天（0度）としました。確認してください。",
    ),
    (
        "pad_outline_ambiguous",
        "Both pad_D and pad_B are given; give one.",
        "pad_D と pad_B の両方があります。どちらか一方にしてください。",
    ),
    (
        "conebranch_seam_crosses_hole",
        "The seams still cross the hole or come closer than the gap; choose the seams.",
        "継ぎ目が穴にかかるか間隔内に入っています。継ぎ目を選んでください。",
    ),
    (
        "conebranch_flat_side_unconfirmed",
        "The flat side of the eccentric cone is not on the drawing: taken as the top; confirm.",
        "偏心円すいの平らな側が図面にないため、天としました。確認してください。",
    ),
    (
        "conebranch_mid_surface_deviation",
        "The neutral surface of the eccentric cone deviates from the developed cone by more than the tolerance.",
        "偏心円すいの中立面が展開した円すいから許容を超えてずれています。",
    ),
    (
        "conebranch_x_not_on_drawing",
        "Branch position on the cone not on the drawing: taken at half the length; confirm.",
        "円すい上の枝の位置が図面にないため、長さの半分としました。確認してください。",
    ),
    (
        "conebranch_branch_incomplete",
        "The branch on this cone is missing a needed value (H or its own values).",
        "この円すいの枝に必要な値（H など）が足りません。",
    ),
    (
        "bend_joints_not_on_drawing",
        "The joint count is not on the drawing: confirm.",
        "切り口の数が図面にありません。確認してください。",
    ),
    (
        "bend_center_vs_drawing",
        "A piece centre length computed differs from the drawing by more than 1 mm.",
        "計算した片の中心の長さが図面と1 mm を超えて違います。",
    ),
    (
        "split_not_integer",
        "The number of plates is not a whole number.",
        "枚数が整数ではありません。",
    ),
    (
        "split_seam",
        "The number of seams is outside the range 1 to the limit.",
        "継ぎ目の数が範囲外です。",
    ),
    (
        "stock_fit",
        "The plate does not fit any stock size.",
        "板がどの定尺にも収まりません。",
    ),
    (
        "pipe_longer_than_stock",
        "The pipe is longer than the longest stock side.",
        "管が最長の定尺より長いです。",
    ),
    (
        "plate_bad",
        "The plate size is not valid.",
        "板の寸法が正しくありません。",
    ),
    (
        "plate_t_vs_table",
        "The thickness differs from the material table.",
        "板厚が材料表と違います。",
    ),
    (
        "plate_off",
        "A trapezoid needs the offset of its top edge.",
        "台形には上の辺の位置が必要です。",
    ),
    (
        "plate_C",
        "The chamfer is stated but its size could not be read.",
        "面取りがありますが寸法を読めていません。",
    ),
    (
        "plate_C_at",
        "The chamfered corner is not given.",
        "面取りする角がありません。",
    ),
    (
        "plate_C_big",
        "The chamfer is longer than an edge.",
        "面取りが辺より長いです。",
    ),
    (
        "plate_hole_d",
        "The hole diameter is stated but could not be read.",
        "穴の径がありますが読めていません。",
    ),
    (
        "plate_hole_pos",
        "The hole position is missing.",
        "穴の位置がありません。",
    ),
    (
        "plate_hole_grid",
        "The hole grid is incomplete or inconsistent.",
        "穴の並びが不足または矛盾しています。",
    ),
    (
        "plate_hole_out",
        "A hole is outside the plate or too near an edge.",
        "穴が板の外か縁に近すぎます。",
    ),
    (
        "plate_hole_invalid",
        "A hole is not valid (diameter or overlap).",
        "穴が正しくありません（径または重なり）。",
    ),
    (
        "rib_count",
        "The number of ribs is not given.",
        "リブの数がありません。",
    ),
    (
        "rib_n1",
        "The number of ribs round the main is not valid.",
        "本管に沿うリブの数が正しくありません。",
    ),
    (
        "rib_top",
        "The rib flat top width is stated but not read.",
        "リブの先の幅がありますが読めていません。",
    ),
    (
        "rib_qty",
        "The rib quantity does not match.",
        "リブの数量が合いません。",
    ),
    (
        "rib_geometry_top",
        "The rib geometry with its flat top is not valid.",
        "リブの形（先の平らな幅）が正しくありません。",
    ),
    (
        "rib_not_on_pipe",
        "The rib does not meet the main pipe.",
        "リブが本管に当たりません。",
    ),
    (
        "rib_taller_than_pipe",
        "The rib is taller than the main pipe allows.",
        "リブが本管に対して高すぎます。",
    ),
    (
        "rib_tip_inside_pipe",
        "The rib tip is inside the pipe.",
        "リブの先が管の内側に入ります。",
    ),
    (
        "mass_vs_table",
        "The computed mass differs from the material table by more than the tolerance.",
        "計算した質量が材料表と許容を超えて違います。",
    ),
    (
        "mass_vs_table_blank",
        "The mass of the blank differs from the material table.",
        "素材の質量が材料表と違います。",
    ),
    (
        "mass_vs_table_cut",
        "The mass of the cut plate differs from the material table.",
        "切断後の質量が材料表と違います。",
    ),
    (
        "size_vs_table",
        "The plate size differs from the material table by more than 1 mm.",
        "板の大きさが材料表と1 mm を超えて違います。",
    ),
    (
        "not_certified",
        "The shape is not certified: a pattern view and an issue entry are given, no DXF.",
        "この形は認証されていません。展開図の表示と課題の記載のみで、DXFは出しません。",
    ),
    (
        "refused",
        "The inputs were refused by the engine; see the refusal code.",
        "入力はエンジンに拒否されました。拒否コードを見てください。",
    ),
];

fn text_of(code: &str) -> (String, String) {
    HOLD_TEXT.iter().find(|h| h.0 == code).map_or_else(
        || (code.to_string(), code.to_string()),
        |h| (h.1.to_string(), h.2.to_string()),
    )
}

/// All hold codes (Need level) this layer can raise, for the exhaustiveness tests.
pub fn hold_codes() -> Vec<&'static str> {
    HOLD_TEXT.iter().map(|h| h.0).collect()
}

// ---------------------------------------------------------------- shape key tables

/// Need and check keys per shape (ZK SHAPE_PARAMS).
pub fn shape_params(shape: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    Some(match shape {
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
    })
}

const STD_KEYS: [&str; 3] = ["nom", "nom2", "pclass"];

// ---------------------------------------------------------------- the working copy of one part

struct Work<'a> {
    inp: &'a PartIn,
    vals: BTreeMap<String, Val>,
    holds: Vec<HoldOut>,
    questions: Vec<QuestionOut>,
    filled: Vec<String>,
}

fn hold_out(level: Level, code: &str, keys: &[String], scope: &'static str) -> HoldOut {
    let (en, ja) = text_of(code);
    let (ke, kj) = if keys.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!(
                " [{}]",
                keys.iter()
                    .map(|k| label_en(k))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            format!(
                "［{}］",
                keys.iter()
                    .map(|k| label_ja(k))
                    .collect::<Vec<_>>()
                    .join("、")
            ),
        )
    };
    HoldOut {
        level: match level {
            Level::Need => "need",
            Level::Info => "info",
            Level::Ok => "ok",
        },
        code: code.to_string(),
        keys: keys.to_vec(),
        scope,
        message_en: format!("{en}{ke}"),
        message_ja: format!("{ja}{kj}"),
    }
}

impl<'a> Work<'a> {
    fn new(inp: &'a PartIn) -> Work<'a> {
        Work {
            inp,
            vals: inp.values.clone(),
            holds: Vec::new(),
            questions: Vec::new(),
            filled: Vec::new(),
        }
    }
    fn num(&self, k: &str) -> Option<f64> {
        self.vals.get(k).and_then(Val::usable)
    }
    fn need(&mut self, level: Level, code: &str, keys: &[&str]) {
        let k: Vec<String> = keys.iter().map(|s| s.to_string()).collect();
        if !self.holds.iter().any(|h| h.code == code && h.keys == k) {
            self.holds.push(hold_out(level, code, &k, "part"));
        }
    }
    fn push_front(&mut self, hs: &[Hold]) {
        for h in hs {
            let k: Vec<String> = h.keys.iter().map(|s| s.to_string()).collect();
            if !self.holds.iter().any(|x| x.code == h.code && x.keys == k) {
                self.holds.push(hold_out(h.level, h.code, &k, "part"));
            }
        }
    }
    fn question(&mut self, code: &'static str, message: &str) {
        if !self.questions.iter().any(|q| q.code == code) {
            self.questions.push(QuestionOut {
                code,
                message: message.to_string(),
            });
        }
    }
    /// Fill a missing key from a confirmed value of the same group (source `group`, an Info hold names it).
    fn fill(&mut self, k: &str, v: Option<f64>) {
        if self.num(k).is_none()
            && self.vals.get(k).is_none_or(|x| x.value.is_none())
            && let Some(v) = v
        {
            self.vals.insert(
                k.to_string(),
                Val {
                    value: Some(v),
                    state: ValState::Confirmed,
                    source: "group".into(),
                },
            );
            self.filled.push(k.to_string());
        }
    }
    /// Need keys that have no confirmed value; proposed ones are named separately.
    fn missing(&self, keys: &[&str]) -> (Vec<String>, Vec<String>) {
        let mut miss = Vec::new();
        let mut prop = Vec::new();
        for k in keys {
            if self.num(k).is_none() {
                miss.push((*k).to_string());
                if self
                    .vals
                    .get(*k)
                    .is_some_and(|v| v.state == ValState::Proposed && v.value.is_some())
                {
                    prop.push((*k).to_string());
                }
            }
        }
        (miss, prop)
    }
    /// A value that must be a whole number >= 1 (split count, joints): `None` = absent, `Some(None)` = not whole.
    fn whole(&mut self, k: &'static str) -> Option<Option<u32>> {
        let v = self.num(k)?;
        if v.is_finite() && v >= 0.0 && v.fract() == 0.0 && v <= f64::from(u32::MAX) {
            Some(Some(v as u32))
        } else {
            self.need(Level::Need, "split_not_integer", &[k]);
            Some(None)
        }
    }
}

// ---------------------------------------------------------------- result assembly

fn blank(inp: &PartIn, group: &str) -> PartResult {
    PartResult {
        no: inp.no.clone(),
        name: inp.name.clone(),
        shape: inp.shape.clone(),
        qty: inp.qty,
        group: group.to_string(),
        state: "computed",
        holds: Vec::new(),
        questions: Vec::new(),
        refusal: None,
        certified: false,
        method: None,
        rules: Vec::new(),
        eps_rep: None,
        eps_rep_certified: None,
        thickness_convention: None,
        thickness_delta: None,
        mid_surface_dev: None,
        taper_flags: Vec::new(),
        pattern: Vec::new(),
        t_mm: None,
        length_mm: None,
        unit_mass_kg: None,
        mass_kg: None,
        gross_mass_kg: None,
        mass_provisional: false,
        bom: None,
        dxf: None,
        dxf_allowed: false,
        dxf_blocked_by: Vec::new(),
        pattern_view_only: false,
    }
}

fn rules_of(list: &[(&str, &str, &str, &str)]) -> Vec<Value> {
    list.iter()
        .map(|(n, v, k, s)| json!({"name": n, "value": v, "kind": k, "status": s}))
        .collect()
}

fn refusal_of(e: &ShapeRefusal) -> RefusalOut {
    RefusalOut {
        code: e.code.to_string(),
        fields: e.fields.iter().map(|f| f.to_string()).collect(),
        message: e.message.clone(),
    }
}

/// Finish a part: state, DXF gate, provisional mass, issue flag. The single place where "any hold blocks the DXF" is
/// decided, so no computing branch can forget it.
fn seal(mut r: PartResult, w: Work<'_>, refusal: Option<RefusalOut>) -> PartResult {
    let mut holds = w.holds;
    for k in &w.filled {
        holds.push(hold_out(
            Level::Info,
            "filled_from_group",
            std::slice::from_ref(k),
            "part",
        ));
    }
    r.holds.extend(holds);
    r.questions.extend(w.questions);
    r.refusal = refusal;
    let (en, ja) = (
        "A value was filled from another part of the same group.".to_string(),
        "同じ組の別の部材の値で補いました。".to_string(),
    );
    for h in r.holds.iter_mut().filter(|h| h.code == "filled_from_group") {
        h.message_en = format!("{en} [{}]", h.keys.join(", "));
        h.message_ja = format!("{ja}［{}］", h.keys.join("、"));
    }
    finish(&mut r);
    r
}

/// Recompute state and the DXF gate from the holds, questions and refusal.
fn finish(r: &mut PartResult) {
    let need: Vec<String> = r
        .holds
        .iter()
        .filter(|h| h.level == "need")
        .map(|h| h.code.clone())
        .collect();
    let mut blocked: Vec<String> = need.clone();
    blocked.extend(r.questions.iter().map(|q| q.code.to_string()));
    if r.refusal.is_some() {
        blocked.push("refused".into());
    }
    if !r.certified {
        blocked.push("not_certified".into());
    }
    blocked.sort();
    blocked.dedup();
    let missing = need.iter().any(|c| c == "missing" || c == "value_proposed");
    r.state = if r.refusal.is_some() {
        "refused"
    } else if missing {
        "held_missing"
    } else if r.state == "not_computed" {
        "not_computed"
    } else if !need.is_empty() || !r.questions.is_empty() {
        "held"
    } else {
        "computed"
    };
    r.mass_provisional = !(need.is_empty() && r.questions.is_empty() && r.refusal.is_none());
    r.dxf = None;
    r.dxf_allowed = blocked.is_empty() && r.certified;
    r.dxf_blocked_by = blocked;
    r.pattern_view_only = !r.pattern.is_empty();
    if r.state == "held_missing" || r.state == "refused" || r.state == "not_computed" {
        r.unit_mass_kg = None;
        r.mass_kg = None;
        r.gross_mass_kg = None;
        r.bom = None;
    }
}

fn set_masses(r: &mut PartResult, unit_kg: f64, gross: Option<f64>) {
    r.unit_mass_kg = Some(unit_kg);
    r.mass_kg = Some(unit_kg * f64::from(r.qty));
    r.gross_mass_kg = gross;
}

fn bom_compare(w: &mut Work<'_>, r: &mut PartResult, unit_kg: f64, gross: Option<f64>) {
    let Some(row) = &w.inp.table else { return };
    let Some(tm) = row.mass.filter(|m| *m > 0.0) else {
        return;
    };
    let (dn, dg) = (unit_kg - tm, gross.map(|g| g - tm));
    let use_gross = dg.is_some_and(|g| g.abs() < dn.abs());
    let d = if use_gross { dg.unwrap_or(dn) } else { dn };
    let rel = d.abs() / tm;
    let ok = rel < order::MASS_TABLE_REL_TOL
        || d.abs() <= 0.5 * 10f64.powi(-(row.mass_decimals as i32)) + 1e-9;
    r.bom = Some(BomDelta {
        table_mass_kg: tm,
        calc_mass_kg: unit_kg,
        gross_mass_kg: gross,
        used: if use_gross { "gross" } else { "calc" },
        delta_kg: d,
        delta_rel: rel,
        within_tolerance: ok,
        tolerance_rel: order::MASS_TABLE_REL_TOL,
    });
    if !ok {
        w.need(Level::Need, "mass_vs_table", &["t"]);
    }
}

fn rect(w: f64, h: f64) -> Vec<[f64; 2]> {
    vec![[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]]
}

fn piece(
    label: &str,
    kind: &'static str,
    w: f64,
    h: f64,
    area: f64,
    t: f64,
    copies: u32,
) -> PatternPiece {
    PatternPiece {
        label: label.to_string(),
        kind,
        w: Some(w),
        h: Some(h),
        area_mm2: Some(area),
        kg: Some(shapes::mass_kg(area, t)),
        copies,
        ..Default::default()
    }
}

fn open(mut pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    if pts.len() > 1 && pts.first() == pts.last() {
        pts.pop();
    }
    pts
}

// ---------------------------------------------------------------- checks common to all parts

fn common_checks(w: &mut Work<'_>, shape: &str) {
    let (need, check) = shape_params(shape).unwrap_or((&[], &[]));
    // proposed values are never computed, also optional ones
    let proposed: Vec<String> = w
        .vals
        .iter()
        .filter(|(_, v)| v.state == ValState::Proposed && v.value.is_some())
        .map(|(k, _)| k.clone())
        .collect();
    if !proposed.is_empty() {
        let h = hold_out(Level::Need, "value_proposed", &proposed, "part");
        w.holds.push(h);
    }
    // a value that is only inferred or from a table is a question for a person
    let inferred: Vec<String> = w
        .vals
        .iter()
        .filter(|(_, v)| {
            v.usable().is_some() && matches!(v.source.as_str(), "inferred" | "standard")
        })
        .map(|(k, _)| k.clone())
        .collect();
    if !inferred.is_empty() {
        let h = hold_out(Level::Need, "source_inferred", &inferred, "part");
        w.holds.push(h);
    }
    // plausibility band
    let mut band: Vec<String> = Vec::new();
    for k in ["D", "D1", "D2", "OD", "main_D", "branch_D", "D_end"] {
        if let Some(v) = w.num(k)
            && v.is_finite()
            && !(BAND_D_MM.0..=BAND_D_MM.1).contains(&v)
        {
            band.push(k.to_string());
        }
    }
    for k in ["t", "main_t", "branch_t"] {
        if let Some(v) = w.num(k)
            && v.is_finite()
            && !(BAND_T_MM.0..=BAND_T_MM.1).contains(&v)
        {
            band.push(k.to_string());
        }
    }
    if let (Some(t), Some(d)) = (w.num("t"), w.num("D").or_else(|| w.num("OD")))
        && t.is_finite()
        && d > 0.0
        && t / d >= BAND_T_OVER_D_MAX
    {
        band.push("t".into());
    }
    band.sort();
    band.dedup();
    if !band.is_empty() {
        let h = hold_out(Level::Need, "out_of_band", &band, "part");
        w.holds.push(h);
    }
    // values the shape does not use are named, never dropped silently
    let unused: Vec<String> = w
        .vals
        .iter()
        .filter(|(k, v)| {
            v.value.is_some()
                && !need.contains(&k.as_str())
                && !check.contains(&k.as_str())
                && !STD_KEYS.contains(&k.as_str())
        })
        .map(|(k, _)| k.clone())
        .collect();
    if !unused.is_empty() {
        let (en, ja) = (
            "A value is not used by this shape.",
            "この形では使わない値があります。",
        );
        let mut h = hold_out(Level::Info, "value_not_used", &unused, "part");
        h.message_en = format!("{en} [{}]", unused.join(", "));
        h.message_ja = format!("{ja}［{}］", unused.join("、"));
        w.holds.push(h);
    }
    let std_given: Vec<String> = STD_KEYS
        .iter()
        .filter(|k| w.vals.get(**k).is_some_and(|v| v.value.is_some()))
        .map(|k| k.to_string())
        .collect();
    if !std_given.is_empty() {
        let mut h = hold_out(
            Level::Info,
            "standard_table_not_applied",
            &std_given,
            "part",
        );
        h.message_en =
            "The standard-table fill is not part of this build: the value is not used.".into();
        h.message_ja = "規格表による補完はこの版にはありません。値は使っていません。".into();
        w.holds.push(h);
    }
}

fn missing_hold(w: &mut Work<'_>, need: &[&str]) -> bool {
    let (miss, _prop) = w.missing(need);
    if miss.is_empty() {
        return false;
    }
    let h = hold_out(Level::Need, "missing", &miss, "part");
    w.holds.push(h);
    true
}

// ---------------------------------------------------------------- per-shape computing

fn compute_cylinder(w: &mut Work<'_>, r: &mut PartResult, rules: &OrderRules) {
    let split = w.whole("split");
    let p = CylParams {
        d: w.num("D"),
        t: w.num("t"),
        l: w.num("L"),
        split: match split {
            Some(Some(s)) => Some(f64::from(s)),
            Some(None) => Some(f64::NAN),
            None => None,
        },
    };
    let Some(c) = order::check_cylinder(&p, &rules.stock, rules.stock_margin) else {
        return;
    };
    w.push_front(&c.holds);
    if c.plates > 0 {
        let t = p.t.unwrap_or(0.0);
        r.rules = rules_of(&shapes::NAMED_RULES);
        r.method = Some("closed_form_cylinder_development".into());
        r.t_mm = p.t;
        r.length_mm = p.l;
        for k in 0..c.plates {
            let mut pc = piece(
                &format!("{}-{}", w.inp.no, k + 1),
                "plate",
                c.plate_w,
                c.plate_h,
                c.plate_w * c.plate_h,
                t,
                1,
            );
            pc.outline = rect(c.plate_w, c.plate_h);
            r.pattern.push(pc);
        }
        set_masses(r, c.kg, None);
        bom_compare(w, r, c.kg, None);
    }
}

fn compute_plate(w: &mut Work<'_>, r: &mut PartResult, rules: &OrderRules) {
    let g = |k: &str| w.num(k);
    let p = PlateParams {
        t: g("t"),
        plate_w: g("plate_W"),
        plate_h: g("plate_H"),
        plate_w2: g("plate_W2"),
        plate_off: g("plate_off"),
        plate_c: g("plate_C"),
        plate_c_stated: false,
        plate_c_at: g("plate_C_at"),
        hole_d: g("hole_d"),
        hole_d_stated: false,
        hole_x: g("hole_x"),
        hole_y: g("hole_y"),
        hole_n: g("hole_n"),
        hole_nx: g("hole_nx"),
        hole_ny: g("hole_ny"),
        hole_px: g("hole_px"),
        hole_py: g("hole_py"),
    };
    let _ = rules;
    let c = order::check_plate(&p, w.inp.table.as_ref());
    w.push_front(&c.holds);
    if let Some(pc) = &c.piece {
        let pl = &pc.plate;
        r.rules = rules_of(&shapes::NAMED_RULES);
        r.method = Some("closed_form_plate_lines_and_circles".into());
        r.t_mm = p.t;
        let t = p.t.unwrap_or(0.0);
        let mut pat = piece(&w.inp.no, "plate", pl.w, pl.h, pl.area, t, pc.qty as u32);
        pat.outline = pl.outline.clone();
        pat.circles = pl
            .hole_centres
            .iter()
            .map(|c| [c[0], c[1], pl.hole_d / 2.0])
            .collect();
        r.pattern.push(pat);
        set_masses(r, pc.kg, Some(pc.gross_kg));
        if w.inp.table.as_ref().is_some_and(|t| t.mass.is_some())
            && !c.holds.iter().any(|h| h.code.starts_with("mass_vs_table"))
        {
            bom_compare(w, r, pc.kg, Some(pc.gross_kg));
        } else if let Some(h) = c.holds.iter().find(|h| h.code.starts_with("mass_vs_table")) {
            let tm = w.inp.table.as_ref().and_then(|t| t.mass).unwrap_or(0.0);
            let d = pc.kg - tm;
            r.bom = Some(BomDelta {
                table_mass_kg: tm,
                calc_mass_kg: pc.kg,
                gross_mass_kg: Some(pc.gross_kg),
                used: if h.code.ends_with("blank") {
                    "gross"
                } else {
                    "calc"
                },
                delta_kg: d,
                delta_rel: if tm > 0.0 { d.abs() / tm } else { 0.0 },
                within_tolerance: h.level == Level::Ok,
                tolerance_rel: order::MASS_TABLE_REL_TOL,
            });
        }
    }
}

fn compute_ring(w: &mut Work<'_>, r: &mut PartResult) {
    let split = match w.whole("split") {
        Some(Some(s)) => s,
        _ => return,
    };
    let (od, id, t) = (
        w.num("OD").unwrap_or(0.0),
        w.num("ID").unwrap_or(0.0),
        w.num("t").unwrap_or(0.0),
    );
    match shapes::ring(RingSpec { od, id, t, split }) {
        Ok(res) => {
            r.rules = rules_of(&shapes::NAMED_RULES);
            r.method = Some("closed_form_annulus".into());
            r.t_mm = Some(t);
            for (k, p) in res.pieces.iter().enumerate() {
                let mut pc = piece(
                    &format!("{}-{}", w.inp.no, k + 1),
                    "annulus_sector",
                    od,
                    od,
                    p.area,
                    t,
                    1,
                );
                if split == 1 {
                    pc.circles.push([od / 2.0, od / 2.0, od / 2.0]);
                    if id > 0.0 {
                        pc.circles.push([od / 2.0, od / 2.0, id / 2.0]);
                    }
                } else {
                    pc.w = None;
                    pc.h = None;
                }
                r.pattern.push(pc);
            }
            set_masses(r, res.mass_kg, None);
            bom_compare(w, r, res.mass_kg, None);
        }
        Err(e) => w_refuse(r, &e),
    }
}

fn w_refuse(r: &mut PartResult, e: &ShapeRefusal) {
    r.refusal = Some(refusal_of(e));
}

fn basis_of(rules: &OrderRules) -> Basis {
    rules.basis
}

fn compute_cone(w: &mut Work<'_>, r: &mut PartResult, rules: &OrderRules, ecc: bool) {
    let split = match w.whole("split") {
        Some(Some(s)) => s,
        Some(None) => return,
        None => 2,
    };
    let spec = ConeSpec {
        d1: w.num("D1").unwrap_or(0.0),
        d2: w.num("D2").unwrap_or(0.0),
        l: w.num("L").unwrap_or(0.0),
        t: w.num("t").unwrap_or(0.0),
        basis: basis_of(rules),
        split,
    };
    r.t_mm = Some(spec.t);
    r.length_mm = Some(spec.l);
    if ecc {
        match shapes::ecc_cone_split(spec) {
            Ok(res) => {
                r.rules = rules_of(&shapes::NAMED_RULES);
                r.method = Some(res.method.to_string());
                r.certified = res.certified;
                for (k, p) in res.pieces.iter().enumerate() {
                    let mut pc = piece(
                        &format!("{}-{}", w.inp.no, k + 1),
                        "eccentric_fan",
                        0.0,
                        0.0,
                        p.area,
                        spec.t,
                        1,
                    );
                    pc.w = None;
                    pc.h = None;
                    r.pattern.push(pc);
                }
                // the quadrature error estimate, not an eps_rep: stated as the area error of the whole
                r.eps_rep = None;
                r.eps_rep_certified = Some(false);
                set_masses(r, res.mass_kg, None);
                bom_compare(w, r, res.mass_kg, None);
            }
            Err(e) => w_refuse(r, &e),
        }
    } else {
        match shapes::cone_split(spec) {
            Ok(res) => {
                r.rules = rules_of(&shapes::NAMED_RULES);
                r.method = Some("closed_form_cone_development".into());
                for k in 0..res.n {
                    let mut pc = piece(
                        &format!("{}-{}", w.inp.no, k + 1),
                        "cone_fan",
                        res.chord,
                        res.height,
                        res.piece_area,
                        spec.t,
                        1,
                    );
                    pc.w = Some(res.chord);
                    pc.h = Some(res.height);
                    r.pattern.push(pc);
                }
                set_masses(r, res.mass_kg, None);
                bom_compare(w, r, res.mass_kg, None);
            }
            Err(e) => w_refuse(r, &e),
        }
    }
}

fn compute_bend(w: &mut Work<'_>, r: &mut PartResult, rules: &OrderRules) {
    let split = match w.whole("split") {
        Some(Some(s)) => s.max(1),
        Some(None) => return,
        None => 1,
    };
    let g = |k: &str| w.num(k);
    let mut spec = BendSpec::new(
        g("D").unwrap_or(0.0),
        g("t").unwrap_or(0.0),
        g("R").unwrap_or(0.0),
        g("angle").unwrap_or(0.0),
        g("joints"),
        g("start_straight").unwrap_or(0.0),
        g("end_straight").unwrap_or(0.0),
    );
    spec.d_end = g("D_end");
    spec.taper_piece = match g("taper_piece") {
        Some(v) if v.is_finite() && v >= 0.0 && v.fract() == 0.0 => Some(v as u32),
        Some(_) => Some(u32::MAX),
        None => None,
    };
    spec.basis = basis_of(rules);
    spec.allow = rules.allow;
    spec.seam_from = if rules.seam_from_bottom {
        SeamFrom::Heel
    } else {
        SeamFrom::Top
    };
    spec.seam_deg = rules.seam_deg;
    spec.split = split;
    r.t_mm = Some(spec.t);
    match bend::bend(&spec) {
        Ok(res) => {
            r.method = Some(res.method.to_string());
            r.certified = res.certified;
            r.rules = res
                .rules
                .iter()
                .map(|(n, v, s)| json!({"name": n, "value": v, "kind": "rule", "status": s}))
                .collect();
            if let Some(q) = res.question {
                w.question("bend_joints_not_on_drawing", q);
            }
            if let Some(tn) = res.tilt_note_deg {
                r.taper_flags.push(format!("tilt_note_deg={tn}"));
            }
            for (m, ps) in res.pieces.iter().enumerate() {
                for (b, band) in ps.bands.iter().enumerate() {
                    let mut pc = piece(
                        &format!("{}-p{}-{}", w.inp.no, m + 1, b + 1),
                        if ps.cone {
                            "bend_cone_piece"
                        } else {
                            "bend_cyl_piece"
                        },
                        band.w,
                        band.h,
                        band.area,
                        spec.t,
                        1,
                    );
                    pc.kg = Some(shapes::mass_kg(band.area, spec.t));
                    r.pattern.push(pc);
                }
            }
            // piece centre lengths against the drawing (ZK start/mid/end_center)
            let np = res.pieces.len();
            let mut off = Vec::new();
            let centres = [
                w.num("start_center"),
                w.num("mid_center"),
                w.num("end_center"),
            ];
            let chk = |key: &'static str, idx: usize, off: &mut Vec<String>| {
                let which = match key {
                    "start_center" => 0,
                    "mid_center" => 1,
                    _ => 2,
                };
                if let Some(v) = centres[which]
                    && let Some(p) = res.pieces.get(idx)
                    && (p.center - v).abs() > 1.0
                {
                    off.push(key.to_string());
                }
            };
            chk("start_center", 0, &mut off);
            chk("end_center", np.saturating_sub(1), &mut off);
            if np > 2 {
                chk("mid_center", 1, &mut off);
            }
            if !off.is_empty() {
                let h = hold_out(Level::Need, "bend_center_vs_drawing", &off, "part");
                w.holds.push(h);
            }
            let gross: f64 = res.pieces.iter().map(|p| p.mass_with_allow_kg).sum();
            set_masses(r, res.total_net_mass_kg, Some(gross));
            bom_compare(w, r, res.total_net_mass_kg, Some(gross));
        }
        Err(e) => w_refuse(r, &e),
    }
}

// ---------------------------------------------------------------- groups

fn group_of(parts: &[PartIn], p: &PartIn) -> String {
    if let Some(g) = &p.group {
        return g.clone();
    }
    let mut cur = p;
    for _ in 0..8 {
        match cur
            .attach
            .as_ref()
            .and_then(|a| parts.iter().find(|q| &q.no == a))
        {
            Some(next) => {
                if let Some(g) = &next.group {
                    return g.clone();
                }
                cur = next;
            }
            None => break,
        }
    }
    cur.no.clone()
}

/// Mirror shared values between two parts of a group. Both present and different (> 1e-6) = mismatch keys.
fn share(a: &mut Work<'_>, ka: &str, b: &mut Work<'_>, kb: &str, mism: &mut Vec<String>) {
    match (a.num(ka), b.num(kb)) {
        (Some(x), Some(y)) if (x - y).abs() > 1e-6 => mism.push(ka.to_string()),
        (Some(x), None) => b.fill(kb, Some(x)),
        (None, Some(y)) => a.fill(ka, Some(y)),
        _ => {}
    }
}

fn signed_e(w: &Work<'_>, key_e: &str) -> Result<f64, ShapeRefusal> {
    signed_offset(
        w.num(key_e).unwrap_or(0.0),
        w.num("e_side"),
        w.num("dir").unwrap_or(0.0),
    )
}

fn compute_tee_group(
    parts: &[PartIn],
    idxs: &[usize],
    rules: &OrderRules,
    gname: &str,
) -> BTreeMap<usize, PartResult> {
    let mut works: BTreeMap<usize, Work<'_>> =
        idxs.iter().map(|i| (*i, Work::new(&parts[*i]))).collect();
    let mut res: BTreeMap<usize, PartResult> = idxs
        .iter()
        .map(|i| (*i, blank(&parts[*i], gname)))
        .collect();
    let mains: Vec<usize> = idxs
        .iter()
        .copied()
        .filter(|i| parts[*i].shape == "tee_main")
        .collect();
    let branches: Vec<usize> = idxs
        .iter()
        .copied()
        .filter(|i| parts[*i].shape == "tee_branch")
        .collect();
    let pads: Vec<usize> = idxs
        .iter()
        .copied()
        .filter(|i| parts[*i].shape == "tee_pad")
        .collect();
    let ribs: Vec<usize> = idxs
        .iter()
        .copied()
        .filter(|i| parts[*i].shape == "tee_rib")
        .collect();
    for i in idxs {
        let shape = parts[*i].shape.clone();
        if let Some(w) = works.get_mut(i) {
            common_checks(w, &shape);
        }
    }
    let main = if mains.len() == 1 {
        Some(mains[0])
    } else {
        None
    };
    if main.is_none() {
        let code = if mains.is_empty() {
            "tee_group_no_main"
        } else {
            "tee_group_many_main"
        };
        for i in branches.iter().chain(&pads).chain(&ribs).chain(&mains) {
            if let Some(w) = works.get_mut(i) {
                w.need(Level::Need, code, &[]);
            }
        }
        // a member without a main in its group is not computed
        return finalize_group(parts, works, res, gname, &[]);
    }
    let m = main.unwrap_or(0);
    // ---- fill shared values (fillTee)
    let mut mism: Vec<String> = Vec::new();
    for b in &branches {
        let (mut wm, mut wb) = (take(&mut works, m), take(&mut works, *b));
        share(&mut wm, "D", &mut wb, "main_D", &mut mism);
        share(&mut wm, "t", &mut wb, "main_t", &mut mism);
        if branches.len() == 1 {
            share(&mut wm, "branch_D", &mut wb, "D", &mut mism);
            share(&mut wm, "branch_t", &mut wb, "t", &mut mism);
            for k in ["branch_x", "dir", "e", "e_side", "branch_angle"] {
                share(&mut wm, k, &mut wb, k, &mut mism);
            }
        }
        works.insert(m, wm);
        works.insert(*b, wb);
    }
    // pads and ribs take main_D, branch_D (and the offset keys) from the main / its branch
    let single_branch = branches.first().copied();
    for pr in pads.iter().chain(&ribs) {
        let (mut wm, mut wp) = (take(&mut works, m), take(&mut works, *pr));
        share(&mut wm, "D", &mut wp, "main_D", &mut mism);
        let src_branch_d = if let Some(b) = single_branch.filter(|_| branches.len() == 1) {
            works.get(&b).and_then(|w| w.num("D"))
        } else {
            wm.num("branch_D")
        };
        wp.fill("branch_D", src_branch_d);
        let keys: &[&str] = if parts[*pr].shape == "tee_pad" {
            &["e", "dir", "e_side", "branch_angle"]
        } else {
            &["e"]
        };
        for k in keys {
            let v = single_branch
                .filter(|_| branches.len() == 1)
                .and_then(|b| works.get(&b).and_then(|w| w.num(k)))
                .or_else(|| wm.num(k));
            wp.fill(k, v);
        }
        works.insert(m, wm);
        works.insert(*pr, wp);
    }
    if !mism.is_empty() {
        mism.sort();
        mism.dedup();
        let h = hold_out(Level::Need, "tee_mismatch", &mism, "group");
        for i in idxs {
            if let Some(w) = works.get_mut(i) {
                w.holds.push(h.clone());
            }
        }
    }
    // ---- missing values per part
    let mut incomplete: BTreeMap<usize, bool> = BTreeMap::new();
    for i in idxs {
        let shape = parts[*i].shape.clone();
        let (need, _) = shape_params(&shape).unwrap_or((&[], &[]));
        let w = works.get_mut(i).expect("work");
        let mut bad = missing_hold(w, need);
        if shape == "tee_pad" {
            let (a, b) = (w.num("pad_D"), w.num("pad_B"));
            if a.is_none() && b.is_none() {
                let h = hold_out(
                    Level::Need,
                    "missing",
                    &["pad_D".into(), "pad_B".into()],
                    "part",
                );
                w.holds.push(h);
                bad = true;
            } else if a.is_some() && b.is_some() {
                w.need(Level::Need, "pad_outline_ambiguous", &["pad_D", "pad_B"]);
                bad = true;
            }
        }
        if shape == "tee_rib" {
            // rib_n1 / rib_n2 are optional in ZK but a rib part with neither is held by check_tee_rib
        }
        incomplete.insert(*i, bad);
    }
    // ---- the main (needs every branch of the group)
    let mut tee_res: Option<tee::TeeMainResult> = None;
    let mut tee_refused: Option<ShapeRefusal> = None;
    {
        let mut main_bad = incomplete[&m];
        let mut specs: Vec<BranchOnMain> = Vec::new();
        let mut refusal_early: Option<ShapeRefusal> = None;
        if branches.is_empty() {
            // single branch described by the main alone
            let w = &works[&m];
            if !main_bad {
                match signed_e(w, "e") {
                    Ok(e) => {
                        let mut b = BranchOnMain::new(
                            "branch",
                            w.num("branch_D").unwrap_or(0.0),
                            w.num("branch_t").unwrap_or(0.0),
                            w.num("branch_x"),
                            w.num("dir").unwrap_or(0.0),
                            e,
                        );
                        b.angle_deg = w.num("branch_angle");
                        specs.push(b);
                    }
                    Err(e) => refusal_early = Some(e),
                }
            }
        } else {
            for b in &branches {
                if incomplete[b] {
                    main_bad = true;
                    let w = works.get_mut(&m).expect("main");
                    w.need(Level::Need, "tee_branch_incomplete", &[]);
                    continue;
                }
                let w = &works[b];
                match signed_e(w, "e") {
                    Ok(e) => {
                        let mut bs = BranchOnMain::new(
                            &parts[*b].no,
                            w.num("D").unwrap_or(0.0),
                            w.num("t").unwrap_or(0.0),
                            w.num("branch_x"),
                            w.num("dir").unwrap_or(0.0),
                            e,
                        );
                        bs.angle_deg = w.num("branch_angle");
                        specs.push(bs);
                    }
                    Err(e) => refusal_early = refusal_early.or(Some(e)),
                }
            }
        }
        if let Some(e) = refusal_early {
            tee_refused = Some(e);
        } else if !main_bad && !specs.is_empty() {
            let w = &works[&m];
            let split = match w.vals.get("split").and_then(Val::usable) {
                Some(v) if v.is_finite() && v >= 1.0 && v.fract() == 0.0 && v <= 12.0 => v as u32,
                Some(_) => 0,
                None => 1,
            };
            if split == 0 {
                works
                    .get_mut(&m)
                    .expect("m")
                    .need(Level::Need, "split_seam", &["split"]);
            } else {
                for s in &specs {
                    let wm = works.get_mut(&m).expect("m");
                    if s.x.is_none() {
                        wm.question("tee_x_not_on_drawing", tee::TEE_X_QUESTION);
                    }
                }
                let wm = &works[&m];
                let dir_given = if branches.is_empty() {
                    wm.num("dir").is_some()
                } else {
                    branches.iter().all(|b| works[b].num("dir").is_some())
                };
                let mut spec = TeeMainSpec::new(
                    wm.num("D").unwrap_or(0.0),
                    wm.num("t").unwrap_or(0.0),
                    wm.num("L").unwrap_or(0.0),
                    specs,
                );
                spec.mount = rules.mount;
                spec.seam_from = if rules.seam_from_bottom {
                    TeeSeamFrom::Bottom
                } else {
                    TeeSeamFrom::Top
                };
                spec.seam_deg = rules.seam_deg;
                spec.seams = split;
                spec.gap = rules.seam_gap;
                match tee::tee_main(&spec) {
                    Ok(t) => tee_res = Some(t),
                    Err(e) => tee_refused = Some(e),
                }
                if !dir_given {
                    works.get_mut(&m).expect("m").question(
                        "tee_dir_not_on_drawing",
                        "branch direction not on the drawing: taken as the top (0 deg), confirm",
                    );
                }
            }
        }
    }
    // ---- fill the result of the main
    {
        let w = works.remove(&m).expect("main work");
        let mut r = res.remove(&m).expect("main res");
        let t = w.num("t");
        r.t_mm = t;
        r.length_mm = w.num("L");
        let mut w = w;
        if let Some(t_res) = &tee_res {
            r.rules = rules_of(t_res.rules);
            r.method = Some(t_res.method.to_string());
            r.certified = t_res.certified;
            let tt = t.unwrap_or(0.0);
            for (k, pl) in t_res.plates.iter().enumerate() {
                let mut pc = piece(
                    &format!("{}-{}", parts[m].no, k + 1),
                    "tee_main_plate",
                    pl.w,
                    pl.h,
                    pl.net_area,
                    tt,
                    1,
                );
                pc.outline = rect(pl.w, pl.h);
                pc.holes = pl
                    .holes
                    .iter()
                    .map(|hi| t_res.holes[*hi].pts.clone())
                    .collect();
                r.pattern.push(pc);
            }
            r.eps_rep = t_res
                .holes
                .iter()
                .map(|h| h.eps_rep)
                .fold(None, |a: Option<f64>, v| Some(a.map_or(v, |x| x.max(v))));
            r.eps_rep_certified =
                Some(t_res.holds.is_empty() && t_res.holes.iter().all(|h| h.eps_rep_certified));
            for h in &t_res.holds {
                let mut ho = hold_out(Level::Need, h.code, &[], "group");
                ho.message_en = h.message.clone();
                w.holds.push(ho);
            }
            for q in &t_res.questions {
                if *q == tee::TEE_X_QUESTION {
                    w.question("tee_x_not_on_drawing", q);
                }
            }
            set_masses(&mut r, t_res.mass_kg, None);
            let mk = t_res.mass_kg;
            bom_compare(&mut w, &mut r, mk, None);
        }
        let refusal = tee_refused.as_ref().map(refusal_of);
        let mut sealed_group_holds: Vec<HoldOut> = Vec::new();
        let r = seal(r, w, refusal);
        for h in &r.holds {
            if h.scope == "group" {
                sealed_group_holds.push(h.clone());
            }
        }
        res.insert(m, r);
        // the group holds of the main (seam crosses hole, lateral < 30, mismatch) go to every member
        for i in idxs.iter().filter(|i| **i != m) {
            if let Some(w) = works.get_mut(i) {
                for h in &sealed_group_holds {
                    if !w.holds.iter().any(|x| x.code == h.code) {
                        w.holds.push(h.clone());
                    }
                }
            }
        }
    }
    // ---- branches
    for b in &branches {
        let mut w = works.remove(b).expect("branch");
        let mut r = res.remove(b).expect("branch res");
        let mut refusal: Option<RefusalOut> = tee_refused.as_ref().map(refusal_of);
        if !incomplete[b] && refusal.is_none() {
            match signed_e(&w, "e") {
                Err(e) => refusal = Some(refusal_of(&e)),
                Ok(e) => {
                    let spec = BranchPieceSpec {
                        d: w.num("D").unwrap_or(0.0),
                        t: w.num("t").unwrap_or(0.0),
                        main_d: w.num("main_D").unwrap_or(0.0),
                        main_t: w.num("main_t").unwrap_or(0.0),
                        h: w.num("H").unwrap_or(0.0),
                        e,
                        mount: rules.mount,
                        angle_deg: w.num("branch_angle"),
                    };
                    match tee::branch_piece(&spec) {
                        Ok(bp) => {
                            r.rules = rules_of(bp.rules);
                            r.method = Some(bp.method.to_string());
                            r.certified = bp.certified;
                            r.t_mm = Some(spec.t);
                            r.length_mm = Some(spec.h);
                            let mut pc = piece(
                                &parts[*b].no,
                                "tee_branch_wrap",
                                bp.circ,
                                bp.longest,
                                bp.net_area,
                                spec.t,
                                1,
                            );
                            pc.outline = open(bp.outline.clone());
                            r.pattern.push(pc);
                            for f in &bp.flags {
                                // BranchPieceResult.flags are advisory in nf-core: here they are holds
                                w.holds.push(hold_out(Level::Need, f, &[], "part"));
                            }
                            set_masses(&mut r, bp.mass_kg, None);
                            bom_compare(&mut w, &mut r, bp.mass_kg, None);
                        }
                        Err(e) => refusal = Some(refusal_of(&e)),
                    }
                }
            }
        }
        if w.num("dir").is_none() && !incomplete[b] {
            w.question(
                "tee_dir_not_on_drawing",
                "branch direction not on the drawing: taken as the top (0 deg), confirm",
            );
        }
        let r = seal(r, w, refusal);
        res.insert(*b, r);
    }
    // ---- pads
    for pr in &pads {
        let mut w = works.remove(pr).expect("pad");
        let mut r = res.remove(pr).expect("pad res");
        let mut refusal: Option<RefusalOut> = None;
        if !incomplete[pr] {
            match signed_e(&w, "e") {
                Err(e) => refusal = Some(refusal_of(&e)),
                Ok(e) => {
                    let split = match w.whole("split") {
                        Some(Some(s)) => Some(s),
                        Some(None) => None,
                        None => None,
                    };
                    let outline = match (w.num("pad_D"), w.num("pad_B")) {
                        (Some(d), None) => PadOutline::Circle(d),
                        (None, Some(b)) => PadOutline::Width(b),
                        _ => PadOutline::Width(0.0),
                    };
                    let spec = PadSpec {
                        main_d: w.num("main_D").unwrap_or(0.0),
                        t: w.num("t").unwrap_or(0.0),
                        d: w.num("branch_D").unwrap_or(0.0),
                        e,
                        angle_deg: w.num("branch_angle"),
                        outline,
                        split,
                        cut: rules.pad_cut,
                    };
                    if !w.holds.iter().any(|h| h.code == "split_not_integer") {
                        match tee_pad::pad(&spec) {
                            Ok(p) => {
                                r.rules = rules_of(&tee_pad::PAD_NAMED_RULES);
                                r.method = Some(tee_pad::PAD_METHOD.to_string());
                                r.certified = p.certified;
                                r.eps_rep = Some(p.eps_rep);
                                r.eps_rep_certified = Some(p.eps_rep_certified);
                                r.t_mm = Some(spec.t);
                                if p.pieces.len() <= 1 || p.split <= 1 {
                                    let mut pc = piece(
                                        &parts[*pr].no,
                                        "tee_pad",
                                        p.w,
                                        p.h,
                                        p.net_area,
                                        spec.t,
                                        1,
                                    );
                                    pc.outline = open(p.outline.clone());
                                    pc.holes = vec![open(p.hole.clone())];
                                    r.pattern.push(pc);
                                } else {
                                    for (k, pp) in p.pieces.iter().enumerate() {
                                        let mut pc = piece(
                                            &format!("{}-{}", parts[*pr].no, k + 1),
                                            "tee_pad_piece",
                                            pp.w,
                                            pp.h,
                                            pp.area,
                                            spec.t,
                                            pp.copies,
                                        );
                                        pc.outline = open(pp.pts.clone());
                                        r.pattern.push(pc);
                                    }
                                }
                                let unit = if p.pieces.len() <= 1 || p.split <= 1 {
                                    p.mass_kg
                                } else {
                                    p.pieces_mass_kg
                                };
                                set_masses(&mut r, unit, Some(p.gross_box_mass_kg));
                                bom_compare(&mut w, &mut r, unit, Some(p.gross_box_mass_kg));
                            }
                            Err(e) => refusal = Some(refusal_of(&e)),
                        }
                    }
                }
            }
        }
        let r = seal(r, w, refusal);
        res.insert(*pr, r);
    }
    // ---- ribs
    let branch_angle =
        single_branch.and_then(|b| works.get(&b).and_then(|w| w.num("branch_angle")));
    let branch_angle = branch_angle.or_else(|| {
        // the rib's branch: the single branch of the group, else the main's own branch angle
        parts[m].values.get("branch_angle").and_then(Val::usable)
    });
    for rb in &ribs {
        let mut w = works.remove(rb).expect("rib");
        let mut r = res.remove(rb).expect("rib res");
        let mut refusal: Option<RefusalOut> = None;
        if !incomplete[rb] {
            // advisory refusal of nf-core becomes a blocking hold here (DECISIONS rib/flags line)
            if let Err(e) = tee::rib_on_branch(branch_angle) {
                if e.code == "tee_rib_on_lateral_unsupported" {
                    w.need(
                        Level::Need,
                        "tee_rib_on_lateral_unsupported",
                        &["branch_angle"],
                    );
                } else {
                    refusal = Some(refusal_of(&e));
                }
            } else {
                let g = |k: &str| w.num(k);
                let p = RibParams {
                    t: g("t"),
                    main_d: g("main_D"),
                    branch_d: g("branch_D"),
                    e: g("e"),
                    rib_b: g("rib_b"),
                    rib_h: g("rib_h"),
                    rib_top: g("rib_top"),
                    rib_top_stated: false,
                    rib_n1: g("rib_n1"),
                    rib_n2: g("rib_n2"),
                    table_qty: w.inp.table.as_ref().and_then(|t| t.qty),
                    table_mass: w.inp.table.as_ref().and_then(|t| t.mass),
                    table_mass_decimals: w.inp.table.as_ref().map_or(0, |t| t.mass_decimals),
                };
                let rp = order::check_tee_rib(&p);
                w.push_front(&rp.holds);
                r.rules = rules_of(&shapes::NAMED_RULES);
                r.method = Some("closed_form_rib_area".into());
                r.t_mm = p.t;
                let t = p.t.unwrap_or(0.0);
                let (mut kg, mut gross) = (0.0, 0.0);
                for (k, o) in rp.pieces.iter().enumerate() {
                    let (pw, ph) = match &o.geo {
                        RibGeo::Circle(s) => (s.b, s.h),
                        RibGeo::Triangle(s) => (s.w, s.h),
                    };
                    let mut pc = piece(
                        &format!("{}-{}", parts[*rb].no, k + 1),
                        "tee_rib",
                        pw,
                        ph,
                        o.area,
                        t,
                        o.n as u32,
                    );
                    pc.kg = Some(shapes::mass_kg(o.area, t));
                    r.pattern.push(pc);
                    kg += o.kg;
                    gross += o.gross_kg;
                }
                if !rp.pieces.is_empty() {
                    set_masses(&mut r, kg, Some(gross));
                    if let Some(h) = rp.holds.iter().find(|h| h.code == "mass_vs_table") {
                        let tm = p.table_mass.unwrap_or(0.0);
                        r.bom = Some(BomDelta {
                            table_mass_kg: tm,
                            calc_mass_kg: kg,
                            gross_mass_kg: Some(gross),
                            used: if (gross - tm).abs() < (kg - tm).abs() {
                                "gross"
                            } else {
                                "calc"
                            },
                            delta_kg: kg - tm,
                            delta_rel: if tm > 0.0 { (kg - tm).abs() / tm } else { 0.0 },
                            within_tolerance: h.level == Level::Ok,
                            tolerance_rel: order::MASS_TABLE_REL_TOL,
                        });
                    }
                }
            }
        }
        let r = seal(r, w, refusal);
        res.insert(*rb, r);
    }
    finalize_group(parts, BTreeMap::new(), res, gname, &[])
}

fn take<'a>(m: &mut BTreeMap<usize, Work<'a>>, k: usize) -> Work<'a> {
    m.remove(&k).expect("work present")
}

/// Parts of a failed group (no main) are sealed here from their own works.
fn finalize_group(
    parts: &[PartIn],
    works: BTreeMap<usize, Work<'_>>,
    mut res: BTreeMap<usize, PartResult>,
    gname: &str,
    _skip: &[usize],
) -> BTreeMap<usize, PartResult> {
    for (i, w) in works {
        let r = res.remove(&i).unwrap_or_else(|| blank(&parts[i], gname));
        let mut w = w;
        let shape = parts[i].shape.clone();
        let (need, _) = shape_params(&shape).unwrap_or((&[], &[]));
        missing_hold(&mut w, need);
        res.insert(i, seal(r, w, None));
    }
    res
}

fn compute_cone_group(
    parts: &[PartIn],
    idxs: &[usize],
    rules: &OrderRules,
    gname: &str,
) -> BTreeMap<usize, PartResult> {
    let mut works: BTreeMap<usize, Work<'_>> =
        idxs.iter().map(|i| (*i, Work::new(&parts[*i]))).collect();
    let mut res: BTreeMap<usize, PartResult> = idxs
        .iter()
        .map(|i| (*i, blank(&parts[*i], gname)))
        .collect();
    let cone = idxs
        .iter()
        .copied()
        .find(|i| matches!(parts[*i].shape.as_str(), "cone" | "eccentric_cone"))
        .expect("group has a cone");
    let branch: Option<usize> = idxs
        .iter()
        .copied()
        .find(|i| parts[*i].shape == "tee_branch");
    let ecc = parts[cone].shape == "eccentric_cone";
    for i in idxs {
        let shape = parts[*i].shape.clone();
        common_checks(works.get_mut(i).expect("w"), &shape);
    }
    // fill (fillCone): branch main_D / main_t from the cone, shared branch values both ways
    let mut mism = Vec::new();
    if let Some(b) = branch {
        let (mut wc, mut wb) = (take(&mut works, cone), take(&mut works, b));
        share(&mut wc, "D1", &mut wb, "main_D", &mut mism);
        share(&mut wc, "t", &mut wb, "main_t", &mut mism);
        share(&mut wc, "branch_D", &mut wb, "D", &mut mism);
        share(&mut wc, "branch_t", &mut wb, "t", &mut mism);
        for k in ["branch_x", "dir", "e", "e_side"] {
            share(&mut wc, k, &mut wb, k, &mut mism);
        }
        works.insert(cone, wc);
        works.insert(b, wb);
    }
    if !mism.is_empty() {
        let h = hold_out(Level::Need, "tee_mismatch", &mism, "group");
        for i in idxs {
            works.get_mut(i).expect("w").holds.push(h.clone());
        }
    }
    let cone_need: &[&str] = &["D1", "D2", "L", "t"];
    let has_branch_keys = {
        let w = &works[&cone];
        ["branch_D", "branch_t"]
            .iter()
            .any(|k| w.vals.get(*k).is_some_and(|v| v.value.is_some()))
    } || branch.is_some();
    let mut cone_bad = missing_hold(works.get_mut(&cone).expect("w"), cone_need);
    let mut branch_bad = false;
    if let Some(b) = branch {
        let (need, _) = shape_params("tee_branch").unwrap_or((&[], &[]));
        // main_D / main_t of a branch on a cone come from the cone
        branch_bad = missing_hold(works.get_mut(&b).expect("w"), need);
    } else if has_branch_keys {
        // a cone with branch keys but no branch part has no H
        let w = works.get_mut(&cone).expect("w");
        let h = hold_out(
            Level::Need,
            "conebranch_branch_incomplete",
            &["H".to_string()],
            "part",
        );
        w.holds.push(h);
        cone_bad = true;
        let _ = branch_bad;
    }
    if !has_branch_keys {
        // plain cone / eccentric reducer
        let mut w = works.remove(&cone).expect("w");
        let mut r = res.remove(&cone).expect("r");
        if !cone_bad {
            compute_cone(&mut w, &mut r, rules, ecc);
        }
        res.insert(cone, seal(r, w, None));
        return res;
    }
    // cone with a branch
    let mut cb_res: Option<conebranch::ConeBranchResult> = None;
    let mut refusal: Option<ShapeRefusal> = None;
    if !cone_bad
        && !branch_bad
        && let Some(b) = branch
    {
        let (wc, wb) = (&works[&cone], &works[&b]);
        match signed_e(wb, "e") {
            Err(e) => refusal = Some(e),
            Ok(e) => {
                let split = match wc.num("split") {
                    Some(v) if v.is_finite() && v >= 2.0 && v.fract() == 0.0 => Some(v as u32),
                    Some(v) => Some(if v.is_finite() { v as u32 } else { 0 }),
                    None => None,
                };
                let spec = ConeBranchSpec {
                    da: wc.num("D1").unwrap_or(0.0),
                    db: wc.num("D2").unwrap_or(0.0),
                    l: wc.num("L").unwrap_or(0.0),
                    t: wc.num("t").unwrap_or(0.0),
                    ecc,
                    flat_deg: wc.num("flat_side"),
                    d: wb.num("D").unwrap_or(0.0),
                    tb: wb.num("t").unwrap_or(0.0),
                    x: wb.num("branch_x"),
                    dir_deg: wb.num("dir").unwrap_or(0.0),
                    e,
                    h: wb.num("H").unwrap_or(0.0),
                    angle_deg: wb.num("branch_angle"),
                    mount: rules.mount,
                    split,
                    seam_gap: rules.seam_gap,
                };
                match conebranch::cone_branch_with(&spec, rules.cone_thickness) {
                    Ok(c) => cb_res = Some(c),
                    Err(e) => refusal = Some(e),
                }
            }
        }
    }
    let refusal_out = refusal.as_ref().map(refusal_of);
    let mut group_holds: Vec<HoldOut> = Vec::new();
    let mut group_qs: Vec<QuestionOut> = Vec::new();
    if let Some(c) = &cb_res {
        for h in &c.holds {
            let mut ho = hold_out(Level::Need, h.code, &[], "group");
            ho.message_en = h.message.clone();
            group_holds.push(ho);
        }
        for q in &c.questions {
            let code = if *q == conebranch::CB_X_QUESTION {
                "conebranch_x_not_on_drawing"
            } else if *q == conebranch::CB_FLAT_QUESTION {
                // the flat-side hold of the result carries the same wording; no second entry
                continue;
            } else {
                "conebranch_question"
            };
            group_qs.push(QuestionOut {
                code,
                message: (*q).to_string(),
            });
        }
    }
    // the cone part
    {
        let mut w = works.remove(&cone).expect("w");
        let mut r = res.remove(&cone).expect("r");
        r.t_mm = w.num("t");
        r.length_mm = w.num("L");
        if let Some(c) = &cb_res {
            fill_cb_common(&mut r, c);
            let t = w.num("t").unwrap_or(0.0);
            let mut unit = 0.0;
            for (k, p) in c.pieces.iter().enumerate() {
                let mut pc = piece(
                    &format!("{}-{}", parts[cone].no, k + 1),
                    "cone_branch_cone_piece",
                    p.w,
                    p.h,
                    p.net_area,
                    t,
                    p.copies,
                );
                pc.outline = open(p.outline.clone());
                if let Some(h) = &p.hole {
                    pc.holes = vec![open(h.clone())];
                }
                pc.kg = Some(p.mass_kg);
                unit += p.mass_kg * f64::from(p.copies);
                r.pattern.push(pc);
            }
            set_masses(&mut r, unit, None);
            for h in &group_holds {
                w.holds.push(h.clone());
            }
            for q in &group_qs {
                w.question(q.code, &q.message);
            }
            bom_compare(&mut w, &mut r, unit, None);
        } else if cone_bad || branch_bad {
            // held for missing values
        }
        res.insert(cone, seal(r, w, refusal_out.clone()));
    }
    if let Some(b) = branch {
        let mut w = works.remove(&b).expect("w");
        let mut r = res.remove(&b).expect("r");
        if let Some(c) = &cb_res {
            fill_cb_common(&mut r, c);
            let t = w.num("t").unwrap_or(0.0);
            r.t_mm = Some(t);
            r.length_mm = w.num("H");
            let mut pc = piece(
                &parts[b].no,
                "cone_branch_wrap",
                c.branch.circ,
                c.branch.longest,
                c.branch.net_area,
                t,
                1,
            );
            let mut ol = vec![[0.0, 0.0], [c.branch.circ, 0.0]];
            ol.extend(c.branch.cut.iter().rev().copied());
            pc.outline = ol;
            r.pattern.push(pc);
            set_masses(&mut r, c.branch.mass_kg, None);
            for h in &group_holds {
                w.holds.push(h.clone());
            }
            for q in &group_qs {
                w.question(q.code, &q.message);
            }
            bom_compare(&mut w, &mut r, c.branch.mass_kg, None);
        }
        res.insert(b, seal(r, w, refusal_out));
    }
    res
}

fn fill_cb_common(r: &mut PartResult, c: &conebranch::ConeBranchResult) {
    r.rules = rules_of(c.rules);
    r.method = Some(c.method.to_string());
    r.certified = c.certified;
    r.eps_rep = Some(c.hole.eps_rep);
    r.eps_rep_certified = Some(false);
    r.thickness_convention = Some(c.thickness_convention.name());
    r.mid_surface_dev = Some(c.mid_surface_dev);
    r.thickness_delta = Some(match &c.thickness_delta {
        Ok(d) => json!({
            "status": "available",
            "other": d.other.name(),
            "cut_shift_max_mm": d.cut_shift_max,
            "cut_shift_marks_mm": d.cut_shift_marks,
            "hole_len_this_mm": d.hole_len_this,
            "hole_len_other_mm": d.hole_len_other,
            "hole_len_delta_mm": d.hole_len_delta,
            "hole_mid_shift_mm": d.hole_mid_shift,
        }),
        Err(u) => json!({"status": "unavailable", "code": u.code, "message": u.message}),
    });
}

// ---------------------------------------------------------------- the order

/// Compute every part of an order. Pure and deterministic: the same parts and rules give the same bytes of JSON.
pub fn compute_order(parts: &[PartIn], rules: &OrderRules) -> OrderResult {
    let mut results: Vec<Option<PartResult>> = (0..parts.len()).map(|_| None).collect();
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, p) in parts.iter().enumerate() {
        groups.entry(group_of(parts, p)).or_default().push(i);
    }
    for (gname, idxs) in &groups {
        let has = |s: &[&str]| idxs.iter().any(|i| s.contains(&parts[*i].shape.as_str()));
        let tee_like = has(&["tee_main", "tee_branch", "tee_pad", "tee_rib"]);
        let cone_like = has(&["cone", "eccentric_cone"]);
        let cone_has_branch_keys = idxs.iter().any(|i| {
            matches!(parts[*i].shape.as_str(), "cone" | "eccentric_cone")
                && ["branch_D", "branch_t"]
                    .iter()
                    .any(|k| parts[*i].values.get(*k).is_some_and(|v| v.value.is_some()))
        });
        if cone_like
            && !has(&["tee_main"])
            && (cone_has_branch_keys || idxs.iter().any(|i| parts[*i].shape == "tee_branch"))
        {
            // a branch on a cone: the cone group; pads and ribs on a cone are not built
            let (c, others): (Vec<usize>, Vec<usize>) = idxs.iter().copied().partition(|i| {
                matches!(
                    parts[*i].shape.as_str(),
                    "cone" | "eccentric_cone" | "tee_branch"
                )
            });
            let out = compute_cone_group(parts, &c, rules, gname);
            for (i, r) in out {
                results[i] = Some(r);
            }
            for i in others {
                results[i] = Some(single(parts, i, rules, gname));
            }
            continue;
        }
        if tee_like {
            let (t, others): (Vec<usize>, Vec<usize>) = idxs.iter().copied().partition(|i| {
                matches!(
                    parts[*i].shape.as_str(),
                    "tee_main" | "tee_branch" | "tee_pad" | "tee_rib"
                )
            });
            let out = compute_tee_group(parts, &t, rules, gname);
            for (i, r) in out {
                results[i] = Some(r);
            }
            for i in others {
                results[i] = Some(single(parts, i, rules, gname));
            }
            continue;
        }
        for i in idxs {
            results[*i] = Some(single(parts, *i, rules, gname));
        }
    }
    OrderResult {
        engine: crate::ENGINE_VERSION,
        compute: COMPUTE_VERSION,
        rules: rules.rules_out(),
        parts: results
            .into_iter()
            .map(|r| r.expect("every part has a result"))
            .collect(),
    }
}

fn single(parts: &[PartIn], i: usize, rules: &OrderRules, gname: &str) -> PartResult {
    let p = &parts[i];
    let mut w = Work::new(p);
    let mut r = blank(p, gname);
    let Some((need, _)) = shape_params(&p.shape) else {
        w.need(Level::Need, "shape_not_computed", &[]);
        r.state = "not_computed";
        return seal(r, w, None);
    };
    match p.shape.as_str() {
        "purchased" | "other" => {
            let mut h = hold_out(Level::Info, "purchased_not_computed", &[], "part");
            h.message_en = text_of("purchased_not_computed").0;
            h.message_ja = text_of("purchased_not_computed").1;
            w.holds.push(h);
            r.state = "not_computed";
            return seal(r, w, None);
        }
        "wye" => {
            w.need(Level::Need, "shape_not_computed", &[]);
            r.state = "not_computed";
            return seal(r, w, None);
        }
        _ => {}
    }
    common_checks(&mut w, &p.shape);
    if missing_hold(&mut w, need) {
        return seal(r, w, None);
    }
    match p.shape.as_str() {
        "cylinder" => compute_cylinder(&mut w, &mut r, rules),
        "plate" => compute_plate(&mut w, &mut r, rules),
        "ring" => compute_ring(&mut w, &mut r),
        "cone" => compute_cone(&mut w, &mut r, rules, false),
        "eccentric_cone" => compute_cone(&mut w, &mut r, rules, true),
        "miter_bend" => compute_bend(&mut w, &mut r, rules),
        _ => {
            w.need(Level::Need, "tee_group_no_main", &[]);
        }
    }
    let refusal = r.refusal.take();
    seal(r, w, refusal)
}

// ---------------------------------------------------------------- material list and issue list data

/// Material list rows (nf-jobout types) of a computed order. A part with a Need hold, a question or a refusal has no
/// mass (provisional numbers never enter the list); every multi-piece part has one row per piece, and the table mass
/// is placed on the first row only.
pub fn material_rows(res: &OrderResult) -> Vec<nf_jobout::MaterialRow> {
    use nf_jobout::{MaterialRow, RowKind};
    let mut rows = Vec::new();
    for p in &res.parts {
        let purchased = matches!(p.shape.as_str(), "purchased" | "other");
        if p.pattern.is_empty() {
            rows.push(MaterialRow {
                no: p.no.clone(),
                name: p.name.clone(),
                shape: p.shape.clone(),
                kind: if purchased {
                    RowKind::Purchased
                } else {
                    RowKind::Piece
                },
                qty: Some(p.qty),
                thickness_mm: p.t_mm,
                length_mm: p.length_mm,
                note_en: if purchased {
                    "purchased or other part: not computed".into()
                } else {
                    format!("not computed ({})", p.state)
                },
                note_ja: if purchased {
                    "購入品・その他：計算していません".into()
                } else {
                    format!("未計算（{}）", p.state)
                },
                ..Default::default()
            });
            continue;
        }
        let multi = p.pattern.len() > 1;
        for (k, pc) in p.pattern.iter().enumerate() {
            let unit = pc.kg;
            rows.push(MaterialRow {
                no: if multi {
                    format!("{}#{}", p.no, k + 1)
                } else {
                    p.no.clone()
                },
                name: p.name.clone(),
                shape: p.shape.clone(),
                kind: RowKind::Piece,
                thickness_mm: p.t_mm,
                qty: Some(p.qty * pc.copies.max(1)),
                width_mm: pc.w,
                height_mm: pc.h,
                length_mm: p.length_mm,
                unit_mass_kg: if p.mass_provisional { None } else { unit },
                mass_calc_kg: if p.mass_provisional {
                    None
                } else {
                    unit.map(|u| u * f64::from(p.qty * pc.copies.max(1)))
                },
                mass_bom_kg: if k == 0 {
                    p.bom.as_ref().map(|b| b.table_mass_kg)
                } else {
                    None
                },
                note_en: if p.mass_provisional {
                    format!("held: mass not final ({})", p.dxf_blocked_by.join(", "))
                } else {
                    "pattern view only: no DXF (shape not certified)".into()
                },
                note_ja: if p.mass_provisional {
                    "保留：質量は未確定".into()
                } else {
                    "展開図の表示のみ：DXFなし（形が未認証）".into()
                },
                ..Default::default()
            });
        }
    }
    rows
}

/// Issue list entries: one per Need hold, per question, per refusal, and one "not certified" entry per part that has a
/// pattern but no DXF (the owner rule of 24 Nov: pattern view and issue entry instead of a DXF).
pub fn issues(res: &OrderResult) -> Vec<nf_jobout::Issue> {
    use nf_jobout::Issue;
    let mut out = Vec::new();
    for p in &res.parts {
        let mk = |code: &str, en: String, ja: String| Issue {
            no: p.no.clone(),
            name: p.name.clone(),
            code: code.to_string(),
            title_en: code.to_string(),
            title_ja: code.to_string(),
            message_en: en,
            message_ja: ja,
            ..Default::default()
        };
        for h in p.holds.iter().filter(|h| h.level == "need") {
            out.push(mk(&h.code, h.message_en.clone(), h.message_ja.clone()));
        }
        for q in &p.questions {
            out.push(mk(q.code, q.message.clone(), text_of(q.code).1));
        }
        if let Some(r) = &p.refusal {
            let (_, ja) = text_of("refused");
            out.push(mk(
                &r.code,
                r.message.clone(),
                format!("{ja}（{}）", r.code),
            ));
        }
        if p.pattern_view_only {
            let (en, ja) = text_of("not_certified");
            out.push(mk("not_certified", en, ja));
        }
    }
    out
}

/// Parts of the order for `nf_jobout::Part` (held = the first blocking code, when no DXF was made).
pub fn jobout_parts(res: &OrderResult) -> Vec<nf_jobout::Part> {
    res.parts
        .iter()
        .map(|p| nf_jobout::Part {
            no: p.no.clone(),
            name: p.name.clone(),
            shape: p.shape.clone(),
            qty: p.qty,
            thickness_mm: p.t_mm,
            held: p.dxf_blocked_by.first().cloned(),
            ..Default::default()
        })
        .collect()
}

/// The result as the JSON the compute endpoint returns.
pub fn to_json(res: &OrderResult) -> Value {
    serde_json::to_value(res).unwrap_or(Value::Null)
}

/// Fingerprint of a result's cut-relevant content (identity of "the same compute"): SHA-256 of the canonical JSON.
pub fn result_fingerprint(res: &OrderResult) -> String {
    use sha2::{Digest, Sha256};
    let v = to_json(res);
    hex::encode(Sha256::digest(v.to_string().as_bytes()))
}
