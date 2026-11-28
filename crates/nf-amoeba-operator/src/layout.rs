//! Where the material list is on a page, found from the positions of the numbers alone (the reader returns
//! numbers, not words). A vision model that reads the table replaces this; the rule is stated so that it can
//! be checked: a row of the list is a line of at least four numbers (item, quantity, blank width, blank height)
//! in the lower right of the page; the last two numbers of the row are the blank.

use nf_core::assign::{BomRow, Reading};

/// Row of the list: reading indices (left to right) of its numbers.
pub fn table_rows(readings: &[Reading], width: f32, height: f32) -> Vec<Vec<usize>> {
    let mut cand: Vec<usize> = (0..readings.len())
        .filter(|&i| readings[i].bbox[0] >= 0.4 * width && readings[i].bbox[1] >= 0.4 * height)
        .collect();
    cand.sort_by(|&a, &b| readings[a].bbox[1].total_cmp(&readings[b].bbox[1]));
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for i in cand {
        let r = &readings[i];
        let cy = (r.bbox[1] + r.bbox[3]) / 2.0;
        let tol = (r.bbox[3] - r.bbox[1]).max(4.0) * 0.6;
        match rows.last_mut() {
            Some(row) if {
                let f = &readings[row[0]];
                ((f.bbox[1] + f.bbox[3]) / 2.0 - cy).abs() <= tol
            } => row.push(i),
            _ => rows.push(vec![i]),
        }
    }
    let mut rows: Vec<Vec<usize>> = rows.into_iter().filter(|r| r.len() >= 4).collect();
    for r in &mut rows {
        r.sort_by(|&a, &b| readings[a].bbox[0].total_cmp(&readings[b].bbox[0]));
    }
    rows
}

fn decimals(text: &str) -> u32 {
    text.trim()
        .split(|c: char| !(c.is_ascii_digit() || c == '.'))
        .find(|t| t.contains('.'))
        .and_then(|t| t.split('.').nth(1))
        .map_or(0, |d| d.len() as u32)
}

/// The blank of each row of the list, as BOM rows named by `parts` in row order. A row without a part name is dropped.
pub fn bom_rows(readings: &[Reading], rows: &[Vec<usize>], parts: &[String]) -> Vec<BomRow> {
    rows.iter()
        .zip(parts)
        .filter_map(|(row, part)| {
            let (w, h) = (&readings[row[row.len() - 2]], &readings[row[row.len() - 1]]);
            Some(BomRow {
                part: part.clone(),
                width: w.value,
                height: h.value,
                decimals: decimals(&w.text).max(decimals(&h.text)),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(id: usize, text: &str, x: f32, y: f32) -> Reading {
        Reading {
            id,
            text: text.into(),
            value: text.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0.0),
            page: 1,
            bbox: [x, y, x + 50.0, y + 14.0],
            confidence: 0.99,
            flags: vec![],
            in_table: false,
        }
    }

    #[test]
    fn the_list_is_the_row_of_four_numbers_in_the_lower_right() {
        let rs = vec![
            r(0, "2372.5", 450.0, 330.0),
            r(1, "φ301.0", 120.0, 270.0),
            r(2, "1", 1122.0, 902.0),
            r(3, "1", 1475.0, 903.0),
            r(4, "945.6", 1523.0, 902.0),
            r(5, "2372.5", 1631.0, 901.0),
            r(6, "2372.5", 526.0, 869.0),
        ];
        let rows = table_rows(&rs, 1766.0, 1249.0);
        assert_eq!(rows, vec![vec![2, 3, 4, 5]]);
        let bom = bom_rows(&rs, &rows, &["shell".to_string()]);
        assert_eq!((bom[0].width, bom[0].height, bom[0].decimals), (945.6, 2372.5, 1));
    }
}
