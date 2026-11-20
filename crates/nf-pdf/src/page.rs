//! Page selection, inherited page attributes, boxes, rotation, the page transform,
//! and the page's content and resources.
//!
//! Page transform. Let the visible box be `[llx lly urx ury]` (CropBox clipped to
//! MediaBox) and `k = UserUnit * mm_per_point`, the millimetres per user-space unit.
//! The map from user space to the upright page, origin at the lower left of the
//! visible page, is (with W = urx - llx, H = ury - lly):
//!   Rotate   0: (x - llx,        y - lly)
//!   Rotate  90: (y - lly,        urx - x)
//!   Rotate 180: (urx - x,        ury - y)
//!   Rotate 270: (ury - y,        x - llx)
//! Each is a rotation (determinant 1) followed by a scale by k.

use std::collections::BTreeSet;

use crate::doc::Doc;
use crate::geom::M;
use crate::lexer::{uint, Dict, Obj};
use crate::limits::{limit, malformed, Fault};
use crate::report::Stats;

#[derive(Clone, Default)]
struct Inherited {
    resources: Option<Obj>,
    media: Option<Obj>,
    crop: Option<Obj>,
    rotate: Option<Obj>,
}

/// The selected page: its dictionary and the attributes it inherits from its ancestors.
pub struct PageInfo {
    pub dict: Dict,
    inh: Inherited,
}

pub struct Selection {
    /// Number of pages in the document.
    pub count: u32,
    pub page: Option<PageInfo>,
}

/// Geometry of the visible page, in user-space units.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub visible: [f64; 4],
    /// 0, 90, 180 or 270.
    pub rotate: u32,
    pub user_unit: f64,
}

/// Finds page `wanted` (1-based) and counts all pages.
pub fn select(doc: &mut Doc, wanted: u32) -> Result<Selection, Fault> {
    let root = doc
        .trailer
        .get(b"Root")
        .cloned()
        .ok_or_else(|| Fault::Malformed("no /Root".into()))?;
    let catalog = doc.resolve(&root)?;
    let Some(cat) = catalog.dict().cloned() else {
        return malformed("document catalogue is not a dictionary");
    };
    let Some(pages) = doc.dict_entry(&cat, b"Pages")? else {
        return malformed("document has no /Pages tree");
    };
    let mut seen = BTreeSet::new();
    let mut count = 0u32;
    let mut found = None;
    walk(
        doc,
        pages,
        Inherited::default(),
        0,
        &mut seen,
        &mut count,
        wanted,
        &mut found,
    )?;
    Ok(Selection { count, page: found })
}

#[allow(clippy::too_many_arguments)]
fn walk(
    doc: &mut Doc,
    node: Obj,
    inh: Inherited,
    depth: usize,
    seen: &mut BTreeSet<u32>,
    count: &mut u32,
    wanted: u32,
    found: &mut Option<PageInfo>,
) -> Result<(), Fault> {
    if depth > doc.lim.max_nesting {
        return malformed("page tree is too deep");
    }
    let Some(d) = node.dict().cloned() else {
        return malformed("page tree node is not a dictionary");
    };
    let mut inh = inh;
    if let Some(v) = d.get(b"Resources") {
        inh.resources = Some(v.clone());
    }
    if let Some(v) = d.get(b"MediaBox") {
        inh.media = Some(v.clone());
    }
    if let Some(v) = d.get(b"CropBox") {
        inh.crop = Some(v.clone());
    }
    if let Some(v) = d.get(b"Rotate") {
        inh.rotate = Some(v.clone());
    }
    let kids = doc.dict_entry(&d, b"Kids")?;
    let is_page = d
        .get(b"Type")
        .and_then(Obj::name)
        .is_some_and(|t| t == b"Page");
    match kids {
        Some(Obj::Arr(items)) if !is_page => {
            for item in items {
                if let Obj::Ref(n, _) = item {
                    if !seen.insert(n) {
                        return malformed("page tree contains a cycle");
                    }
                    if seen.len() > doc.lim.max_objects {
                        return limit("objects", "the page tree is too large");
                    }
                }
                let child = doc.resolve(&item)?;
                walk(
                    doc,
                    child,
                    inh.clone(),
                    depth + 1,
                    seen,
                    count,
                    wanted,
                    found,
                )?;
            }
        }
        _ => {
            *count = count.saturating_add(1);
            if *count == wanted {
                *found = Some(PageInfo { dict: d, inh });
            }
        }
    }
    Ok(())
}

/// A rectangle given as four numbers (any order), normalised; `None` when degenerate.
fn rect(doc: &mut Doc, o: Option<&Obj>) -> Result<Option<[f64; 4]>, Fault> {
    let Some(o) = o else {
        return Ok(None);
    };
    let Some(items) = doc.resolve(o)?.arr().map(|a| a.to_vec()) else {
        return Ok(None);
    };
    let [a, b, c, d] = items.as_slice() else {
        return Ok(None);
    };
    let mut v = [0.0f64; 4];
    for (slot, item) in v.iter_mut().zip([a, b, c, d]) {
        match doc.resolve(item)?.num() {
            Some(x) if x.is_finite() => *slot = x,
            _ => return Ok(None),
        }
    }
    let r = [
        v[0].min(v[2]),
        v[1].min(v[3]),
        v[0].max(v[2]),
        v[1].max(v[3]),
    ];
    Ok((r[2] > r[0] && r[3] > r[1]).then_some(r))
}

/// Visible box, rotation and UserUnit. Problems that do not stop the conversion are warnings.
pub fn geometry(doc: &mut Doc, info: &PageInfo, stats: &mut Stats) -> Result<Geometry, Fault> {
    let Some(media) = rect(doc, info.inh.media.as_ref())? else {
        return malformed("the page has no valid MediaBox");
    };
    let mut visible = media;
    if let Some(crop) = rect(doc, info.inh.crop.as_ref())? {
        let inter = [
            media[0].max(crop[0]),
            media[1].max(crop[1]),
            media[2].min(crop[2]),
            media[3].min(crop[3]),
        ];
        if inter[2] > inter[0] && inter[3] > inter[1] {
            visible = inter;
        } else {
            stats.warn(
                "cropbox_outside_mediabox",
                "CropBox does not overlap MediaBox; MediaBox used",
            );
        }
    }
    let rotate = match info.inh.rotate.as_ref() {
        None => 0,
        Some(o) => match doc.resolve(o)?.num().and_then(uint) {
            Some(r) if r.is_multiple_of(90) => r % 360,
            _ => {
                stats.warn(
                    "rotate_invalid",
                    "/Rotate is not a multiple of 90; 0 was used",
                );
                0
            }
        },
    };
    let user_unit = match info.dict.get(b"UserUnit") {
        None => 1.0,
        Some(o) => match doc.resolve(o)?.num() {
            Some(u) if u.is_finite() && u > 0.0 => u,
            _ => {
                stats.warn(
                    "userunit_invalid",
                    "/UserUnit is not a positive number; 1 was used",
                );
                1.0
            }
        },
    };
    Ok(Geometry {
        visible,
        rotate,
        user_unit,
    })
}

/// Transform from page user space to output millimetres, and the upright page size in mm.
pub fn transform(geo: &Geometry, mm_per_point: f64) -> (M, (f64, f64)) {
    let [llx, lly, urx, ury] = geo.visible;
    let k = geo.user_unit * mm_per_point;
    let (w, h) = (urx - llx, ury - lly);
    let base = match geo.rotate {
        90 => M {
            a: 0.0,
            b: -1.0,
            c: 1.0,
            d: 0.0,
            e: -lly,
            f: urx,
        },
        180 => M {
            a: -1.0,
            b: 0.0,
            c: 0.0,
            d: -1.0,
            e: urx,
            f: ury,
        },
        270 => M {
            a: 0.0,
            b: 1.0,
            c: -1.0,
            d: 0.0,
            e: ury,
            f: -llx,
        },
        _ => M {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: -llx,
            f: -lly,
        },
    };
    let m = M {
        a: base.a * k,
        b: base.b * k,
        c: base.c * k,
        d: base.d * k,
        e: base.e * k,
        f: base.f * k,
    };
    let size = if geo.rotate == 90 || geo.rotate == 270 {
        (h * k, w * k)
    } else {
        (w * k, h * k)
    };
    (m, size)
}

/// The page's /Resources, inherited from its ancestors when absent.
pub fn resources(doc: &mut Doc, info: &PageInfo) -> Result<Dict, Fault> {
    match info.inh.resources.as_ref() {
        None => Ok(Dict::default()),
        Some(o) => match doc.resolve(o)? {
            Obj::Dict(d) => Ok(d),
            _ => Ok(Dict::default()),
        },
    }
}

/// Decoded page content: all /Contents streams, joined with a line break.
pub fn contents(doc: &mut Doc, info: &PageInfo) -> Result<Vec<u8>, Fault> {
    let Some(c) = doc.dict_entry(&info.dict, b"Contents")? else {
        return Ok(Vec::new());
    };
    let items = match c {
        Obj::Arr(v) => v,
        other => vec![other],
    };
    let mut out = Vec::new();
    for item in items {
        match doc.resolve(&item)? {
            Obj::Stream(s) => {
                let data = doc.stream_data(&s, None)?;
                out.extend_from_slice(&data);
                out.push(b'\n');
            }
            Obj::Null => {}
            _ => return malformed("page /Contents is not a stream"),
        }
    }
    Ok(out)
}

/// Number of annotations on the page (they are not converted).
pub fn annotation_count(doc: &mut Doc, info: &PageInfo) -> Result<usize, Fault> {
    Ok(match doc.dict_entry(&info.dict, b"Annots")? {
        Some(Obj::Arr(v)) => v.len(),
        _ => 0,
    })
}
