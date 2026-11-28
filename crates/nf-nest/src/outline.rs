//! Part outline from a DXF text: the cut-layer geometry must form exactly one closed loop. Holes and open
//! contours are refused here (v1 does not nest parts with holes), so such parts are excluded, never guessed.

use crate::geom::{self, P};
use nf_dxf::{Entity, is_cut_layer};

const JOIN: f64 = 1e-3;
const ARC_STEP_DEG: f64 = 5.0;

fn arc_pts(c: P, r: f64, start: f64, end: f64) -> Vec<P> {
    let mut sweep = end - start;
    if sweep <= 0.0 {
        sweep += 360.0;
    }
    let n = (sweep / ARC_STEP_DEG).ceil().max(2.0) as usize;
    (0..=n)
        .map(|i| {
            let a = (start + sweep * i as f64 / n as f64).to_radians();
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

/// The single closed cut loop of a DXF text, as a polygon (no closing repeat).
pub fn outline_from_dxf(text: &str) -> Result<Vec<P>, String> {
    let (ents, unknown) = nf_dxf::read(text)?;
    if !unknown.is_empty() {
        return Err(format!("unknown DXF entities: {}", unknown.join(", ")));
    }
    // each cut entity as a path of points
    let mut paths: Vec<Vec<P>> = Vec::new();
    for e in &ents {
        if !is_cut_layer(e.layer()) {
            continue;
        }
        match e {
            Entity::Line { a, b, .. } => paths.push(vec![*a, *b]),
            Entity::Arc {
                c,
                r,
                start_deg,
                end_deg,
                ..
            } => paths.push(arc_pts(*c, *r, *start_deg, *end_deg)),
            Entity::Polyline { pts, closed, .. } => {
                let mut v = pts.clone();
                if *closed && !v.is_empty() {
                    v.push(v[0]);
                }
                paths.push(v);
            }
            Entity::Text { .. } => {}
        }
    }
    if paths.is_empty() {
        return Err("no cut geometry on the CUT layers".into());
    }
    // chain the paths into loops
    let same = |a: P, b: P| (a.0 - b.0).abs() < JOIN && (a.1 - b.1).abs() < JOIN;
    let mut loops: Vec<Vec<P>> = Vec::new();
    while let Some(mut cur) = paths.pop() {
        loop {
            if cur.len() > 2 && same(cur[0], *cur.last().expect("non-empty")) {
                break;
            }
            let end = *cur.last().expect("non-empty");
            let Some(k) = paths
                .iter()
                .position(|p| same(p[0], end) || same(*p.last().expect("non-empty"), end))
            else {
                break;
            };
            let mut next = paths.swap_remove(k);
            if !same(next[0], end) {
                next.reverse();
            }
            cur.extend(next.into_iter().skip(1));
        }
        if cur.len() < 3 || !same(cur[0], *cur.last().expect("non-empty")) {
            return Err("the cut geometry is not a closed loop".into());
        }
        cur.pop();
        loops.push(cur);
    }
    if loops.len() != 1 {
        return Err(format!(
            "{} closed loops on the CUT layers; v1 nests one outline per part (holes not supported)",
            loops.len()
        ));
    }
    let poly = loops.pop().expect("one loop");
    if geom::area(&poly) <= 0.0 {
        return Err("the outline has no area".into());
    }
    Ok(poly)
}
