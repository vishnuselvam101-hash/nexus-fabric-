# Page images from PDF pages (`crates/nf-raster`, program `nf-raster`)

Status: NFJ-1 lane S1-L0 (spike plus child). Not yet started by the server; the server side (a `rasterconv.rs` beside `pdfconv.rs`) is a later lane. Scanned pages are the main input of the product, and `nf-pdf2dxf` refuses them (`raster_page`); this program turns any page into an image the page viewer and the reader can use. The page image is a picture of the paper, not a measurement: it is never a DXF and carries no scale claim beyond the page size in points that the report states.

## Commands

```
nf-raster info --in <pdf> --out <json>
nf-raster page <N> --in <pdf> --out <png> --report <json> [--dpi 200] [--gray]
```

- `info`: page count and, for each page, size in points, `/Rotate`, kind (`scanned`: a page-covering image and at most 10 painted paths, the same rule as `raster_page` of nf-pdf; `vector`; `mixed`; `blank`; `unknown` with an error code) and counts of images and lines.
- `page`: PNG of the upright page, 8-bit gray (`--gray`, or when no image has colour) or RGB, at 30 to 1200 dpi (default 200), with the dpi in the pHYs chunk. Size is `round(page points x dpi / 72)` (the page is `CropBox` clipped to `MediaBox`, rotated by `/Rotate`, times `/UserUnit`).
- Exit codes and files are as for `nf-pdf2dxf`: 0 done; 2 refused; 3 limit exceeded; 70 internal error; one JSON line on stderr for every failure; the report file holds `status: incomplete` until the answer replaces it; the PNG is written to `<out>.partial` and renamed only after success.

## What is drawn

- Image XObjects and inline images, in paint order, composited through their matrices (rotation, flips, several images on one page). Box-filter reduction by whole factors, then bilinear sampling: a 1-bit scan at its own resolution is reproduced bit for bit (tests compare against the source bitmap).
- Filters: Flate, ASCIIHex, ASCII85, RunLength (nf-pdf's bounded filters, with PNG and TIFF predictors) and the image filters DCT, CCITTFaxDecode (G3 1-D, G3 2-D, G4), JBIG2Decode (with `JBIG2Globals`; the crate implements generic, symbol, text, halftone and refinement regions, only generic regions with arithmetic coding and MMR are covered by tests here). Colour: DeviceGray/RGB/CMYK, Cal*, ICCBased by component count, Indexed, Separation and one-colour DeviceN as a tint, ImageMask stencils (painted in the fill gray), `/Decode`, 1/2/4/8/16 bits.
- Vector fallback: stroked paths and the outlines of filled paths as lines (dashes, width, gray from the device colour operators; white fills and strokes are skipped, so a white background rectangle does not draw a border). No text (no fonts), no fill areas, no clipping, no shadings, no patterns: a vector page is a line drawing of its geometry. This is deliberate for a fallback; vector pages go to `nf-pdf2dxf`.
- Not applied (a warning in the report, the image is drawn without it): `/SMask`, `/Mask`. Refused with a code: JPXDecode (`unsupported_filter`), an ImageMask in DCT (`unsupported_filter`), bits per component other than 1, 2, 4, 8, 16 (`unsupported_bpc`), LZWDecode (`unsupported_filter`), other colour spaces (`unsupported_colorspace`).

## Refusals (exit 2) and limits (exit 3)

Codes of nf-pdf (`malformed`, `encrypted`, `unsupported_filter`, `page_out_of_range`, `invalid_page`, `input_unreadable`, `output_unwritable`, `invalid_arguments`) plus `unsupported_colorspace`, `image_decode_failed` (corrupt JPEG, CCITT or JBIG2 data) and `invalid_dpi`. A truncated image (fewer rows than declared) is drawn with the rest left white and the warning `image_truncated`; an image with no data at all is refused.

`limit_exceeded` with `limit` naming: everything in `docs/backend/PDF_TO_DXF.md` (input 64 MiB, decoded bytes 256 MiB, expansion ratio 100:1, objects, nesting, operators, path segments), plus, all implementation limits of this program (memory and time, not engineering values):

| `limit` | value | where |
|---|---|---|
| `image_pixels` | 100,000,000 pixels in one image | `PdfDoc::load_image` (declared size), CCITT/JBIG2/JPEG decoders (their own size) |
| `image_bytes_total` | 800,000,000 bytes of decoded images per page | `render` |
| `output_pixels` | 200,000,000 bytes of canvas (pixels times channels) | `render`; lower the dpi |
| `scene_items` | 2,000,000 lines and images, at most 4,096 image placements | `interp.rs` capture |
| `draw_work` | 1,500,000,000 pixel visits of lines and image placements on one page | `Canvas` (charged per stroke segment, per reduction and per image row block) |
| `wall_clock` | 60 s in the library; the parent stops the worker after 150 s | `Deadline`, `nf-raster` parent |
| `cpu_time` | 120 s (RLIMIT_CPU) | sandbox |

## Isolation (same as `nf-pdf2dxf`, `crates/nf-raster/src/bin/nf-raster/sandbox.rs`)

The parent opens the input, creates the output and report files and starts a copy of itself as the worker with no environment, the input as standard input and a pipe as standard output. The worker, before it reads any input: dies with its parent (PR_SET_PDEATHSIG); sets RLIMIT_AS 3 GiB, CPU 120 s, FSIZE 256 MiB, NOFILE 16, no core file; applies Landlock with no rule (no file, no TCP, no abstract UNIX socket, no signals); applies a seccomp allow-list (read, write, memory without PROT_EXEC, futex, clock, exit; everything else, among them socket, open, openat, execve, clone and io_uring, fails with EPERM). The report's `isolation` object states what was applied, and the caller must refuse a result without it (as `pdfconv.rs` does). Windows: Job Object (`nf-pdf-winjob`). The image decoders therefore run with no files and no threads: the JPEG decoder is chosen for that (see below).

`--isolation-probe` makes the worker attempt forbidden operations after each step. Verified output (Linux 7.0, Landlock ABI 8):

```
landlock: read /etc/hostname: refused (EACCES)      seccomp: socket inet tcp: refused (EPERM)
landlock: tcp connect 127.0.0.1:9: refused (EACCES) seccomp: socket unix: refused (EPERM)
landlock: run /bin/true: refused (EACCES)           seccomp: read the input by name: refused (EPERM)
                                                    seccomp: run /bin/true: refused (EPERM)
```

Tests: `the_worker_is_isolated`, `a_killed_program_leaves_an_incomplete_report` (`tests/cli.rs`).

## Dependencies and licences (all new crates pure Rust; nothing is downloaded at run time)

| crate | version | licence | unsafe code |
|---|---|---|---|
| hayro-jbig2 (JBIG2) | 0.4.0 | Apache-2.0 OR MIT | `forbid(unsafe_code)`; used with `default-features = false` (no `image`, no SIMD) |
| hayro-ccitt (CCITT G3/G4) | 0.5.0 | Apache-2.0 OR MIT | `forbid(unsafe_code)` |
| zune-jpeg (JPEG) | 0.5.15 | MIT OR Apache-2.0 OR Zlib | with `default-features = false` (no `x86`, no `neon`) the crate is `forbid(unsafe_code)` |
| zune-core | 0.5.3 | MIT OR Apache-2.0 OR Zlib | none (the word appears only in option names) |
| crc32fast, flate2 (miniz_oxide), sha2 | already in the workspace | MIT/Apache-2.0 | as before |

All are in `packaging/about.toml`'s accepted list. `crates/nf-raster` is `#![forbid(unsafe_code)]`. The jbig2 crate was chosen over writing a decoder: it is the decoder of the hayro PDF renderer, pure Rust, and decodes the two generated JBIG2 fixtures to the same pixels as CCITT G4 of the same bitmap and the same dark-pixel count as poppler. `jpeg-decoder` was tried first and rejected: without rayon it starts a thread for images above 128x128 pixels, which the seccomp filter (rightly) forbids; the filter was not loosened. Offline use: the crates are in the Cargo registry cache and pinned in `Cargo.lock` (`--locked`); this repository has no `vendor/` directory for any crate, so none was added. `cargo-about` is not installed on this host, so `packaging/THIRD_PARTY_NOTICES.txt` was not regenerated; run `packaging/generate_notices.sh` before packaging nf-raster.

## Changes to nf-pdf

Additive only (its 116 tests pass unchanged): `pub mod raster` (`PdfDoc`: open, page count, `scene`, `load_image`), a capture mode of the interpreter (`RunOpts::capture`, off by default: lines and image placements in pixels instead of DXF), tracking of line width and device colours, inline-image dictionaries, `Doc::filter_stages` made crate-visible. The converter's output is unchanged.

## Tests (`crates/nf-raster/tests`)

`tests/fixtures/make_fixtures.py` makes every fixture from the synthetic sample pages of `nf-reader-data` (clean pages, generated; no partner drawings). The JBIG2 fixtures are written by a small MQ encoder in that script (generic region, template 0) and by wrapping CCITT G4 data (MMR); poppler decodes them identically. Fixtures: DCT gray, RGB and CMYK; CCITT G4 and G3 1-D; Flate gray with PNG predictor, Flate 1-bit with `/Decode [1 0]`, Indexed 4-bit; JBIG2 arithmetic and MMR; ImageMask; two images on one page; `/Rotate 90` with an offset MediaBox and a CropBox; `/Rotate 180`; A3 at 200 dpi in G4 and DCT; the vector sample; eight hostile files (garbage, truncated, page-tree loop, encrypted, corrupt JPEG, JPX, 100000 x 100000 declared size, a 200 MiB Flate bomb).

- every scanned fixture: non-blank, size within 1 px of the page size at 200 dpi, page count, pHYs;
- G4, G3, Flate 1-bit and ImageMask renderings are pixel-identical to each other and to the source bitmap; JBIG2 (both codings) is identical to G4 of the same crop;
- rotation by 180 and 90 with boxes checked against the upright rendering;
- every hostile file gives its coded refusal; 28,000 random mutations (bit flips, truncation, overwrites) of seven fixtures and random garbage never panic (`NF_RASTER_FUZZ_ROUNDS=4000`);
- A3 200 dpi under 3 s (measured 0.49 s G4, 0.64 s DCT in the library, 0.6 to 0.7 s for the whole program including the sandbox).

JBIG2: `hayro-jbig2` allocates region bitmaps without a bound, so a region of impossible size would abort the worker (code 70). The region segment headers of the embedded stream are walked first and a region above `image_pixels` is a coded limit. Regions inside `JBIG2Globals` and sizes declared by symbol dictionaries are not pre-checked; the worker's memory limit still ends those with code 70.
