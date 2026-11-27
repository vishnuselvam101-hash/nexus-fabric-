//! Page composition: views, dimension lines, notes, BOM, title block. Every printed number carries a `Source`.

use crate::layout::*;
use crate::rng::Rng;
use crate::source::{Source, linear_tolerance};
use nf_core::job::{JobOutput, PatternView};
use serde_json::json;

pub struct Ids {
    pub drawing_no: String,
    pub date: String,
    pub material: &'static str,
}

pub const MATERIALS: [&str; 5] = ["SS400", "SUS304", "SUS316L", "STPG370", "A5052"];

fn t2(lang: Lang, en: &'static str, ja: &'static str) -> &'static str {
    if lang == Lang::En { en } else { ja }
}

pub fn part_name(fam: &str, part: &str, lang: Lang) -> &'static str {
    match (fam, part) {
        (_, "shell") => t2(lang, "Cylinder shell", "円筒シェル"),
        ("cone_frustum", _) => t2(lang, "Cone frustum", "円錐台"),
        (_, "reducer") => t2(lang, "Eccentric reducer", "偏心レデューサ"),
        (_, "header") => t2(lang, "Header", "ヘッダ管"),
        _ => t2(lang, "Branch stub", "枝管"),
    }
}
pub fn drawing_name(fam: &str, lang: Lang) -> &'static str {
    match fam {
        "header_with_branch" => t2(lang, "Header with branch", "枝付きヘッダ"),
        "cylinder_shell" => part_name(fam, "shell", lang),
        "cone_frustum" | "eccentric_reducer" => part_name(fam, "reducer", lang),
        _ => part_name(fam, "branch", lang),
    }
}

pub fn bbox(p: &PatternView) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    );
    for c in p.curves.iter().filter(|c| c.layer == "CUT_OUTER") {
        for q in &c.points {
            x0 = x0.min(q[0]);
            x1 = x1.max(q[0]);
            y0 = y0.min(q[1]);
            y1 = y1.max(q[1]);
        }
    }
    (x0, y0, x1, y1)
}

struct Fit {
    xmin: f64,
    ymax: f64,
    s: f64,
    ox: f64,
    oy: f64,
}
impl Fit {
    fn new(pts: &[(f64, f64)], r: B) -> Fit {
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for (x, y) in pts {
            x0 = x0.min(*x);
            x1 = x1.max(*x);
            y0 = y0.min(*y);
            y1 = y1.max(*y);
        }
        let (w, h) = ((x1 - x0).max(1e-6), (y1 - y0).max(1e-6));
        let s = ((r[2] - r[0]) / w).min((r[3] - r[1]) / h);
        Fit {
            xmin: x0,
            ymax: y1,
            s,
            ox: r[0] + ((r[2] - r[0]) - w * s) / 2.0,
            oy: r[1] + ((r[3] - r[1]) - h * s) / 2.0,
        }
    }
    fn p(&self, x: f64, y: f64) -> (f64, f64) {
        (
            self.ox + (x - self.xmin) * self.s,
            self.oy + (self.ymax - y) * self.s,
        )
    }
}

type Cand = (String, Source, &'static str, (f64, f64), f64);

const FS_DIM: f64 = 25.0;

fn tol_for(p: &Page, rng: &mut Rng, nominal: f64) -> Option<f64> {
    if !rng.chance(0.3) {
        return None;
    }
    linear_tolerance(&p.gtol.to_string(), nominal)
}

fn arrow(p: &mut Page, x: f64, y: f64, dx: f64, dy: f64) {
    let a = 13.0 * p.u;
    let (nx, ny) = (-dy, dx);
    let b = ((x + dx * a + nx * a * 0.3), (y + dy * a + ny * a * 0.3));
    let c = ((x + dx * a - nx * a * 0.3), (y + dy * a - ny * a * 0.3));
    // an arrowhead that would touch text is left out: a line never runs through a written number
    let g = 1.5 * p.u;
    let hit = |s: B| p.items.iter().any(|i| seg_hits(s, i.b, g));
    if hit([x, y, b.0, b.1]) || hit([x, y, c.0, c.1]) || hit([b.0, b.1, c.0, c.1]) {
        return;
    }
    if p.style == Style::Jis {
        p.poly(&[(x, y), b, c], true, 1.0, "#000");
    } else {
        p.poly(&[b, (x, y), c], false, 1.6 * p.u, "none");
    }
}

#[allow(clippy::too_many_arguments)]
fn hdim(
    p: &mut Page,
    rng: &mut Rng,
    x0: f64,
    x1: f64,
    y: f64,
    ext_y: f64,
    part: &str,
    prefix: &str,
    v: f64,
    src: Source,
) {
    let u = p.u;
    let fs = FS_DIM * u;
    let tol = tol_for(p, rng, v);
    let jis = p.style == Style::Jis;
    let base = if jis { y + 0.33 * fs } else { y - 8.0 * u };
    let mid = (x0 + x1) / 2.0;
    let dec = p.dec;
    let tol_txt = tol.map(|t| format!(" \u{00B1}{t}")).unwrap_or_default();
    let est = text_w(&format!("{prefix}{v:.dec$}{tol_txt}"), fs);
    // text first (it must not cross anything already drawn), then the lines around it
    let mut stub = false;
    let mut placed = est + 44.0 * u < (x1 - x0)
        && p.num(
            "dimension",
            part,
            prefix,
            v,
            "mm",
            tol,
            src.clone(),
            mid,
            base,
            Anchor::Middle,
            fs,
            jis,
        )
        .is_some();
    if !placed {
        stub = true;
        placed = p
            .num(
                "dimension",
                part,
                prefix,
                v,
                "mm",
                tol,
                src.clone(),
                x1 + 16.0 * u,
                y + 0.33 * fs,
                Anchor::Start,
                fs,
                false,
            )
            .is_some();
        if !placed {
            stub = false;
            let _ = p.num(
                "dimension",
                part,
                prefix,
                v,
                "mm",
                tol,
                src,
                mid,
                y + fs + 10.0 * u,
                Anchor::Middle,
                fs,
                false,
            );
        }
    }
    p.dline(x0, ext_y + 5.0 * u, x0, y + 10.0 * u, 1.2 * u);
    p.dline(x1, ext_y + 5.0 * u, x1, y + 10.0 * u, 1.2 * u);
    p.dline(x0, y, x1, y, 1.2 * u);
    arrow(p, x0, y, 1.0, 0.0);
    arrow(p, x1, y, -1.0, 0.0);
    if stub {
        p.dline(x1, y, x1 + 12.0 * u, y, 1.2 * u);
    }
}

#[allow(clippy::too_many_arguments)]
fn vdim(
    p: &mut Page,
    rng: &mut Rng,
    y0: f64,
    y1: f64,
    x: f64,
    ext_x: f64,
    left: bool,
    part: &str,
    prefix: &str,
    v: f64,
    src: Source,
) {
    let u = p.u;
    let fs = FS_DIM * u;
    let d = if left { -1.0 } else { 1.0 };
    let tol = tol_for(p, rng, v);
    let base = (y0 + y1) / 2.0 + 0.33 * fs;
    let (ax, anchor) = if left {
        (x - 12.0 * u, Anchor::End)
    } else {
        (x + 12.0 * u, Anchor::Start)
    };
    let _ = p.num(
        "dimension",
        part,
        prefix,
        v,
        "mm",
        tol,
        src,
        ax,
        base,
        anchor,
        fs,
        false,
    );
    p.dline(ext_x + d * 5.0 * u, y0, x + d * 10.0 * u, y0, 1.2 * u);
    p.dline(ext_x + d * 5.0 * u, y1, x + d * 10.0 * u, y1, 1.2 * u);
    p.dline(x, y0, x, y1, 1.2 * u);
    arrow(p, x, y0, 0.0, 1.0);
    arrow(p, x, y1, 0.0, -1.0);
}

/// Number in the cell's right-hand column with a leader line to `target` that crosses no text.
/// Returns whether it was placed.
#[allow(clippy::too_many_arguments)]
fn callout(
    p: &mut Page,
    rng: &mut Rng,
    cell: B,
    target: (f64, f64),
    y_pref: f64,
    part: &str,
    prefix: &str,
    v: f64,
    unit: &'static str,
    src: Source,
) -> bool {
    let u = p.u;
    let fs = FS_DIM * u;
    let x = cell[2] - 165.0 * u;
    let tol = if unit == "mm" {
        tol_for(p, rng, v)
    } else {
        None
    };
    for k in 0..14 {
        let off = ((k + 1) / 2) as f64 * if k % 2 == 1 { 1.0 } else { -1.0 };
        let base = (y_pref + off * 46.0 * u).clamp(cell[1] + 60.0 * u, cell[3] - 60.0 * u);
        p.leader = Some(target);
        let placed = p.num(
            "dimension",
            part,
            prefix,
            v,
            unit,
            tol,
            src.clone(),
            x,
            base,
            Anchor::Start,
            fs,
            false,
        );
        p.leader = None;
        if let Some(b) = placed {
            p.line(
                b[0] - 4.0 * u,
                (b[1] + b[3]) / 2.0,
                target.0,
                target.1,
                1.0 * u,
            );
            p.raw(&format!(
                "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"#000\"/>",
                target.0,
                target.1,
                3.5 * u
            ));
            return true;
        }
    }
    false
}

fn label(p: &mut Page, cell: B, s: &str, part: &str, what: &str) {
    let u = p.u;
    let _ = p.word(
        "label",
        s,
        part,
        what,
        cell[0] + 40.0 * u,
        cell[1] + 44.0 * u,
        Anchor::Start,
        23.0 * u,
        true,
    );
}

fn flat(p: &mut Page, rng: &mut Rng, out: &JobOutput, part: &PatternView, fam: &str, cell: B) {
    let u = p.u;
    let region = [
        cell[0] + 40.0 * u,
        cell[1] + 80.0 * u,
        cell[2] - 400.0 * u,
        cell[3] - 180.0 * u,
    ];
    let (xmin, ymin, xmax, ymax) = bbox(part);
    let corners = [(xmin, ymin), (xmax, ymax)];
    let f = Fit::new(&corners, region);
    let ln = t2(p.lang, "DEVELOPMENT: ", "展開図：");
    label(
        p,
        cell,
        &format!("{ln}{}", part_name(fam, &part.part, p.lang)),
        &part.part,
        "view label",
    );
    for c in part.curves.iter().chain(part.holes.iter()) {
        let pts: Vec<(f64, f64)> = c.points.iter().map(|q| f.p(q[0], q[1])).collect();
        let sw = if c.layer == "CUT_OUTER" {
            2.4 * u
        } else {
            1.8 * u
        };
        p.poly(&pts, false, sw, "none");
    }
    let (l, r, t, b) = (
        f.p(xmin, ymax).0,
        f.p(xmax, ymin).0,
        f.p(xmin, ymax).1,
        f.p(xmax, ymin).1,
    );
    let _ = t;
    let bx = Source::Bbox {
        part: part.part.clone(),
        axis: "x".into(),
    };
    let by = Source::Bbox {
        part: part.part.clone(),
        axis: "y".into(),
    };
    hdim(
        p,
        rng,
        l,
        r,
        b + 52.0 * u,
        b,
        &part.part,
        "",
        xmax - xmin,
        bx,
    );
    vdim(
        p,
        rng,
        f.p(xmin, ymax).1,
        b,
        r + 48.0 * u,
        r,
        false,
        &part.part,
        "",
        ymax - ymin,
        by,
    );
    // callouts: curve lengths of curved edges and a few engine quantities
    let mut cands: Vec<Cand> = vec![];
    let mid_of = |name: &str| -> (f64, f64) {
        part.curves
            .iter()
            .chain(part.holes.iter())
            .find(|c| c.name == name)
            .map(|c| {
                let q = c.points[c.points.len() / 2];
                f.p(q[0], q[1])
            })
            .unwrap_or(((l + r) / 2.0, (t + b) / 2.0))
    };
    for c in part.curves.iter().chain(part.holes.iter()) {
        if c.kind != "line" {
            let v = c.length;
            cands.push((
                "L".into(),
                Source::CurveLength {
                    part: part.part.clone(),
                    curve: c.name.clone(),
                },
                "mm",
                mid_of(&c.name),
                v,
            ));
        }
    }
    for (q, pre, target) in [
        ("slant_length", "", "seam_a"),
        ("seam_gap", "", "seam_b"),
        ("s_min", "Smin ", "saddle_cut"),
        ("s_max", "Smax ", "saddle_cut"),
    ] {
        if let Some(v) = part.quantities.get(q) {
            cands.push((
                pre.into(),
                Source::Quantity {
                    part: part.part.clone(),
                    name: q.into(),
                    factor: 1.0,
                },
                "mm",
                mid_of(target),
                *v,
            ));
        }
    }
    let keep = (cands.len()).min(2 + rng.int(0, 2) as usize);
    let mut y = t + 30.0 * u;
    while cands.len() > keep {
        let i = rng.int(0, cands.len() as i64 - 1) as usize;
        cands.remove(i);
    }
    for (pre, src, unit, target, v) in cands {
        callout(p, rng, cell, target, y, &part.part, &pre, v, unit, src);
        y += 90.0 * u;
    }
    let _ = out;
}

fn req(field: &str, factor: f64) -> Source {
    Source::Request {
        field: field.into(),
        factor,
    }
}
fn rf(out: &JobOutput, field: &str) -> f64 {
    crate::source::request_field(&out.request, field).unwrap_or(0.0)
}

fn schematic(p: &mut Page, rng: &mut Rng, out: &JobOutput, fam: &str, cell: B) {
    let u = p.u;
    let fs = FS_DIM * u;
    let region = [
        cell[0] + 250.0 * u,
        cell[1] + 80.0 * u,
        cell[2] - 400.0 * u,
        cell[3] - 190.0 * u,
    ];
    label(
        p,
        cell,
        t2(p.lang, "FRONT VIEW", "正面図"),
        "page",
        "view label",
    );
    // A dimension line that spans the whole width is a diameter (phi); R is only used with a half-span leader.
    let (pre, fac) = ("\u{03C6}", 2.0);
    let part0 = out
        .patterns
        .first()
        .map(|x| x.part.clone())
        .unwrap_or_default();
    match fam {
        "cylinder_shell" | "cone_frustum" | "eccentric_reducer" => {
            let (l, r1, r2, e, rf1, lf) = match fam {
                "cylinder_shell" => (
                    rf(out, "length"),
                    rf(out, "r_dev"),
                    rf(out, "r_dev"),
                    0.0,
                    "r_dev",
                    "length",
                ),
                "cone_frustum" => (rf(out, "h"), rf(out, "r1"), rf(out, "r2"), 0.0, "r1", "h"),
                _ => (
                    rf(out, "h"),
                    rf(out, "r1"),
                    rf(out, "r2"),
                    rf(out, "e"),
                    "r1",
                    "h",
                ),
            };
            let rf2 = if fam == "cylinder_shell" {
                "r_dev"
            } else {
                "r2"
            };
            let pts = [(0.0, -r1), (l, -r2 + e), (l, r2 + e), (0.0, r1)];
            let f = Fit::new(&pts, region);
            let poly: Vec<(f64, f64)> = pts.iter().map(|(x, y)| f.p(*x, *y)).collect();
            p.poly(&poly, true, 2.4 * u, "none");
            let cy = f.p(0.0, 0.0).1;
            p.segs
                .push([poly[0].0 - 25.0 * u, cy, poly[1].0 + 25.0 * u, cy]);
            p.raw(&format!(
                "<line x1=\"{:.1}\" y1=\"{cy:.1}\" x2=\"{:.1}\" y2=\"{cy:.1}\" stroke=\"#000\" stroke-width=\"{:.1}\" stroke-dasharray=\"{:.0} {:.0} {:.0} {:.0}\"/>",
                poly[0].0 - 25.0 * u, poly[1].0 + 25.0 * u, 1.2 * u, 30.0 * u, 6.0 * u, 6.0 * u, 6.0 * u
            ));
            let bottom = poly.iter().map(|q| q.1).fold(f64::MIN, f64::max);
            let (xl, xr) = (poly[0].0, poly[1].0);
            hdim(
                p,
                rng,
                xl,
                xr,
                bottom + 55.0 * u,
                bottom,
                &part0,
                "",
                l,
                req(lf, 1.0),
            );
            vdim(
                p,
                rng,
                poly[3].1,
                poly[0].1,
                xl - 45.0 * u,
                xl,
                true,
                &part0,
                pre,
                r1 * fac,
                req(rf1, fac),
            );
            if fam != "cylinder_shell" {
                vdim(
                    p,
                    rng,
                    poly[2].1,
                    poly[1].1,
                    xr + 45.0 * u,
                    xr,
                    false,
                    &part0,
                    pre,
                    r2 * fac,
                    req(rf2, fac),
                );
            }
            if fam == "eccentric_reducer" {
                // eccentricity: offset dimension between the two axes, beside the large-end dimension
                let (ya, yb) = (f.p(l, 0.0).1, f.p(l, e).1);
                if (ya - yb).abs() > 12.0 * u {
                    let w2 = text_w(&format!("{pre}{:.1}0000", r2 * fac), fs);
                    let xe = xr + 45.0 * u + 12.0 * u + w2 + 40.0 * u;
                    vdim(
                        p,
                        rng,
                        yb.min(ya),
                        yb.max(ya),
                        xe,
                        xr,
                        false,
                        &part0,
                        "e",
                        e,
                        req("e", 1.0),
                    );
                }
            }
        }
        _ => {
            let asm = fam == "header_with_branch";
            let (hr, br, beta, send) = (
                rf(out, "header_radius"),
                rf(out, "branch_radius"),
                rf(out, "beta_deg"),
                rf(out, "s_end"),
            );
            let (lh, st) = if asm {
                (rf(out, "header_length"), rf(out, "branch_station"))
            } else {
                (4.0 * hr, 2.0 * hr)
            };
            let (dx, dy) = (beta.to_radians().cos(), beta.to_radians().sin());
            let (nx, ny) = (-dy, dx);
            let c = [
                (st + br * nx, br * ny),
                (st - br * nx, -br * ny),
                (st - br * nx + send * dx, -br * ny + send * dy),
                (st + br * nx + send * dx, br * ny + send * dy),
            ];
            let mut pts = vec![(0.0, -hr), (lh, -hr), (lh, hr), (0.0, hr)];
            pts.extend_from_slice(&c);
            let f = Fit::new(&pts, region);
            let hp: Vec<(f64, f64)> = pts[..4].iter().map(|(x, y)| f.p(*x, *y)).collect();
            let bp: Vec<(f64, f64)> = c.iter().map(|(x, y)| f.p(*x, *y)).collect();
            p.poly(&hp, true, 2.4 * u, "none");
            p.poly(&bp, true, 2.4 * u, "none");
            let cy = f.p(0.0, 0.0).1;
            p.segs
                .push([hp[0].0 - 25.0 * u, cy, hp[1].0 + 25.0 * u, cy]);
            p.raw(&format!(
                "<line x1=\"{:.1}\" y1=\"{cy:.1}\" x2=\"{:.1}\" y2=\"{cy:.1}\" stroke=\"#000\" stroke-width=\"{:.1}\" stroke-dasharray=\"{:.0} {:.0} {:.0} {:.0}\"/>",
                hp[0].0 - 25.0 * u, hp[1].0 + 25.0 * u, 1.2 * u, 30.0 * u, 6.0 * u, 6.0 * u, 6.0 * u
            ));
            let hb = hp[0].1.max(hp[3].1);
            let all_bottom = hp
                .iter()
                .chain(bp.iter())
                .map(|q| q.1)
                .fold(f64::MIN, f64::max);
            let (xl, xr) = (hp[0].0, hp[1].0);
            let ps = f.p(st, 0.0);
            if asm {
                hdim(
                    p,
                    rng,
                    xl,
                    xr,
                    all_bottom + 55.0 * u,
                    hb.max(all_bottom),
                    "header",
                    "",
                    lh,
                    req("header_length", 1.0),
                );
                hdim(
                    p,
                    rng,
                    xl,
                    ps.0,
                    all_bottom + 55.0 * u + fs + 40.0 * u,
                    hb.max(all_bottom),
                    "header",
                    "",
                    st,
                    req("branch_station", 1.0),
                );
            }
            let ytop = hp[0].1.min(hp[3].1);
            let (top_y, bot_y) = (hp[0].1.min(hp[3].1), hp[0].1.max(hp[3].1));
            let _ = ytop;
            vdim(
                p,
                rng,
                top_y,
                bot_y,
                xl - 45.0 * u,
                xl,
                true,
                if asm { "header" } else { "page" },
                pre,
                hr * fac,
                req("header_radius", fac),
            );
            // branch: diameter/radius and length as callouts
            let far_mid = ((bp[2].0 + bp[3].0) / 2.0, (bp[2].1 + bp[3].1) / 2.0);
            let side_mid = ((bp[1].0 + bp[2].0) / 2.0, (bp[1].1 + bp[2].1) / 2.0);
            let part_b = if asm { "branch" } else { part0.as_str() };
            // branch size: phi with a leader to the end edge, or R with a leader to its centre and a half-span
            let (bpre, bfac) = if rng.chance(0.5) {
                ("\u{03C6}", 2.0)
            } else {
                ("R", 1.0)
            };
            let placed = callout(
                p,
                rng,
                cell,
                far_mid,
                far_mid.1,
                part_b,
                bpre,
                br * bfac,
                "mm",
                req("branch_radius", bfac),
            );
            if placed && bfac == 1.0 {
                p.dline(far_mid.0, far_mid.1, bp[3].0, bp[3].1, 1.2 * u);
            }
            callout(
                p,
                rng,
                cell,
                side_mid,
                side_mid.1 + 70.0 * u,
                part_b,
                "",
                send,
                "mm",
                req("s_end", 1.0),
            );
            // angle marker at the branch axis foot
            let ar = 90.0 * u;
            let a1 = (ps.0 + ar, ps.1);
            let a2 = (ps.0 + ar * dx, ps.1 - ar * dy);
            let large = if beta > 180.0 { 1 } else { 0 };
            for i in 0..12 {
                let (t0, t1) = (
                    beta.to_radians() * i as f64 / 12.0,
                    beta.to_radians() * (i + 1) as f64 / 12.0,
                );
                p.segs.push([
                    ps.0 + ar * t0.cos(),
                    ps.1 - ar * t0.sin(),
                    ps.0 + ar * t1.cos(),
                    ps.1 - ar * t1.sin(),
                ]);
            }
            p.raw(&format!(
                "<path d=\"M {:.1} {:.1} A {ar:.1} {ar:.1} 0 {large} 0 {:.1} {:.1}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{:.1}\"/>",
                a1.0, a1.1, a2.0, a2.1, 1.2 * u
            ));
            let mid_a = (beta / 2.0).to_radians();
            let tx = ps.0 + (ar + 26.0 * u) * mid_a.cos();
            let ty = ps.1 - (ar + 26.0 * u) * mid_a.sin();
            let _ = p.num(
                "dimension",
                part_b,
                "",
                beta,
                "deg",
                None,
                req("beta_deg", 1.0),
                tx,
                ty + 0.33 * fs,
                Anchor::Start,
                fs,
                true,
            );
        }
    }
}

fn notes_and_title(
    p: &mut Page,
    rng: &mut Rng,
    out: &JobOutput,
    fam: &str,
    ids: &Ids,
    rc: B,
) -> f64 {
    let u = p.u;
    let lang = p.lang;
    // general tolerance note + a few plain notes
    let std = if p.style == Style::Iso {
        format!("ISO 2768-{}", p.gtol)
    } else {
        format!("JIS B 0405-{}", p.gtol)
    };
    let tol_txt = format!("{}{std}", t2(lang, "General tolerance: ", "普通公差："));
    let it = Item {
        kind: "tolerance_note",
        text: tol_txt.clone(),
        value: json!(tol_txt),
        unit: "",
        tol: None,
        tol_source: None,
        src: Some(Source::ToleranceTable {
            class: p.gtol.to_string(),
        }),
        part: "page".into(),
        decimals: None,
        b: [0.0; 4],
    };
    let mut y = rc[1] + 40.0 * u;
    p.emit(it, rc[0], y, Anchor::Start, 22.0 * u, false, false);
    let pool_en = [
        "Dimensions in mm.",
        "Remove burrs and sharp edges.",
        "Surface finish as rolled.",
        "Seam welds not shown.",
        "Marking on outside surface.",
        "Do not scale the drawing.",
    ];
    let pool_ja = [
        "寸法の単位はミリメートル。",
        "バリ及び鋭いエッジを除去すること。",
        "表面仕上げは圧延のままとする。",
        "溶接継目は図示しない。",
        "刻印は外面に施す。",
        "図面から寸法を読み取らないこと。",
    ];
    let mut idx: Vec<usize> = (0..6).collect();
    for i in (1..6).rev() {
        idx.swap(i, rng.int(0, i as i64) as usize);
    }
    for &i in idx.iter().take(rng.int(2, 4) as usize) {
        y += 38.0 * u;
        let s = if lang == Lang::En {
            pool_en[i]
        } else {
            pool_ja[i]
        };
        p.word(
            "note",
            s,
            "page",
            "note text",
            rc[0],
            y,
            Anchor::Start,
            22.0 * u,
            false,
        );
    }
    // projection symbol: first angle (ISO) or third angle (JIS)
    let (sx, sy) = (rc[0] + 20.0 * u, y + 40.0 * u);
    let tz = [
        (sx, sy),
        (sx + 60.0 * u, sy + 12.0 * u),
        (sx + 60.0 * u, sy + 48.0 * u),
        (sx, sy + 60.0 * u),
    ];
    p.poly(&tz, true, 1.8 * u, "none");
    let cx = if p.style == Style::Iso {
        sx + 130.0 * u
    } else {
        sx - 70.0 * u
    };
    p.raw(&format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{:.1}\"/>", cx + 30.0 * u, sy + 30.0 * u, 30.0 * u, 1.8 * u));
    p.raw(&format!("<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"{:.1}\" fill=\"none\" stroke=\"#000\" stroke-width=\"{:.1}\"/>", cx + 30.0 * u, sy + 30.0 * u, 13.0 * u, 1.2 * u));

    // title block (bottom of the right column)
    let ty0 = rc[3] - 300.0 * u;
    let labels_en = ["Title", "Drawing No.", "Material", "Scale", "Date", "Sheet"];
    let labels_ja = ["品名", "図番", "材質", "尺度", "日付", "枚数"];
    let vals = [
        drawing_name(fam, lang).to_string(),
        ids.drawing_no.clone(),
        ids.material.to_string(),
        "NTS".to_string(),
        ids.date.clone(),
        "1/1".to_string(),
    ];
    let whats = [
        "part name",
        "drawing number",
        "material",
        "scale",
        "date",
        "sheet",
    ];
    let lw = 200.0 * u;
    p.rect([rc[0], ty0, rc[2], rc[3]], 2.4 * u);
    p.line(rc[0] + lw, ty0, rc[0] + lw, rc[3], 1.5 * u);
    for i in 0..6 {
        let ry = ty0 + 50.0 * u * i as f64;
        if i > 0 {
            p.line(rc[0], ry, rc[2], ry, 1.5 * u);
        }
        let l = if lang == Lang::En {
            labels_en[i]
        } else {
            labels_ja[i]
        };
        p.word(
            "label",
            l,
            "page",
            "title label",
            rc[0] + 10.0 * u,
            ry + 34.0 * u,
            Anchor::Start,
            22.0 * u,
            false,
        );
        p.word(
            "title",
            &vals[i],
            "page",
            whats[i],
            rc[0] + lw + 12.0 * u,
            ry + 35.0 * u,
            Anchor::Start,
            24.0 * u,
            i == 0,
        );
    }
    let _ = out;
    ty0
}

fn bom(p: &mut Page, out: &JobOutput, fam: &str, ids: &Ids, rc: B, ty0: f64) {
    let u = p.u;
    let lang = p.lang;
    let n = out.patterns.len();
    let rh = 46.0 * u;
    let y1 = ty0 - 24.0 * u;
    let y0 = y1 - rh * (n as f64 + 1.0);
    let cw = [56.0, 224.0, 120.0, 56.0, 122.0, 122.0];
    let heads_en = ["No.", "Name", "Material", "Qty", "Blank W", "Blank H"];
    let heads_ja = ["番号", "名称", "材質", "数量", "展開幅", "展開高さ"];
    p.rect([rc[0], y0, rc[2], y1], 2.4 * u);
    let mut xs = vec![rc[0]];
    for w in cw {
        xs.push(xs.last().unwrap() + w * u);
    }
    for x in &xs[1..xs.len() - 1] {
        p.line(*x, y0, *x, y1, 1.5 * u);
    }
    for r in 0..=n {
        let ry = y0 + rh * r as f64;
        if r > 0 {
            p.line(rc[0], ry, rc[2], ry, 1.5 * u);
        }
        let base = ry + 32.0 * u;
        let fs = 21.0 * u;
        for c in 0..6 {
            let x = xs[c] + 8.0 * u;
            if r == 0 {
                let s = if lang == Lang::En {
                    heads_en[c]
                } else {
                    heads_ja[c]
                };
                p.word(
                    "bom_header",
                    s,
                    "page",
                    "bom header",
                    x,
                    base,
                    Anchor::Start,
                    fs,
                    true,
                );
                continue;
            }
            let part = &out.patterns[r - 1];
            let (bx0, by0, bx1, by1) = bbox(part);
            match c {
                0 => {
                    let it = Item {
                        kind: "bom",
                        text: format!("{r}"),
                        value: json!(r),
                        unit: "",
                        tol: None,
                        tol_source: None,
                        src: Some(Source::Structure {
                            what: "item number".into(),
                        }),
                        part: part.part.clone(),
                        decimals: None,
                        b: [0.0; 4],
                    };
                    p.emit(it, x, base, Anchor::Start, fs, false, false);
                }
                1 => {
                    let nm = part_name(fam, &part.part, lang);
                    let fs1 = fs.min((cw[1] - 14.0) * u / text_w(nm, 1.0));
                    p.word(
                        "bom",
                        nm,
                        &part.part,
                        "part name",
                        x,
                        base,
                        Anchor::Start,
                        fs1,
                        false,
                    );
                }
                2 => {
                    p.word(
                        "bom",
                        ids.material,
                        &part.part,
                        "material",
                        x,
                        base,
                        Anchor::Start,
                        fs,
                        false,
                    );
                }
                3 => {
                    let it = Item {
                        kind: "bom",
                        text: "1".into(),
                        value: json!(1),
                        unit: "pcs",
                        tol: None,
                        tol_source: None,
                        src: Some(Source::Structure {
                            what: "quantity".into(),
                        }),
                        part: part.part.clone(),
                        decimals: None,
                        b: [0.0; 4],
                    };
                    p.emit(it, x, base, Anchor::Start, fs, false, false);
                }
                _ => {
                    let (v, axis) = if c == 4 {
                        (bx1 - bx0, "x")
                    } else {
                        (by1 - by0, "y")
                    };
                    p.num(
                        "bom",
                        &part.part,
                        "",
                        v,
                        "mm",
                        None,
                        Source::Bbox {
                            part: part.part.clone(),
                            axis: axis.into(),
                        },
                        x,
                        base,
                        Anchor::Start,
                        fs,
                        false,
                    );
                }
            }
        }
    }
}

pub fn compose(p: &mut Page, rng: &mut Rng, out: &JobOutput, fam: &str, ids: &Ids) {
    let u = p.u;
    let (w, h) = (p.w, p.h);
    p.rect([20.0 * u, 20.0 * u, w - 20.0 * u, h - 20.0 * u], 3.0 * u);
    let rc = [
        w - 40.0 * u - 700.0 * u,
        40.0 * u,
        w - 40.0 * u,
        h - 40.0 * u,
    ];
    let ty0 = notes_and_title(p, rng, out, fam, ids, rc);
    bom(p, out, fam, ids, rc, ty0);
    let va = [40.0 * u, 40.0 * u, rc[0] - 30.0 * u, h - 40.0 * u];
    let vh = va[3] - va[1];
    let n = out.patterns.len();
    if n == 1 {
        let (top, bot) = (
            [va[0], va[1], va[2], va[1] + vh / 2.0 - 8.0 * u],
            [va[0], va[1] + vh / 2.0 + 8.0 * u, va[2], va[3]],
        );
        let (c_sch, c_flat) = if rng.chance(0.5) {
            (top, bot)
        } else {
            (bot, top)
        };
        schematic(p, rng, out, fam, c_sch);
        flat(p, rng, out, &out.patterns[0], fam, c_flat);
    } else {
        let split = va[1] + vh * 0.42;
        let mx = (va[0] + va[2]) / 2.0;
        schematic(p, rng, out, fam, [va[0], va[1], va[2], split - 8.0 * u]);
        flat(
            p,
            rng,
            out,
            &out.patterns[0],
            fam,
            [va[0], split + 8.0 * u, mx - 6.0 * u, va[3]],
        );
        flat(
            p,
            rng,
            out,
            &out.patterns[1],
            fam,
            [mx + 6.0 * u, split + 8.0 * u, va[2], va[3]],
        );
    }
}
