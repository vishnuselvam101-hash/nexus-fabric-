//! The TABLES section: LAYER, LTYPE, STYLE and BLOCK_RECORD entries. Other tables (VPORT, DIMSTYLE,
//! APPID, ...) are not modelled and are ignored.

use crate::drawing::{records, Drawing, Layer, LineType, Record, TextStyle};
use crate::error::CadError;
use crate::header::real;
use crate::text::TextEncoding;
use crate::tokenizer::Pair;

pub(crate) fn read(pairs: &[Pair], enc: &TextEncoding, out: &mut Drawing) -> Result<(), CadError> {
    for rec in records(pairs)? {
        match rec.kind {
            "LAYER" => out.layers.push(layer(&rec, enc)?),
            "LTYPE" => out.line_types.push(line_type(&rec, enc)?),
            "STYLE" => out.text_styles.push(text_style(&rec, enc)?),
            "BLOCK_RECORD" => out.blocks.push(rec.required_text(2, enc)?),
            _ => {}
        }
    }
    Ok(())
}

/// Group 62 is the colour index; a negative value means the layer is off. Group 70 bit 1 is frozen.
fn layer(rec: &Record<'_>, enc: &TextEncoding) -> Result<Layer, CadError> {
    let colour = rec.int(62)?.unwrap_or(7);
    let flags = rec.int(70)?.unwrap_or(0);
    Ok(Layer {
        name: rec.required_text(2, enc)?,
        color: colour.saturating_abs(),
        line_type: rec.text(6, enc)?,
        on: colour >= 0,
        frozen: flags & 1 != 0,
    })
}

/// Group 73 gives the number of elements; it must match the group 49 values that follow.
fn line_type(rec: &Record<'_>, enc: &TextEncoding) -> Result<LineType, CadError> {
    let elements = rec
        .body
        .iter()
        .filter(|p| p.code == 49)
        .map(real)
        .collect::<Result<Vec<f64>, CadError>>()?;
    if let Some(declared) = rec.int(73)? {
        if usize::try_from(declared).ok() != Some(elements.len()) {
            return Err(rec.invalid(73));
        }
    }
    Ok(LineType {
        name: rec.required_text(2, enc)?,
        description: rec.text(3, enc)?.unwrap_or_default(),
        elements,
    })
}

fn text_style(rec: &Record<'_>, enc: &TextEncoding) -> Result<TextStyle, CadError> {
    Ok(TextStyle {
        name: rec.required_text(2, enc)?,
        font: rec.text(3, enc)?.unwrap_or_default(),
        height: rec.real(40)?.unwrap_or(0.0),
        width_factor: rec.real(41)?.unwrap_or(1.0),
    })
}
