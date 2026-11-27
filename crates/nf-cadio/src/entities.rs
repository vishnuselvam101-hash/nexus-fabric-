//! The ENTITIES section. Entity set 1: LINE, POINT, CIRCLE, ARC, LWPOLYLINE, POLYLINE with its VERTEX
//! records, TEXT, MTEXT and SOLID. Other kinds are counted in `Drawing::skipped`.

use crate::drawing::{records, skip, warn_extrusion, Drawing, Entity, Record, Vertex};
use crate::error::{CadError, Reason};
use crate::header::real;
use crate::text::{self, TextEncoding};
use crate::tokenizer::Pair;

pub(crate) fn read(pairs: &[Pair], enc: &TextEncoding, out: &mut Drawing) -> Result<(), CadError> {
    let recs = records(pairs)?;
    let mut i = 0;
    while let Some(rec) = recs.get(i) {
        match rec.kind {
            "POLYLINE" => {
                let mut end = i + 1;
                while recs.get(end).is_some_and(|r| r.kind == "VERTEX") {
                    end += 1;
                }
                if recs.get(end).map(|r| r.kind) != Some("SEQEND") {
                    return Err(CadError::at(rec.line, Reason::UnterminatedPolyline));
                }
                polyline(rec, &recs[i + 1..end], enc, out)?;
                i = end + 1;
            }
            "VERTEX" | "SEQEND" => return Err(CadError::at(rec.line, Reason::OrphanVertex)),
            _ => {
                entity(rec, enc, out)?;
                i += 1;
            }
        }
    }
    Ok(())
}

fn entity(rec: &Record<'_>, enc: &TextEncoding, out: &mut Drawing) -> Result<(), CadError> {
    let layer = rec.text(8, enc)?.unwrap_or_else(|| "0".to_string());
    let off_z = extrusion_not_z(rec)?;
    let made = match rec.kind {
        "LINE" => Entity::Line {
            layer,
            start: rec.required_point(10)?,
            end: rec.required_point(11)?,
            non_z_extrusion: off_z,
        },
        "POINT" => Entity::Point {
            layer,
            at: rec.required_point(10)?,
            non_z_extrusion: off_z,
        },
        "CIRCLE" => Entity::Circle {
            layer,
            center: rec.required_point(10)?,
            radius: radius(rec)?,
            non_z_extrusion: off_z,
        },
        "ARC" => Entity::Arc {
            layer,
            center: rec.required_point(10)?,
            radius: radius(rec)?,
            start_angle: rec.required_real(50)?,
            end_angle: rec.required_real(51)?,
            non_z_extrusion: off_z,
        },
        "LWPOLYLINE" => Entity::LwPolyline {
            layer,
            closed: rec.int(70)?.unwrap_or(0) & 1 != 0,
            vertices: lw_vertices(rec)?,
            non_z_extrusion: off_z,
        },
        "TEXT" => {
            let raw = rec.text(1, enc)?.unwrap_or_default();
            Entity::Text {
                layer,
                at: rec.required_point(10)?,
                height: rec.required_real(40)?,
                rotation: rec.real(50)?.unwrap_or(0.0),
                plain: text::plain(&raw, false),
                raw,
                non_z_extrusion: off_z,
            }
        }
        "MTEXT" => {
            let raw = mtext_raw(rec, enc)?;
            Entity::MText {
                layer,
                at: rec.required_point(10)?,
                height: rec.required_real(40)?,
                plain: text::plain(&raw, true),
                raw,
                non_z_extrusion: off_z,
            }
        }
        "SOLID" => {
            let c3 = rec.required_point(12)?;
            Entity::Solid {
                layer,
                corners: [
                    rec.required_point(10)?,
                    rec.required_point(11)?,
                    c3,
                    rec.point(13)?.unwrap_or(c3),
                ],
                non_z_extrusion: off_z,
            }
        }
        other => {
            skip(out, other);
            return Ok(());
        }
    };
    if off_z {
        warn_extrusion(out, rec.kind, rec.line);
    }
    out.entities.push(made);
    Ok(())
}

/// POLYLINE and its VERTEX records (`vertices` is the slice between them, all VERTEX records).
fn polyline(
    rec: &Record<'_>,
    vertices: &[Record<'_>],
    enc: &TextEncoding,
    out: &mut Drawing,
) -> Result<(), CadError> {
    let flags = rec.int(70)?.unwrap_or(0);
    if flags & (16 | 64) != 0 {
        // Polygon mesh (16) or polyface mesh (64): not modelled.
        skip(out, "POLYLINE (mesh)");
        return Ok(());
    }
    let layer = rec.text(8, enc)?.unwrap_or_else(|| "0".to_string());
    let off_z = extrusion_not_z(rec)?;
    let mut verts = Vec::with_capacity(vertices.len());
    for v in vertices {
        let at = v.required_point(10)?;
        verts.push(Vertex {
            x: at.x,
            y: at.y,
            z: at.z,
            bulge: v.real(42)?.unwrap_or(0.0),
        });
    }
    out.entities.push(Entity::Polyline {
        layer,
        closed: flags & 1 != 0,
        is_3d: flags & 8 != 0,
        vertices: verts,
        non_z_extrusion: off_z,
    });
    if off_z {
        warn_extrusion(out, "POLYLINE", rec.line);
    }
    Ok(())
}

/// The vertices of a LWPOLYLINE. Each vertex is group 10 (x), group 20 (y) and an optional group 42
/// (bulge) that applies to the vertex before it. Group 38 is the elevation for every vertex.
fn lw_vertices(rec: &Record<'_>) -> Result<Vec<Vertex>, CadError> {
    let z = rec.real(38)?.unwrap_or(0.0);
    // x, y (once group 20 is seen), bulge
    let mut rows: Vec<(f64, Option<f64>, f64)> = Vec::new();
    for p in rec.body {
        match p.code {
            10 => rows.push((real(p)?, None, 0.0)),
            20 => match rows.last_mut() {
                Some(row) if row.1.is_none() => row.1 = Some(real(p)?),
                _ => return Err(rec.missing(10)),
            },
            42 => match rows.last_mut() {
                Some(row) => row.2 = real(p)?,
                None => return Err(rec.missing(10)),
            },
            _ => {}
        }
    }
    rows.into_iter()
        .map(|(x, y, bulge)| {
            let y = y.ok_or_else(|| rec.missing(20))?;
            Ok(Vertex { x, y, z, bulge })
        })
        .collect()
}

/// The string of an MTEXT: every group 3 part in order, then group 1. Joined before decoding, so a
/// multi-byte character split across parts decodes whole.
fn mtext_raw(rec: &Record<'_>, enc: &TextEncoding) -> Result<String, CadError> {
    let mut latin1 = String::new();
    for p in rec.body.iter().filter(|p| p.code == 3) {
        latin1.push_str(&p.value);
    }
    if let Some(p) = rec.pair(1) {
        latin1.push_str(&p.value);
    }
    text::decode(&latin1, enc).map_err(|reason| CadError::at(rec.line, reason))
}

/// True when the extrusion direction (groups 210, 220, 230; default 0, 0, 1) is not +Z. A positive
/// z with x and y zero is +Z. Anything else, including a negative z, is not.
fn extrusion_not_z(rec: &Record<'_>) -> Result<bool, CadError> {
    let x = rec.real(210)?.unwrap_or(0.0);
    let y = rec.real(220)?.unwrap_or(0.0);
    let z = rec.real(230)?.unwrap_or(1.0);
    Ok(!(x == 0.0 && y == 0.0 && z > 0.0))
}

/// A radius: a real number, not negative.
fn radius(rec: &Record<'_>) -> Result<f64, CadError> {
    let r = rec.required_real(40)?;
    if r < 0.0 {
        return Err(rec.invalid(40));
    }
    Ok(r)
}
