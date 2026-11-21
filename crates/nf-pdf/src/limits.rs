//! Hard limits, the internal fault type and the wall-clock deadline.
//!
//! Every bound the converter enforces lives in [`Limits`]. Each one is an implementation
//! limit of this converter: a bound on memory and time chosen for this program, not an
//! engineering value and not a value taken from a standard. Tests may lower them to keep
//! fixtures small.

use std::time::{Duration, Instant};

/// Every bound the converter enforces.
#[derive(Clone, Debug, PartialEq)]
pub struct Limits {
    /// Size of the whole input file (64 MiB).
    pub max_input_bytes: usize,
    /// Total bytes materialised from streams (raw copies and filter output), 256 MiB.
    pub max_decoded_bytes: usize,
    /// Decompression-bomb guard: a filter output larger than `ratio_check_min_bytes`
    /// may not exceed this multiple of its encoded size.
    pub max_ratio: usize,
    /// Filter outputs up to this size are never checked against `max_ratio`.
    pub ratio_check_min_bytes: usize,
    /// Indirect objects (cross-reference entries and objects actually parsed).
    pub max_objects: usize,
    /// Cross-reference sections followed through /Prev.
    pub max_xref_sections: usize,
    /// Content-stream operators, all pages-and-forms included.
    pub max_operators: u64,
    /// Operands waiting for an operator.
    pub max_operands: usize,
    /// Path construction segments (lines, curve pieces after flattening, rectangle edges).
    pub max_path_segments: u64,
    /// Nesting of arrays, dictionaries and objects.
    pub max_nesting: usize,
    /// Items (elements or key/value entries) in one array or dictionary.
    pub max_collection_items: usize,
    /// Items in all arrays and dictionaries: kept from the document structure over the whole
    /// conversion, and, counted separately, in the operands waiting for one content operator.
    pub max_total_items: u64,
    /// Nesting of q/Q graphics-state saves.
    pub max_graphics_depth: usize,
    /// Nesting of Form XObject invocations.
    pub max_form_depth: usize,
    /// Text show operations.
    pub max_text_items: u64,
    /// Size of the DXF output file.
    pub max_output_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_input_bytes: 64 << 20,
            max_decoded_bytes: 256 << 20,
            max_ratio: 100,
            ratio_check_min_bytes: 4 << 20,
            max_objects: 200_000,
            max_xref_sections: 1_000,
            max_operators: 2_000_000,
            max_operands: 1_024,
            max_path_segments: 2_000_000,
            max_nesting: 64,
            max_collection_items: 1_000_000,
            max_total_items: 4_000_000,
            max_graphics_depth: 256,
            max_form_depth: 16,
            max_text_items: 500_000,
            max_output_bytes: 64 << 20,
        }
    }
}

/// Internal failure. Mapped to a refusal or a limit at the API boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum Fault {
    /// The file structure or a content stream cannot be read.
    Malformed(String),
    /// A stream filter or feature this converter does not implement.
    Unsupported(String),
    /// The file is encrypted.
    Encrypted,
    /// A named limit was exceeded.
    Limit(&'static str, String),
    /// An explicit refusal with its machine-readable code (for example "raster_page").
    Refused(&'static str, String),
    /// The wall-clock budget ran out.
    Timeout,
}

/// Shorthand for a malformed-structure fault.
pub fn malformed<T>(msg: impl Into<String>) -> Result<T, Fault> {
    Err(Fault::Malformed(msg.into()))
}

/// Shorthand for a limit fault.
pub fn limit<T>(name: &'static str, msg: impl Into<String>) -> Result<T, Fault> {
    Err(Fault::Limit(name, msg.into()))
}

/// Every loop that can run long checks the deadline after this many iterations of its own
/// counter (never on a data-dependent position, which a crafted file could make skip every check).
pub const CHECK_EVERY: u64 = 4096;

/// Wall-clock budget. Checked inside every loop that can run long.
#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    end: Instant,
}

impl Deadline {
    pub fn after(d: Duration) -> Self {
        // Capped so that adding it to `now` cannot overflow.
        let capped = d.min(Duration::from_secs(86_400));
        Deadline {
            end: Instant::now() + capped,
        }
    }

    pub fn check(&self) -> Result<(), Fault> {
        if Instant::now() >= self.end {
            Err(Fault::Timeout)
        } else {
            Ok(())
        }
    }
}
