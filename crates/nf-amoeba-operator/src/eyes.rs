//! Eyes: the readings of a page picture, as the server stored them.
//!
//! The operator does not read pictures itself. The server runs the number reader, stores every reading in
//! `order_readings` and gives each an id; the operator takes the readings from the server and confirms a value only
//! by naming the id of the reading it comes from. What the operator claims about a reading is never trusted.

use nf_core::assign::Reading;

pub struct PageRead {
    /// `Reading.id` is the index in this list.
    pub readings: Vec<Reading>,
    /// The server's reading id of each entry of `readings`, by index.
    pub ids: Vec<i64>,
    pub width: f32,
    pub height: f32,
    /// Who read the page (the reader's name as the server recorded it).
    pub reader: String,
}

impl PageRead {
    /// From the JSON the server gives for a page (`readings`, `width`, `height`). A reading without a number is left
    /// out: it cannot fill a slot.
    pub fn from_server(page: i64, v: &serde_json::Value) -> PageRead {
        let (mut readings, mut ids) = (Vec::new(), Vec::new());
        let mut reader = String::new();
        for r in v["readings"].as_array().into_iter().flatten() {
            let (Some(id), Some(value)) = (r["id"].as_i64(), r["value"].as_f64()) else {
                continue;
            };
            let bbox: [f32; 4] =
                std::array::from_fn(|k| r["bbox"][k].as_f64().unwrap_or_default() as f32);
            if reader.is_empty() {
                reader = r["reader_id"].as_str().unwrap_or_default().to_string();
            }
            readings.push(Reading {
                id: readings.len(),
                text: r["text"].as_str().unwrap_or_default().to_string(),
                value,
                page,
                bbox,
                confidence: r["confidence"].as_f64().unwrap_or_default() as f32,
                flags: r["flags"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|f| f.as_str().map(String::from))
                    .collect(),
                in_table: false,
            });
            ids.push(id);
        }
        if reader.is_empty() {
            reader = v["reader"]
                .as_str()
                .unwrap_or("the server's reader")
                .to_string();
        }
        PageRead {
            readings,
            ids,
            width: v["width"].as_f64().unwrap_or_default() as f32,
            height: v["height"].as_f64().unwrap_or_default() as f32,
            reader,
        }
    }
}
