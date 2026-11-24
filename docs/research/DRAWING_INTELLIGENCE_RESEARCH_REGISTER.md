# Drawing Intelligence Research Register

Version 0.1 · Sources accessed 18 November 2026 · Companion to [Spec 03](../specifications/03_DRAWING_INTELLIGENCE_AND_INGESTION_SPECIFICATION.md) and the [Dependency Licensing Review](DEPENDENCY_LICENSING_REVIEW.md)

> Engineering research, not legal advice, and **not a selection**. "Licence (verified)" means the LICENSE file in the project's official repository was read on the access date; it does not cover bundled third-party code, model weights or training data unless stated. **Versions were not recorded in this pass** — each candidate's exact version, weights hash and licence must be pinned and re-checked when it enters an evaluation. Capability statements are general knowledge to be tested, not benchmark results. No candidate has been run on any drawing.

## 1. Technology candidates

### PDF parsing and rendering

| Candidate | Official source | Licence | Commercial / on-premises | Known limitations (to test) | Benchmark needs |
|---|---|---|---|---|---|
| PDFium | pdfium.googlesource.com | BSD-3-style + Apache-2.0 (verified, licensing review) | Suitable in principle; bundled third-party licences to check | C++ library (isolated worker); content-stream access level via public API to be confirmed | Path/text extraction fidelity on CAD-plotted PDFs; Japanese CID fonts |
| `pdfium-render` (Rust binding) | github.com/ajrcarey/pdfium-render | MIT OR Apache-2.0 (verified) | Suitable in principle | Binding coverage; needs a PDFium binary | As above |
| `lopdf` (Rust) | github.com/J-F-Liu/lopdf | MIT (verified) | Suitable | Low-level object access; content interpretation, fonts and rendering are ours to build | Robustness on malformed files; effort estimate |
| `pdf` (pdf-rs) | github.com/pdf-rs/pdf | MIT-style text (verified; confirm SPDX) | Suitable | Maturity | As above |
| pdf.js | github.com/mozilla/pdf.js | Apache-2.0 (verified) | Suitable | JavaScript runtime in the pipeline; candidate as *independent renderer* for the cross-check and for client display | Rendering agreement with primary parser |
| qpdf | github.com/qpdf/qpdf | Apache-2.0 (verified) | Suitable | Structural tool (validation, repair, decryption), not content interpretation | Use as structural validator |
| MuPDF / PyMuPDF | github.com/ArtifexSoftware/mupdf | AGPL-3.0 (verified) or commercial | Not without commercial licence ⚖ | — | Only if a quotation is pursued |
| Poppler | poppler.freedesktop.org | GPL (not verified in this pass) | Presumed unsuitable for linking ⚖ | — | — |

### Raster processing

| Candidate | Source | Licence | Notes | Benchmark needs |
|---|---|---|---|---|
| OpenCV | github.com/opencv/opencv | Apache-2.0 (verified, 4.x branch) | Broad; C++; optional modules and bundled codecs have their own licences; some algorithms historically patent-encumbered (check per module) | Thin-line preservation; determinism across builds |
| Leptonica | github.com/DanBloomberg/leptonica | Custom permissive, BSD-style (file header seen; confirm full text) | Document-image operations; Tesseract dependency | — |
| `image` (Rust) | github.com/image-rs/image | MIT (verified; dual licence to confirm) | Decoding; sandbox-friendly | Decoder robustness on malformed TIFF/JPEG |
| libtiff / libjpeg / libpng | — | Not checked | If used via other libraries | — |

### OCR (local execution)

| Candidate | Source | Licence | Commercial / on-premises | Known limitations (to test) | Benchmark needs |
|---|---|---|---|---|---|
| Tesseract | github.com/tesseract-ocr/tesseract | Apache-2.0 (verified) | Suitable | General-purpose; behaviour on rotated dimension text and small symbols | Japanese (horizontal/vertical), digits, symbols; alternatives output |
| Tesseract trained data (`tessdata_best`) | github.com/tesseract-ocr/tessdata_best | Apache-2.0 (verified) | Suitable | Training-data provenance not reviewed | — |
| PaddleOCR | github.com/PaddlePaddle/PaddleOCR | Apache-2.0 (verified for the repository) | Code suitable; **each model's weights and training-data terms to be checked individually** | Framework dependency; determinism | Same test set as above |
| EasyOCR | github.com/JaidedAI/EasyOCR | Apache-2.0 (verified, repository) | Weights terms to check | — | — |
| docTR | github.com/mindee/doctr | Apache-2.0 (verified, repository) | Weights terms to check | Japanese support to confirm | — |
| manga-ocr | github.com/kha-white/manga-ocr | Apache-2.0 (verified, repository) | Weights/training-data terms to check | Trained for a different domain (comics); included only as a Japanese-recognition comparison point | — |
| In-house dimension-token recogniser | — | Ours | Trained on synthetic renderings (tier S) and, with permission, tenant data | Must be built and validated | Numeric false-accept rate |
| Vision-language models (local) | Various | Varies widely: permissive, research-only, use-restricted | Only models whose weights permit commercial redistribution | Non-determinism unless constrained; hallucinated text is a specific hazard for dimensions — restricted to origin-6 candidates | Proposal quality only; never numeric authority |
| Cloud OCR / reading APIs | Vendors | Contract | Only under tenant automated-reading policy; never required | Data leaves the site | Comparison where authorised |

### CAD parsing

| Candidate | Source | Licence | Notes |
|---|---|---|---|
| ezdxf | github.com/mozman/ezdxf | MIT (verified, licensing review) | Python; mature DXF coverage; candidate reader in Z1 and independent validator (Spec 06). Using it for both ingestion and output validation would reduce independence — avoid dual use |
| Rust DXF crates | crates.io | Not checked | Coverage and maintenance to assess |
| Own tag-level DXF reader | — | Ours | Format is documented; gives full control of error reporting; effort to estimate |
| DWG: ODA SDK / LibreDWG | see licensing review §6 | Proprietary membership / GPL-3.0-or-later | Undecided ⚖ |
| STEP | via kernel port (candidate kernels in Spec 01 §4) | per kernel | AP coverage and PMI support to test |
| JWW (Jw_cad) | Not researched | Unknown | T-14: confirm need with factory first |

### View detection and layout

No off-the-shelf component is assumed adequate. Direction: rule-based hypothesis generation over extracted geometry/annotations, optional learned proposals (document-layout or detection models, licences per model), deterministic evaluation (Spec 03 §5.5). Published research on engineering-drawing understanding, dimension–geometry association and reconstruction from orthographic views to be surveyed (T-10).

### Local automated-reading inference runtimes

| Candidate | Source | Licence | Notes |
|---|---|---|---|
| ONNX Runtime | github.com/microsoft/onnxruntime | Believed MIT (**not verified**: licence file not found at the path tried) | Portable; CPU/GPU providers; determinism per provider to test |
| OpenVINO | github.com/openvinotoolkit/openvino | Apache-2.0 (verified) | CPU-oriented; hardware fit depends on `FR-07-1` |
| llama.cpp | github.com/ggerganov/llama.cpp | MIT (verified) | For local language/vision-language models, if any are used |

Model file formats that can execute code on load are prohibited (Spec 03 §11).

## 2. Research and decision items

| ID | Item | Type |
|---|---|---|
| T-01 | Content-stream access and font handling of each PDF candidate on CAD-plotted Japanese PDFs | Benchmark |
| T-02 | Recognition of text drawn as outlines/stroked fonts (render-then-recognise accuracy) | Benchmark |
| T-03 | Circle/arc recognition from Béziers and segment chains: residual models and thresholds | Analysis + tier S |
| T-04 | Vectorisation methods for scanned line drawings; positional uncertainty model | Literature + benchmark |
| T-05 | Scale and anisotropy estimation robustness | Benchmark |
| T-06 | OCR engines on Japanese drawings: accuracy, alternatives quality, determinism | Benchmark (needs authorised data) |
| T-07 | Design and training of the dimension-token recogniser; synthetic data realism | R&D |
| T-08 | Whether engine agreement plus measurement consistency yields an acceptable numeric false-accept rate (input to Spec 05 decision on DI-33) | Benchmark |
| T-09 | Score calibration methods and their stability across drawing sources | Analysis |
| T-10 | Literature: engineering-drawing interpretation, dimension association, 3D reconstruction from orthographic views | Literature review |
| T-11 | Line-role classification from width/dash clusters; reliability on scans | Benchmark |
| T-12 | Welding-symbol and surface-symbol recognition; structured representation | R&D + standards |
| T-13 | Title-block template mechanism and effort per customer | Design + factory input |
| T-14 | Need for JWW/SXF and available reader technology/licences | Factory input, then research |
| T-15 | DWG path | Licensing decision ⚖ |
| T-16 | DXF reader: own vs library; Shift-JIS and legacy dialect handling | Benchmark |
| T-17 | STEP AP and PMI coverage of candidate kernels | Benchmark |
| T-18 | Sandboxing technology per supported OS; cost of process-per-file | Spec 07 |
| T-19 | Deterministic inference: runtime settings and cross-hardware equivalence | Benchmark |
| T-20 | Review UI that makes wrong readings/associations visible; anti-rubber-stamping | UX research |
| T-21 | Synthetic drawing generator (JA/EN styles, degradations) | Build (test tooling) |

## 3. Standards and references to obtain (none verified)

| ID | Subject | Note |
|---|---|---|
| N-01 | Japanese standards for technical drawing practice: projection methods, line types, dimensioning, reference/not-to-scale conventions | Standard numbers deliberately not quoted until confirmed |
| N-02 | General tolerances (linear/angular; welded constructions; sheared/cut parts) | Needed for general-tolerance note interpretation |
| N-03 | Welding symbols; surface texture symbols | Needed for §3.3 structured parsing |
| N-04 | Pipe nominal sizes and dimensions; structural section tables; material grade designations | Needed for *standard-derived* values (DI-31); table data must come from the standards themselves or a licensed database |
| N-05 | ISO equivalents of the above for English-language drawings | — |
| N-06 | PDF specification (ISO 32000) — content streams, `UserUnit`, optional content, encryption/permissions | Authoritative reference for §1.2 |
| N-07 | Autodesk DXF reference (group codes, `$INSUNITS`, DIMENSION entity) | Authoritative reference for §1.2 |
| N-08 | ISO 10303-21 and the relevant application protocols | STEP |

## 4. Licensing and legal items raised by this specification ⚖

| ID | Item |
|---|---|
| L-01 | Per-model review of weights and training-data terms for every OCR/vision model (code licence is not enough) |
| L-02 | Redistribution rights for weights in on-premises installations |
| L-03 | Handling of PDFs with usage restrictions (permission flags) and of password-protected files supplied by customers |
| L-04 | Use of standards tables (copyright in standards; licensed data sources) |
| L-05 | Tenant permission model for using corrections and drawings in tuning, including drawings owned by the tenant's customers |
| L-06 | Fonts: embedded fonts in customer PDFs; CJK fonts shipped for rendering |
| L-07 | Third-party codecs bundled with imaging libraries |
| L-08 | AGPL/GPL exposure (MuPDF, Poppler, LibreDWG) — excluded by default |

## Sources (licence files read 18 November 2026)

- https://github.com/PaddlePaddle/PaddleOCR (LICENSE)
- https://github.com/opencv/opencv (LICENSE, branch 4.x)
- https://github.com/mozilla/pdf.js (LICENSE)
- https://github.com/J-F-Liu/lopdf (LICENSE)
- https://github.com/qpdf/qpdf (LICENSE.txt)
- https://github.com/tesseract-ocr/tesseract (LICENSE) · https://github.com/tesseract-ocr/tessdata_best (LICENSE)
- https://github.com/JaidedAI/EasyOCR (LICENSE)
- https://github.com/mindee/doctr (LICENSE)
- https://github.com/kha-white/manga-ocr (LICENSE)
- https://github.com/ajrcarey/pdfium-render (LICENSE.md)
- https://github.com/pdf-rs/pdf (LICENSE)
- https://github.com/DanBloomberg/leptonica (leptonica-license.txt, header only)
- https://github.com/ArtifexSoftware/mupdf (COPYING)
- https://github.com/image-rs/image (LICENSE-MIT)
- https://github.com/ggerganov/llama.cpp (LICENSE)
- https://github.com/openvinotoolkit/openvino (LICENSE)
- Earlier: PDFium, ezdxf, LibreDWG, ODA — see the Dependency Licensing Review
