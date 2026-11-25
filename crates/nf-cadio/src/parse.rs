//! Splits the pairs into sections and enforces the limits.

use crate::error::{CadError, Reason};
use crate::header::{int_value, parse_variables};
use crate::tokenizer::{tokenize, Pair};
use crate::{Document, Limits, Section};

pub(crate) fn parse(input: &[u8], limits: &Limits) -> Result<Document, CadError> {
    if input.len() > limits.max_bytes {
        return Err(CadError::at(
            0,
            Reason::TooLarge {
                bytes: input.len(),
                limit: limits.max_bytes,
            },
        ));
    }
    let pairs = tokenize(input)?;
    let mut doc = Document::default();
    let mut entities = 0usize;
    let mut eof = false;
    let mut i = 0;
    while i < pairs.len() {
        let pair = &pairs[i];
        if eof {
            return Err(structure(pair.line, "data after 0 EOF"));
        }
        if pair.code == 999 {
            doc.comments.push(pair.value.clone());
            i += 1;
            continue;
        }
        if pair.code != 0 {
            return Err(structure(
                pair.line,
                "expected 0 SECTION, 0 EOF or a 999 comment",
            ));
        }
        match pair.value.as_str() {
            "EOF" => {
                eof = true;
                i += 1;
            }
            "SECTION" => {
                let name = match pairs.get(i + 1) {
                    Some(name) if name.code == 2 => name,
                    _ => return Err(structure(pair.line, "SECTION without a name (group 2)")),
                };
                let start = i + 2;
                let end = section_end(&pairs, start, pair.line)?;
                let section = Section {
                    name: name.value.clone(),
                    line: pair.line,
                    pairs: pairs[start..end].to_vec(),
                };
                check_body(&section, limits, &mut entities)?;
                if section.name == "HEADER" {
                    doc.header.extend(parse_variables(&section.pairs)?);
                }
                doc.sections.push(section);
                i = end + 1;
            }
            _ => return Err(structure(pair.line, "expected 0 SECTION or 0 EOF")),
        }
    }
    if !eof {
        let line = pairs.last().map_or(0, |p| p.line);
        return Err(CadError::at(line, Reason::MissingEof));
    }
    Ok(doc)
}

fn structure(line: usize, detail: &'static str) -> CadError {
    CadError::at(line, Reason::Structure { detail })
}

/// Index of the `0 ENDSEC` that closes a section whose body starts at `start`.
fn section_end(pairs: &[Pair], start: usize, section_line: usize) -> Result<usize, CadError> {
    for (j, pair) in pairs.iter().enumerate().skip(start) {
        if pair.code == 0 {
            match pair.value.as_str() {
                "ENDSEC" => return Ok(j),
                "SECTION" | "EOF" => {
                    return Err(structure(pair.line, "section not closed by 0 ENDSEC"));
                }
                _ => {}
            }
        }
    }
    let line = pairs.last().map_or(section_line, |p| p.line);
    Err(structure(line, "section not closed by 0 ENDSEC"))
}

/// Per-section checks: record boundaries, extended-data braces, the entity count and table counts.
fn check_body(section: &Section, limits: &Limits, entities: &mut usize) -> Result<(), CadError> {
    let counts_entities = matches!(section.name.as_str(), "ENTITIES" | "BLOCKS");
    let is_tables = section.name == "TABLES";
    let is_header = section.name == "HEADER";
    let body = &section.pairs;
    let mut depth = 0usize;
    for (j, pair) in body.iter().enumerate() {
        match pair.code {
            0 => {
                if is_header {
                    return Err(structure(pair.line, "record inside HEADER"));
                }
                if depth != 0 {
                    return Err(CadError::at(pair.line, Reason::UnbalancedBraces));
                }
                if counts_entities && pair.value != "ENDBLK" {
                    *entities += 1;
                    if *entities > limits.max_entities {
                        return Err(CadError::at(
                            pair.line,
                            Reason::TooManyEntities {
                                limit: limits.max_entities,
                            },
                        ));
                    }
                }
            }
            1002 => match pair.value.as_str() {
                "{" => {
                    depth += 1;
                    if depth > limits.max_depth {
                        return Err(CadError::at(
                            pair.line,
                            Reason::TooDeep {
                                limit: limits.max_depth,
                            },
                        ));
                    }
                }
                "}" => {
                    if depth == 0 {
                        return Err(CadError::at(pair.line, Reason::UnbalancedBraces));
                    }
                    depth -= 1;
                }
                _ => {
                    return Err(structure(
                        pair.line,
                        "extended-data group 1002 is not { or }",
                    ))
                }
            },
            70 if is_tables && j >= 2 && is_table_count(body, j) => {
                let declared = int_value(pair)?;
                if declared < 0 {
                    return Err(CadError::at(
                        pair.line,
                        Reason::InvalidValue {
                            code: pair.code,
                            text: pair.value.chars().take(32).collect(),
                        },
                    ));
                }
                if !usize::try_from(declared).is_ok_and(|d| d <= limits.max_entities) {
                    return Err(CadError::at(
                        pair.line,
                        Reason::DeclaredCountTooLarge {
                            declared,
                            limit: limits.max_entities,
                        },
                    ));
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(CadError::at(section.line, Reason::UnbalancedBraces));
    }
    Ok(())
}

/// True when the pair at `j` is the record count of a table header: `0 TABLE`, `2 NAME`, `70 count`.
fn is_table_count(body: &[Pair], j: usize) -> bool {
    body[j - 2].code == 0 && body[j - 2].value == "TABLE" && body[j - 1].code == 2
}
