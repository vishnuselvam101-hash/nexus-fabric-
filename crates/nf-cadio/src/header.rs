//! Header variables: `9 $NAME` followed by its value pairs, with typed values.

use std::collections::BTreeMap;

use crate::error::{CadError, Reason};
use crate::tokenizer::Pair;

/// The value of one header variable.
#[derive(Debug, Clone, PartialEq)]
pub enum HeaderValue {
    /// A string (group codes 0 to 9, 100 to 109, 300 to 369, 390 to 399, 410 to 419, 430 to 439, 470 to 481).
    Text(String),
    /// An integer (group codes 60 to 99, 160 to 179, 270 to 299, 370 to 389, 400 to 409, 420 to 429, 440 to 459).
    Int(i64),
    /// A finite real (group codes 10 to 59, 110 to 149, 210 to 239, 460 to 469).
    Real(f64),
    /// Coordinates written as 10 and 20, or 10, 20 and 30.
    Point(Vec<f64>),
    /// Any other combination, kept as (group code, raw value).
    Raw(Vec<(u16, String)>),
}

enum Kind {
    Text,
    Int,
    Real,
    Other,
}

fn kind(code: u16) -> Kind {
    match code {
        0..=9 | 100..=109 | 300..=369 | 390..=399 | 410..=419 | 430..=439 | 470..=481 => Kind::Text,
        10..=59 | 110..=149 | 210..=239 | 460..=469 => Kind::Real,
        60..=99 | 160..=179 | 270..=299 | 370..=389 | 400..=409 | 420..=429 | 440..=459 => {
            Kind::Int
        }
        _ => Kind::Other,
    }
}

/// Reads the body of a HEADER section (the pairs between its name and `0 ENDSEC`). Comments (999) are
/// skipped. A variable set twice is refused: two values for one variable (for example two `$INSUNITS`)
/// would otherwise be resolved silently.
pub(crate) fn parse_variables(body: &[Pair]) -> Result<BTreeMap<String, HeaderValue>, CadError> {
    let mut vars = BTreeMap::new();
    let mut current: Option<(String, Vec<&Pair>)> = None;
    for pair in body {
        if pair.code == 999 {
            continue;
        }
        if pair.code == 9 {
            if let Some((name, group)) = current.take() {
                vars.insert(name, typed(&group)?);
            }
            let name = pair.value.trim();
            if !name.starts_with('$') || name.len() < 2 {
                return Err(CadError::at(pair.line, Reason::HeaderNameNotDollar));
            }
            if vars.contains_key(name) {
                return Err(CadError::at(
                    pair.line,
                    Reason::DuplicateHeaderVariable {
                        name: name.to_string(),
                    },
                ));
            }
            current = Some((name.to_string(), Vec::new()));
        } else {
            match current.as_mut() {
                Some((_, group)) => group.push(pair),
                None => {
                    return Err(CadError::at(pair.line, Reason::HeaderValueWithoutName));
                }
            }
        }
    }
    if let Some((name, group)) = current.take() {
        vars.insert(name, typed(&group)?);
    }
    Ok(vars)
}

fn typed(group: &[&Pair]) -> Result<HeaderValue, CadError> {
    match group {
        [] => Ok(HeaderValue::Raw(Vec::new())),
        [only] => scalar(only),
        _ => {
            let codes: Vec<u16> = group.iter().map(|p| p.code).collect();
            if codes == [10, 20] || codes == [10, 20, 30] {
                let coords = group
                    .iter()
                    .map(|p| real(p))
                    .collect::<Result<Vec<f64>, CadError>>()?;
                Ok(HeaderValue::Point(coords))
            } else {
                Ok(HeaderValue::Raw(
                    group.iter().map(|p| (p.code, p.value.clone())).collect(),
                ))
            }
        }
    }
}

fn scalar(pair: &Pair) -> Result<HeaderValue, CadError> {
    Ok(match kind(pair.code) {
        Kind::Text => HeaderValue::Text(pair.value.clone()),
        Kind::Int => HeaderValue::Int(int_value(pair)?),
        Kind::Real => HeaderValue::Real(real(pair)?),
        Kind::Other => HeaderValue::Raw(vec![(pair.code, pair.value.clone())]),
    })
}

/// An integer value, or a refusal naming the code.
pub(crate) fn int_value(pair: &Pair) -> Result<i64, CadError> {
    pair.value.trim().parse::<i64>().map_err(|_| invalid(pair))
}

fn real(pair: &Pair) -> Result<f64, CadError> {
    match pair.value.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => Err(invalid(pair)),
    }
}

fn invalid(pair: &Pair) -> CadError {
    CadError::at(
        pair.line,
        Reason::InvalidValue {
            code: pair.code,
            text: pair.value.chars().take(32).collect(),
        },
    )
}
