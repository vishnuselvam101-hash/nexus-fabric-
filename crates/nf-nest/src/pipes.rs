//! 1D pipe / bar cut list: first-fit decreasing on stock lengths, with the kerf counted between cuts.

use crate::NestError;

#[derive(Clone, Debug, PartialEq)]
pub struct PipeIn {
    pub id: String,
    pub len_mm: f64,
    pub qty: u32,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Bar {
    /// (part id, cut length), in cutting order.
    pub cuts: Vec<(String, f64)>,
    /// Cut lengths plus the kerf between them.
    pub used_mm: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PipePlan {
    pub stock_mm: f64,
    pub kerf_mm: f64,
    pub bars: Vec<Bar>,
}

impl PipePlan {
    /// Cut length over stock length, in percent, over all bars.
    pub fn utilisation(&self) -> f64 {
        if self.bars.is_empty() {
            return 0.0;
        }
        let cut: f64 = self
            .bars
            .iter()
            .flat_map(|b| b.cuts.iter().map(|c| c.1))
            .sum();
        cut / (self.stock_mm * self.bars.len() as f64) * 100.0
    }
}

/// First-fit decreasing. A cut must fit one stock length on its own.
pub fn cut_pipes(items: &[PipeIn], stock_mm: f64, kerf_mm: f64) -> Result<PipePlan, NestError> {
    if !(stock_mm > 0.0 && stock_mm <= 20000.0) || !(0.0..=2.0).contains(&kerf_mm) {
        return Err(NestError::new(
            "stock must be above 0 and at most 20000 mm; kerf 0 to 2 mm (UNVERIFIED rules)",
        ));
    }
    if items.len() > crate::MAX_PARTS {
        return Err(NestError::new(format!(
            "at most {} part kinds per cut list",
            crate::MAX_PARTS
        )));
    }
    let mut cuts: Vec<(String, f64)> = Vec::new();
    for it in items {
        if !(it.len_mm > 0.0) {
            return Err(NestError::new(format!("pipe {} has no length", it.id)));
        }
        if it.len_mm > stock_mm {
            return Err(NestError::new(format!(
                "pipe {} ({} mm) is longer than the stock ({stock_mm} mm)",
                it.id, it.len_mm
            )));
        }
        for _ in 0..it.qty {
            cuts.push((it.id.clone(), it.len_mm));
        }
    }
    cuts.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut bars: Vec<Bar> = Vec::new();
    for c in cuts {
        let slot = bars.iter().position(|b| {
            let add = if b.cuts.is_empty() {
                c.1
            } else {
                c.1 + kerf_mm
            };
            b.used_mm + add <= stock_mm + 1e-9
        });
        let bar = match slot {
            Some(i) => &mut bars[i],
            None => {
                bars.push(Bar::default());
                bars.last_mut().expect("just pushed")
            }
        };
        bar.used_mm += if bar.cuts.is_empty() {
            c.1
        } else {
            c.1 + kerf_mm
        };
        bar.cuts.push(c);
    }
    Ok(PipePlan {
        stock_mm,
        kerf_mm,
        bars,
    })
}
