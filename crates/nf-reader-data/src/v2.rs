//! Generator v2: 11 generator-owned families (plates, rings, brackets, flat sheets, ducts, tanks, bends, tees, wyes,
//! branch-on-cone, reducers) plus the 5 engine families of v1, with weld and surface-finish symbols, title block,
//! BOM and general notes, and five scan kinds (clean, scan, fax, faint, hand). Truth schema `nf-reader-data/2`.
//! A printed number is either an engine value (v1 families, `Source::Request` etc.), a generator-owned spec
//! parameter (`Source::Spec`, stored under `spec.params`) or a closed-form value of stored inputs
//! (`Source::Derived`, see `derive.rs`). `resolve_v2` re-derives every spec and derived value from the truth line.

use crate::derive::{Inputs, derive};
use crate::layout::{Anchor, B, FONT, Item, Lang, Page, Style, esc};
use crate::params::sample;
use crate::render::{Fx, render_pdf, render_png, svg};
use crate::rng::{Rng, mix};
use crate::run::{Spec, build};
use crate::source::{Source, linear_tolerance, round_to};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub const SCHEMA: &str = "nf-reader-data/2";
pub const NEW_FAMILIES: [&str; 11] = [
    "plate_holes",
    "ring_flange",
    "bracket_gusset",
    "flat_sheet",
    "duct_transition",
    "tank_shell",
    "mitred_bend",
    "tee_lateral",
    "wye",
    "branch_on_cone",
    "reducer_concentric",
];
pub const ENGINE_FAMILIES: [&str; 5] = [
    "cylinder_shell",
    "cone_frustum",
    "eccentric_reducer",
    "branch_stub",
    "header_with_branch",
];
pub const DEGS: [&str; 5] = ["clean", "scan", "fax", "faint", "hand"];

/// All families of the v2 set in stratification order (engine first, then generator-owned).
pub fn all_families() -> Vec<&'static str> {
    ENGINE_FAMILIES
        .iter()
        .chain(NEW_FAMILIES.iter())
        .copied()
        .collect()
}

fn fam_code(f: &str) -> &'static str {
    match f {
        "cylinder_shell" => "CS",
        "cone_frustum" => "CF",
        "eccentric_reducer" => "ER",
        "branch_stub" => "BS",
        "header_with_branch" => "HB",
        "plate_holes" => "PL",
        "ring_flange" => "RF",
        "bracket_gusset" => "BK",
        "flat_sheet" => "FS",
        "duct_transition" => "DT",
        "tank_shell" => "TK",
        "mitred_bend" => "MB",
        "tee_lateral" => "TE",
        "wye" => "WY",
        "branch_on_cone" => "BC",
        _ => "RD",
    }
}

fn names(fam: &str, lang: Lang) -> &'static str {
    let (en, ja) = match fam {
        "plate_holes" => ("PLATE WITH HOLES", "穴あきプレート"),
        "ring_flange" => ("FLANGE RING", "リングフランジ"),
        "bracket_gusset" => ("GUSSET BRACKET", "ガセット付きブラケット"),
        "flat_sheet" => ("FLAT PATTERN", "展開図"),
        "duct_transition" => ("DUCT TRANSITION", "角から丸へのダクト"),
        "tank_shell" => ("TANK SHELL", "タンク胴"),
        "mitred_bend" => ("MITRED BEND", "マイター曲がり管"),
        "tee_lateral" => ("LATERAL TEE", "支管付きティー"),
        "wye" => ("WYE", "ワイ継手"),
        "branch_on_cone" => ("BRANCH ON CONE", "円錐の枝管"),
        "reducer_concentric" => ("CONCENTRIC REDUCER", "同心レデューサ"),
        "cylinder_shell" => ("CYLINDER SHELL", "円筒胴"),
        "cone_frustum" => ("CONE FRUSTUM", "円錐台"),
        "eccentric_reducer" => ("ECCENTRIC REDUCER", "偏心レデューサ"),
        "branch_stub" => ("BRANCH STUB", "枝管"),
        _ => ("HEADER WITH BRANCH", "主管と枝管"),
    };
    if lang == Lang::En { en } else { ja }
}

/// Printed text of a plain number (rounding and degree sign as in `Page::num`, no tolerance).
fn num_text(prefix: &str, v: f64, unit: &str, dec: usize) -> String {
    let mut t = format!("{prefix}{:.dec$}", round_to(v, dec));
    if unit == "deg" {
        t.push('\u{00B0}');
    }
    t
}

/// Proper intersection test of two segments [x0,y0,x1,y1].
fn seg_intersect(a: [f64; 4], b: [f64; 4]) -> bool {
    let o = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let (p1, p2) = ((a[0], a[1]), (a[2], a[3]));
    let (p3, p4) = ((b[0], b[1]), (b[2], b[3]));
    o(p3, p4, p1) * o(p3, p4, p2) < 0.0 && o(p1, p2, p3) * o(p1, p2, p4) < 0.0
}

const CELL_HALF: f64 = 0.32;

fn rot_box(b: B, th: f64, cx: f64, cy: f64) -> B {
    let (s, c) = th.to_radians().sin_cos();
    let mut r = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for (x, y) in [(b[0], b[1]), (b[2], b[1]), (b[2], b[3]), (b[0], b[3])] {
        let (dx, dy) = (x - cx, y - cy);
        let (nx, ny) = (cx + dx * c - dy * s, cy + dx * s + dy * c);
        r[0] = r[0].min(nx);
        r[1] = r[1].min(ny);
        r[2] = r[2].max(nx);
        r[3] = r[3].max(ny);
    }
    r
}

fn r1(b: B) -> Value {
    json!([
        (b[0] * 10.0).round() / 10.0,
        (b[1] * 10.0).round() / 10.0,
        (b[2] * 10.0).round() / 10.0,
        (b[3] * 10.0).round() / 10.0
    ])
}

/// Vertices of a drawn circle (a multiple of 4, so the extreme points are exact).
const CIRCLE_PTS: usize = 96;

/// Generator mix. `Default` is the original distribution; `Weak1` weights the failure modes of the zero-shot 8B
/// (error analysis, 50 test pages): dense small dimension labels, long BOM tables, and negative-example clutter
/// (title-block fields, revision tables, grid references, numbered notes, part and drawing numbers) that is drawn
/// but never in the answer key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mix {
    #[default]
    Default,
    Weak1,
}

impl Mix {
    pub fn from_name(s: &str) -> Option<Mix> {
        match s {
            "default" => Some(Mix::Default),
            "weak1" => Some(Mix::Weak1),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Mix::Default => "default",
            Mix::Weak1 => "weak1",
        }
    }
}

/// Generator-owned page under construction: the page canvas, parameters, and the shape label of each placed item.
pub struct Ctx {
    pub p: Page,
    pub r: Rng,
    pub par: BTreeMap<String, f64>,
    pub sc: f64,
    pub ox: f64,
    pub oy: f64,
    pub part: String,
    pub shape: String,
    pub shapes: Vec<String>,
    pub gtol: char,
    pub u: f64,
    pub fs: f64,
    pub w: f64,
    pub h: f64,
    /// Flat views that the engine refused (no view drawn).
    pub refused: usize,
    /// Linear dimensions whose drawn extent disagrees with the label (beyond one pixel).
    pub geom_fail: usize,
    /// Polylines (mm, current view) drawn since the last `fit`: the geometry the drawn-vs-label checks measure.
    pub vpolys: Vec<Vec<(f64, f64)>>,
    /// Set while a label is deliberately not tied to one drawn element (checked explicitly or not geometric).
    pub skip_chk: bool,
    /// Weak1 mix: title-block and note text is clutter (not in the key), the BOM has many rows.
    pub weak: bool,
    pub weak_bom: bool,
    /// Distance of a leader label from its target (px).
    pub lead_d: f64,
}

impl Ctx {
    fn set(&mut self, k: &str, v: f64) {
        self.par.insert(k.to_string(), v);
    }
    fn px(&self, x: f64, y: f64) -> (f64, f64) {
        (self.ox + x * self.sc, self.oy - y * self.sc)
    }
    /// Fit mm bounding box `bb` into cell `cell` (px), centred, leaving room for dimensions.
    fn fit(&mut self, cell: [f64; 4], bb: [f64; 4]) {
        self.vpolys.clear();
        let m = 110.0 * self.u;
        let cw = (cell[2] - cell[0] - 2.0 * m).max(10.0);
        let ch = (cell[3] - cell[1] - 2.0 * m).max(10.0);
        let bw = (bb[2] - bb[0]).max(1.0);
        let bh = (bb[3] - bb[1]).max(1.0);
        self.sc = (cw / bw).min(ch / bh);
        let (cxp, cyp) = ((cell[0] + cell[2]) / 2.0, (cell[1] + cell[3]) / 2.0);
        let (cxm, cym) = ((bb[0] + bb[2]) / 2.0, (bb[1] + bb[3]) / 2.0);
        self.ox = cxp - cxm * self.sc;
        self.oy = cyp + cym * self.sc;
    }
    fn cell(&self, i: usize) -> [f64; 4] {
        let u = self.u;
        match i {
            0 => [30.0 * u, 30.0 * u, CELL_HALF * self.w, self.h - 30.0 * u],
            1 => [
                CELL_HALF * self.w,
                30.0 * u,
                0.62 * self.w,
                self.h - 30.0 * u,
            ],
            _ => [30.0 * u, 30.0 * u, 0.62 * self.w, self.h - 30.0 * u],
        }
    }
    fn outline(&mut self, pts: &[(f64, f64)], fill: &str) {
        self.vpolys.push(pts.to_vec());
        let q: Vec<(f64, f64)> = pts.iter().map(|&(x, y)| self.px(x, y)).collect();
        self.p.poly(&q, true, 1.6 * self.u, fill);
    }
    fn circle(&mut self, cx: f64, cy: f64, r: f64) {
        let pts: Vec<(f64, f64)> = (0..CIRCLE_PTS)
            .map(|k| {
                let a = k as f64 * std::f64::consts::TAU / CIRCLE_PTS as f64;
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        self.outline(&pts, "none");
    }
    fn mline(&mut self, a: (f64, f64), b: (f64, f64)) {
        self.vpolys.push(vec![a, b]);
        let (x0, y0) = self.px(a.0, a.1);
        let (x1, y1) = self.px(b.0, b.1);
        self.p.line(x0, y0, x1, y1, 1.2 * self.u);
    }
    fn cands(&self, x: f64, y: f64, d: f64) -> Vec<(f64, f64, Anchor)> {
        let f = self.fs;
        vec![
            (x + d, y + 0.35 * f, Anchor::Start),
            (x - d, y + 0.35 * f, Anchor::End),
            (x, y - d, Anchor::Middle),
            (x, y + d + 0.7 * f, Anchor::Middle),
            (x + d, y - d * 0.5, Anchor::Start),
            (x - d, y - d * 0.5, Anchor::End),
            (x + d, y + d, Anchor::Start),
            (x - d, y + d, Anchor::End),
        ]
    }
    #[allow(clippy::too_many_arguments)]
    fn put_num(
        &mut self,
        kind: &'static str,
        prefix: &str,
        v: f64,
        unit: &'static str,
        tol: Option<f64>,
        src: Source,
        c: &[(f64, f64, Anchor)],
    ) -> Option<B> {
        let part = self.part.clone();
        for &(x, y, a) in c {
            if let Some(b) = self.p.num(
                kind,
                &part,
                prefix,
                v,
                unit,
                tol,
                src.clone(),
                x,
                y,
                a,
                self.fs,
                true,
            ) {
                let s = self.shape.clone();
                self.shapes.push(s);
                return Some(b);
            }
        }
        None
    }
    #[allow(clippy::too_many_arguments)]
    fn put_word(
        &mut self,
        kind: &'static str,
        s: &str,
        what: &str,
        x: f64,
        y: f64,
        a: Anchor,
        bold: bool,
        fs: f64,
    ) -> Option<B> {
        let part = self.part.clone();
        let b = self.p.word(kind, s, &part, what, x, y, a, fs, bold)?;
        let sh = self.shape.clone();
        self.shapes.push(sh);
        Some(b)
    }
    /// Drawn extent (mm, from the drawn endpoints) against the printed value: one pixel plus 0.5 %.
    fn check_extent(&mut self, drawn_mm: f64, v: f64) {
        let tol = 1.0 / self.sc + 0.005 * v.abs();
        if (drawn_mm - v).abs() > tol {
            self.geom_fail += 1;
        }
    }
    /// A diameter/width label must match a drawn element of the current view: a polyline whose bounding box side or
    /// one of whose edges has that length (one pixel plus 0.5 %).
    fn check_size(&mut self, v: f64) {
        let tol = 1.0 / self.sc + 0.005 * v.abs();
        let ok = self.vpolys.iter().any(|p| {
            let b = bbox(p);
            let closed = p.len() > 2;
            let n = p.len();
            (b[2] - b[0] - v).abs() <= tol
                || (b[3] - b[1] - v).abs() <= tol
                || (0..if closed { n } else { n - 1 }).any(|i| {
                    let (a, c) = (p[i], p[(i + 1) % n]);
                    ((a.0 - c.0).hypot(a.1 - c.1) - v).abs() <= tol
                })
        });
        if !ok {
            self.geom_fail += 1;
        }
    }
    /// An angle label must match the angle between two drawn straight edges of the current view, or between an edge
    /// and the page axes (either the angle or its supplement; 0.25 deg plus 0.3 %).
    fn check_angle(&mut self, v: f64) {
        let mut dirs: Vec<f64> = vec![0.0, 90.0];
        for p in self.vpolys.iter().filter(|p| p.len() != CIRCLE_PTS) {
            let n = p.len();
            let closed = n > 2;
            for i in 0..if closed { n } else { n - 1 } {
                let (a, c) = (p[i], p[(i + 1) % n]);
                if (a.0 - c.0).hypot(a.1 - c.1) > 1e-9 {
                    dirs.push((c.1 - a.1).atan2(c.0 - a.0).to_degrees().rem_euclid(180.0));
                }
            }
        }
        let tol = 0.25 + 0.003 * v.abs();
        let ok = dirs.iter().any(|a| {
            dirs.iter().any(|b| {
                let d = (a - b).abs();
                (d - v).abs() <= tol || (d - (180.0 - v)).abs() <= tol
            })
        });
        if !ok {
            self.geom_fail += 1;
        }
    }
    /// Explicit comparison of a value measured from drawn points with its label.
    fn check_val(&mut self, drawn: f64, v: f64, tol: f64) {
        if (drawn - v).abs() > tol {
            self.geom_fail += 1;
        }
    }
    fn tol(&mut self, v: f64) -> Option<f64> {
        if self.r.chance(0.2) {
            linear_tolerance(&self.gtol.to_string(), v)
        } else {
            None
        }
    }
    /// Leader from (tx,ty) to a printed number placed beside it.
    #[allow(clippy::too_many_arguments)]
    fn leader(
        &mut self,
        tx: f64,
        ty: f64,
        prefix: &str,
        v: f64,
        unit: &'static str,
        src: Source,
        kind: &'static str,
    ) -> bool {
        let c = self.cands(tx, ty, self.lead_d);
        let text = num_text(prefix, v, unit, self.p.dec);
        let u = self.u;
        for &(x, y, a) in &c {
            // the candidate box and the leader from the target to its nearest corner must cross nothing placed so far
            let w = crate::layout::text_w(&text, self.fs);
            let xl = match a {
                Anchor::Start => x,
                Anchor::Middle => x - w / 2.0,
                Anchor::End => x - w,
            };
            let b = Page::text_box(xl, y, &text, self.fs);
            let ex = if (b[0] + b[2]) / 2.0 > tx { b[0] } else { b[2] };
            let ey = if (b[1] + b[3]) / 2.0 > ty { b[1] } else { b[3] };
            let ls = [tx, ty, ex, ey];
            if self
                .p
                .items
                .iter()
                .any(|i| crate::layout::seg_hits(ls, i.b, 2.0 * u))
                || self.p.segs.iter().any(|sg| seg_intersect(ls, *sg))
            {
                continue;
            }
            if self
                .put_num(kind, prefix, v, unit, None, src.clone(), &[(x, y, a)])
                .is_some()
            {
                self.p.line(tx, ty, ex, ey, 1.0 * u);
                if !self.skip_chk && kind == "dimension" {
                    if unit == "mm" && prefix.contains('\u{00D8}') {
                        self.check_size(v);
                    } else if unit == "deg" {
                        self.check_angle(v);
                    }
                }
                return true;
            }
        }
        false
    }
    /// Horizontal dimension between px x-positions xa, xb of a feature at px height y; line at y+off (off<0 above).
    #[allow(clippy::too_many_arguments)]
    fn dim_h(
        &mut self,
        xa: f64,
        xb: f64,
        y: f64,
        off: f64,
        v: f64,
        src: Source,
        prefix: &str,
        tol: Option<f64>,
    ) -> bool {
        let u = self.u;
        let yd = y + off;
        let s = off.signum();
        let mid = (xa + xb) / 2.0;
        let f = self.fs;
        let c = [
            (mid, yd - 0.3 * f, Anchor::Middle),
            (mid, yd + 1.0 * f, Anchor::Middle),
        ];
        let c = if s > 0.0 { [c[1], c[0]] } else { c };
        if self
            .put_num("dimension", prefix, v, "mm", tol, src, &c)
            .is_none()
        {
            return false;
        }
        self.check_extent((xb - xa).abs() / self.sc, v);
        let sw = 1.0 * u;
        self.p.dline(xa, y + s * 4.0 * u, xa, yd + s * 3.0 * u, sw);
        self.p.dline(xb, y + s * 4.0 * u, xb, yd + s * 3.0 * u, sw);
        self.p.dline(xa, yd, xb, yd, sw);
        for (tip, dir) in [(xa, 1.0), (xb, -1.0)] {
            self.p.poly(
                &[
                    (tip, yd),
                    (tip + dir * 7.0 * u, yd - 2.5 * u),
                    (tip + dir * 7.0 * u, yd + 2.5 * u),
                ],
                true,
                0.8 * u,
                "#000",
            );
        }
        true
    }
    /// Vertical dimension between px y-positions ya, yb of a feature at px x = xf; line at xf+off (off>0 right).
    #[allow(clippy::too_many_arguments)]
    fn dim_v(
        &mut self,
        xf: f64,
        ya: f64,
        yb: f64,
        off: f64,
        v: f64,
        src: Source,
        prefix: &str,
        tol: Option<f64>,
    ) -> bool {
        let u = self.u;
        let xd = xf + off;
        let s = off.signum();
        let ym = (ya + yb) / 2.0;
        let f = self.fs;
        let c = [
            (xd + 5.0 * u, ym + 0.35 * f, Anchor::Start),
            (xd - 5.0 * u, ym + 0.35 * f, Anchor::End),
        ];
        let c = if s > 0.0 { c } else { [c[1], c[0]] };
        if self
            .put_num("dimension", prefix, v, "mm", tol, src, &c)
            .is_none()
        {
            return false;
        }
        self.check_extent((yb - ya).abs() / self.sc, v);
        let sw = 1.0 * u;
        let d = if yb > ya { 1.0 } else { -1.0 };
        self.p.dline(xf + s * 4.0 * u, ya, xd + s * 3.0 * u, ya, sw);
        self.p.dline(xf + s * 4.0 * u, yb, xd + s * 3.0 * u, yb, sw);
        self.p.dline(xd, ya, xd, yb, sw);
        for (tip, sg) in [(ya, d), (yb, -d)] {
            self.p.poly(
                &[
                    (xd, tip),
                    (xd - 2.5 * u, tip + sg * 7.0 * u),
                    (xd + 2.5 * u, tip + sg * 7.0 * u),
                ],
                true,
                0.8 * u,
                "#000",
            );
        }
        true
    }
    /// Standard weld symbol (fillet triangle with leader) and its leg size; text placed first so it never collides.
    fn weld(&mut self, ax: f64, ay: f64, leg: f64) {
        let u = self.u;
        let c = [
            (ax + 46.0 * u, ay - 40.0 * u, Anchor::Start),
            (ax - 46.0 * u, ay - 40.0 * u, Anchor::End),
        ];
        if self
            .put_num(
                "weld",
                "a",
                leg,
                "mm",
                None,
                Source::Spec {
                    field: "weld_leg".into(),
                    factor: 1.0,
                },
                &c,
            )
            .is_some()
        {
            self.p.line(ax, ay, ax + 30.0 * u, ay - 36.0 * u, 1.0 * u);
            self.p.poly(
                &[(ax, ay), (ax + 12.0 * u, ay), (ax, ay - 12.0 * u)],
                true,
                1.0 * u,
                "none",
            );
        }
    }
    /// Surface-finish check symbol with its Ra value.
    fn finish(&mut self, ax: f64, ay: f64, ra: f64) {
        let u = self.u;
        let c = [
            (ax + 30.0 * u, ay + 6.0 * u, Anchor::Start),
            (ax - 30.0 * u, ay + 6.0 * u, Anchor::End),
        ];
        if self
            .put_num(
                "surface_finish",
                "Ra ",
                ra,
                "um",
                None,
                Source::Spec {
                    field: "ra".into(),
                    factor: 1.0,
                },
                &c,
            )
            .is_some()
        {
            self.p.poly(
                &[
                    (ax, ay),
                    (ax + 8.0 * u, ay - 12.0 * u),
                    (ax + 24.0 * u, ay + 14.0 * u),
                ],
                false,
                1.2 * u,
                "none",
            );
        }
    }
}

// ---------------------------------------------------------------- engine-backed flat patterns

/// Draw polygons given in mm (developed coordinates) into a view cell; `shape` names the element.
fn dev_view(c: &mut Ctx, cell: [f64; 4], polys: &[Vec<[f64; 2]>], shape: &str) -> bool {
    if polys.is_empty() {
        c.refused += 1;
        return false;
    }
    let all: Vec<(f64, f64)> = polys.iter().flatten().map(|p| (p[0], p[1])).collect();
    c.shape = shape.into();
    c.fit(cell, bbox(&all));
    for p in polys {
        let q: Vec<(f64, f64)> = p.iter().map(|q| (q[0], q[1])).collect();
        c.outline(&q, "none");
    }
    true
}

/// Triangulated development of the lateral surface of a rectangle-to-round transition.
/// Base vertices P_i (rectangle perimeter, n points) and top vertices Q_j (circle, n points) form a zigzag strip of
/// triangles; each triangle is placed in the plane from its three 3-D edge lengths, next to the previous one.
fn triangulated_development(a: f64, b: f64, dd: f64, h: f64, n: usize) -> Vec<Vec<[f64; 2]>> {
    let per = 2.0 * (a + b);
    let mut p3: Vec<[f64; 3]> = vec![];
    for i in 0..n {
        let s = per * i as f64 / n as f64;
        // walk the rectangle perimeter from the middle of the first side
        let s = (s + a / 2.0).rem_euclid(per);
        let (x, y) = if s < a {
            (-a / 2.0 + s, -b / 2.0)
        } else if s < a + b {
            (a / 2.0, -b / 2.0 + (s - a))
        } else if s < 2.0 * a + b {
            (a / 2.0 - (s - a - b), b / 2.0)
        } else {
            (-a / 2.0, b / 2.0 - (s - 2.0 * a - b))
        };
        p3.push([x, y, 0.0]);
    }
    let mut q3: Vec<[f64; 3]> = vec![];
    for j in 0..n {
        let t = std::f64::consts::TAU * j as f64 / n as f64;
        q3.push([dd / 2.0 * t.cos(), dd / 2.0 * t.sin(), h]);
    }
    // zigzag: P0 Q0 P1 Q1 ... closed back to P0
    let mut v: Vec<[f64; 3]> = vec![];
    for i in 0..n {
        v.push(p3[i]);
        v.push(q3[i]);
    }
    let dist = |p: [f64; 3], q: [f64; 3]| {
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    };
    let m = v.len();
    let mut d2: Vec<[f64; 2]> = vec![[0.0, 0.0]; m];
    d2[1] = [dist(v[0], v[1]), 0.0];
    let mut tris: Vec<Vec<[f64; 2]>> = vec![];
    // place vertex k from k-2 and k-1 (triangle k-2, k-1, k), on the side away from the previous apex
    for k in 2..m {
        let (a_, b_) = (k - 2, k - 1);
        let (la, lb, lc) = (dist(v[a_], v[k]), dist(v[b_], v[k]), dist(v[a_], v[b_]));
        let (pa, pb) = (d2[a_], d2[b_]);
        let ex = [pb[0] - pa[0], pb[1] - pa[1]];
        let len = (ex[0] * ex[0] + ex[1] * ex[1]).sqrt().max(1e-12);
        let x = (la * la - lb * lb + lc * lc) / (2.0 * lc);
        let y = (la * la - x * x).max(0.0).sqrt();
        let ux = [ex[0] / len, ex[1] / len];
        let perp = [-ux[1], ux[0]];
        // the previous strip lies on the side of vertex k-3 (if any); the new apex goes to the other side
        let side = if k >= 3 {
            let prev = d2[k - 3];
            let s_prev = (prev[0] - pa[0]) * perp[0] + (prev[1] - pa[1]) * perp[1];
            if s_prev > 0.0 { -1.0 } else { 1.0 }
        } else {
            1.0
        };
        d2[k] = [
            pa[0] + ux[0] * x + side * perp[0] * y,
            pa[1] + ux[1] * x + side * perp[1] * y,
        ];
        tris.push(vec![d2[k - 2], d2[k - 1], d2[k]]);
    }
    tris
}

/// Intersection of the lines p + s u and q + t v.
fn isect(p: (f64, f64), u: (f64, f64), q: (f64, f64), v: (f64, f64)) -> (f64, f64) {
    let den = u.0 * v.1 - u.1 * v.0;
    let s = ((q.0 - p.0) * v.1 - (q.1 - p.1) * v.0) / den;
    (p.0 + s * u.0, p.1 + s * u.1)
}

/// Place developed pieces (each at its own origin, `w` wide) side by side with a gap and draw them in one cell.
fn dev_row(c: &mut Ctx, cell: [f64; 4], pieces: &[(Vec<Vec<[f64; 2]>>, f64)], shape: &str) -> bool {
    let wmax = pieces.iter().map(|p| p.1).fold(0.0, f64::max);
    let mut x0 = 0.0;
    let mut polys: Vec<Vec<[f64; 2]>> = vec![];
    for (pl, w) in pieces {
        for q in pl {
            polys.push(q.iter().map(|p| [p[0] + x0, p[1]]).collect());
        }
        x0 += w + 0.15 * wmax;
    }
    dev_view(c, cell, &polys, shape)
}

#[allow(clippy::too_many_arguments)]
fn wye_flat(
    c: &mut Ctx,
    cell: [f64; 4],
    d: f64,
    a1: f64,
    a2: f64,
    l1: f64,
    l2: f64,
    lm: f64,
) -> bool {
    let spec = nf_core::wye::WyeSpec {
        d,
        t: 3.0,
        beta1_deg: a1,
        beta2_deg: a2,
        l: lm,
        h1: l1,
        h2: l2,
        branch_d: None,
        branch_t: None,
        cone_s: None,
        split: Some(1),
        basis: nf_core::shapes::Basis::Outer,
        amp: nf_core::shapes::Basis::Outer,
    };
    match nf_core::wye::wye(&spec) {
        Ok(r) => {
            let row: Vec<(Vec<Vec<[f64; 2]>>, f64)> = r
                .pieces
                .iter()
                .take(3)
                .filter_map(|p| p.plates.first().map(|q| (vec![q.clone()], p.w)))
                .collect();
            dev_row(c, cell, &row, "wye_flat")
        }
        Err(_) => {
            c.refused += 1;
            false
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn branch_on_cone_flat(
    c: &mut Ctx,
    cell: [f64; 4],
    d1: f64,
    d2: f64,
    h: f64,
    rb: f64,
    zt: f64,
    lb: f64,
) -> bool {
    let spec = nf_core::conebranch::ConeBranchSpec {
        da: d2,
        db: d1,
        l: h,
        t: 2.0,
        ecc: false,
        flat_deg: None,
        d: 2.0 * rb,
        tb: 2.0,
        x: Some(zt),
        dir_deg: 0.0,
        e: 0.0,
        h: lb,
        angle_deg: Some(90.0),
        mount: nf_core::tee::Mount::SetOn,
        split: Some(2),
        seam_gap: None,
    };
    match nf_core::conebranch::cone_branch(&spec) {
        Ok(r) => {
            let polys: Vec<Vec<[f64; 2]>> = r
                .pieces
                .first()
                .map(|p| vec![p.outline.clone()])
                .unwrap_or_default();
            dev_view(c, cell, &polys, "cone_flat")
        }
        Err(_) => {
            c.refused += 1;
            false
        }
    }
}

fn tee_branch_flat(c: &mut Ctx, cell: [f64; 4], dm: f64, db: f64, hb: f64, alpha: f64) -> bool {
    let spec = nf_core::tee::BranchPieceSpec {
        d: db,
        t: 2.0,
        main_d: dm,
        main_t: 2.0,
        h: hb,
        e: 0.0,
        mount: nf_core::tee::Mount::SetOn,
        angle_deg: Some(alpha),
    };
    match nf_core::tee::branch_piece(&spec) {
        Ok(r) => dev_view(c, cell, std::slice::from_ref(&r.outline), "branch_flat"),
        Err(_) => {
            c.refused += 1;
            false
        }
    }
}

/// Stadium (slot) outline centred at (cx,cy), centre distance `len_c` along x, width `w`.
fn stadium(cx: f64, cy: f64, len_c: f64, w: f64) -> Vec<(f64, f64)> {
    let r = w / 2.0;
    let (xl, xr) = (cx - len_c / 2.0, cx + len_c / 2.0);
    let mut pts = vec![];
    for k in 0..=12 {
        let a = (90.0 + 180.0 * k as f64 / 12.0).to_radians();
        pts.push((xl + r * a.cos(), cy + r * a.sin()));
    }
    for k in 0..=12 {
        let a = (-90.0 + 180.0 * k as f64 / 12.0).to_radians();
        pts.push((xr + r * a.cos(), cy + r * a.sin()));
    }
    pts
}

fn bbox(pts: &[(f64, f64)]) -> [f64; 4] {
    let mut b = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for &(x, y) in pts {
        b[0] = b[0].min(x);
        b[1] = b[1].min(y);
        b[2] = b[2].max(x);
        b[3] = b[3].max(y);
    }
    b
}

fn sp(f: &str) -> Source {
    Source::Spec {
        field: f.to_string(),
        factor: 1.0,
    }
}

fn der(name: &str, inp: &[(&str, f64)]) -> Source {
    let inputs: Inputs = inp.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    Source::Derived {
        name: name.to_string(),
        inputs,
    }
}

/// Value of a closed-form derived quantity (the same formula `resolve_v2` re-derives from the truth line).
fn dv(name: &str, inp: &[(&str, f64)]) -> f64 {
    let inputs: Inputs = inp.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    derive(name, &inputs).unwrap_or(f64::NAN)
}

fn stepd(r: &mut Rng, lo: f64, hi: f64, step: f64) -> f64 {
    if hi < lo { lo } else { r.stepped(lo, hi, step) }
}

// ---------------------------------------------------------------- families

fn plate(c: &mut Ctx) {
    let l = stepd(&mut c.r, 120.0, 600.0, 5.0);
    let w = stepd(&mut c.r, 60.0, 300.0, 5.0);
    let t = *c.r.pick(&[3.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0]);
    let ch = *c.r.pick(&[0.0, 3.0, 5.0, 8.0]);
    let d = *c.r.pick(&[6.0, 8.0, 9.0, 10.0, 12.0, 14.0, 16.0, 18.0]);
    let nx: usize = if l >= 300.0 && c.r.chance(0.5) { 3 } else { 2 };
    let one_row = c.r.chance(0.5);
    let ex = 20.0;
    let pitch = ((l - 2.0 * ex) / (nx - 1) as f64).floor();
    let slot = c.r.chance(0.4);
    let (sl_c, sl_w) = (
        *c.r.pick(&[10.0, 15.0, 20.0, 25.0]),
        *c.r.pick(&[6.0, 8.0, 10.0]),
    );
    for (k, v) in [
        ("L", l),
        ("W", w),
        ("t", t),
        ("chamfer", ch),
        ("hole_d", d),
        ("pitch", pitch),
        ("n_col", nx as f64),
    ] {
        c.set(k, v);
    }
    let ey = 15.0;
    let ys: Vec<f64> = if one_row {
        vec![w / 2.0]
    } else {
        vec![ey, w - ey]
    };
    let pts: Vec<(f64, f64)> = if ch > 0.0 {
        vec![
            (ch, 0.0),
            (l - ch, 0.0),
            (l, ch),
            (l, w - ch),
            (l - ch, w),
            (ch, w),
            (0.0, w - ch),
            (0.0, ch),
        ]
    } else {
        vec![(0.0, 0.0), (l, 0.0), (l, w), (0.0, w)]
    };
    c.fit(c.cell(2), [0.0, 0.0, l, w]);
    c.shape = "outline".into();
    c.outline(&pts, "none");
    c.shape = "hole".into();
    for k in 0..nx {
        for &y in &ys {
            c.circle(ex + k as f64 * pitch, y, d / 2.0);
        }
    }
    if slot {
        c.set("slot_c", sl_c);
        c.set("slot_w", sl_w);
        let sp_pts = stadium(l / 2.0, w / 2.0, sl_c, sl_w);
        c.shape = "slot".into();
        c.outline(&sp_pts, "none");
    }
    c.shape = "dims".into();
    let (x0, y0) = c.px(0.0, 0.0);
    let (xl, _) = c.px(l, 0.0);
    let (_, yt) = c.px(0.0, w);
    let u = c.u;
    let (tl, tw) = (c.tol(l), c.tol(w));
    c.dim_h(x0, xl, y0, 42.0 * u, l, sp("L"), "", tl);
    c.dim_v(x0, y0, yt, -42.0 * u, w, sp("W"), "", tw);
    if nx >= 2 {
        let (xa, _) = c.px(ex, 0.0);
        let (xb, _) = c.px(ex + pitch, 0.0);
        c.dim_h(xa, xb, yt, -42.0 * u, pitch, sp("pitch"), "", None);
    }
    let r = d / 2.0;
    let (hx, hy) = c.px(ex + 0.7 * r, ys[0] + 0.7 * r);
    c.leader(
        hx,
        hy,
        &format!("{}-\u{00D8}", nx * ys.len()),
        d,
        "mm",
        sp("hole_d"),
        "dimension",
    );
    let (tx, ty) = c.px(l, w);
    c.leader(tx, ty, "t=", t, "mm", sp("t"), "dimension");
    if ch > 0.0 {
        let (cx_, cy_) = c.px(l - ch / 2.0, w - ch / 2.0);
        c.leader(cx_, cy_, "C", ch, "mm", sp("chamfer"), "dimension");
    }
    if slot {
        let (sx, sy) = c.px(
            l / 2.0 + sl_c / 2.0 + sl_w / 2.0 * 0.7,
            w / 2.0 + sl_w / 2.0 * 0.7,
        );
        c.leader(
            sx,
            sy,
            "",
            sl_c + sl_w,
            "mm",
            der("slot_overall", &[("slot_c", sl_c), ("slot_w", sl_w)]),
            "dimension",
        );
        let (wx, wy) = c.px(l / 2.0, w / 2.0 + sl_w / 2.0);
        c.leader(wx, wy, "", sl_w, "mm", sp("slot_w"), "dimension");
    }
}

fn ring_flange(c: &mut Ctx) {
    let od = stepd(&mut c.r, 120.0, 400.0, 5.0);
    let bd = *c.r.pick(&[6.0, 8.0, 10.0, 12.0]);
    let hi = (od * 0.5).min(od - 2.0 * bd - 16.0).max(50.0);
    let id = stepd(&mut c.r, 50.0, hi, 5.0);
    let t = *c.r.pick(&[8.0, 10.0, 12.0, 14.0, 16.0, 20.0]);
    let n = *c.r.pick(&[4.0, 6.0, 8.0, 12.0]);
    let bcd = ((id + od) / 2.0).floor();
    for (k, v) in [
        ("od", od),
        ("id", id),
        ("t", t),
        ("n", n),
        ("bcd", bcd),
        ("bolt_d", bd),
    ] {
        c.set(k, v);
    }
    c.shape = "flange".into();
    c.fit(c.cell(0), [-od / 2.0, -od / 2.0, od / 2.0, od / 2.0]);
    c.circle(0.0, 0.0, od / 2.0);
    c.circle(0.0, 0.0, id / 2.0);
    c.shape = "bolt".into();
    for k in 0..n as usize {
        let a = k as f64 * std::f64::consts::TAU / n;
        c.circle(bcd / 2.0 * a.cos(), bcd / 2.0 * a.sin(), bd / 2.0);
    }
    c.shape = "dims".into();
    let s45 = std::f64::consts::FRAC_1_SQRT_2;
    let (ax, ay) = c.px(od / 2.0 * s45, od / 2.0 * s45);
    c.leader(ax, ay, "\u{00D8}", od, "mm", sp("od"), "dimension");
    let (bx, by) = c.px(-id / 2.0 * s45, -id / 2.0 * s45);
    c.leader(bx, by, "\u{00D8}", id, "mm", sp("id"), "dimension");
    let (kx, ky) = c.px(0.0, bcd / 2.0);
    // pitch-circle diameter from the drawn bolt-hole centres (bounding-box centres of the bolt circles)
    let pcd = c
        .vpolys
        .iter()
        .map(|p| {
            let b = bbox(p);
            ((b[0] + b[2]) / 2.0).hypot((b[1] + b[3]) / 2.0) * 2.0
        })
        .fold(0.0, f64::max);
    c.check_val(pcd, bcd, 1.0 / c.sc + 0.005 * bcd);
    c.skip_chk = true;
    c.leader(kx, ky, "\u{00D8}", bcd, "mm", sp("bcd"), "dimension");
    c.skip_chk = false;
    let a0 = std::f64::consts::PI / n;
    let (px_, py_) = c.px(bcd / 2.0 * a0.cos() * 0.98, bcd / 2.0 * a0.sin() * 0.98);
    c.leader(
        px_,
        py_,
        &format!("{}-\u{00D8}", n as i64),
        bd,
        "mm",
        sp("bolt_d"),
        "dimension",
    );
    let (qx, qy) = c.px(bcd / 2.0 * (a0).cos(), -bcd / 2.0 * (a0).sin());
    c.leader(
        qx,
        qy,
        "P=",
        dv("bolt_pitch", &[("bcd", bcd), ("n", n)]),
        "mm",
        der("bolt_pitch", &[("bcd", bcd), ("n", n)]),
        "dimension",
    );
    // section through the ring
    c.shape = "section".into();
    c.fit(c.cell(1), [id / 2.0, 0.0, od / 2.0, t]);
    let sec = [
        (id / 2.0, 0.0),
        (od / 2.0, 0.0),
        (od / 2.0, t),
        (id / 2.0, t),
    ];
    c.outline(&sec, "none");
    c.shape = "dims".into();
    let (xa, ya) = c.px(id / 2.0, t);
    let (xb, _) = c.px(od / 2.0, t);
    let u = c.u;
    c.dim_h(
        xa,
        xb,
        ya,
        -42.0 * u,
        (od - id) / 2.0,
        der("ring_width", &[("od", od), ("id", id)]),
        "",
        None,
    );
    let (xr, ya2) = c.px(od / 2.0, 0.0);
    let (_, yb2) = c.px(od / 2.0, t);
    c.dim_v(xr, ya2, yb2, 42.0 * u, t, sp("t"), "", None);
}

fn bracket_gusset(c: &mut Ctx) {
    let la = stepd(&mut c.r, 60.0, 300.0, 5.0);
    let hb = stepd(&mut c.r, 50.0, 250.0, 5.0);
    let t = *c.r.pick(&[4.0, 5.0, 6.0, 8.0, 10.0]);
    let g = stepd(&mut c.r, 20.0, ((la.min(hb)) / 2.0).floor().max(20.0), 5.0);
    let d = *c.r.pick(&[8.0, 10.0, 12.0]);
    for (k, v) in [
        ("La", la),
        ("Hb", hb),
        ("t", t),
        ("gusset", g),
        ("hole_d", d),
    ] {
        c.set(k, v);
    }
    c.fit(c.cell(2), [0.0, 0.0, la, hb]);
    c.shape = "plate".into();
    c.outline(
        &[(0.0, 0.0), (la, 0.0), (la, t), (t, t), (t, hb), (0.0, hb)],
        "none",
    );
    c.shape = "gusset".into();
    c.outline(&[(t, t), (t + g, t), (t, t + g)], "none");
    c.shape = "hole".into();
    c.circle(la - 20.0, t / 2.0, d / 2.0);
    c.circle(t / 2.0, hb - 20.0, d / 2.0);
    c.shape = "dims".into();
    let (x0, y0) = c.px(0.0, 0.0);
    let (xa, _) = c.px(la, 0.0);
    let (_, yh) = c.px(0.0, hb);
    let u = c.u;
    c.dim_h(x0, xa, y0, 42.0 * u, la, sp("La"), "", None);
    c.dim_v(x0, y0, yh, -42.0 * u, hb, sp("Hb"), "", None);
    let (gx0, gy) = c.px(t, t);
    let (gx1, _) = c.px(t + g, t);
    c.dim_h(gx0, gx1, gy, -42.0 * u, g, sp("gusset"), "", None);
    let (hx, hy) = c.px(la - 20.0 + 0.7 * d / 2.0, t / 2.0 + 0.7 * d / 2.0);
    c.leader(hx, hy, "2-\u{00D8}", d, "mm", sp("hole_d"), "dimension");
    let (tx, ty) = c.px(la, t);
    c.leader(tx, ty, "t=", t, "mm", sp("t"), "dimension");
}

fn flat_sheet(c: &mut Ctx) {
    let w = stepd(&mut c.r, 60.0, 200.0, 5.0);
    let l1 = stepd(&mut c.r, 20.0, 120.0, 5.0);
    let l2 = stepd(&mut c.r, 20.0, 120.0, 5.0);
    let t = *c.r.pick(&[1.2, 1.5, 2.0, 3.0]);
    let r = 2.0 * t;
    let theta = *c.r.pick(&[90.0, 60.0, 45.0]);
    let ba = dv("bend_allow", &[("theta", theta), ("r", r), ("t", t)]);
    let blank = dv(
        "flat_blank",
        &[("l1", l1), ("l2", l2), ("theta", theta), ("r", r), ("t", t)],
    );
    for (k, v) in [
        ("W", w),
        ("l1", l1),
        ("l2", l2),
        ("t", t),
        ("r", r),
        ("theta", theta),
    ] {
        c.set(k, v);
    }
    c.shape = "sheet".into();
    c.fit(c.cell(2), [0.0, 0.0, w, blank]);
    c.outline(&[(0.0, 0.0), (w, 0.0), (w, blank), (0.0, blank)], "none");
    c.shape = "bend_line".into();
    c.mline((0.0, l1), (w, l1));
    c.mline((0.0, l1 + ba), (w, l1 + ba));
    c.shape = "dims".into();
    let (x0, y0) = c.px(0.0, 0.0);
    let (xw, _) = c.px(w, 0.0);
    let (_, yb) = c.px(0.0, blank);
    let (xr, _) = c.px(w, 0.0);
    let u = c.u;
    c.dim_h(x0, xw, y0, 42.0 * u, w, sp("W"), "", None);
    c.dim_v(
        xr,
        y0,
        yb,
        42.0 * u,
        blank,
        der(
            "flat_blank",
            &[("l1", l1), ("l2", l2), ("theta", theta), ("r", r), ("t", t)],
        ),
        "BL ",
        None,
    );
    let (ly, lx) = c.px(0.0, l1 / 2.0);
    c.leader(ly, lx, "", l1, "mm", sp("l1"), "dimension");
    let (bx, by) = c.px(w / 2.0, l1 + ba / 2.0);
    c.leader(
        bx,
        by,
        "BA ",
        ba,
        "mm",
        der("bend_allow", &[("theta", theta), ("r", r), ("t", t)]),
        "dimension",
    );
    let (ax, ay) = c.px(w, blank);
    // the bend angle of a flat blank is not drawn (the blank is flat): no drawn angle to compare
    c.skip_chk = true;
    c.leader(ax, ay, "", theta, "deg", sp("theta"), "dimension");
    c.skip_chk = false;
    let (tx, ty) = c.px(0.0, blank);
    c.leader(tx, ty, "t=", t, "mm", sp("t"), "dimension");
}

fn duct_transition(c: &mut Ctx) {
    let a = stepd(&mut c.r, 150.0, 500.0, 5.0);
    let b = stepd(&mut c.r, 100.0, 400.0, 5.0);
    let dd = stepd(&mut c.r, 150.0, 450.0, 5.0);
    let h = stepd(&mut c.r, 100.0, 400.0, 5.0);
    for (k, v) in [("A", a), ("B", b), ("D", dd), ("H", h)] {
        c.set(k, v);
    }
    let u = c.u;
    // view 0: plan (rectangle A x B with the round end D)
    let left = c.cell(0);
    let lmid = (left[1] + left[3]) / 2.0;
    c.shape = "plan".into();
    c.fit(
        [left[0], left[1], left[2], lmid],
        [-a / 2.0, -b / 2.0, a / 2.0, b / 2.0],
    );
    c.outline(
        &[
            (-a / 2.0, -b / 2.0),
            (a / 2.0, -b / 2.0),
            (a / 2.0, b / 2.0),
            (-a / 2.0, b / 2.0),
        ],
        "none",
    );
    c.circle(0.0, 0.0, dd / 2.0);
    c.shape = "dims".into();
    let (x0, y0) = c.px(-a / 2.0, -b / 2.0);
    let (x1, _) = c.px(a / 2.0, -b / 2.0);
    let (xr, yb0) = c.px(a / 2.0, -b / 2.0);
    let (_, yb1) = c.px(a / 2.0, b / 2.0);
    c.dim_h(x0, x1, y0, 42.0 * u, a, sp("A"), "", None);
    c.dim_v(xr, yb0, yb1, 42.0 * u, b, sp("B"), "", None);
    let s45 = std::f64::consts::FRAC_1_SQRT_2;
    let (dx, dy) = c.px(dd / 2.0 * s45, dd / 2.0 * s45);
    c.leader(dx, dy, "\u{00D8}", dd, "mm", sp("D"), "dimension");
    let (px_, py_) = c.px(a / 2.0, b / 2.0);
    c.leader(
        px_,
        py_,
        "P=",
        dv("rect_perimeter", &[("a", a), ("b", b)]),
        "mm",
        der("rect_perimeter", &[("a", a), ("b", b)]),
        "dimension",
    );
    // developed pattern of the lateral surface, triangulated (bottom half of the left cell)
    let tris = triangulated_development(a, b, dd, h, 8);
    dev_view(c, [left[0], lmid, left[2], left[3]], &tris, "development");
    // view 1 (top half of the right cell): front elevation, width A at the rectangular end
    let big = a.max(dd);
    let right = c.cell(1);
    let mid = (right[1] + right[3]) / 2.0;
    c.shape = "front".into();
    c.fit(
        [right[0], right[1], right[2], mid],
        [-big / 2.0, 0.0, big / 2.0, h],
    );
    c.outline(
        &[
            (-a / 2.0, 0.0),
            (a / 2.0, 0.0),
            (dd / 2.0, h),
            (-dd / 2.0, h),
        ],
        "none",
    );
    c.shape = "dims".into();
    let (xe, y0e) = c.px(big / 2.0, 0.0);
    let (_, yhe) = c.px(big / 2.0, h);
    c.dim_v(xe, y0e, yhe, 42.0 * u, h, sp("H"), "", None);
    // view 2 (bottom half): side elevation, width B at the rectangular end
    let bigs = b.max(dd);
    c.shape = "side".into();
    c.fit(
        [right[0], mid, right[2], right[3]],
        [-bigs / 2.0, 0.0, bigs / 2.0, h],
    );
    c.outline(
        &[
            (-b / 2.0, 0.0),
            (b / 2.0, 0.0),
            (dd / 2.0, h),
            (-dd / 2.0, h),
        ],
        "none",
    );
    c.shape = "dims".into();
    let (sx0, sy0) = c.px(-b / 2.0, 0.0);
    let (sx1, _) = c.px(b / 2.0, 0.0);
    c.dim_h(sx0, sx1, sy0, 42.0 * u, b, sp("B"), "", None);
    let (sxe, sy0e) = c.px(bigs / 2.0, 0.0);
    let (_, syhe) = c.px(bigs / 2.0, h);
    c.dim_v(sxe, sy0e, syhe, 42.0 * u, h, sp("H"), "", None);
}

fn tank_shell(c: &mut Ctx) {
    let d = stepd(&mut c.r, 300.0, 1200.0, 10.0);
    let l = stepd(&mut c.r, 600.0, 2400.0, 10.0);
    let nd: f64 = *c.r.pick(&[50.0, 80.0, 100.0, 150.0]);
    let pad_od = (nd * 1.8).floor();
    let pad_t = *c.r.pick(&[8.0, 10.0, 12.0]);
    let nx = stepd(
        &mut c.r,
        (0.2 * l / 10.0).ceil() * 10.0,
        (0.8 * l / 10.0).floor() * 10.0,
        10.0,
    );
    let head = c.r.int(0, 2); // 0 ellipsoidal 2:1, 1 torispherical (crown D, spherical-cap approximation), 2 hemispherical
    let rr = d / 2.0;
    // head depth per type: the closed form the truth line re-derives (see derive.rs)
    let (h, hsrc) = match head {
        0 => (
            dv("head_height", &[("od", d), ("ratio", 2.0)]),
            der("head_height", &[("od", d), ("ratio", 2.0)]),
        ),
        1 => (
            dv(
                "head_depth",
                &[("od", d), ("rc_ratio", 1.0), ("rk_ratio", 0.06)],
            ),
            der(
                "head_depth",
                &[("od", d), ("rc_ratio", 1.0), ("rk_ratio", 0.06)],
            ),
        ),
        _ => (
            dv(
                "head_depth",
                &[("od", d), ("rc_ratio", 0.5), ("rk_ratio", 0.0)],
            ),
            der(
                "head_depth",
                &[("od", d), ("rc_ratio", 0.5), ("rk_ratio", 0.0)],
            ),
        ),
    };
    let rc = match head {
        1 => d,
        _ => d / 2.0,
    };
    let _ = rc;
    for (k, v) in [
        ("D", d),
        ("L", l),
        ("nozzle_d", nd),
        ("nozzle_x", nx),
        ("pad_od", pad_od),
        ("pad_t", pad_t),
        ("head_type", head as f64),
    ] {
        c.set(k, v);
    }
    let top = rr + 0.8 * nd + pad_t;
    // side view: head outlines, straight shell between tangent lines x = 0 and x = l
    let mut pts = vec![];
    let head_pts = |c: &mut Ctx, left: bool| -> Vec<(f64, f64)> {
        let _ = c;
        let mut v = vec![];
        if head == 0 {
            for k in 0..=16 {
                let f = -90.0 + 180.0 * k as f64 / 16.0;
                let (x, y) = (-h * f.to_radians().cos(), rr * f.to_radians().sin());
                v.push(if left { (x, y) } else { (l - x, y) });
            }
        } else {
            // crown radius rc, knuckle radius rk tangent to the shell at (0, +-D/2); upper half then mirrored
            let rk = if head == 1 { 0.06 * d } else { 0.0 };
            let kx = 0.0;
            let ky = rr - rk;
            let xc = ((rc - rk).powi(2) - (rr - rk).powi(2)).sqrt();
            let (ccx, ccy) = (xc, 0.0);
            let (tx, ty) = (
                ccx + (kx - ccx) * rc / (rc - rk),
                ccy + (ky - ccy) * rc / (rc - rk),
            );
            let phi_t = (ty - ky).atan2(tx - kx);
            let psi_t = (ty - ccy).atan2(tx - ccx);
            let mut up: Vec<(f64, f64)> = vec![];
            for k in 0..=8 {
                let f = std::f64::consts::FRAC_PI_2
                    + (phi_t - std::f64::consts::FRAC_PI_2) * k as f64 / 8.0;
                up.push((kx + rk * f.cos(), ky + rk * f.sin()));
            }
            for k in 1..=12 {
                let f = psi_t + (std::f64::consts::PI - psi_t) * k as f64 / 12.0;
                up.push((ccx + rc * f.cos(), ccy + rc * f.sin()));
            }
            // full left head: lower half (mirror of upper, from the flange) to the apex, then upper back to the flange
            let mut v: Vec<(f64, f64)> = up.iter().map(|&(x, y)| (x, -y)).collect();
            v.extend(up.iter().rev().skip(1).copied());
            return if left {
                v
            } else {
                v.into_iter().map(|(x, y)| (l - x, y)).collect()
            };
        }
        v
    };
    pts.extend(head_pts(c, true));
    let mut right = head_pts(c, false);
    right.reverse();
    pts.extend(right);
    let bb = [-h, -rr, l + h, top];
    c.shape = "shell".into();
    c.fit(c.cell(0), bb);
    c.outline(&pts, "none");
    c.shape = "head".into();
    c.outline(
        &[
            (-pad_od / 2.0 + nx, rr),
            (nx - pad_od / 2.0, rr + pad_t),
            (nx + pad_od / 2.0, rr + pad_t),
            (nx + pad_od / 2.0, rr),
        ],
        "none",
    );
    c.shape = "nozzle".into();
    c.outline(
        &[
            (nx - nd / 2.0, rr + pad_t),
            (nx - nd / 2.0, top),
            (nx + nd / 2.0, top),
            (nx + nd / 2.0, rr + pad_t),
        ],
        "none",
    );
    c.shape = "dims".into();
    let (lx, ly) = c.px(0.0, -rr);
    let (rx, _) = c.px(l, -rr);
    let u = c.u;
    let tl = c.tol(l);
    c.dim_h(lx, rx, ly, 42.0 * u, l, sp("L"), "", tl);
    let (nxp, _) = c.px(nx, -rr);
    c.dim_h(lx, nxp, ly, 84.0 * u, nx, sp("nozzle_x"), "", None);
    let (dx, dy) = c.px(l * 0.3, rr);
    c.leader(dx, dy, "\u{00D8}", d, "mm", sp("D"), "dimension");
    let (nx2, ny2) = c.px(nx + nd / 2.0, top);
    c.leader(nx2, ny2, "\u{00D8}", nd, "mm", sp("nozzle_d"), "dimension");
    let (px2, py2) = c.px(nx + pad_od / 2.0, rr + pad_t);
    c.leader(
        px2,
        py2,
        "\u{00D8}",
        pad_od,
        "mm",
        sp("pad_od"),
        "dimension",
    );
    let (hx, hy) = c.px(l + h * 0.5, -rr * 0.5);
    c.leader(hx, hy, "h=", h, "mm", hsrc, "dimension");
    // end view: outer circle, nozzle and pad circles at the top of the shell
    c.shape = "end".into();
    c.fit(c.cell(1), [-rr, -rr, rr, rr + pad_od / 2.0 + 2.0 * pad_t]);
    c.circle(0.0, 0.0, rr);
    c.circle(0.0, rr + pad_t * 0.5, pad_od / 2.0);
    c.circle(0.0, rr + pad_t * 0.5, nd / 2.0);
    c.shape = "dims".into();
    let s45 = std::f64::consts::FRAC_1_SQRT_2;
    let (ex, ey) = c.px(-rr * s45, -rr * s45);
    c.leader(ex, ey, "\u{00D8}", d, "mm", sp("D"), "dimension");
    let (nxo, nyo) = c.px(nd / 2.0 * s45, rr + pad_t * 0.5 + nd / 2.0 * s45);
    c.leader(nxo, nyo, "\u{00D8}", nd, "mm", sp("nozzle_d"), "dimension");
    let _ = rc;
}

/// Developed outline of one cylindrical bend piece from the closed-form cut z(psi) = (A + B cos psi) / (1 + D cos psi)
/// (`nf_core::bend::cut_z`): front cut, then back cut reversed; x is the arc along the outer-basis circumference from
/// the seam (45 deg from the heel).
fn bend_piece_outline(
    m: &nf_core::bend::BendModel,
    p: &nf_core::bend::PieceModel,
) -> Vec<[f64; 2]> {
    let rb = p.rho0 + m.ao;
    let ph = 135f64.to_radians();
    let n = 240;
    let psi = |i: usize| ph + std::f64::consts::TAU * i as f64 / n as f64;
    let mut v: Vec<[f64; 2]> = (0..=n)
        .map(|i| {
            [
                rb * (psi(i) - ph),
                nf_core::bend::cut_z(p, psi(i), false) - p.zlo,
            ]
        })
        .collect();
    v.extend((0..=n).rev().map(|i| {
        [
            rb * (psi(i) - ph),
            nf_core::bend::cut_z(p, psi(i), true) - p.zlo,
        ]
    }));
    v
}

fn mitred_bend(c: &mut Ctx) {
    let od = *c.r.pick(&[40.0, 60.0, 76.0, 89.0, 114.0, 140.0, 168.0]);
    let rk = *c.r.pick(&[1.5, 2.0, 2.5]);
    let r = od * rk;
    let theta = *c.r.pick(&[90.0, 60.0, 45.0]);
    let n = *c.r.pick(&[3.0, 4.0, 5.0]);
    let ri = r - od / 2.0;
    let ro = r + od / 2.0;
    for (k, v) in [("od", od), ("r", r), ("theta", theta), ("n", n)] {
        c.set(k, v);
    }
    // n - 1 joints, n pieces; the centre line is the polygon circumscribed about the arc of radius R, so the cut
    // planes pass through the bend centre at phi_j = theta (2j - 1) / (2J) and the corners sit at radius R / cos(rho)
    let jn = n as usize - 1;
    let rho = (theta / (2.0 * jn as f64)).to_radians();
    let cr = 1.0 / rho.cos();
    let phis: Vec<f64> = (1..=jn)
        .map(|j| theta * (2 * j - 1) as f64 / (2.0 * jn as f64))
        .collect();
    let at = |rad: f64, deg: f64| (rad * deg.to_radians().cos(), rad * deg.to_radians().sin());
    let ring = |rad: f64| -> Vec<(f64, f64)> {
        let mut v = vec![at(rad, 0.0)];
        v.extend(phis.iter().map(|&p| at(rad * cr, p)));
        v.push(at(rad, theta));
        v
    };
    let mut pts = ring(ro);
    pts.extend(ring(ri).into_iter().rev());
    c.shape = "bend".into();
    c.fit(c.cell(0), bbox(&pts));
    c.outline(&pts, "none");
    for &p in &phis {
        c.mline(at(ri * cr, p), at(ro * cr, p));
    }
    c.shape = "dims".into();
    let (xa, ya) = c.px(ri, 0.0);
    let (xb, _) = c.px(ro, 0.0);
    let u = c.u;
    c.dim_h(xa, xb, ya, 42.0 * u, od, sp("od"), "", None);
    let (cx_, cy_) = c.px(0.0, 0.0);
    // centre-line radius measured from the drawn end faces
    let (p0o, p0i) = (pts[0], pts[pts.len() - 1]);
    let r_drawn = (p0o.0.hypot(p0o.1) + p0i.0.hypot(p0i.1)) / 2.0;
    c.check_val(r_drawn, r, 1.0 / c.sc + 0.005 * r);
    c.leader(cx_, cy_, "R", r, "mm", sp("r"), "dimension");
    let am = (theta / 2.0).to_radians();
    let (mx, my) = c.px(ro * am.cos(), ro * am.sin());
    c.leader(mx, my, "", theta, "deg", sp("theta"), "dimension");
    // mitre cut angle from square: half the deflection between the first two drawn centre-line segments
    let mut cl = vec![at(r, 0.0)];
    cl.extend(phis.iter().map(|&p| at(r * cr, p)));
    cl.push(at(r, theta));
    let ang = |a: (f64, f64), b: (f64, f64)| (b.1 - a.1).atan2(b.0 - a.0);
    let defl = (ang(cl[1], cl[2]) - ang(cl[0], cl[1])).to_degrees().abs();
    let mitre = dv("mitre_cut", &[("theta", theta), ("n", n)]);
    c.check_val(defl / 2.0, mitre, 0.25 + 0.003 * mitre);
    let (jx, jy) = c.px(
        ro * cr * phis[0].to_radians().cos(),
        ro * cr * phis[0].to_radians().sin(),
    );
    c.skip_chk = true;
    c.leader(
        jx,
        jy,
        "mitre ",
        mitre,
        "deg",
        der("mitre_cut", &[("theta", theta), ("n", n)]),
        "dimension",
    );
    c.skip_chk = false;
    let (lx, ly) = c.px(r * am.cos(), r * am.sin());
    c.leader(
        lx,
        ly,
        "L=",
        dv("arc_length", &[("r", r), ("theta", theta)]),
        "mm",
        der("arc_length", &[("r", r), ("theta", theta)]),
        "dimension",
    );
    // developed pieces: an end piece and a middle piece (all middle pieces are the same)
    let mut spec = nf_core::bend::BendSpec::new(od, 3.0, r, theta, Some(jn as f64), 0.0, 0.0);
    spec.basis = nf_core::shapes::Basis::Outer;
    match nf_core::bend::bend(&spec) {
        Ok(res) => {
            let m = &res.model;
            let mut row: Vec<(Vec<Vec<[f64; 2]>>, f64)> = vec![];
            for idx in [0usize, 1] {
                if let Some(p) = m.pieces.get(idx) {
                    let o = bend_piece_outline(m, p);
                    let w = o.iter().map(|q| q[0]).fold(0.0, f64::max);
                    row.push((vec![o], w));
                }
            }
            dev_row(c, c.cell(1), &row, "bend_flat");
        }
        Err(_) => c.refused += 1,
    }
}

fn tee_lateral(c: &mut Ctx) {
    let dm = *c.r.pick(&[60.0, 89.0, 114.0, 168.0, 219.0]);
    let db_opts: Vec<f64> = [40.0, 60.0, 76.0, 89.0, 114.0]
        .into_iter()
        .filter(|x| *x < dm)
        .collect();
    let db = *c.r.pick(&db_opts);
    let alpha = *c.r.pick(&[45.0, 60.0, 90.0]);
    let hb = stepd(&mut c.r, dm * 1.2, dm * 2.5, 5.0);
    // a lateral branch cuts a longer hole (db / sin alpha): widen the pad so the outline encloses it
    let pad = (db * 1.6 / f64::to_radians(alpha).sin()).floor();
    let padt = *c.r.pick(&[6.0, 8.0, 10.0]);
    let l = 3.0 * dm;
    for (k, v) in [
        ("D", dm),
        ("d", db),
        ("alpha", alpha),
        ("hb", hb),
        ("pad_od", pad),
        ("pad_t", padt),
    ] {
        c.set(k, v);
    }
    let s_end = hb / alpha.to_radians().sin();
    let dir = (alpha.to_radians().cos(), alpha.to_radians().sin());
    let nrm = (-dir.1, dir.0);
    let e = (dir.0 * s_end, dir.1 * s_end);
    let top = (hb + db / 2.0).max(dm / 2.0);
    c.shape = "run".into();
    c.fit(c.cell(0), [-l / 2.0, -dm / 2.0, l / 2.0, top + pad * 0.3]);
    c.outline(
        &[
            (-l / 2.0, -dm / 2.0),
            (l / 2.0, -dm / 2.0),
            (l / 2.0, dm / 2.0),
            (-l / 2.0, dm / 2.0),
        ],
        "none",
    );
    c.shape = "branch".into();
    let h2 = db / 2.0;
    for sg in [1.0, -1.0] {
        c.mline(
            (sg * nrm.0 * h2, sg * nrm.1 * h2),
            (e.0 + sg * nrm.0 * h2, e.1 + sg * nrm.1 * h2),
        );
    }
    c.mline(
        (e.0 + nrm.0 * h2, e.1 + nrm.1 * h2),
        (e.0 - nrm.0 * h2, e.1 - nrm.1 * h2),
    );
    c.shape = "pad".into();
    c.outline(
        &[
            (-pad / 2.0, dm / 2.0),
            (pad / 2.0, dm / 2.0),
            (pad / 2.0, dm / 2.0 + padt),
            (-pad / 2.0, dm / 2.0 + padt),
        ],
        "none",
    );
    c.shape = "dims".into();
    let (xa, ya) = c.px(-l / 2.0, -dm / 2.0);
    let (_, yb) = c.px(-l / 2.0, dm / 2.0);
    let u = c.u;
    c.dim_v(xa, ya, yb, -42.0 * u, dm, sp("D"), "", None);
    let (bx, by) = c.px(e.0 + nrm.0 * db / 2.0, e.1 + nrm.1 * db / 2.0);
    c.leader(bx, by, "\u{00D8}", db, "mm", sp("d"), "dimension");
    let (px_, py_) = c.px(pad / 2.0, dm / 2.0 + padt);
    c.leader(px_, py_, "\u{00D8}", pad, "mm", sp("pad_od"), "dimension");
    if alpha == 90.0 && nf_core::tee::rib_on_branch(Some(alpha)).is_ok() {
        // ribs stand on the pad (top at dm/2 + padt) against the branch wall, inside the pad outline
        let rib = (0.25 * db).floor().max(10.0);
        let yb = dm / 2.0 + padt;
        c.set("rib_len", rib);
        c.shape = "rib".into();
        for sg in [1.0, -1.0] {
            c.outline(
                &[(sg * h2, yb), (sg * (h2 + rib), yb), (sg * h2, yb + rib)],
                "none",
            );
        }
        c.shape = "dims".into();
        let (rx, ry) = c.px(h2 + 0.35 * rib, yb + 0.35 * rib);
        c.leader(rx, ry, "rib=", rib, "mm", sp("rib_len"), "dimension");
    }
    let (ax, ay) = c.px(0.45 * dir.0 * s_end, 0.45 * dir.1 * s_end);
    c.leader(ax, ay, "", alpha, "deg", sp("alpha"), "dimension");
    let (hx, hy) = c.px(-l / 4.0, hb);
    c.leader(
        hx,
        hy,
        "H=",
        hb + dm / 2.0,
        "mm",
        der("tee_outlet", &[("hb", hb), ("od", dm)]),
        "dimension",
    );
    let cell = c.cell(1);
    let mid = (cell[1] + cell[3]) / 2.0;
    tee_branch_flat(c, [cell[0], cell[1], cell[2], mid], dm, db, hb, alpha);
    tee_pad_flat(
        c,
        [cell[0], mid, cell[2], cell[3]],
        dm,
        db,
        pad,
        padt,
        alpha,
    );
}

/// Developed pad plate and its hole from `nf_core::tee_pad::pad` (the engine's polylines, drawn as given).
fn tee_pad_flat(
    c: &mut Ctx,
    cell: [f64; 4],
    dm: f64,
    db: f64,
    pad: f64,
    padt: f64,
    alpha: f64,
) -> bool {
    let spec = nf_core::tee_pad::PadSpec {
        main_d: dm,
        t: padt,
        d: db,
        e: 0.0,
        angle_deg: Some(alpha),
        outline: nf_core::tee_pad::PadOutline::Circle(pad),
        split: None,
        cut: nf_core::tee_pad::SplitCut::Cross,
    };
    match nf_core::tee_pad::pad(&spec) {
        Ok(r) => dev_view(c, cell, &[r.outline.clone(), r.hole.clone()], "pad_flat"),
        Err(_) => {
            c.refused += 1;
            false
        }
    }
}

fn wye(c: &mut Ctx) {
    let d = *c.r.pick(&[60.0, 89.0, 114.0]);
    let a1 = *c.r.pick(&[30.0, 45.0]);
    let a2 = *c.r.pick(&[30.0, 45.0, 60.0]);
    let l1 = stepd(&mut c.r, d * 2.0, d * 4.0, 5.0);
    let l2 = stepd(&mut c.r, d * 2.0, d * 4.0, 5.0);
    let lm = (2.5 * d).floor();
    for (k, v) in [
        ("D", d),
        ("a1", a1),
        ("a2", a2),
        ("l1", l1),
        ("l2", l2),
        ("Lm", lm),
    ] {
        c.set(k, v);
    }
    let e1 = (a1.to_radians().cos() * l1, a1.to_radians().sin() * l1);
    let e2 = (a2.to_radians().cos() * l2, -a2.to_radians().sin() * l2);
    let bb = [
        -lm,
        (e2.1 - d / 2.0).min(-d / 2.0),
        e1.0.max(e2.0) + d,
        (e1.1 + d / 2.0).max(d / 2.0),
    ];
    // true silhouette of the equal-diameter wye: main edges to the joints, leg end faces square to the leg axes,
    // the crotch where the two inner generatrices meet
    let h = d / 2.0;
    let (u1, u2) = (
        (a1.to_radians().cos(), a1.to_radians().sin()),
        (a2.to_radians().cos(), -a2.to_radians().sin()),
    );
    let (n1, n2) = ((-u1.1, u1.0), (-u2.1, u2.0));
    let off = |p: (f64, f64), n: (f64, f64), k: f64| (p.0 + n.0 * k, p.1 + n.1 * k);
    // n2 = rot90(u2) points up (towards leg 1): the inner side of leg 2 is +n2, the outer side -n2
    let p1 = isect(off((0.0, 0.0), n1, h), u1, (0.0, h), (1.0, 0.0));
    let cc = isect(off((0.0, 0.0), n1, -h), u1, off((0.0, 0.0), n2, h), u2);
    let p2 = isect(off((0.0, 0.0), n2, -h), u2, (0.0, -h), (1.0, 0.0));
    let sil = vec![
        (-lm, -h),
        (-lm, h),
        p1,
        off(e1, n1, h),
        off(e1, n1, -h),
        cc,
        off(e2, n2, h),
        off(e2, n2, -h),
        p2,
    ];
    c.shape = "main".into();
    c.fit(c.cell(0), bbox(&sil));
    c.outline(&sil, "none");
    let _ = bb;
    c.shape = "dims".into();
    let (mx, my) = c.px(-lm / 2.0, d / 2.0);
    c.leader(mx, my, "\u{00D8}", d, "mm", sp("D"), "dimension");
    let (bx, by) = c.px(e1.0 * 0.5, e1.1 * 0.5);
    c.leader(bx, by, "L1=", l1, "mm", sp("l1"), "dimension");
    let (ax, ay) = c.px(e2.0 * 0.5, e2.1 * 0.5);
    c.leader(ax, ay, "L2=", l2, "mm", sp("l2"), "dimension");
    let (q1x, q1y) = c.px(0.75 * e1.0, 0.75 * e1.1);
    c.leader(q1x, q1y, "", a1, "deg", sp("a1"), "dimension");
    let (q2x, q2y) = c.px(0.75 * e2.0, 0.75 * e2.1);
    c.leader(q2x, q2y, "", a2, "deg", sp("a2"), "dimension");
    let (hx, hy) = c.px(e2.0, e2.1);
    c.leader(
        hx,
        hy,
        "H=",
        dv(
            "wye_span",
            &[("l1", l1), ("a1", a1), ("l2", l2), ("a2", a2)],
        ),
        "mm",
        der(
            "wye_span",
            &[("l1", l1), ("a1", a1), ("l2", l2), ("a2", a2)],
        ),
        "dimension",
    );
    wye_flat(c, c.cell(1), d, a1, a2, l1, l2, lm);
}

fn branch_on_cone(c: &mut Ctx) {
    let r1 = stepd(&mut c.r, 100.0, 300.0, 5.0);
    let r2 = stepd(&mut c.r, 40.0, ((r1 * 0.7) / 5.0).floor() * 5.0, 5.0);
    let h = stepd(&mut c.r, 150.0, 500.0, 5.0);
    let rb = *c.r.pick(&[20.0, 30.0, 40.0, 50.0]);
    let zt = stepd(
        &mut c.r,
        ((h * 0.25) / 5.0).ceil() * 5.0,
        ((h * 0.6) / 5.0).floor() * 5.0,
        5.0,
    );
    let lb = r1 + 120.0;
    let yb = h - zt;
    let wall = r1 - (r1 - r2) * yb / h;
    for (k, v) in [
        ("D1", 2.0 * r1),
        ("D2", 2.0 * r2),
        ("H", h),
        ("db", 2.0 * rb),
        ("z_top", zt),
        ("branch_len", lb),
    ] {
        c.set(k, v);
    }
    c.shape = "cone".into();
    c.fit(c.cell(0), [-r1, 0.0, r1 + lb, h]);
    c.outline(&[(-r1, 0.0), (-r2, h), (r2, h), (r1, 0.0)], "none");
    c.shape = "branch".into();
    c.outline(
        &[
            (wall, yb - rb),
            (wall + lb, yb - rb),
            (wall + lb, yb + rb),
            (wall, yb + rb),
        ],
        "none",
    );
    c.shape = "dims".into();
    let (bx, by) = c.px(r1 * 0.7, 0.0);
    c.leader(bx, by, "\u{00D8}", 2.0 * r1, "mm", sp("D1"), "dimension");
    let (tx, ty) = c.px(-r2 * 0.5, h);
    c.leader(tx, ty, "\u{00D8}", 2.0 * r2, "mm", sp("D2"), "dimension");
    let (lx, ly) = c.px(-r1, 0.0);
    let (_, ty2) = c.px(0.0, h);
    let u = c.u;
    c.dim_v(lx, ly, ty2, -42.0 * u, h, sp("H"), "", None);
    let (sx, sy) = c.px((r1 + r2) / 2.0 * 0.6, h / 2.0);
    c.leader(
        sx,
        sy,
        "S=",
        dv("cone_slant", &[("r1", r1), ("r2", r2), ("h", h)]),
        "mm",
        der("cone_slant", &[("r1", r1), ("r2", r2), ("h", h)]),
        "dimension",
    );
    let (ax, ay) = c.px(-(r1 + r2) / 2.0 * 0.5, h / 2.0);
    c.leader(
        ax,
        ay,
        "",
        dv("cone_half_angle", &[("r1", r1), ("r2", r2), ("h", h)]),
        "deg",
        der("cone_half_angle", &[("r1", r1), ("r2", r2), ("h", h)]),
        "dimension",
    );
    let (dbx, dby) = c.px(wall + lb, yb + rb);
    c.leader(dbx, dby, "\u{00D8}", 2.0 * rb, "mm", sp("db"), "dimension");
    let (zx, zy) = c.px(-r1 * 0.4, yb);
    c.leader(zx, zy, "z=", zt, "mm", sp("z_top"), "dimension");
    branch_on_cone_flat(c, c.cell(1), 2.0 * r1, 2.0 * r2, h, rb, zt, lb);
}

fn reducer_concentric(c: &mut Ctx) {
    let d1 = stepd(&mut c.r, 60.0, 200.0, 10.0);
    let d2 = stepd(&mut c.r, 30.0, ((d1 * 0.8) / 5.0).floor() * 5.0, 5.0);
    let l = stepd(&mut c.r, 60.0, 200.0, 5.0);
    let stub = 30.0;
    for (k, v) in [("D1", d1), ("D2", d2), ("L", l), ("stub", stub)] {
        c.set(k, v);
    }
    c.shape = "body".into();
    c.fit(
        c.cell(0),
        [-l / 2.0 - stub, -d1 / 2.0, l / 2.0 + stub, d1 / 2.0],
    );
    c.outline(
        &[
            (-l / 2.0 - stub, -d1 / 2.0),
            (-l / 2.0, -d1 / 2.0),
            (l / 2.0, -d2 / 2.0),
            (l / 2.0 + stub, -d2 / 2.0),
            (l / 2.0 + stub, d2 / 2.0),
            (l / 2.0, d2 / 2.0),
            (-l / 2.0, d1 / 2.0),
            (-l / 2.0 - stub, d1 / 2.0),
        ],
        "none",
    );
    c.shape = "dims".into();
    let (x0, y0) = c.px(-l / 2.0, -d1 / 2.0);
    let (x1, _) = c.px(l / 2.0, -d1 / 2.0);
    let u = c.u;
    c.dim_h(x0, x1, y0, 42.0 * u, l, sp("L"), "", None);
    let (ax, ay) = c.px(-l / 2.0 - stub * 0.5, d1 / 2.0);
    c.leader(ax, ay, "\u{00D8}", d1, "mm", sp("D1"), "dimension");
    let (bx, by) = c.px(l / 2.0 + stub * 0.5, d2 / 2.0);
    c.leader(bx, by, "\u{00D8}", d2, "mm", sp("D2"), "dimension");
    let (cx_, cy_) = c.px(0.0, (d1 + d2) / 4.0);
    c.leader(
        cx_,
        cy_,
        "",
        dv(
            "cone_half_angle",
            &[("r1", d1 / 2.0), ("r2", d2 / 2.0), ("h", l)],
        ),
        "deg",
        der(
            "cone_half_angle",
            &[("r1", d1 / 2.0), ("r2", d2 / 2.0), ("h", l)],
        ),
        "dimension",
    ); // developed sector of the cone (closed form, checked against nf-core cone_split)
    let (r1, r2) = (d1 / 2.0, d2 / 2.0);
    let rl = r1.max(r2);
    let rs = r1.min(r2);
    let sin_a = (rl - rs) / (l * l + (rl - rs) * (rl - rs)).sqrt();
    let n = 2.0;
    let big = dv(
        "sector_outer",
        &[("r1", r1), ("r2", r2), ("h", l), ("n", n)],
    );
    let small = dv(
        "sector_inner",
        &[("r1", r1), ("r2", r2), ("h", l), ("n", n)],
    );
    let ang = dv(
        "sector_angle_deg",
        &[("r1", r1), ("r2", r2), ("h", l), ("n", n)],
    );
    let engine_ok = match nf_core::shapes::cone_split(nf_core::shapes::ConeSpec {
        d1,
        d2,
        l,
        t: 2.0,
        basis: nf_core::shapes::Basis::Outer,
        split: 2,
    }) {
        Ok(e) => {
            (e.piece_angle - std::f64::consts::PI * sin_a).abs() < 1e-6
                || (e.piece_angle - 180.0 * sin_a).abs() < 1e-6
        }
        Err(_) => false,
    };
    if !engine_ok {
        c.geom_fail += 1;
    }
    let th = ang.to_radians();
    let mut poly: Vec<[f64; 2]> = vec![];
    for k in 0..=24 {
        let t = th * k as f64 / 24.0;
        poly.push([big * t.cos(), big * t.sin()]);
    }
    for k in (0..=24).rev() {
        let t = th * k as f64 / 24.0;
        poly.push([small * t.cos(), small * t.sin()]);
    }
    dev_view(c, c.cell(1), &[poly], "sector");
    c.shape = "dims".into();
    let (mx, my) = c.px(big * (th / 2.0).cos(), big * (th / 2.0).sin());
    let _ = (mx, my);
}

// ---------------------------------------------------------------- title, BOM, notes, symbols

fn title_block(c: &mut Ctx, drawing_no: &str, material: &str, date: &str, scale: &str) {
    let (w, h, u) = (c.w, c.h, c.u);
    let (x0, y0, x1, y1) = (0.64 * w, 0.84 * h, w - 20.0 * u, h - 20.0 * u);
    c.shape = "title".into();
    c.p.rect([x0, y0, x1, y1], 1.6 * u);
    let row = (y1 - y0) / 6.0;
    let f = 20.0 * u;
    let lang = c.p.lang;
    let name = names(&c.part.clone(), lang);
    let tx = x0 + 14.0 * u;
    let rows: [(&'static str, String, &str); 6] = [
        ("title", name.to_string(), "drawing_name"),
        ("title", format!("NO. {drawing_no}"), "drawing_no"),
        (
            "title",
            format!(
                "{} {material}",
                if lang == Lang::En {
                    "MATERIAL"
                } else {
                    "材質"
                }
            ),
            "material",
        ),
        (
            "title",
            format!(
                "{} {scale}",
                if lang == Lang::En { "SCALE" } else { "尺度" }
            ),
            "scale",
        ),
        ("tolerance_note", String::new(), "general_tolerance"),
        (
            "title",
            format!("{} {date}", if lang == Lang::En { "DATE" } else { "日付" }),
            "date",
        ),
    ];
    for (i, (kind, s, what)) in rows.iter().enumerate() {
        let y = y0 + row * (i as f64 + 0.7);
        if *kind == "tolerance_note" {
            let (std, cls) = if c.p.style == crate::layout::Style::Iso {
                ("ISO 2768-", c.gtol)
            } else {
                ("JIS B 0405-", c.gtol)
            };
            let txt = format!("{std}{cls}");
            let it = Item {
                kind: "tolerance_note",
                text: txt.clone(),
                value: json!(cls.to_string()),
                unit: "",
                tol: None,
                tol_source: None,
                src: Some(Source::ToleranceTable {
                    class: cls.to_string(),
                }),
                part: c.part.clone(),
                decimals: None,
                b: [0.0; 4],
            };
            if let Some(b) = c.p.emit(it, tx, y, Anchor::Start, f, false, false) {
                let _ = b;
                c.shapes.push("title".into());
            }
        } else {
            c.put_word(
                "title",
                s,
                what,
                tx,
                y,
                Anchor::Start,
                i == 0,
                if i == 0 { f * 1.3 } else { f },
            );
        }
    }
}

/// Dense-dimension pages: many small, crowded labels. Each one is the exact length of a straight edge drawn in the last
/// view (truth: derived `edge_len` from the drawn coordinate differences).
fn dense_edges(c: &mut Ctx, r: &mut Rng, want: usize) {
    let u = c.u;
    let mut cand: Vec<((f64, f64), (f64, f64))> = vec![];
    for p in c
        .vpolys
        .iter()
        .filter(|p| p.len() != CIRCLE_PTS && p.len() <= 40)
    {
        let n = p.len();
        for i in 0..if n > 2 { n } else { n - 1 } {
            let (a, b) = (p[i], p[(i + 1) % n]);
            if (a.0 - b.0).hypot(a.1 - b.1) * c.sc >= 28.0 * u {
                cand.push((a, b));
            }
        }
    }
    for i in (1..cand.len()).rev() {
        let j = r.int(0, i as i64) as usize;
        cand.swap(i, j);
    }
    let (fs0, ld0) = (c.fs, c.lead_d);
    c.fs = fs0 * 0.72;
    c.lead_d = 26.0 * u;
    c.shape = "dims".into();
    for (a, b) in cand.into_iter().take(want) {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let inp = [("dx", dx), ("dy", dy)];
        let v = dv("edge_len", &inp);
        let (x, y) = c.px((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        c.leader(x, y, "", v, "mm", der("edge_len", &inp), "dimension");
    }
    c.fs = fs0;
    c.lead_d = ld0;
}

/// Negative examples: text a reader must not report as a dimension. Drawn in the image, never in the answer key.
fn weak1_clutter(p: &mut Page, r: &mut Rng, drawing_no: &str) {
    let (w, h, u) = (p.w, p.h, p.u);
    let ja = p.lang == Lang::Ja;
    let f = 17.0 * u;
    // grid references along the top and left inside edges of the frame
    for i in 0..6 {
        let x = w * (i as f64 + 0.5) / 6.0;
        p.clutter_text(
            &((b'A' + i as u8) as char).to_string(),
            x,
            50.0 * u,
            Anchor::Middle,
            f,
            false,
        );
        p.clutter_text(
            &(i + 1).to_string(),
            40.0 * u,
            h * (i as f64 + 0.5) / 6.0,
            Anchor::Middle,
            f,
            false,
        );
    }
    // revision table above the bill of materials
    let (x0, x1) = (0.64 * w, w - 20.0 * u);
    let (y0, rh) = (0.31 * h, 0.05 * h);
    let rows = r.int(2, 3) as usize;
    p.rect([x0, y0, x1, y0 + rh * (rows as f64 + 1.0)], 1.2 * u);
    for k in 1..=rows {
        let y = y0 + rh * k as f64;
        p.line(x0, y, x1, y, 1.0 * u);
    }
    let cols = [0.03, 0.14, 0.38, 0.9];
    let head = if ja {
        ["改訂", "日付", "内容", "担当"]
    } else {
        ["REV", "DATE", "DESCRIPTION", "BY"]
    };
    let what = if ja {
        ["初版", "寸法修正", "図面番号変更"]
    } else {
        ["FIRST ISSUE", "DIM CORRECTED", "DWG NO CHANGED"]
    };
    for (i, s) in head.iter().enumerate() {
        p.clutter_text(
            s,
            x0 + cols[i] * (x1 - x0),
            y0 + 0.62 * rh,
            Anchor::Start,
            f,
            true,
        );
    }
    for (k, what_k) in what.iter().enumerate().take(rows) {
        let y = y0 + rh * (k as f64 + 1.62);
        let date = format!("2026-{:02}-{:02}", r.int(1, 12), r.int(1, 28));
        let rev = ((b'A' + k as u8) as char).to_string();
        for (i, s) in [rev.as_str(), date.as_str(), what_k, "VS"]
            .iter()
            .enumerate()
        {
            p.clutter_text(s, x0 + cols[i] * (x1 - x0), y, Anchor::Start, f, false);
        }
    }
    // numbered notes, part and drawing numbers at free spots
    let pool_en = [
        "NOTE 1. WELD PER JIS Z 3801".to_string(),
        "2. PAINT 2 COATS 80 MICRON".to_string(),
        "REF DWG NO. 4417-203".to_string(),
        "P/N 4400-217-03".to_string(),
        "PART NO. 25-A113".to_string(),
        "SHT 1 OF 2".to_string(),
        format!("DWG NO. {drawing_no}"),
        "ITEM 3 QTY 2".to_string(),
        "3. HYDROTEST 1.5 X DESIGN".to_string(),
    ];
    let pool_ja = [
        "注1. 溶接はJIS Z 3801による".to_string(),
        "2. 塗装 2回 80 ミクロン".to_string(),
        "参照図 4417-203".to_string(),
        "品番 4400-217-03".to_string(),
        "部品番号 25-A113".to_string(),
        "第1葉 全2葉".to_string(),
        format!("図面番号 {drawing_no}"),
        "3. 水圧試験 設計圧の1.5倍".to_string(),
    ];
    let pool: &[String] = if ja { &pool_ja } else { &pool_en };
    for _ in 0..r.int(6, 10) {
        let s = pool[r.int(0, pool.len() as i64 - 1) as usize].clone();
        for _ in 0..40 {
            let (x, y) = (w * r.range(0.04, 0.58), h * r.range(0.08, 0.95));
            if p.clutter_text(&s, x, y, Anchor::Start, f, false) {
                break;
            }
        }
    }
}

fn bom(c: &mut Ctx) {
    let (w, h, u) = (c.w, c.h, c.u);
    let (x0, y0, x1, y1) = (0.64 * w, 0.60 * h, w - 20.0 * u, 0.84 * h - 4.0 * u);
    c.shape = "bom".into();
    c.p.rect([x0, y0, x1, y1], 1.2 * u);
    let lang = c.p.lang;
    let n = if c.weak_bom { c.r.int(6, 11) } else { 0 };
    let rowh = (y1 - y0) / if c.weak_bom { n as f64 + 1.0 } else { 5.0 };
    let f = if c.weak_bom {
        (19.0 * u).min(rowh * 0.62)
    } else {
        19.0 * u
    };
    let cols = [0.03, 0.12, 0.62, 0.78];
    let head = if lang == Lang::En {
        ["NO.", "PART", "QTY", "MATERIAL"]
    } else {
        ["番号", "部品名", "員数", "材質"]
    };
    for (i, s) in head.iter().enumerate() {
        let x = x0 + cols[i] * (x1 - x0);
        let y = y0 + rowh * 0.7;
        c.shape = "bom_header".into();
        c.put_word("bom_header", s, "bom_header", x, y, Anchor::Start, false, f);
    }
    let n = if c.weak_bom { n } else { c.r.int(2, 4) };
    let mats = ["SS400", "SUS304", "A5052", "STPG370"];
    let parts_en = ["BODY", "COVER", "GUSSET", "PAD", "NUT"];
    let parts_ja = ["本体", "カバー", "ガセット", "パッド", "ナット"];
    for i in 0..n as usize {
        let y = y0 + rowh * (i as f64 + 1.7);
        let k = c.r.int(0, 4) as usize;
        let no = format!("{}", i + 1);
        c.shape = "bom".into();
        c.put_word(
            "bom",
            &no,
            "bom",
            x0 + cols[0] * (x1 - x0),
            y,
            Anchor::Start,
            false,
            f,
        );
        let pn = if lang == Lang::En {
            parts_en[k]
        } else {
            parts_ja[k]
        };
        c.put_word(
            "bom",
            pn,
            "bom",
            x0 + cols[1] * (x1 - x0),
            y,
            Anchor::Start,
            false,
            f,
        );
        let q = c.r.int(1, 8) as f64;
        let qx = x0 + cols[2] * (x1 - x0);
        let it = Item {
            kind: "bom",
            text: format!("{}", q as i64),
            value: json!(q as i64),
            unit: "pcs",
            tol: None,
            tol_source: None,
            src: Some(Source::Structure {
                what: "quantity".into(),
            }),
            part: c.part.clone(),
            decimals: Some(0),
            b: [0.0; 4],
        };
        if c.p
            .emit(it, qx, y, Anchor::Start, f, false, false)
            .is_some()
        {
            c.shapes.push("bom".into());
        }
        c.put_word(
            "bom",
            mats[k % mats.len()],
            "bom",
            x0 + cols[3] * (x1 - x0),
            y,
            Anchor::Start,
            false,
            f,
        );
    }
}

fn notes(c: &mut Ctx) {
    let (w, h, u) = (c.w, c.h, c.u);
    let lang = c.p.lang;
    let x = 0.64 * w;
    let mut y = 60.0 * u;
    let f = 19.0 * u;
    c.shape = "notes".into();
    let lines_en = [
        "UNLESS OTHERWISE SPECIFIED",
        "DIMENSIONS IN mm",
        "DEBURR ALL EDGES",
        "BREAK SHARP EDGES 0.5 MAX",
    ];
    let lines_ja = [
        "特記なき限り",
        "寸法単位 mm",
        "全ての角をバリ取り",
        "鋭い角面取り 0.5 以下",
    ];
    let lines = if lang == Lang::En { lines_en } else { lines_ja };
    for s in lines {
        c.put_word("note", s, "note", x, y + f, Anchor::Start, false, f);
        y += 1.9 * f;
    }
    let ra = *c.r.pick(&[0.8, 1.6, 3.2, 6.3, 12.5]);
    c.set("ra_general", ra);
    let _ = h;
    let (tx, ty) = (x, y + f);
    let txt = if lang == Lang::En {
        "SURFACE Ra"
    } else {
        "表面粗さ Ra"
    };
    c.put_num(
        "surface_finish",
        &format!("{txt} "),
        ra,
        "um",
        None,
        sp("ra_general"),
        &[(tx, ty, Anchor::Start)],
    );
}

/// Handwritten-looking note: a rotated italic line with a number. Degradation 'hand'.
fn hand_note(c: &mut Ctx) {
    let choices = [
        ("BREAK EDGES", 0.5),
        ("CHECK FIT", 2.0),
        ("REAM", 6.0),
        ("TOL CHECK", 1.0),
    ];
    let (label, v0) = *c.r.pick(&choices);
    let dec = c.p.dec;
    let v = round_to(v0, dec);
    let txt = format!("{label} {v:.dec$}");
    let fs = c.fs * 0.95;
    let ang = c.r.range(-4.0, 4.0);
    let x = c.w * c.r.range(0.05, 0.55);
    let y = c.h * c.r.range(0.08, 0.92);
    let b = Page::text_box(x, y, &txt, fs);
    if !c.p.free(b) {
        return;
    }
    // stroke-jitter handwriting: each glyph gets its own rotation and baseline wobble (no handwriting font installed)
    let mut el = String::new();
    let mut gx = x;
    for ch in txt.chars() {
        let gr = c.r.range(-3.0, 3.0);
        let gy = y + c.r.range(-1.5, 1.5) * c.u;
        el.push_str(&format!(
            "<text x=\"{gx:.1}\" y=\"{gy:.1}\" font-size=\"{fs:.1}\" font-family=\"{FONT}\" font-style=\"italic\" transform=\"rotate({gr:.2} {gx:.1} {gy:.1})\" fill=\"#000\">{}</text>",
            esc(&ch.to_string())
        ));
        gx += crate::layout::glyph_adv(ch) * fs;
    }
    let el = format!("<g transform=\"rotate({ang:.2} {x:.1} {y:.1})\">{el}</g>");
    c.p.raw(&el);
    c.p.text_body.push_str(&el);
    c.p.text_body.push('\n');
    let pad = 0.2 * fs;
    let bp = [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad];
    let rb = rot_box(bp, ang, x, y);
    c.p.items.push(Item {
        kind: "note",
        text: txt.clone(),
        value: json!(v),
        unit: "mm",
        tol: None,
        tol_source: None,
        src: Some(Source::Structure {
            what: "hand_note".into(),
        }),
        part: c.part.clone(),
        decimals: Some(dec),
        b: rb,
    });
    c.shapes.push("hand_note".into());
    c.p.segs.push([b[0], b[1], b[2], b[1]]);
    c.p.segs.push([b[2], b[3], b[0], b[3]]);
}

// ---------------------------------------------------------------- page

pub struct Page2 {
    pub refused: usize,
    pub geom_fail: usize,
    pub page: Page,
    pub shapes: Vec<String>,
    pub params: BTreeMap<String, f64>,
    pub fam: &'static str,
    pub seed: u64,
    pub fx: Option<Fx>,
    pub deg: &'static str,
    pub w: u32,
    pub h: u32,
    pub part_id: String,
}

pub fn build_new(
    seed: u64,
    k: u64,
    fam: &'static str,
    lang: Lang,
    style: Style,
    deg: &'static str,
    mix_: Mix,
) -> Page2 {
    let mut rl = Rng::new(mix(&[seed, 20]));
    let w = rl.int(1600, 2400) as f64;
    let h = (w / std::f64::consts::SQRT_2).round();
    let dec = if rl.chance(0.5) { 1 } else { 2 };
    let gtol = *rl.pick(&['f', 'm', 'm', 'c']);
    let page = Page::new(w, h, style, lang, dec, gtol);
    let u = w / 2000.0;
    let mut c = Ctx {
        p: page,
        r: Rng::new(mix(&[seed, 21])),
        par: BTreeMap::new(),
        sc: 1.0,
        ox: 0.0,
        oy: 0.0,
        part: fam.to_string(),
        shape: String::new(),
        shapes: vec![],
        gtol,
        u,
        fs: 22.0 * u,
        w,
        h,
        refused: 0,
        geom_fail: 0,
        vpolys: vec![],
        skip_chk: false,
        weak: mix_ == Mix::Weak1,
        weak_bom: false,
        lead_d: 60.0 * u,
    };
    c.p.rect([20.0 * u, 20.0 * u, w - 20.0 * u, h - 20.0 * u], 2.2 * u);
    match fam {
        "plate_holes" => plate(&mut c),
        "ring_flange" => ring_flange(&mut c),
        "bracket_gusset" => bracket_gusset(&mut c),
        "flat_sheet" => flat_sheet(&mut c),
        "duct_transition" => duct_transition(&mut c),
        "tank_shell" => tank_shell(&mut c),
        "mitred_bend" => mitred_bend(&mut c),
        "tee_lateral" => tee_lateral(&mut c),
        "wye" => wye(&mut c),
        "branch_on_cone" => branch_on_cone(&mut c),
        _ => reducer_concentric(&mut c),
    }
    let mut rw = Rng::new(mix(&[seed, 23]));
    if c.weak {
        c.weak_bom = rw.chance(0.8);
        if rw.chance(0.7) {
            let want = rw.int(8, 16) as usize;
            dense_edges(&mut c, &mut rw, want);
        }
    }
    // weld and surface-finish symbols (about half the pages), anchored on the first view's frame
    let mut ri = Rng::new(mix(&[seed, 22]));
    if ri.chance(0.5) {
        let leg = *ri.pick(&[3.0, 4.0, 5.0, 6.0, 8.0]);
        c.set("weld_leg", leg);
        c.shape = "weld".into();
        let ax = c.cell(2)[0] + (c.cell(2)[2] - c.cell(2)[0]) * ri.range(0.1, 0.9);
        let ay = c.cell(2)[3] - 60.0 * u;
        c.weld(ax, ay, leg);
    }
    if ri.chance(0.5) {
        let ra = *ri.pick(&[0.8, 1.6, 3.2, 6.3, 12.5]);
        c.set("ra", ra);
        c.shape = "finish".into();
        let ax = c.cell(2)[0] + 60.0 * u;
        let ay = c.cell(2)[1] + 80.0 * u + ri.range(0.0, 200.0) * u;
        c.finish(ax, ay, ra);
    }
    let drawing_no = format!("NFR-{}-{:08X}", fam_code(fam), seed & 0xFFFF_FFFF);
    let date = format!("2026-{:02}-{:02}", rl.int(1, 12), rl.int(1, 28));
    let material = *rl.pick(&["SS400", "SUS304", "SUS316L", "STPG370", "A5052"]);
    let scale = *rl.pick(&["1:1", "1:2", "1:5", "2:1"]);
    title_block(&mut c, &drawing_no, material, &date, scale);
    bom(&mut c);
    notes(&mut c);
    if c.weak {
        weak1_clutter(&mut c.p, &mut rw, &drawing_no);
    }
    if deg == "hand" {
        for _ in 0..2 {
            hand_note(&mut c);
        }
    }
    let fx = match deg {
        "scan" => {
            let mut rf = Rng::new(mix(&[seed, 3]));
            let mut skew = rf.range(0.3, 3.0);
            if rf.chance(0.5) {
                skew = -skew;
            }
            let low = rf.chance(0.4);
            Some(Fx {
                skew,
                blur: if rf.chance(0.6) {
                    *rf.pick(&[0.6, 1.0, 1.4])
                } else {
                    0.0
                },
                noise: *rf.pick(&[0.6, 1.0, 1.6]),
                lo: if low { rf.int(15, 35) as u32 } else { 0 },
                hi: if low { rf.int(65, 85) as u32 } else { 100 },
                jpeg: rf.int(25, 70) as u32,
                seed: rf.int(1, 1_000_000) as u64,
                fax: false,
            })
        }
        "fax" => Some(Fx {
            skew: 0.0,
            blur: 0.6,
            noise: 0.8,
            lo: 0,
            hi: 100,
            jpeg: 0,
            seed: mix(&[seed, 4]) % 1_000_000,
            fax: true,
        }),
        _ => None,
    };
    let part_id = format!("{}-{}", fam_code(fam), drawing_no);
    let _ = k;
    Page2 {
        refused: c.refused,
        geom_fail: c.geom_fail,
        page: c.p,
        shapes: c.shapes,
        params: c.par,
        fam,
        seed,
        fx,
        deg,
        w: w as u32,
        h: h as u32,
        part_id,
    }
}

// ANSWER-KEY POLICY (one rule for every page, every family, every mix: default and weak1; train, val and test).
//   READINGS, in the key: dimensions (linear, diameter, radius, angle, thickness callouts), BOM table cells (header,
//   part, quantity, material), tolerance notes (general tolerance class), surface finish, weld symbols.
//   CLUTTER, never in the key but printed: title-block fields (name, drawing no., material, scale, date), notes text
//   (general notes, hand notes), revision table, grid letters and numbers, part and drawing numbers, numbered notes.
// Items of kind `title` and `note` are demoted to clutter here, so engine-backed and generator-owned pages agree.
// The truth file lists clutter as objects with id and box so tests can compare by element, never by text.
const CLUTTER_KINDS: [&str; 2] = ["title", "note"];

/// Key items with their shape labels, and clutter as (text, box).
type KeySplit<'a> = (Vec<(&'a Item, String)>, Vec<(String, B)>);

/// Split a page into key items and clutter.
fn key_split<'a>(page: &'a Page, shapes: &'a [String]) -> KeySplit<'a> {
    let mut keep = vec![];
    let mut clutter: Vec<(String, B)> = page
        .clutter
        .iter()
        .cloned()
        .zip(page.cboxes.iter().copied())
        .collect();
    for (i, it) in page.items.iter().enumerate() {
        if CLUTTER_KINDS.contains(&it.kind) {
            clutter.push((it.text.clone(), it.b));
        } else {
            keep.push((it, shapes.get(i).cloned().unwrap_or_default()));
        }
    }
    (keep, clutter)
}

fn clutter_json(pid: &str, i: usize, text: &str, b: B, skew: f64, w: f64, h: f64) -> Value {
    json!({
        "id": format!("{pid}-c{i:03}"),
        "text": text,
        "box": r1(rot_box(b, skew, w / 2.0, h / 2.0)),
        "box_page": r1(b),
    })
}

/// One truth item as a JSON object (schema v2).
#[allow(clippy::too_many_arguments)]
fn item_json(
    it: &Item,
    shape: &str,
    pid: &str,
    idx: usize,
    lang: Lang,
    style: Style,
    fam: &str,
    part_id: &str,
    seed: u64,
    skew: f64,
    w: f64,
    h: f64,
) -> Value {
    let rb = rot_box(it.b, skew, w / 2.0, h / 2.0);
    json!({
        "id": format!("{pid}-{idx:03}"),
        "type": it.kind,
        "text": it.text,
        "value": it.value,
        "unit": it.unit,
        "tolerance": it.tol.as_ref().map(|t| json!({"plus": t.plus, "minus": t.minus})),
        "tolerance_source": it.tol_source,
        "source": it.src,
        "decimals": it.decimals,
        "box": r1(rb),
        "box_page": r1(it.b),
        "page": 0,
        "language": lang.name(),
        "style": style.name(),
        "family": fam,
        "part": it.part,
        "part_id": part_id,
        "seed": seed,
        "shape": shape,
    })
}

/// Page-level truth fields shared by v1 and v2 pages.
pub fn page_truth(p: &P2Meta, items: Vec<Value>) -> Value {
    json!({
        "schema": SCHEMA,
        "page_id": p.pid,
        "split": "smoke",
        "family": p.fam,
        "part_id": p.part_id,
        "seed": p.seed,
        "language": p.lang.name(),
        "style": p.style.name(),
        "scan": p.deg == "scan",
        "degradation": p.deg,
        "scan_effects": p.fx.as_ref().filter(|_| p.deg == "scan" || p.deg == "fax").map(|f| json!({"skew_deg": (f.skew * 1000.0).round() / 1000.0, "blur_sigma": f.blur, "noise": f.noise,
            "level_lo_pct": f.lo, "level_hi_pct": f.hi, "jpeg_quality": f.jpeg, "noise_seed": f.seed, "fax": f.fax})),
        "width": p.w,
        "height": p.h,
        "decimals": p.dec,
        "general_tolerance": p.gtol.to_string(),
        "png": format!("smoke/png/{}.png", p.pid),
        "pdf": format!("smoke/pdf/{}.pdf", p.pid),
        "engine": p.engine,
        "request": p.request,
        "spec": {"family": p.fam, "params": p.spec_params},
        "refused": p.refused,
        "geom_fail": p.geom_fail,
        "items": items,
    })
}

/// Page metadata needed to write a truth line.
pub struct P2Meta {
    pub pid: String,
    pub fam: &'static str,
    pub part_id: String,
    pub seed: u64,
    pub lang: Lang,
    pub style: Style,
    pub deg: &'static str,
    pub fx: Option<Fx>,
    pub w: u32,
    pub h: u32,
    pub dec: usize,
    pub gtol: char,
    pub engine: String,
    pub request: Value,
    pub spec_params: Value,
    pub refused: usize,
    pub geom_fail: usize,
}

/// Write PNG and PDF for a page. Faint pages get lighter lines; fax pages are 1-bit (see render.rs).
#[allow(clippy::too_many_arguments)]
pub fn write_images(
    root: &Path,
    split: &str,
    pid: &str,
    page: &Page,
    w: u32,
    h: u32,
    deg: &str,
    fx: Option<&Fx>,
) -> Result<(), String> {
    let dir_png = root.join(split).join("png");
    let dir_pdf = root.join(split).join("pdf");
    std::fs::create_dir_all(&dir_png).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir_pdf).map_err(|e| e.to_string())?;
    let skew = fx.map(|f| f.skew).unwrap_or(0.0);
    let paper = if deg == "scan" { "#f3f1ea" } else { "#ffffff" };
    let faint = |s: String| {
        if deg == "faint" {
            s.replace("stroke=\"#000\"", "stroke=\"#a6a6a6\"")
        } else {
            s
        }
    };
    let s_png = faint(svg(page, skew, paper));
    render_png(&s_png, w, h, &dir_png.join(format!("{pid}.png")), fx)?;
    render_pdf(
        &faint(svg(page, 0.0, "#ffffff")),
        &dir_pdf.join(format!("{pid}.pdf")),
    )
}

fn build_one(
    seed: u64,
    k: u64,
    fam: &'static str,
    lang: Lang,
    style: Style,
    deg: &'static str,
    mix_: Mix,
) -> Result<(String, P2Meta, Page, Vec<String>, f64), String> {
    let _ = k;
    let pid = format!("smoke-{k:05}");
    if ENGINE_FAMILIES.contains(&fam) {
        let mut rp = Rng::new(mix(&[seed, 1]));
        let (req, out) = sample(fam, &mut rp);
        let scan = deg == "scan";
        let sp_ = Spec {
            split: "smoke".into(),
            k,
            fam,
            lang,
            style,
            scan,
            seed,
        };
        let b = build(&sp_, &req, out);
        let fx = match deg {
            "scan" => b.fx.clone(),
            "fax" => Some(Fx {
                skew: 0.0,
                blur: 0.6,
                noise: 0.8,
                lo: 0,
                hi: 100,
                jpeg: 0,
                seed: mix(&[seed, 4]) % 1_000_000,
                fax: true,
            }),
            _ => None,
        };
        let skew = if scan {
            b.fx.as_ref().map(|f| f.skew).unwrap_or(0.0)
        } else {
            0.0
        };
        let shapes: Vec<String> = b.page.items.iter().map(|it| it.part.clone()).collect();
        let items: Vec<Value> = b
            .page
            .items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                item_json(
                    it,
                    &it.part,
                    &pid,
                    i,
                    lang,
                    style,
                    fam,
                    &format!("{}-{}", fam_code(fam), seed),
                    seed,
                    skew,
                    b.w as f64,
                    b.h as f64,
                )
            })
            .collect();
        let deg_eff: &'static str = if deg == "hand" { "clean" } else { deg };
        let meta = P2Meta {
            pid: pid.clone(),
            fam,
            part_id: format!("{}-{}", fam_code(fam), seed),
            seed,
            lang,
            style,
            deg: deg_eff,
            fx: fx.clone(),
            w: b.w,
            h: b.h,
            dec: b.page.dec,
            gtol: b.gtol,
            engine: b.out.engine.clone(),
            request: serde_json::to_value(&b.out.request).unwrap_or(Value::Null),
            spec_params: serde_json::to_value(&b.out.request).unwrap_or(Value::Null),
            refused: 0,
            geom_fail: 0,
        };
        let _ = items;
        let mut page = b.page;
        if mix_ == Mix::Weak1 {
            // engine-backed pages keep their layout; the negative examples go into the free space around it
            let mut rw = Rng::new(mix(&[seed, 23]));
            weak1_clutter(
                &mut page,
                &mut rw,
                &format!("NFR-{}-{:08X}", fam_code(fam), seed & 0xFFFF_FFFF),
            );
        }
        Ok((pid, meta, page, shapes, skew))
    } else {
        let p2 = build_new(seed, k, fam, lang, style, deg, mix_);
        let skew = p2.fx.as_ref().map(|f| f.skew).unwrap_or(0.0);
        let meta = P2Meta {
            pid: pid.clone(),
            fam,
            part_id: p2.part_id.clone(),
            seed,
            lang,
            style,
            deg,
            fx: p2.fx.clone(),
            w: p2.w,
            h: p2.h,
            dec: p2.page.dec,
            gtol: p2.page.gtol,
            engine: "none (generator-owned geometry)".into(),
            request: json!({}),
            spec_params: serde_json::to_value(&p2.params).unwrap_or(Value::Null),
            refused: p2.refused,
            geom_fail: p2.geom_fail,
        };
        Ok((pid, meta, p2.page, p2.shapes, skew))
    }
}

/// Generate `n` smoke pages into `out/smoke/{png,pdf}` and `out/smoke/truth.jsonl`. Deterministic by `seed`.
pub fn generate_smoke(out: &Path, n: u64, seed: u64, threads: usize) -> Result<usize, String> {
    generate_smoke_mix(out, n, seed, threads, Mix::Default)
}

/// As `generate_smoke`, with a named generator mix (`Mix::Weak1`: see [`Mix`]).
pub fn generate_smoke_mix(
    out: &Path,
    n: u64,
    seed: u64,
    threads: usize,
    mix_: Mix,
) -> Result<usize, String> {
    let split_dir = out.join("smoke");
    std::fs::create_dir_all(&split_dir).map_err(|e| e.to_string())?;
    let fams = all_families();
    let ks: Vec<u64> = (0..n).collect();
    let mut res: Vec<(u64, Result<String, String>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads.max(1))
            .map(|t| {
                let ks = &ks;
                let fams = &fams;
                sc.spawn(move || {
                    let mut v = vec![];
                    for &k in ks.iter().skip(t).step_by(threads.max(1)) {
                        let fam = fams[(k % fams.len() as u64) as usize];
                        let lang = if (k / 16) % 2 == 0 {
                            Lang::En
                        } else {
                            Lang::Ja
                        };
                        let style = if (k / 32) % 2 == 0 {
                            Style::Iso
                        } else {
                            Style::Jis
                        };
                        let deg = DEGS[(k % 5) as usize];
                        let sd = mix(&[seed, 0x5A, k]);
                        let r = build_one(sd, k, fam, lang, style, deg, mix_).and_then(
                            |(pid, meta, page, shapes, skew)| {
                                write_images(
                                    out,
                                    "smoke",
                                    &pid,
                                    &page,
                                    meta.w,
                                    meta.h,
                                    meta.deg,
                                    meta.fx.as_ref(),
                                )?;
                                let (kept, clut) = key_split(&page, &shapes);
                                let items: Vec<Value> = kept
                                    .iter()
                                    .enumerate()
                                    .map(|(i, (it, shape))| {
                                        item_json(
                                            it,
                                            shape,
                                            &pid,
                                            i,
                                            meta.lang,
                                            meta.style,
                                            meta.fam,
                                            &meta.part_id,
                                            meta.seed,
                                            skew,
                                            meta.w as f64,
                                            meta.h as f64,
                                        )
                                    })
                                    .collect();
                                let mut tv = page_truth(&meta, items);
                                tv["mix"] = json!(mix_.name());
                                // printed but not in the answer key: negative examples
                                tv["clutter"] = json!(
                                    clut.iter()
                                        .enumerate()
                                        .map(|(i, (t, b))| clutter_json(
                                            &pid,
                                            i,
                                            t,
                                            *b,
                                            skew,
                                            meta.w as f64,
                                            meta.h as f64
                                        ))
                                        .collect::<Vec<_>>()
                                );
                                Ok(tv.to_string())
                            },
                        );
                        v.push((k, r));
                    }
                    v
                })
            })
            .collect();
        hs.into_iter()
            .flat_map(|h| h.join().unwrap_or_default())
            .collect()
    });
    res.sort_by_key(|x| x.0);
    use std::io::Write;
    let mut f = std::io::BufWriter::new(
        std::fs::File::create(split_dir.join("truth.jsonl")).map_err(|e| e.to_string())?,
    );
    let mut count = 0;
    for (k, l) in res {
        let l = l.map_err(|e| format!("smoke page {k}: {e}"))?;
        writeln!(f, "{l}").map_err(|e| e.to_string())?;
        count += 1;
    }
    f.flush().map_err(|e| e.to_string())?;
    Ok(count)
}

/// Re-derive a v2 truth value from the page's stored spec and inputs. `None` for engine values (re-run the engine)
/// and for structural numbers.
pub fn resolve_v2(params: &Value, src: &Source) -> Option<f64> {
    match src {
        Source::Spec { field, factor } => params.get(field)?.as_f64().map(|v| v * factor),
        Source::Derived { name, inputs } => derive(name, inputs),
        _ => None,
    }
}
