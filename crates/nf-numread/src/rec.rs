//! Text recognition (SVTR/CTC, PP-OCRv4): a text crop -> characters with their time step.

use crate::image_in::Gray;
use crate::{Error, Params};
use rten::Model;
use rten_tensor::prelude::*;
use rten_tensor::Tensor;

/// One decoded character: the glyph, its CTC time step and its probability.
#[derive(Clone, Debug)]
pub struct Ch {
    pub glyph: String,
    pub step: usize,
    pub prob: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    pub chars: Vec<Ch>,
    /// Number of CTC time steps over the crop width.
    pub steps: usize,
    /// Per step: probability mass of the classes that stand for a diameter sign in this model's
    /// alphabet (it has no phi; it answers with Phi, p or nothing).
    pub phi: Vec<f32>,
}

impl Line {
    /// Typical distance in CTC steps between neighbouring characters (a default for short lines).
    pub fn pitch(&self) -> f32 {
        let mut d: Vec<usize> = self
            .chars
            .windows(2)
            .map(|w| w[1].step - w[0].step)
            .collect();
        if d.is_empty() {
            return 2.0;
        }
        d.sort_unstable();
        d[d.len() / 2].max(1) as f32
    }

    pub fn mean_prob(&self) -> f32 {
        if self.chars.is_empty() {
            return 0.0;
        }
        self.chars.iter().map(|c| c.prob).sum::<f32>() / self.chars.len() as f32
    }
}

/// `dict` is the PaddleOCR key list; index 0 of the model output is the CTC blank, the last is a space.
pub fn recognise(model: &Model, dict: &[String], crop: &Gray, p: &Params) -> Result<Line, Error> {
    let h = p.rec_height;
    let w = ((crop.w as f32 * h as f32 / crop.h as f32).ceil() as usize).clamp(16, p.rec_max_width);
    let rs = crop.resize(w, h);
    let mut data = Vec::with_capacity(3 * w * h);
    for _ in 0..3 {
        for &v in &rs.data {
            data.push((f32::from(v) / 255.0 - 0.5) / 0.5);
        }
    }
    let input: Tensor<f32> = Tensor::from_data(&[1, 3, h, w], data);
    let out = model
        .run_one(input.view().into(), None)
        .map_err(|e| Error::Model(format!("recogniser: {e}")))?;
    let out: Tensor<f32> = out
        .try_into()
        .map_err(|_| Error::Model("recogniser output is not f32".into()))?;
    let shape = out.shape().to_vec();
    if shape.len() != 3 || shape[0] != 1 {
        return Err(Error::Model(format!("recogniser output shape {shape:?}")));
    }
    let (steps, classes) = (shape[1], shape[2]);
    let v = out.to_vec();
    let phi_classes: Vec<usize> = dict
        .iter()
        .enumerate()
        .filter(|(_, g)| matches!(g.as_str(), "Φ" | "p" | "Ф" | "ϕ" | "Ø" | "ø"))
        .map(|(i, _)| i)
        .collect();
    let mut phi = Vec::with_capacity(steps);
    let mut chars = Vec::new();
    let mut prev = 0usize;
    for t in 0..steps {
        let row = &v[t * classes..(t + 1) * classes];
        let sum: f32 = row.iter().sum();
        let norm = if (sum - 1.0).abs() < 0.05 {
            1.0
        } else {
            sum.max(f32::MIN_POSITIVE)
        };
        phi.push(phi_classes.iter().filter_map(|&i| row.get(i)).sum::<f32>() / norm);
        let (mut best, mut bv) = (0usize, f32::MIN);
        for (i, &x) in row.iter().enumerate() {
            if x > bv {
                bv = x;
                best = i;
            }
        }
        if best != 0 && best != prev {
            // The model already outputs a softmax; normalise only if a row is not a distribution.
            let prob = bv / norm;
            if let Some(g) = dict.get(best) {
                chars.push(Ch {
                    glyph: g.clone(),
                    step: t,
                    prob,
                });
            }
        }
        prev = best;
    }
    Ok(Line { chars, steps, phi })
}
