//! General reader for ASCII DXF text: group code and value pairs, sections, header variables with
//! typed values, units from `$INSUNITS`, and hard limits.
//!
//! Every malformed or over-limit input is refused with a [`CadError`] that gives the reason and the
//! 1-based line. Nothing panics on input. Sections are kept as raw pairs, so later parsers (tables,
//! entities) can read any section, known or not.
//!
//! This crate shares no code with `nf-dxf` or `nf-digest`. `nf-dxf` appears only as a dev-dependency
//! that writes test fixtures.

#![forbid(unsafe_code)]

mod drawing;
mod error;
mod header;
mod parse;
mod tables;
mod text;
mod tokenizer;
mod units;

use std::collections::BTreeMap;
use std::io::Read;

pub use drawing::{Drawing, Entity, Layer, LineType, Point3, TextStyle, Vertex};
pub use error::{CadError, Reason};
pub use header::HeaderValue;
pub use tokenizer::Pair;
pub use units::Units;

/// Hard limits. Exceeding one is a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest accepted input, in bytes. Default 32 MiB.
    pub max_bytes: usize,
    /// Largest number of group code and value pairs. Default 4 million, which bounds the memory a
    /// parsed document can hold (about 70 bytes a pair) to a few hundred MB.
    pub max_pairs: usize,
    /// Largest number of entity records (group 0 records in ENTITIES and BLOCKS, not counting ENDBLK),
    /// and the largest record count a table header may declare.
    pub max_entities: usize,
    /// Deepest extended-data nesting (group 1002 `{` inside `}`).
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_bytes: 32 << 20,
            max_pairs: 4_000_000,
            max_entities: 1_000_000,
            max_depth: 16,
        }
    }
}

/// One section, from `0 SECTION` to `0 ENDSEC`.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    /// The name in group 2, for example `ENTITIES`.
    pub name: String,
    /// Line of the `0 SECTION` record.
    pub line: usize,
    /// The records after the name pair and before `0 ENDSEC`, raw, for every section.
    pub pairs: Vec<Pair>,
}

/// A parsed DXF file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Document {
    /// Header variables by name, for example `$ACADVER`. A name set twice is refused.
    pub header: BTreeMap<String, HeaderValue>,
    /// Every section in file order.
    pub sections: Vec<Section>,
    /// Group 999 comments found outside sections.
    pub comments: Vec<String>,
}

impl Document {
    /// Parses with the default [`Limits`].
    pub fn parse(input: &[u8]) -> Result<Document, CadError> {
        Document::parse_with(input, &Limits::default())
    }

    /// Parses with the given limits.
    pub fn parse_with(input: &[u8], limits: &Limits) -> Result<Document, CadError> {
        parse::parse(input, limits)
    }

    /// Reads `reader` and parses it with the default [`Limits`]. At most `max_bytes + 1` bytes are read,
    /// so an endless reader is refused rather than read forever.
    pub fn parse_reader(reader: impl Read) -> Result<Document, CadError> {
        Document::parse_reader_with(reader, &Limits::default())
    }

    /// Reads `reader` and parses it with the given limits. At most `max_bytes + 1` bytes are read: the
    /// cap is enforced by the read itself, before any byte is parsed.
    pub fn parse_reader_with(reader: impl Read, limits: &Limits) -> Result<Document, CadError> {
        let cap = u64::try_from(limits.max_bytes)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let mut bytes = Vec::new();
        reader.take(cap).read_to_end(&mut bytes).map_err(|e| {
            CadError::at(
                0,
                Reason::ReadFailed {
                    message: e.to_string(),
                },
            )
        })?;
        parse::parse(&bytes, limits)
    }

    /// The value of a header variable, for example `variable("$INSUNITS")`.
    pub fn variable(&self, name: &str) -> Option<&HeaderValue> {
        self.header.get(name)
    }

    /// The first section with this name.
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    /// Reads the TABLES and ENTITIES sections into a [`Drawing`]. A bad value, a missing required group,
    /// or non-ASCII text the file's encoding cannot decode is a refusal.
    pub fn drawing(&self) -> Result<Drawing, CadError> {
        drawing::build(self)
    }

    /// Units from `$INSUNITS`. Missing, zero, non-integer or unrecognised values give `Units::Unknown`.
    pub fn units(&self) -> Units {
        match self.variable("$INSUNITS") {
            Some(HeaderValue::Int(code)) => Units::from_insunits(*code),
            _ => Units::Unknown,
        }
    }
}
