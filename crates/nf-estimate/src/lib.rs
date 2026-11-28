//! Quote and cost estimate from verified geometry (PAR-7).
//!
//! Input is the stored engine result of an order (`order_results.result_json`). Mass, cut length,
//! weld length and pierce count are geometry. Money is only the factory's own rates: every rate
//! defaults to unset, and an unset rate holds the parts that need it ("set your rates"). Only computed,
//! unheld parts are priced; held parts are listed as "not priced". Nothing here approves or releases.
//! Exports go through nf-jobout (CSV formula-injection guard, XLSX writer).

use nf_jobout::xlsx::{Cell, Sheet};
use nf_jobout::{cells_to_csv, marking};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::f64::consts::PI;

/// Named rule for the density (a physical value, not a price). UNVERIFIED until the factory confirms it.
pub const DENSITY_RULE: &str = "STEEL_DENSITY nominal 7850 kg/m3 (UNVERIFIED: factory to confirm its own density)";
const STEEL_DENSITY_KG_M3: f64 = 7850.0;

/// The factory's own rates. Zero or empty means "not set": the parts that need it are held.
#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Rates {
    pub currency: String,
    pub material_price_per_kg: f64,
    pub density_kg_m3: f64,
    /// Cut cost per metre by thickness: the first band with `t_mm <= max_t_mm` applies.
    pub cut_cost_per_m: Vec<CutBand>,
    pub weld_cost_per_m: f64,
    pub labour_rate_per_h: f64,
    pub labour_min_per_piece: f64,
    pub margin_pct: f64,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct CutBand {
    pub max_t_mm: f64,
    pub per_m: f64,
}

impl Default for Rates {
    fn default() -> Self {
        Rates {
            currency: String::new(),
            material_price_per_kg: 0.0,
            density_kg_m3: STEEL_DENSITY_KG_M3,
            cut_cost_per_m: Vec::new(),
            weld_cost_per_m: 0.0,
            labour_rate_per_h: 0.0,
            labour_min_per_piece: 0.0,
            margin_pct: 0.0,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Reason {
    pub en: String,
    pub ja: String,
}

fn reason(en: impl Into<String>, ja: impl Into<String>) -> Reason {
    Reason { en: en.into(), ja: ja.into() }
}

#[derive(Serialize, Clone, Debug)]
pub struct Costs {
    pub material: f64,
    pub cut: f64,
    pub weld: f64,
    pub labour: f64,
    pub subtotal: f64,
    pub margin: f64,
    pub price: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct PartEst {
    pub no: String,
    pub name: String,
    pub shape: String,
    pub qty: u32,
    pub t_mm: f64,
    pub pieces: u64,
    pub mass_kg: f64,
    pub cut_m: f64,
    pub weld_m: f64,
    pub pierces: u64,
    /// None when a rate needed by this part is not set (see `reason`).
    pub costs: Option<Costs>,
    pub reason: Option<Reason>,
}

#[derive(Serialize, Clone, Debug)]
pub struct NotPriced {
    pub no: String,
    pub name: String,
    pub reason: Reason,
}

/// Sums over one group of parts. Counts are u64 and saturate; money stays zero in the unpriced group.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Sums {
    pub parts: u64,
    pub mass_kg: f64,
    pub cut_m: f64,
    pub weld_m: f64,
    pub pierces: u64,
    pub pieces: u64,
    pub subtotal: f64,
    pub margin: f64,
    pub price: f64,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Totals {
    /// Computed parts with costs (every needed rate set, part not held).
    pub priced: Sums,
    /// Computed parts with geometry but no costs (a needed rate is unset). Geometry only; no money.
    pub unpriced: Sums,
}

#[derive(Serialize, Clone, Debug)]
pub struct Estimate {
    /// "draft" when every computed part is priced and no rate is unset; otherwise "incomplete".
    pub status: &'static str,
    pub currency: String,
    pub density_rule: &'static str,
    pub parts: Vec<PartEst>,
    pub not_priced: Vec<NotPriced>,
    /// Rates the factory has not set yet (names as in `Rates`).
    pub rates_unset: Vec<&'static str>,
    pub totals: Totals,
    /// What the weld length counts (cylinder seams only; a lower bound).
    pub weld_basis: Reason,
    pub marking_en: String,
    pub marking_ja: String,
}

#[derive(Deserialize)]
struct OrderIn {
    #[serde(default)]
    parts: Vec<PartIn>,
}

#[derive(Deserialize)]
struct PartIn {
    no: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    shape: String,
    #[serde(default)]
    qty: u32,
    state: String,
    #[serde(default)]
    holds: Vec<Value>,
    #[serde(default)]
    questions: Vec<Value>,
    #[serde(default)]
    mass_provisional: bool,
    t_mm: Option<f64>,
    length_mm: Option<f64>,
    #[serde(default)]
    pattern: Vec<PieceIn>,
}

#[derive(Deserialize)]
struct PieceIn {
    area_mm2: Option<f64>,
    #[serde(default)]
    copies: u32,
    #[serde(default)]
    outline: Vec<[f64; 2]>,
    #[serde(default)]
    holes: Vec<Vec<[f64; 2]>>,
    /// centre x, centre y, radius
    #[serde(default)]
    circles: Vec<[f64; 3]>,
}

/// Length of a closed polyline (the last point joins back to the first); open input is closed here.
fn contour_mm(pts: &[[f64; 2]]) -> f64 {
    if pts.len() < 2 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..pts.len() {
        let a = pts[i];
        let b = pts[(i + 1) % pts.len()];
        s += ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt();
    }
    s
}

/// Why a computed part is not priced at all (held, refused, provisional, or no usable geometry).
fn hold_reason(p: &PartIn) -> Option<Reason> {
    if p.state != "computed" {
        return Some(reason(
            format!("not priced: state {}", p.state),
            format!("未計算のため見積対象外（状態 {}）", p.state),
        ));
    }
    // Info-level holds (e.g. "stock_checked") are notes; only need-level holds keep a part out.
    let blocking = p.holds.iter().filter(|h| h.get("level").and_then(Value::as_str) != Some("info"));
    if let Some(code) = blocking.clone().find_map(|h| h.get("code").and_then(Value::as_str)) {
        return Some(reason(format!("not priced: hold {code}"), format!("保留のため見積対象外: {code}")));
    }
    if let Some(code) = p.questions.iter().find_map(|q| q.get("code").and_then(Value::as_str)) {
        return Some(reason(format!("not priced: open question {code}"), format!("未回答の質問のため見積対象外: {code}")));
    }
    if p.mass_provisional {
        return Some(reason("not priced: provisional result (open question)", "暫定結果のため見積対象外（未回答の質問）"));
    }
    if p.pattern.is_empty() {
        return Some(reason("not priced: no pattern (purchased or not computed)", "展開なし（購入品または未計算）のため見積対象外"));
    }
    match p.t_mm {
        Some(t) if t > 0.0 => {}
        _ => return Some(reason("not priced: thickness missing", "板厚がないため見積対象外")),
    }
    if p.pattern.iter().any(|pc| pc.area_mm2.is_none()) {
        return Some(reason("not priced: area not computed", "面積が未計算のため見積対象外"));
    }
    None
}

/// Rates that are unset (zero or empty). Any of these holds every computed part.
pub fn unset_rates(r: &Rates) -> Vec<&'static str> {
    let mut u = Vec::new();
    if r.currency.trim().is_empty() {
        u.push("currency");
    }
    if r.density_kg_m3 <= 0.0 {
        u.push("density_kg_m3");
    }
    if r.material_price_per_kg <= 0.0 {
        u.push("material_price_per_kg");
    }
    if r.cut_cost_per_m.is_empty() {
        u.push("cut_cost_per_m");
    }
    if r.labour_rate_per_h <= 0.0 {
        u.push("labour_rate_per_h");
    }
    if r.labour_min_per_piece <= 0.0 {
        u.push("labour_min_per_piece");
    }
    if r.margin_pct <= 0.0 {
        u.push("margin_pct");
    }
    u
}

struct Geo {
    t: f64,
    pieces: u64,
    mass_kg: f64,
    cut_mm: f64,
    weld_mm: f64,
    pierces: u64,
}

fn geometry(p: &PartIn, r: &Rates) -> Geo {
    let t = p.t_mm.unwrap_or(0.0);
    let mut g = Geo { t, pieces: 0, mass_kg: 0.0, cut_mm: 0.0, weld_mm: 0.0, pierces: 0 };
    for pc in &p.pattern {
        let copies = pc.copies as u64;
        let c = copies as f64;
        let area = pc.area_mm2.unwrap_or(0.0);
        // mm3 * (kg/m3) * 1e-9 = kg
        g.mass_kg += area * t * r.density_kg_m3 * 1e-9 * c;
        let circles: f64 = pc.circles.iter().map(|[_, _, rad]| 2.0 * PI * rad).sum();
        let holes: f64 = pc.holes.iter().map(|h| contour_mm(h)).sum();
        g.cut_mm += (contour_mm(&pc.outline) + holes + circles) * c;
        let outlets = 1 + pc.holes.len() as u64 + pc.circles.len() as u64;
        g.pierces = g.pierces.saturating_add(outlets.saturating_mul(copies));
        g.pieces = g.pieces.saturating_add(copies);
    }
    // One longitudinal seam per rolled cylinder shell (lower bound: split joints are not in the result).
    if p.shape == "cylinder" {
        g.weld_mm = p.length_mm.unwrap_or(0.0) * p.qty.max(1) as f64;
    }
    g
}

fn price(g: &Geo, r: &Rates, bands: &[CutBand]) -> Result<Costs, Reason> {
    let band = bands.iter().filter(|b| g.t <= b.max_t_mm).min_by(|a, b| a.max_t_mm.total_cmp(&b.max_t_mm));
    let Some(band) = band else {
        return Err(reason(
            format!("set your rates: no cut cost for {} mm thickness", g.t),
            format!("板厚 {} mm の切断単価が未設定", g.t),
        ));
    };
    let weld_m = g.weld_mm / 1000.0;
    if weld_m > 0.0 && r.weld_cost_per_m <= 0.0 {
        return Err(reason("set your rates: weld_cost_per_m", "溶接単価が未設定"));
    }
    let material = g.mass_kg * r.material_price_per_kg;
    let cut = g.cut_mm / 1000.0 * band.per_m;
    let weld = weld_m * r.weld_cost_per_m;
    let labour = g.pieces as f64 * r.labour_min_per_piece / 60.0 * r.labour_rate_per_h;
    let subtotal = material + cut + weld + labour;
    let margin = subtotal * r.margin_pct / 100.0;
    Ok(Costs { material, cut, weld, labour, subtotal, margin, price: subtotal + margin })
}

/// Refuses negative or non-finite rates: a negative price is never a valid quote input.
pub fn validate_rates(r: &Rates) -> Result<(), String> {
    let mut vals: Vec<(&str, f64)> = vec![
        ("density_kg_m3", r.density_kg_m3),
        ("material_price_per_kg", r.material_price_per_kg),
        ("weld_cost_per_m", r.weld_cost_per_m),
        ("labour_rate_per_h", r.labour_rate_per_h),
        ("labour_min_per_piece", r.labour_min_per_piece),
        ("margin_pct", r.margin_pct),
    ];
    for b in &r.cut_cost_per_m {
        vals.push(("cut_cost_per_m.max_t_mm", b.max_t_mm));
        vals.push(("cut_cost_per_m.per_m", b.per_m));
    }
    for (name, v) in vals {
        if !v.is_finite() || v < 0.0 {
            return Err(format!("rate {name} must be a finite number >= 0"));
        }
    }
    Ok(())
}

/// The weld length basis, stated on every output (EN, JA).
pub fn weld_basis() -> Reason {
    reason(
        "Weld length counts cylinder longitudinal seams only (lower bound; other welds are not included).",
        "溶接長は円筒部材の縦継ぎ目のみを数えます（下限値。その他の溶接は含みません）。",
    )
}

/// Build the estimate from a stored engine result (JSON of `nf_core::order_compute::OrderResult`).
pub fn estimate(order: &Value, rates: &Rates) -> Result<Estimate, String> {
    validate_rates(rates)?;
    let o: OrderIn = serde_json::from_value(order.clone()).map_err(|e| format!("order result: {e}"))?;
    let unset = unset_rates(rates);
    let unset_names = unset.join(", ");
    let mut bands = rates.cut_cost_per_m.clone();
    bands.sort_by(|a, b| a.max_t_mm.total_cmp(&b.max_t_mm));
    let mut est = Estimate {
        status: "incomplete",
        currency: rates.currency.clone(),
        density_rule: DENSITY_RULE,
        parts: Vec::new(),
        not_priced: Vec::new(),
        rates_unset: unset.clone(),
        totals: Totals::default(),
        weld_basis: weld_basis(),
        marking_en: marking(&None, false),
        marking_ja: marking(&None, true),
    };
    for p in &o.parts {
        if let Some(rs) = hold_reason(p) {
            est.not_priced.push(NotPriced { no: p.no.clone(), name: p.name.clone(), reason: rs });
            continue;
        }
        let g = geometry(p, rates);
        let costs = if !unset.is_empty() {
            Err(reason(
                format!("set your rates: {unset_names}"),
                format!("単価が未設定: {unset_names}"),
            ))
        } else {
            price(&g, rates, &bands)
        };
        let (costs, rs) = match costs {
            Ok(c) => (Some(c), None),
            Err(rs) => (None, Some(rs)),
        };
        let sums = if costs.is_some() { &mut est.totals.priced } else { &mut est.totals.unpriced };
        sums.parts += 1;
        sums.mass_kg += g.mass_kg;
        sums.cut_m += g.cut_mm / 1000.0;
        sums.weld_m += g.weld_mm / 1000.0;
        sums.pierces = sums.pierces.saturating_add(g.pierces);
        sums.pieces = sums.pieces.saturating_add(g.pieces);
        if let Some(c) = &costs {
            sums.subtotal += c.subtotal;
            sums.margin += c.margin;
            sums.price += c.price;
        }
        est.parts.push(PartEst {
            no: p.no.clone(),
            name: p.name.clone(),
            shape: p.shape.clone(),
            qty: p.qty,
            t_mm: g.t,
            pieces: g.pieces,
            mass_kg: g.mass_kg,
            cut_m: g.cut_mm / 1000.0,
            weld_m: g.weld_mm / 1000.0,
            pierces: g.pierces,
            costs,
            reason: rs,
        });
    }
    est.status = if est.not_priced.is_empty() && est.totals.unpriced.parts == 0 && !est.parts.is_empty() {
        "draft"
    } else {
        "incomplete"
    };
    Ok(est)
}

fn n(v: f64, d: usize) -> Cell {
    Cell::Num(format!("{:.*}", d, v))
}

fn t(s: &str) -> Cell {
    Cell::Text(s.to_string())
}

/// Table rows of the quote in EN or JA headers. Text cells pass the CSV formula guard in nf-jobout.
pub fn rows(e: &Estimate, ja: bool) -> Vec<Vec<Cell>> {
    let head = if ja {
        ["部材番号", "部材名", "形状", "数量", "板厚 (mm)", "枚数", "質量 (kg)", "切断長 (m)", "溶接長 (m)", "穴明け回数", "材料費", "切断費", "溶接費", "工賃", "小計", "利益", "見積額", "備考"]
    } else {
        ["No", "Name", "Shape", "Qty", "t (mm)", "Pieces", "Mass (kg)", "Cut (m)", "Weld (m)", "Pierces", "Material", "Cut cost", "Weld cost", "Labour", "Subtotal", "Margin", "Price", "Note"]
    };
    let mut out = vec![head.iter().map(|h| t(h)).collect::<Vec<_>>()];
    let note = |r: &Reason| if ja { r.ja.clone() } else { r.en.clone() };
    for p in &e.parts {
        let mut row = vec![t(&p.no), t(&p.name), t(&p.shape), n(p.qty as f64, 0), n(p.t_mm, 2), n(p.pieces as f64, 0), n(p.mass_kg, 3), n(p.cut_m, 3), n(p.weld_m, 3), n(p.pierces as f64, 0)];
        match (&p.costs, &p.reason) {
            (Some(c), _) => row.extend([n(c.material, 2), n(c.cut, 2), n(c.weld, 2), n(c.labour, 2), n(c.subtotal, 2), n(c.margin, 2), n(c.price, 2), t("")]),
            (None, r) => {
                row.extend(std::iter::repeat_n(Cell::Empty, 7));
                row.push(t(&r.as_ref().map(note).unwrap_or_default()));
            }
        }
        out.push(row);
    }
    for h in &e.not_priced {
        let mut row = vec![t(&h.no), t(&h.name)];
        row.extend(std::iter::repeat_n(Cell::Empty, 15));
        row.push(t(&note(&h.reason)));
        out.push(row);
    }
    out.push(vec![]);
    let (pl, ul) = if ja {
        ("合計（見積済み部材）", "計算済み・未見積（単価未設定）: 形状数値のみ、金額なし")
    } else {
        ("Total: priced parts", "Computed, not priced (rate unset): geometry only, no money")
    };
    let sum_row = |label: &str, s: &Sums, money: bool| {
        let mut r = vec![t(label), t(""), t(""), t(""), t(""), n(s.pieces as f64, 0), n(s.mass_kg, 3), n(s.cut_m, 3), n(s.weld_m, 3), n(s.pierces as f64, 0)];
        if money {
            r.extend([Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty, n(s.subtotal, 2), n(s.margin, 2), n(s.price, 2), t("")]);
        } else {
            r.extend(std::iter::repeat_n(Cell::Empty, 8));
        }
        r
    };
    out.push(sum_row(pl, &e.totals.priced, true));
    out.push(sum_row(ul, &e.totals.unpriced, false));
    let status = match (ja, e.status) {
        (false, s) => format!("Status: {s}"),
        (true, "draft") => "状態: 下書き".to_string(),
        (true, _) => "状態: 不完全（未見積の部材または未設定の単価あり）".to_string(),
    };
    out.push(vec![t(&status)]);
    out.push(vec![t(if ja { &e.weld_basis.ja } else { &e.weld_basis.en })]);
    out.push(vec![t(if ja { "通貨" } else { "Currency" }), t(if e.currency.is_empty() { "(not set)" } else { &e.currency })]);
    out.push(vec![t(if ja { &e.marking_ja } else { &e.marking_en })]);
    out
}

pub fn csv(e: &Estimate, ja: bool) -> Vec<u8> {
    cells_to_csv(&rows(e, ja))
}

pub fn xlsx(e: &Estimate) -> Result<Vec<u8>, nf_jobout::ZipError> {
    nf_jobout::xlsx::write_xlsx(&[
        Sheet { name: "Quote".into(), rows: rows(e, false) },
        Sheet { name: "見積".into(), rows: rows(e, true) },
    ])
}

/// Plain EN then JA summary. Figures only; no release or approval wording.
pub fn summary_text(e: &Estimate) -> String {
    let price = |v: f64| if e.rates_unset.is_empty() { format!("{v:.2} {}", e.currency) } else { "-".into() };
    let set = e.rates_unset.join(", ");
    let (p, u) = (&e.totals.priced, &e.totals.unpriced);
    format!(
        "Quote draft: {} ({})\nComputed parts: {}; priced: {}; computed but not priced (rate unset): {}; held: {}\nPriced parts: mass {:.3} kg, cut {:.3} m, weld {:.3} m, pierces {}\nComputed, not priced: mass {:.3} kg, cut {:.3} m, weld {:.3} m, pierces {}\nTotal price (priced parts): {}{}\nWeld: {}\n\n見積下書き: {}\n計算済み部材: {}、見積済み: {}、計算済み・未見積（単価未設定）: {}、見積対象外（保留）: {}\n見積済み部材: 質量 {:.3} kg、切断長 {:.3} m、溶接長 {:.3} m、穴明け {}\n計算済み・未見積: 質量 {:.3} kg、切断長 {:.3} m、溶接長 {:.3} m、穴明け {}\n見積額（見積済み部材）: {}{}\n溶接長: {}\n",
        e.status,
        e.marking_en,
        e.parts.len(),
        p.parts,
        u.parts,
        e.not_priced.len(),
        p.mass_kg,
        p.cut_m,
        p.weld_m,
        p.pierces,
        u.mass_kg,
        u.cut_m,
        u.weld_m,
        u.pierces,
        price(p.price),
        if set.is_empty() { String::new() } else { format!(" (rates not set: {set})") },
        e.weld_basis.en,
        e.status,
        e.parts.len(),
        p.parts,
        u.parts,
        e.not_priced.len(),
        p.mass_kg,
        p.cut_m,
        p.weld_m,
        p.pierces,
        u.mass_kg,
        u.cut_m,
        u.weld_m,
        u.pierces,
        price(p.price),
        if set.is_empty() { String::new() } else { format!("（未設定の単価: {set}）") },
        e.weld_basis.ja,
    )
}
