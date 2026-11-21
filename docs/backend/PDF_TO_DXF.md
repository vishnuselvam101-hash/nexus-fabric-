# PDF to DXF converter (`crates/nf-pdf`, binary `nf-pdf2dxf`)

Status: phase 1. A deterministic, offline converter from one vector PDF page to DXF R12 (AC1009), as a library and a command-line program. Not yet connected to the server or the interface. Outputs are marked NOT RELEASED.

## What is converted

- Stroke paths `S s B B* b b*` go to `PDF_STROKE`. Closed subpaths (`h`, `s`, `re`) are closed polylines.
- Dashed strokes (non-empty `d`) go to `PDF_DASHED` as the on-pieces of the pattern. Dash lengths scale by the square root of |det| of the CTM at paint time (exact for uniform scaling).
- Filled-only paths `f F f*` go to `PDF_FILL` as outlines. `B` and `b` produce both. `--no-fill` removes `PDF_FILL` output.
- Curves are flattened in output millimetres. For a cubic with `M = max(|P0-2P1+P2|, |P1-2P2+P3|)` the piece count is `n = ceil(sqrt(3M / (4 tol)))`, and the chord error is at most `3M / (4 n^2) <= tol`. Measured against 4001 exact samples per test curve: 0.00707 mm at tol 0.01, 0.03395 mm at tol 0.05, 0.33333 mm at tol 0.5.
- `q Q cm` with the full matrix; Form XObjects (depth limit 16); `/Rotate` 0, 90, 180, 270; MediaBox and CropBox origin (visible box = CropBox clipped to MediaBox); `/UserUnit`.
- Text (`Tj TJ ' "`) becomes `TEXT` on `PDF_TEXT` only where the decoding is reliable: WinAnsi; the ASCII ranges of StandardEncoding and MacRomanEncoding; `/Differences` with ASCII or `uniXXXX` names; ToUnicode CMaps (`bfchar`, `bfrange`) for simple and Identity-H fonts. Anything else is skipped and counted with a reason. Characters outside printable ASCII (and `%`, `^`) are written as `?`.
- Dimension text is never read as geometry or as a scale.

## Scale rule

- One PDF user-space unit is 1/72 inch of paper, multiplied by `/UserUnit` when present.
- One PDF point on paper is 25.4/72 mm. A drawing scale 1:N therefore gives `mm per point = 25.4/72 * N` (1:10 gives 3.527777...).
- Output millimetres per user unit = mm per point * UserUnit.
- Give either `--mm-per-point X` or `--scale 1:N`. With neither, the converter refuses (`scale_required`). It never guesses.

## Accuracy statement

The DXF reproduces what is drawn in the PDF, at the scale supplied. A drawing is not necessarily to scale. Dimension text is not interpreted. The result is not a flat pattern, has not been checked against any manufacturing requirement, and carries the NOT RELEASED marking on layer `NF_NOT_RELEASED`.

## Output

- DXF R12 (AC1009), layers `PDF_STROKE` (7), `PDF_DASHED` (3), `PDF_FILL` (5), `PDF_TEXT` (2), `NF_NOT_RELEASED` (1). Origin at the lower-left of the visible page, upright.
- Leading 999 comment lines: `NF-SOURCE`, `NF-SOURCE-SHA256`, `NF-PAGE`, `NF-SCALE-MM-PER-POINT`, `NF-UNITS: mm`, `NF-CHORD-TOLERANCE-MM`, `NF-CONVERTER-VERSION`, `NF-USER-UNIT`, `NF-PATHS-PAINTED`, `NF-SEGMENTS`, `NF-TEXT-SKIPPED`.
- Two TEXT notes above the page on `NF_NOT_RELEASED`: `CONVERTED FROM PDF - NOT VERIFIED - NOT RELEASED` and the scale used.
- JSON report (`--report`): status, refusal, source hash, page and page count, scale, user unit, rotation, boxes, tolerance, counts, bounding box (mm), warnings with counts, text skipped by reason, output hash, and the accuracy notes.

## Refusals (exit 2) and limits (exit 3)

Refusals carry a code and a message on stderr as JSON, and no DXF is written:
`encrypted`, `malformed`, `unsupported_filter`, `page_out_of_range`, `raster_page` (a page-covering image and at most 10 painted paths: scanned drawings cannot be converted reliably and are not supported), `no_vector_paths`, `scale_required`, `scale_ambiguous`, `invalid_scale`, `invalid_tolerance`, `invalid_page`, `invalid_arguments`, `input_unreadable`, `output_unwritable`.

Limit exceedances (exit 3, `code: limit_exceeded`, `limit` names the bound). Defaults are in `Limits`:

| limit | default |
|---|---|
| `input_bytes` | 64 MiB |
| `decoded_bytes` (total materialised stream bytes) | 256 MiB |
| `compression_ratio` (filter output above 4 MiB and above 100 times its input) | 100:1 |
| `objects` | 200,000 |
| `xref_sections` | 1,000 |
| `operators` | 2,000,000 |
| `operands` (waiting for an operator) | 1,024 |
| `path_segments` (curve pieces and dash pieces included) | 2,000,000 |
| `nesting` (arrays and dictionaries) | 64 |
| `graphics_depth` (q/Q) | 256 |
| `form_depth` | 16 |
| `text_items` | 500,000 |
| `output_bytes` | 64 MiB |
| `wall_clock` | 30 s |

Warnings, not refusals (in the report): clipping not applied; shadings not converted; pattern fills converted as outlines; optional content ignored (all content drawn); annotations not converted; unknown or malformed operators ignored; colours, line widths, caps, joins, fill rules not represented.

Filters: FlateDecode (PNG predictors 10 to 15, TIFF predictor 2 at 8 bits), ASCIIHexDecode, ASCII85Decode, RunLengthDecode. Image data is never decoded. Anything else in a content stream is `unsupported_filter`.

## Isolation (required before the server runs it)

- Run `nf-pdf2dxf` as a separate process, with no network: `unshare -n` where available.
- Launch with no inherited environment (`env -i` or `env_clear`), a fresh empty temporary working directory holding the input, and a kill timeout longer than the 30 s internal budget (for example 60 s).
- On Unix the binary applies its own limits after opening its files and before parsing: RLIMIT_AS 2 GiB, RLIMIT_CPU 60 s, RLIMIT_FSIZE 96 MiB, RLIMIT_NOFILE 16, RLIMIT_CORE 0. It uses the rustix crate for these, with no unsafe code of its own.
- A death by signal or exit code 70 is a failure. A DXF exists only after a successful exit 0.
- Windows: this build applies no limits itself. The parent must use a separate process, a Job Object with memory and CPU limits and kill-on-close, a process timeout, and a network block. Until that is in place Windows is not isolated.

## Fuzzing

- `tests/fuzz.rs`: seeded and deterministic. Mutations of valid files (bit flips, byte edits, token insertion, deletion, duplication, truncation, splices), random operator streams with huge and NaN-like numbers, and deep nesting.
- Release run, 200,000 cases: 0 panics, 0 cases over the 1 s per-case bound, slowest case 193 ms. Outcomes: 23,184 converted, 162,390 refused, 14,426 limit exceeded (the run was made under a 4 GiB address-space cap).
- A regression test covers the tiny-dash-pattern case found during development.
- cargo-fuzz is not installed in this environment, so no `cargo fuzz` target was added.

## Dependencies and licences

Fetched from crates.io and pinned in `Cargo.lock`. No C code, no network code, no OCR, no AI. The PDF parser is our own; no PDF crate is used.

- flate2 1.1.11, MIT OR Apache-2.0, with the pure-Rust backend miniz_oxide 0.9.1 (MIT OR Zlib OR Apache-2.0), adler2 (0BSD OR MIT OR Apache-2.0), crc32fast (MIT OR Apache-2.0), simd-adler32 (MIT).
- sha2 0.10.9, MIT OR Apache-2.0, with digest, block-buffer, crypto-common, cpufeatures, cfg-if, generic-array (MIT), typenum (MIT OR Apache-2.0).
- rustix 1.1.6, Unix only, Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT, with bitflags (MIT OR Apache-2.0) and linux-raw-sys (Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT).
- Tests: nf-dxf (this workspace) for re-reading the DXF; ReportLab 5.0.1 (generator `tests/fixtures/gen_reportlab.py`, run as `python3 -I`) for three small real fixtures.

## Not supported

Scanned or raster drawings (refused); encrypted files (refused); shadings and patterns (not converted); clipping (not applied); annotations (not converted); optional content (all drawn); fonts that need predefined CMaps, vertical writing, Type 3 glyph procedures, symbolic fonts without a ToUnicode map (text skipped); LZW and image filters in content (refused); colour, line width, caps, joins and fill rules (not represented); characters outside printable ASCII in text (written as `?`).

## Tests

`cargo test -p nf-pdf`: unit tests, geometry and container variants, refusals and every named limit, round trip through nf-dxf, command-line behaviour, real ReportLab files, regressions, and a fuzz run (3,000 cases in debug; `NF_PDF_FUZZ_CASES` overrides). Release fuzz: `cargo test --release -p nf-pdf --test fuzz`.
