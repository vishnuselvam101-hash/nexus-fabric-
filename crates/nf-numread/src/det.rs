//! Text detection (DBNet, PP-OCRv4): probability map -> connected components -> boxes.

use crate::image_in::Gray;
use crate::{Error, Params};
use rten::Model;
use rten_tensor::prelude::*;
use rten_tensor::Tensor;

/// Boxes [x0, y0, x1, y1] in pixels of `g`, in scan order (top to bottom, left to right).
pub fn detect(model: &Model, g: &Gray, p: &Params) -> Result<Vec<[f32; 4]>, Error> {
    let long = g.w.max(g.h) as f32;
    let s = (p.det_side as f32 / long).min(4.0);
    let nw = (((g.w as f32 * s) / 32.0).round().max(1.0) as usize) * 32;
    let nh = (((g.h as f32 * s) / 32.0).round().max(1.0) as usize) * 32;
    let small = g.resize(nw, nh);
    let mut data = Vec::with_capacity(3 * nw * nh);
    for (m, sd) in [(0.485f32, 0.229f32), (0.456, 0.224), (0.406, 0.225)] {
        for &v in &small.data {
            data.push((f32::from(v) / 255.0 - m) / sd);
        }
    }
    let input: Tensor<f32> = Tensor::from_data(&[1, 3, nh, nw], data);
    let out = model
        .run_one(input.view().into(), None)
        .map_err(|e| Error::Model(format!("detector: {e}")))?;
    let out: Tensor<f32> = out
        .try_into()
        .map_err(|_| Error::Model("detector output is not f32".into()))?;
    let shape = out.shape().to_vec();
    if shape.len() != 4 || shape[2] != nh || shape[3] != nw {
        return Err(Error::Model(format!(
            "detector output shape {shape:?}, expected [1,1,{nh},{nw}]"
        )));
    }
    let prob = out.to_vec();
    let sx = g.w as f32 / nw as f32;
    let sy = g.h as f32 / nh as f32;
    let mut seen = vec![false; nw * nh];
    let mut boxes = Vec::new();
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for y0 in 0..nh {
        for x0 in 0..nw {
            let i0 = y0 * nw + x0;
            if seen[i0] || prob[i0] <= p.bin_thresh {
                continue;
            }
            seen[i0] = true;
            stack.push((x0, y0));
            let (mut x1, mut y1, mut x2, mut y2) = (x0, y0, x0, y0);
            let (mut sum, mut n) = (0f32, 0u32);
            while let Some((x, y)) = stack.pop() {
                sum += prob[y * nw + x];
                n += 1;
                x1 = x1.min(x);
                x2 = x2.max(x);
                y1 = y1.min(y);
                y2 = y2.max(y);
                let mut push = |xx: usize, yy: usize| {
                    let ii = yy * nw + xx;
                    if !seen[ii] && prob[ii] > p.bin_thresh {
                        seen[ii] = true;
                        stack.push((xx, yy));
                    }
                };
                if x > 0 {
                    push(x - 1, y);
                }
                if x + 1 < nw {
                    push(x + 1, y);
                }
                if y > 0 {
                    push(x, y - 1);
                }
                if y + 1 < nh {
                    push(x, y + 1);
                }
            }
            if n < p.min_pixels || (sum / n as f32) < p.box_thresh {
                continue;
            }
            let (bw, bh) = ((x2 - x1 + 1) as f32, (y2 - y1 + 1) as f32);
            let d = bw * bh * p.unclip / (2.0 * (bw + bh));
            boxes.push([
                ((x1 as f32 - d).max(0.0)) * sx,
                ((y1 as f32 - d).max(0.0)) * sy,
                (((x2 + 1) as f32 + d).min(nw as f32)) * sx,
                (((y2 + 1) as f32 + d).min(nh as f32)) * sy,
            ]);
        }
    }
    Ok(boxes)
}
