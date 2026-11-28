//! jagua-rs 2.0.0 (MPL-2.0, used unmodified, pinned in Cargo.toml) as the placement engine. Its collision engine
//! decides every placement. Items are inflated by half the separation, so the separation is the kerf plus the part
//! gap plus a small margin; the output outline is then recomputed here from the pose, and the independent checker
//! verifies it. The engine works in f32, so the margin also covers rounding.

use crate::geom::{self, EPS, P};
use crate::{Nest, NestError, PartIn, Placed, Rules, Sheet, out_of_time};
use jagua_rs::collision_detection::CDEConfig;
use jagua_rs::entities::Layout;
use jagua_rs::geometry::DTransformation;
use jagua_rs::geometry::fail_fast::SPSurrogateConfig;
use jagua_rs::io::ext_repr::{
    ExtContainer, ExtItem, ExtOrientation, ExtRotation, ExtSPolygon, ExtShape,
};
use jagua_rs::io::import::Importer;
use std::sync::Arc;
use std::time::Instant;

/// Extra separation in mm above kerf + gap, covering f32 rounding of the engine.
pub const MARGIN: f64 = 0.02;

pub const ENGINE: &str = "jagua-rs 2.0.0 (MPL-2.0)";

fn s<E: std::fmt::Display>(e: E) -> NestError {
    NestError::new(e.to_string())
}

/// Nest with jagua-rs. Errors name the engine failure (the caller falls back to the bottom-left fill), except a
/// `nest_timeout`, which is final.
pub fn nest(
    parts: &[PartIn],
    rules: &Rules,
    prepared: &[(usize, u32, Vec<P>)],
    deadline: Instant,
) -> Result<Nest, NestError> {
    let clr = rules.clearance();
    let edge = rules.edge_mm;
    if rules.sheet_w - 2.0 * edge <= 0.0 || rules.sheet_h - 2.0 * edge <= 0.0 {
        return Err(NestError::new("sheet smaller than its edge margin"));
    }
    let imp = Importer::new(
        CDEConfig {
            item_surrogate_config: SPSurrogateConfig::none(),
        },
        None,
        None,
    )
    .with_min_item_separation((clr + MARGIN) as f32)
    .map_err(s)?;
    let angles: Vec<f32> = rules.rotations.iter().map(|&r| r as f32).collect();
    let mut items = Vec::with_capacity(parts.len());
    for (i, p) in parts.iter().enumerate() {
        let c = geom::centre(&p.outline);
        let ext = ExtItem {
            id: i as u64,
            orientation: ExtOrientation {
                rotation: ExtRotation::Discrete {
                    angles: angles.clone(),
                },
                reflection_axes: vec![],
            },
            shape: jagua_rs::io::ext_repr::ExtShape::SimplePolygon(ExtSPolygon(
                c.iter().map(|&(x, y)| (x as f32, y as f32)).collect(),
            )),
            min_quality: None,
        };
        items.push(imp.import_item(&ext, i).map_err(s)?);
    }
    let container = ExtContainer {
        id: 0,
        shape: ExtShape::Rectangle {
            x_min: edge as f32,
            y_min: edge as f32,
            width: (rules.sheet_w - 2.0 * edge) as f32,
            height: (rules.sheet_h - 2.0 * edge) as f32,
        },
        zones: vec![],
    };
    let mut layouts: Vec<Layout> = Vec::new();
    let mut sheets: Vec<Sheet> = Vec::new();
    for (i, copy, centred) in prepared {
        out_of_time(deadline)?;
        let mut done = false;
        for si in 0..=sheets.len() {
            if si == sheets.len() {
                let cont = imp.import_container(&container).map_err(s)?;
                layouts.push(Layout::new(cont));
                sheets.push(Sheet::default());
            }
            if let Some((rot, t, pts)) = best(
                &mut layouts[si],
                &sheets[si],
                &items[*i],
                centred,
                rules,
                deadline,
            )? {
                let pk = layouts[si].place_item(&items[*i], dt(rot, t));
                debug_assert!(layouts[si].placed_items().contains_key(pk));
                sheets[si].placed.push(Placed {
                    id: parts[*i].id.clone(),
                    copy: *copy,
                    rot,
                    pose: t,
                    pts,
                });
                done = true;
                break;
            }
            if si + 1 == sheets.len() && sheets[si].placed.is_empty() {
                return Err(NestError::coded(
                    "nest_fit",
                    format!("part {} does not fit the stock sheet", parts[*i].id),
                ));
            }
        }
        if !done {
            return Err(NestError::coded("nest_internal", "a part was not placed"));
        }
    }
    Ok(Nest {
        rules: rules.clone(),
        parts: parts.to_vec(),
        sheets,
        engine: ENGINE.into(),
    })
}

fn dt(rot: u32, t: P) -> DTransformation {
    DTransformation::new((rot as f32).to_radians(), (t.0 as f32, t.1 as f32))
}

/// Lowest then leftmost pose the engine accepts on one sheet, over the allowed rotations. Each trial is placed in
/// the engine, asked whether the layout is collision free, and removed again.
fn best(
    layout: &mut Layout,
    sheet: &Sheet,
    item: &Arc<jagua_rs::entities::Item>,
    centred: &[P],
    rules: &Rules,
    deadline: Instant,
) -> Result<Option<(u32, P, Vec<P>)>, NestError> {
    // The engine inflates each item by half the separation, so neighbours and the container edge are kept a full
    // separation (plus a sliver so touching inflated outlines are not refused) away.
    let g = rules.clearance() + MARGIN + 1e-3;
    let e = rules.edge_mm + g;
    let cands = crate::corners(sheet, g, e);
    let mut best: Option<(u32, P, Vec<P>, P)> = None;
    for &rot in &rules.rotations {
        let r = geom::rotate(centred, rot);
        let (x0, y0, x1, y1) = geom::bbox(&r);
        let (w, h) = (x1 - x0, y1 - y0);
        for &c in &cands {
            out_of_time(deadline)?;
            if c.0 + w > rules.sheet_w - e + EPS || c.1 + h > rules.sheet_h - e + EPS {
                continue;
            }
            if let Some((_, _, _, bc)) = &best {
                if !crate::lower_left(c, *bc) {
                    break;
                }
            }
            let (t, pts) = geom::place_corner(centred, rot, c);
            let pk = layout.place_item(item, dt(rot, t));
            let free = layout.is_collision_free();
            layout.remove_item(pk);
            if free {
                best = Some((rot, t, pts, c));
                break;
            }
        }
    }
    Ok(best.map(|(rot, t, pts, _)| (rot, t, pts)))
}
