//! Splits the pairs into sections and enforces the limits.
//!
//! The input is read twice. The first pass refuses bad tokens and too many pairs and stores nothing.
//! The second pass builds the sections one pair at a time and checks each record as it arrives, so a
//! refused input never holds a second copy of its pairs. Section bodies are moved, never cloned.

use crate::error::{CadError, Reason};
use crate::header::{int_value, parse_variables};
use crate::tokenizer::{for_each_pair, Pair};
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
    for_each_pair(input, limits.max_pairs, |_| Ok(()))?;
    let mut builder = Builder::new(limits);
    for_each_pair(input, limits.max_pairs, |pair| builder.push(pair))?;
    builder.finish()
}

/// Where the reader is between pairs.
enum State {
    /// Outside every section: `0 SECTION`, `0 EOF` or a 999 comment.
    Top,
    /// After `0 SECTION` at `line`, waiting for its group 2 name.
    Name { line: usize },
    /// Inside a section.
    Body(Open),
    /// After `0 EOF`.
    Eof,
}

/// A section being read.
struct Open {
    name: String,
    /// Line of the `0 SECTION` record.
    line: usize,
    pairs: Vec<Pair>,
    /// Extended-data nesting depth (group 1002 braces).
    depth: usize,
    /// True from `0 TABLE` in TABLES up to the next record.
    in_table_header: bool,
    /// True once the group 70 of the current table header has been checked. That group 70 is the
    /// record count, and it comes before the first record of the table in both R12 and R2000 layouts.
    table_count_seen: bool,
}

struct Builder<'a> {
    limits: &'a Limits,
    doc: Document,
    state: State,
    entities: usize,
    has_header: bool,
    /// Line of the last pair seen (0 before the first). Used for end-of-input refusals.
    last_line: usize,
}

impl<'a> Builder<'a> {
    fn new(limits: &'a Limits) -> Self {
        Builder {
            limits,
            doc: Document::default(),
            state: State::Top,
            entities: 0,
            has_header: false,
            last_line: 0,
        }
    }

    fn push(&mut self, pair: Pair) -> Result<(), CadError> {
        self.last_line = pair.line;
        match std::mem::replace(&mut self.state, State::Top) {
            State::Top => self.top(pair),
            State::Name { line } => self.name(pair, line),
            State::Body(open) => self.body(open, pair),
            State::Eof => Err(CadError::at(pair.line, Reason::DataAfterEof)),
        }
    }

    fn top(&mut self, pair: Pair) -> Result<(), CadError> {
        if pair.code == 999 {
            self.doc.comments.push(pair.value);
            return Ok(());
        }
        match (pair.code, pair.value.as_str()) {
            (0, "SECTION") => self.state = State::Name { line: pair.line },
            (0, "EOF") => self.state = State::Eof,
            _ => return Err(CadError::at(pair.line, Reason::RecordOutsideSection)),
        }
        Ok(())
    }

    fn name(&mut self, pair: Pair, line: usize) -> Result<(), CadError> {
        if pair.code != 2 {
            return Err(CadError::at(line, Reason::SectionWithoutName));
        }
        if pair.value.trim() != pair.value {
            return Err(CadError::at(pair.line, Reason::SectionNameWhitespace));
        }
        if pair.value == "HEADER" {
            if self.has_header {
                return Err(CadError::at(line, Reason::DuplicateHeader));
            }
            self.has_header = true;
        }
        self.state = State::Body(Open {
            name: pair.value,
            line,
            pairs: Vec::new(),
            depth: 0,
            in_table_header: false,
            table_count_seen: false,
        });
        Ok(())
    }

    fn body(&mut self, mut open: Open, pair: Pair) -> Result<(), CadError> {
        if pair.code == 0 {
            match pair.value.as_str() {
                "ENDSEC" => return self.close(open),
                "SECTION" | "EOF" => {
                    return Err(CadError::at(pair.line, Reason::SectionNotClosed));
                }
                _ => {}
            }
        }
        open.record(pair, self.limits, &mut self.entities)?;
        self.state = State::Body(open);
        Ok(())
    }

    fn close(&mut self, open: Open) -> Result<(), CadError> {
        if open.depth != 0 {
            return Err(CadError::at(open.line, Reason::UnbalancedBraces));
        }
        if open.name == "HEADER" {
            self.doc.header = parse_variables(&open.pairs)?;
        }
        self.doc.sections.push(Section {
            name: open.name,
            line: open.line,
            pairs: open.pairs,
        });
        self.state = State::Top;
        Ok(())
    }

    fn finish(self) -> Result<Document, CadError> {
        match self.state {
            State::Eof => Ok(self.doc),
            State::Top => Err(CadError::at(self.last_line, Reason::MissingEof)),
            State::Name { line } => Err(CadError::at(line, Reason::SectionWithoutName)),
            State::Body(_) => Err(CadError::at(self.last_line, Reason::SectionNotClosed)),
        }
    }
}

impl Open {
    /// Checks one pair of the body, then keeps it. `entities` counts across sections.
    fn record(
        &mut self,
        pair: Pair,
        limits: &Limits,
        entities: &mut usize,
    ) -> Result<(), CadError> {
        let line = pair.line;
        match pair.code {
            0 => {
                if self.name == "HEADER" {
                    return Err(CadError::at(line, Reason::RecordInHeader));
                }
                if self.depth != 0 {
                    return Err(CadError::at(line, Reason::UnbalancedBraces));
                }
                if matches!(self.name.as_str(), "ENTITIES" | "BLOCKS") && pair.value != "ENDBLK" {
                    *entities += 1;
                    if *entities > limits.max_entities {
                        return Err(CadError::at(
                            line,
                            Reason::TooManyEntities {
                                limit: limits.max_entities,
                            },
                        ));
                    }
                }
                self.in_table_header = self.name == "TABLES" && pair.value == "TABLE";
                self.table_count_seen = false;
            }
            1002 => match pair.value.as_str() {
                "{" => {
                    self.depth += 1;
                    if self.depth > limits.max_depth {
                        return Err(CadError::at(
                            line,
                            Reason::TooDeep {
                                limit: limits.max_depth,
                            },
                        ));
                    }
                }
                "}" => {
                    if self.depth == 0 {
                        return Err(CadError::at(line, Reason::UnbalancedBraces));
                    }
                    self.depth -= 1;
                }
                _ => return Err(CadError::at(line, Reason::ExtendedDataGroup)),
            },
            70 if self.in_table_header && !self.table_count_seen => {
                self.table_count_seen = true;
                check_table_count(&pair, limits)?;
            }
            _ => {}
        }
        self.pairs.push(pair);
        Ok(())
    }
}

/// The record count of a table header: a non-negative integer no larger than `max_entities`.
fn check_table_count(pair: &Pair, limits: &Limits) -> Result<(), CadError> {
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
    Ok(())
}
