//! The canvas: white paper, lines and image placements.
//!
//! Coordinates arrive in PDF page space scaled to pixels (origin lower left, y up); the canvas
//! is stored top row first, so a point `(x, y)` lands in row `height - y`.

// Indexing here is on fixed three-element arrays with loop bounds of three.
#![allow(clippy::indexing_slicing)]

use nf_pdf::raster::{Deadline, Fault, Matrix};

use crate::decode::{luma, Pixmap};

pub struct Canvas {
    pub w: u32,
    pub h: u32,
    pub ch: usize,
    pub data: Vec<u8>,
}

impl Canvas {
    pub fn new(w: u32, h: u32, ch: usize) -> Self {
        Canvas {
            w,
            h,
            ch,
            data: vec![255; (w as usize) * (h as usize) * ch],
        }
    }

    fn at(&mut self, x: i64, y: i64) -> Option<&mut [u8]> {
        if x < 0 || y < 0 || x >= i64::from(self.w) || y >= i64::from(self.h) {
            return None;
        }
        let i = (y as usize * self.w as usize + x as usize) * self.ch;
        self.data.get_mut(i..i + self.ch)
    }

    /// Darkens pixel (x, y) toward `gray` by `cover` (0..=1); never lightens.
    fn darken(&mut self, x: i64, y: i64, cover: f32, gray: u8) {
        if let Some(px) = self.at(x, y) {
            let v = (255.0 - cover * f32::from(255 - gray)).round() as u8;
            for c in px {
                if v < *c {
                    *c = v;
                }
            }
        }
    }

    /// A line of `width` pixels along the polyline (y up). Stamps discs every half pixel.
    pub fn polyline(&mut self, pts: &[(f64, f64)], closed: bool, width: f64, gray: u8) {
        let r = (width.max(1.0) / 2.0).min(50.0);
        let n = pts.len();
        for i in 0..n.saturating_sub(1) + usize::from(closed && n > 2) {
            let (Some(&p), Some(&q)) = (pts.get(i), pts.get((i + 1) % n)) else {
                continue;
            };
            self.segment(p, q, r, gray);
        }
        if n == 1 {
            if let Some(&p) = pts.first() {
                self.segment(p, p, r, gray);
            }
        }
    }

    fn segment(&mut self, p: (f64, f64), q: (f64, f64), r: f64, gray: u8) {
        let hh = f64::from(self.h);
        let (mut x0, mut y0, mut x1, mut y1) = (p.0, hh - p.1, q.0, hh - q.1);
        if ![x0, y0, x1, y1].iter().all(|v| v.is_finite()) {
            return;
        }
        // Clip to the canvas grown by the radius (Liang-Barsky), so that huge segments cost
        // only their visible length.
        let (xmin, ymin) = (-r - 1.0, -r - 1.0);
        let (xmax, ymax) = (f64::from(self.w) + r + 1.0, hh + r + 1.0);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        for (pp, qq) in [
            (-dx, x0 - xmin),
            (dx, xmax - x0),
            (-dy, y0 - ymin),
            (dy, ymax - y0),
        ] {
            if pp == 0.0 {
                if qq < 0.0 {
                    return;
                }
            } else {
                let t = qq / pp;
                if pp < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
            }
        }
        if t0 > t1 {
            return;
        }
        (x0, y0, x1, y1) = (x0 + t0 * dx, y0 + t0 * dy, x0 + t1 * dx, y0 + t1 * dy);
        let len = (x1 - x0).hypot(y1 - y0);
        let steps = ((len * 2.0).ceil() as u64).max(1);
        let reach = r.ceil() as i64 + 1;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let (cx, cy) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            let (ix, iy) = (cx.floor() as i64, cy.floor() as i64);
            for yy in iy - reach..=iy + reach {
                for xx in ix - reach..=ix + reach {
                    let d = (xx as f64 + 0.5 - cx).hypot(yy as f64 + 0.5 - cy);
                    let cover = (r + 0.5 - d).clamp(0.0, 1.0) as f32;
                    if cover > 0.0 {
                        self.darken(xx, yy, cover, gray);
                    }
                }
            }
        }
    }

    /// Paints `img` through `ctm` (unit square to pixels, y up). `fill` is the stencil colour.
    pub fn image(
        &mut self,
        img: &Pixmap,
        ctm: &Matrix,
        fill: u8,
        dl: &Deadline,
    ) -> Result<(), Fault> {
        let det = ctm.a * ctm.d - ctm.b * ctm.c;
        if !det.is_finite() || det.abs() < 1e-9 || img.w == 0 || img.h == 0 {
            return Ok(());
        }
        let hh = f64::from(self.h);
        // Source pixels per destination pixel along each image axis.
        let fx = (f64::from(img.w) / ctm.a.hypot(ctm.b)).floor().max(1.0) as u32;
        let fy = (f64::from(img.h) / ctm.c.hypot(ctm.d)).floor().max(1.0) as u32;
        let shrunk;
        let src: &Pixmap = if fx > 1 || fy > 1 {
            shrunk = shrink(img, fx.min(img.w), fy.min(img.h));
            &shrunk
        } else {
            img
        };
        let corners = [
            (ctm.e, ctm.f),
            (ctm.a + ctm.e, ctm.b + ctm.f),
            (ctm.c + ctm.e, ctm.d + ctm.f),
            (ctm.a + ctm.c + ctm.e, ctm.b + ctm.d + ctm.f),
        ];
        let xs = corners.iter().map(|p| p.0);
        let ys = corners.iter().map(|p| hh - p.1);
        let x_lo = xs.clone().fold(f64::MAX, f64::min).floor().max(0.0);
        let x_hi = xs.fold(f64::MIN, f64::max).ceil().min(f64::from(self.w));
        let y_lo = ys.clone().fold(f64::MAX, f64::min).floor().max(0.0);
        let y_hi = ys.fold(f64::MIN, f64::max).ceil().min(hh);
        if !(x_lo.is_finite() && x_hi.is_finite() && y_lo.is_finite() && y_hi.is_finite()) {
            return Ok(());
        }
        let (sw, sh) = (f64::from(src.w), f64::from(src.h));
        for y in (y_lo as i64)..(y_hi as i64) {
            if y % 64 == 0 {
                dl.check()?;
            }
            let yy = hh - (y as f64 + 0.5);
            for x in (x_lo as i64)..(x_hi as i64) {
                let xx = x as f64 + 0.5;
                let (px, py) = (xx - ctm.e, yy - ctm.f);
                let u = (ctm.d * px - ctm.c * py) / det;
                let v = (-ctm.b * px + ctm.a * py) / det;
                if !(0.0..=1.0).contains(&u) || !(0.0..=1.0).contains(&v) {
                    continue;
                }
                let (sx, sy) = (u * sw - 0.5, (1.0 - v) * sh - 0.5);
                let samp = sample(src, sx, sy);
                if src.mask {
                    let a = samp[0] / 255.0;
                    let ch = self.ch;
                    if let Some(px) = self.at(x, y) {
                        for c in px.iter_mut().take(ch) {
                            *c = (f32::from(*c) * (1.0 - a) + f32::from(fill) * a).round() as u8;
                        }
                    }
                } else if let Some(px) = self.at(x, y) {
                    match (px.len(), src.ch) {
                        (1, 1) => {
                            if let Some(o) = px.first_mut() {
                                *o = samp[0].round() as u8;
                            }
                        }
                        (1, _) => {
                            if let Some(o) = px.first_mut() {
                                *o = luma(
                                    samp[0].round() as u8,
                                    samp[1].round() as u8,
                                    samp[2].round() as u8,
                                );
                            }
                        }
                        (_, 1) => {
                            for o in px.iter_mut() {
                                *o = samp[0].round() as u8;
                            }
                        }
                        _ => {
                            for (o, s) in px.iter_mut().zip(samp) {
                                *o = s.round() as u8;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Bilinear sample at (x, y) in source pixel units (pixel centres at integers); up to 3 channels.
fn sample(img: &Pixmap, x: f64, y: f64) -> [f32; 3] {
    let x0 = x.floor();
    let y0 = y.floor();
    let (fx, fy) = ((x - x0) as f32, (y - y0) as f32);
    let get = |xi: f64, yi: f64| -> [f32; 3] {
        let xi = xi.clamp(0.0, f64::from(img.w) - 1.0) as usize;
        let yi = yi.clamp(0.0, f64::from(img.h) - 1.0) as usize;
        let i = (yi * img.w as usize + xi) * img.ch;
        let mut o = [0f32; 3];
        for (k, slot) in o.iter_mut().enumerate() {
            *slot = img
                .data
                .get(i + k.min(img.ch - 1))
                .map_or(255.0, |&v| f32::from(v));
        }
        o
    };
    let (a, b, c, d) = (
        get(x0, y0),
        get(x0 + 1.0, y0),
        get(x0, y0 + 1.0),
        get(x0 + 1.0, y0 + 1.0),
    );
    let mut out = [0f32; 3];
    for k in 0..3 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bot = c[k] + (d[k] - c[k]) * fx;
        out[k] = top + (bot - top) * fy;
    }
    out
}

/// Box-filter reduction by whole factors; the last partial block averages what exists.
fn shrink(img: &Pixmap, fx: u32, fy: u32) -> Pixmap {
    let nw = img.w.div_ceil(fx);
    let nh = img.h.div_ceil(fy);
    let ch = img.ch;
    let mut out = Vec::with_capacity(nw as usize * nh as usize * ch);
    let mut acc = vec![0u32; nw as usize * ch];
    let mut cnt = vec![0u32; nw as usize];
    for by in 0..nh {
        acc.fill(0);
        cnt.fill(0);
        for y in (by * fy)..((by + 1) * fy).min(img.h) {
            let row = img
                .data
                .get(y as usize * img.w as usize * ch..(y as usize + 1) * img.w as usize * ch)
                .unwrap_or(&[]);
            for (x, px) in row.chunks_exact(ch).enumerate() {
                let bx = x / fx as usize;
                if let Some(c) = cnt.get_mut(bx) {
                    *c += 1;
                }
                for (k, &v) in px.iter().enumerate() {
                    if let Some(a) = acc.get_mut(bx * ch + k) {
                        *a += u32::from(v);
                    }
                }
            }
        }
        for (bx, c) in cnt.iter().enumerate() {
            for k in 0..ch {
                let a = acc.get(bx * ch + k).copied().unwrap_or(0);
                out.push(((a + c / 2) / (*c).max(1)) as u8);
            }
        }
    }
    Pixmap {
        w: nw,
        h: nh,
        ch,
        data: out,
        mask: img.mask,
    }
}
