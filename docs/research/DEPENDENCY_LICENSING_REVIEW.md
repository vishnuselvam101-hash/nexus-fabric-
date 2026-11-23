# Dependency Licensing Review (initial)

Version 0.1 · Sources accessed 18 November 2026

> **This is engineering research, not legal advice.** It records what the licensors publish and which questions follow for NEXUS FABRIC. Every item marked ⚖ needs review by qualified counsel before the dependency is adopted. Items marked **TBV** were not confirmed from a primary source in this pass.

## 1. Why this matters for this product

NEXUS FABRIC is intended to be proprietary and to be **distributed** (on-premises servers, desktop applications, possibly mobile apps). Distribution is what activates most copyleft obligations. Hosted-only use changes the analysis for GPL/LGPL but not for AGPL. The architecture keeps options open by isolating native libraries behind process boundaries ([ADR-002](../architecture/ADR-002-rust-and-native-library-integration.md)) and deferring kernel selection ([ADR-003](../architecture/ADR-003-cad-kernel-selection-strategy.md)); process isolation is an engineering measure and must not be assumed to resolve any licence question by itself ⚖.

## 2. Open CASCADE Technology (OCCT)

| Item | Finding | Source |
|---|---|---|
| Licence | "OCCT is distributed under LGPL 2.1 with an additional exception." | occt3d.com/open-cascade-technology (redirect target of dev.opencascade.org/resources/licensing) |
| Version current at review | 8.0.1 listed as latest release | same |
| Exception | "Open CASCADE Technology exception (version 1.0)" to LGPL 2.1: permits distributing object code that incorporates material from the library's header files under your own terms, with prominent notice in documentation that the code uses or is based on OCCT | `OCCT_LGPL_EXCEPTION.txt` in the OCCT repository (github.com/Open-Cascade-SAS/OCCT) |
| Commercial | The vendor page advertises support, training, maintenance and "commercial components"; it does not describe an alternative commercial licence for the core | occt3d.com page above |

**Implications to confirm ⚖**

- LGPL 2.1 allows linking from proprietary software but carries duties on distribution: licence text and notices, source for the library and any modifications to it, and the ability for the recipient to use a modified version of the library with the application (straightforward with dynamic linking or a separate worker process; harder with static linking).
- Any change we make to OCCT itself (bug fixes, patches) must be made available under LGPL when distributed.
- The exception addresses header/template material; its exact scope should be read by counsel against how our shim includes OCCT headers.
- Mobile app-store distribution of LGPL code is contentious (static linking, user replaceability). Current architecture keeps OCCT server/worker-side only (ADR-007); if that changes this must be re-reviewed.
- Which optional OCCT modules or third-party components bundled in a given build carry other licences (TBV per build configuration).
- "Commercial components" (e.g. vendor DXF/DWG or other add-ons) are separately licensed; terms not public.

## 3. CGAL

| Item | Finding | Source |
|---|---|---|
| Licences | "Starting from release 4.0, CGAL is available under the GNU General Public License (GPL) and the LGPL v3+." | cgal.org/license.html |
| Split | "the Kernel and Support libraries are under the LGPL"; "most geometric algorithms and data structures are under the GPL" | same |
| Per-package authority | Package Overview page lists each package's licence; the LICENSE file in the source distribution is authoritative | same |
| Commercial | Commercial licences for CGAL or parts of it are sold by GeometryFactory | same |

Per-package licences, CGAL 6.2.1 manual (doc.cgal.org/latest/Manual/packages.html):

| Package | Licence | Relevance to us |
|---|---|---|
| 2D and 3D Linear Geometry Kernel | LGPL | Exact predicates/constructions |
| dD Kernel | LGPL | — |
| Number Types | LGPL | Exact/interval number types |
| 2D Polygons | LGPL | Basic polygon ops |
| Halfedge Data Structures | LGPL | — |
| 2D Arrangements | **GPL** | Robust curve arrangements |
| 2D Regularized Boolean Set-Operations | **GPL** | Cut-profile booleans |
| 2D Straight Skeleton and Polygon Offsetting | **GPL** | Kerf offsets |
| 2D Minkowski Sums | **GPL** | Offsets, nesting |
| 2D Triangulations | **GPL** | Meshing |
| Polygon Mesh Processing | **GPL** | Fold-back / mesh checks |
| 3D Boolean Operations on Nef Polyhedra | **GPL** | Exact 3D booleans |
| Surface Mesh | **GPL** | — |
| AABB Tree | **GPL** | Distance queries |
| dD Spatial Searching | **GPL** | — |

Packages not in this table were not checked. Licences can change between releases; re-check at the version actually adopted.

**Implications to confirm ⚖**

- The packages most useful to us for safety-critical 2D profile work are GPL. Using them in distributed proprietary software without a commercial licence is presumed incompatible with keeping our code closed.
- Whether running GPL code in a separate process avoids the combined-work question is a legal judgement we must not assume.
- Even the LGPL parts are **LGPL v3+**, which adds terms beyond LGPL 2.1 (e.g. installation-information provisions for consumer devices) — relevant if ever shipped client-side.
- CGAL depends on GMP/MPFR (LGPL) for exact number types (TBV for the configuration used).
- **Action:** request a GeometryFactory quotation covering the packages above for on-premises redistribution; in parallel benchmark permissively licensed alternatives so the decision is not forced.

## 4. Other geometry candidates

| Candidate | Licence | Status |
|---|---|---|
| Parasolid, ACIS, CGM | Proprietary; terms and pricing not public | Quotation and redistribution terms needed ⚖ |
| `truck`, Fornjot (Rust B-rep) | TBV | Read each repository's LICENSE at evaluation |
| Clipper2, Boost.Geometry | TBV (believed permissive) | Verify |
| Rust `geo`, `i_overlay`, `robust` | TBV | Verify |
| GMP / MPFR; Rust `rug` | LGPL (TBV exact versions) | Same LGPL considerations as §2 |
| `opencascade-rs` bindings | Not stated in README (TBV); bundles and builds OCCT from source by default, so OCCT terms apply to the result | README, github.com/bschwind/opencascade-rs |

## 5. PDF processing

| Library | Licence | Notes | Source |
|---|---|---|---|
| PDFium | BSD-3-Clause-style and Apache-2.0 texts in LICENSE | Bundled third-party components have their own licences (TBV) | pdfium.googlesource.com LICENSE |
| MuPDF / PyMuPDF | AGPL or commercial licence from Artifex | AGPL obligations extend to network use; presumed unsuitable without a commercial licence ⚖ | pymupdf.readthedocs.io (about); artifex.com |
| Poppler | GPL (TBV) | Presumed unsuitable for linking ⚖ | — |
| pdf.js, Rust `lopdf` / `pdf` | TBV | Evaluate capability first | — |

PDF files may embed fonts with their own licences; we extract and render for interpretation but should not redistribute embedded fonts in outputs (TBV ⚖).

## 6. DWG / DXF

| Technology | Licence | Notes | Source |
|---|---|---|---|
| DXF format | Published by Autodesk as documentation | We write our own DXF writer; rights in the documentation and any trademark usage ("DXF", "AutoCAD") ⚖ | Autodesk DXF reference |
| ezdxf (Python) | MIT | Candidate *independent* DXF reader for Z3 validation | github.com/mozman/ezdxf LICENSE |
| ODA Drawings SDK | Proprietary, membership-based; terms not on the product page (60-day trial offered) | Reads/writes DWG AC1009–AC1032 per vendor | opendesign.com/products/drawings |
| LibreDWG | GPL v3 or later | Reader/writer; GPL makes linking presumed unsuitable; separate-process use ⚖ | LibreDWG README |
| DWG format / "DWG" name | Proprietary format; Autodesk trademark claims (TBV) | ⚖ |  |

**Open decision:** DWG path — (a) ODA membership, (b) DXF-only at first with DWG converted by the customer, (c) GPL tool as an isolated optional component subject to counsel. Recommendation pending cost and legal input: (b) for the first pilot unless the factory's inputs are predominantly DWG (**factory information required**).

## 7. OCR and automated-reading models

| Component | Licence | Notes | Source |
|---|---|---|---|
| Tesseract | Apache-2.0 | Trained data files licensed separately (TBV) | github.com/tesseract-ocr/tesseract LICENSE |
| PaddleOCR | Apache-2.0 (secondary sources; TBV from repository) | Model weights and training-data terms must be checked individually | — |
| Other vision / layout / VLM models | Varies widely (permissive, research-only, RAIL-type use restrictions) | **Code licence ≠ weights licence ≠ training-data terms** | — |
| External reading APIs | Contract terms | Data retention, training use, region of processing; must satisfy customer automated-reading policy | — |

**Rules proposed**

- Record for each model: code licence, weights licence, dataset provenance, permitted commercial use, redistribution right (needed for on-premises).
- No research-only or non-commercial weights in any shipped build.
- Fine-tuning on customer drawings only with explicit recorded permission; resulting weights are tenant-scoped unless agreed otherwise ⚖.

## 8. Rust crates and general third-party dependencies

Nothing is adopted yet. Process to put in place before the first line of product code:

- Automated licence gate in CI (e.g. `cargo-deny` or equivalent) with an allow-list (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode — to be confirmed by counsel); MPL-2.0 and LGPL require case review; GPL/AGPL/SSPL and unlicensed code are blocked by default.
- Same gate for Python (Z1) and JavaScript (clients) dependency trees, including transitive dependencies.
- Generated third-party notices file and SBOM per release.
- Review of `-sys` crates: they wrap native libraries whose licences differ from the crate's.
- Statically linked Rust plus LGPL native code needs an explicit compliance approach ⚖.
- Fonts, icons, CJK font files and map/symbol assets are dependencies too.

## 9. Distribution implications by deployment mode

| Mode | Distribution? | Principal questions ⚖ |
|---|---|---|
| Managed private cloud | Generally no binary distribution | AGPL still applies to network use; contractual terms of commercial components |
| On-premises server | Yes | LGPL duties (notices, library source, replaceability); GPL packages need commercial licences or removal; model-weight redistribution rights |
| Desktop application | Yes | As above; installer must carry notices; keep native geometry in backend where possible |
| Mobile / tablet apps | Yes, via app stores | LGPL/GPL compatibility with store terms and static linking; current design ships no geometry libraries in clients |

## 10. Items requiring legal review (summary)

1. OCCT: LGPL-2.1 + exception compliance design for on-premises distribution; scope of the exception for our shim.
2. CGAL: any GPL package; commercial licence scope and redistribution rights; LGPL v3+ terms.
3. Whether process separation changes the copyleft analysis for GPL/AGPL components.
4. DWG: technology choice, format and trademark issues.
5. PDF library choice (AGPL/GPL exposure); embedded fonts.
6. Reading-model weights and datasets; external reading-service contracts; customer-data training permissions.
7. Allow-list of licences for automated gating; treatment of MPL/LGPL crates.
8. Export-control classification and Japanese-law considerations for customer drawing data.
9. Patent landscape and freedom to operate (Document 09).

## 11. Not yet investigated

Commercial kernel terms; licences marked TBV; OCCT bundled third-party components; desktop shell and UI framework licences; database/object-store/queue licences (including source-available licences that restrict hosted offerings); CJK font licences; trained-data licences for OCR.

## Sources

- https://occt3d.com/open-cascade-technology/index.html
- https://github.com/Open-Cascade-SAS/OCCT (OCCT_LGPL_EXCEPTION.txt, LICENSE_LGPL_21.txt)
- https://www.cgal.org/license.html
- https://doc.cgal.org/latest/Manual/packages.html (CGAL 6.2.1)
- https://github.com/bschwind/opencascade-rs
- https://pdfium.googlesource.com/pdfium/+/refs/heads/main/LICENSE
- https://pymupdf.readthedocs.io/en/latest/about.html · https://artifex.com/blog/pymupdf-acquired-by-artifex
- https://github.com/LibreDWG/libredwg
- https://github.com/mozman/ezdxf
- https://www.opendesign.com/products/drawings
- https://github.com/tesseract-ocr/tesseract
