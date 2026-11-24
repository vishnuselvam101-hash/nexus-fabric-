# 03 — Drawing Intelligence and Ingestion Specification

Version 0.1 (draft for review) · 18 November 2026
Parents: [Master Plan](../MASTER_PLAN.md) · [Spec 01](01_SYSTEM_ARCHITECTURE_SPECIFICATION.md) · [Spec 02](02_MATHEMATICAL_ENGINEERING_SPECIFICATION.md) (provisional) · [ADR-005](../architecture/ADR-005-drawing-processing-architecture.md)
Companions: [Observation Schema](../architecture/drawing-intelligence/OBSERVATION_SCHEMA.md) · [Japanese Drawing Vocabulary](../architecture/drawing-intelligence/JAPANESE_DRAWING_VOCABULARY.md) · [Test Plan](../benchmark/DRAWING_INTELLIGENCE_TEST_PLAN.md) · [Research Register](../research/DRAWING_INTELLIGENCE_RESEARCH_REGISTER.md)

**Status.** A specification for review; nothing here is a measured capability and no dependency is selected. No factory drawings have been seen: every statement about what real drawings contain is an expectation to be tested against authorised samples (`FR-03-1`). References to Japanese and international drawing standards are from the author's knowledge and are **unverified** (research register). Requirement IDs `DI-nn`.

---

## 0. Governing principles

1. **Observations, not geometry.** This subsystem lives in trust zones Z0–Z1. Its only product is a set of typed observation records (ARCH-01). It never writes parameters, geometry or DXF coordinates.
2. **Two questions kept apart for every piece of information:**
   - *What does the drawing state?* (its **authority**: an explicit written dimension is authoritative; a line's drawn length is only evidence.)
   - *How reliably did we read it?* (its **acquisition**: read from vector text, by OCR, measured from pixels, typed by a person.)

   A written dimension read by OCR is authoritative information with an uncertain *reading*. It does not become usable as authoritative until the reading is validated (§3.6). This is the central design decision of this specification.
3. **No invention.** A dimension that is not on the drawing is reported as missing (DI-40). Standard-table lookups and manufacturing assumptions are separate, labelled classes.
4. **No silent resolution.** Conflicts become conflict records with the affected observations (DI-50).
5. **Confidence is not evidence.** A recogniser's confidence can raise the amount of review; it can never lower the evidence required, and it is not an input to any release gate (MATH-50). No interpretation, validated or not, can by itself trigger release.
6. **Source is never modified.** The original file is immutable; every derived coordinate is traceable to source coordinates through a recorded transform chain (DI-20).

---

## 1. Input format architecture

### 1.1 Common ingestion stage (all formats, zone Z0)

| Step | Requirement |
|---|---|
| Receive | Store bytes unmodified; SHA-256; record uploader, time, tenant, customer automated-reading policy in force (→ `SourceArtifact`) |
| Identify | Type by content signature and structural probe, not by extension or declared MIME type. Mismatch between extension and content is recorded |
| Limits | Per-tenant configurable limits: file size, page count, pixel count per image, object/entity count, nesting depth, decompressed size and ratio, processing time and memory per step. Limits are set from benchmarks, not assumed |
| Sandbox | All parsing in a separate process with no network, no filesystem beyond its inputs/outputs, CPU/memory/time caps, and a fresh process per file (Spec 01 §7, §8) |
| Outcome | Exactly one of: `accepted`, `accepted_with_unsupported_content` (itemised), `rejected{reason}`. Never a partial result presented as complete (DI-01) |

**Problem files.**

| Condition | Handling |
|---|---|
| Corrupted / truncated | Parser repair is permitted only if the repair is recorded item by item; a repaired file is `accepted_with_unsupported_content` and forces review. If structure cannot be established → `rejected{corrupt}` |
| Encrypted, password required | `rejected{encrypted}`; the user may supply the password, which is used in memory and not stored |
| Encrypted with usage restrictions only (opens without password) | Not bypassed automatically. Flagged for a policy/legal decision (research register L-03); until decided → `rejected{restricted}` with an explanation |
| Malformed but parseable | Each deviation recorded; conservative interpretation; review flag |
| Unusually large | Limits above; large-but-valid files go to a high-resource queue, not silently truncated. Truncation of any kind is forbidden (DI-02) |
| Potentially malicious | Active content (JavaScript, launch actions, embedded files, external references, macros) is never executed or followed; its presence is recorded. Embedded files are not extracted automatically. Polyglot or signature-conflicting files → `rejected{suspicious}` |
| Password-protected archives, unknown formats | `rejected{unsupported_format}` |

### 1.2 Per-format requirements

For each format: **V**alidation · **P**arsing · **C**oordinates & units · **L**ayers/entities · **M**etadata · **E**rror detection · **U**nsupported content · **S**ecurity · **Pr**ovenance.

#### Vector PDF (page content is drawing operators)

- **V** Cross-reference table and object structure parse; every page classified (§1.3).
- **P** Interpret each page's content stream fully: path construction and painting operators, text operators, graphics state (current transformation matrix, line width, dash pattern, colour, clipping), form XObjects (recursively, with their matrices), inline and XObject images, optional content groups (layers), and annotations (kept **separate** from page content — a reviewer's markup is not part of the drawing).
- **C** PDF user space is 1/72 inch per unit unless `UserUnit` says otherwise; apply page `Rotate`, crop/media boxes and the full CTM stack. Result: sheet coordinates in mm, origin at the lower-left of the unrotated media box, y up, with the exact transform recorded. **Sheet coordinates are not part coordinates**: the drawing scale is a separate observation per view (§5.4), and paper size from the PDF is evidence about the sheet, not about the part.
- **L** Preserve: path identity and order, stroke width, dash pattern, colour, fill/stroke, layer membership, clipping visibility. Curves kept as Béziers; circles and arcs are *recognised* from Bézier groups by fitting with a recorded residual (they are not native PDF objects) — the residual is part of the observation.
- **M** Producer/creator strings (the originating CAD system is useful evidence), dates, page labels, embedded fonts and their encodings.
- **E** Text that cannot be mapped to Unicode (missing/incorrect `ToUnicode`, custom-encoded CID fonts — expected with Japanese fonts); text drawn as outline paths or stroked glyphs (common in CAD plots; no text objects exist); invisible text layers from a previous OCR (rendering mode 3) — treated as *third-party OCR output*, acquisition class OCR, not as vector text; white-on-white or clipped-away content; overlapping duplicate paths.
- **U** Shadings/patterns, transparency groups, 3D annotations (U3D/PRC), unusual blend modes: recorded; they do not affect geometry extraction unless they hide content, which is detected by render comparison.
- **S** No script execution, no external fetch, no embedded-file extraction; font programs parsed only by the sandboxed parser.
- **Pr** Each observation references page index, object/operator index within the content stream (stable path identifier), and sheet-coordinate region.
- **Cross-check (DI-03).** Each vector page is also rendered to raster by an independent renderer; extracted text and geometry are compared with the rendering (text presence, coverage of ink by extracted paths). Disagreement beyond a threshold marks the page `extraction_unreliable` and adds the raster pipeline as a second reading.

#### Scanned PDF (page content is one or more images)

- **V/P** Extract embedded images at native resolution and bit depth with their placement matrix; do **not** re-render the page at an arbitrary resolution (resampling destroys information). Multiple image tiles/strips per page are reassembled with recorded placement.
- **C** Pixel coordinates (column, row; origin top-left) of the source image are the primary coordinates. Declared resolution (image size vs placement) is recorded as *declared*, not trusted (§2.2).
- **L/M** Compression type (JBIG2 and lossy JPEG are recorded because they can substitute or blur glyphs — JBIG2 pattern substitution is a known source of silently wrong digits in scans; such pages carry a mandatory digit-reading risk flag, DI-04), colour space, scanner/producer metadata.
- **E/U/S/Pr** As raster images below; hidden OCR text layers handled as above.

#### Raster images (PNG, JPEG, TIFF)

- **V** Decoder-level validation in the sandbox; pixel-count and decompression limits; TIFF: multi-page, tiles/strips, unusual compressions and photometric interpretations enumerated; unsupported variants rejected explicitly.
- **C** Pixel coordinates; declared DPI (often absent or wrong; fax-origin images may have non-square pixels) recorded as declared; EXIF orientation applied as a recorded transform; photographs (perspective, lens distortion) are identified as such and carry a higher uncertainty class (§2).
- **M** EXIF/TIFF tags, colour profile, capture device.
- **Pr** Source pixel region for every observation.

#### DXF

- **V** Section structure, group-code/value pairing, declared version (`$ACADVER`), handle uniqueness, table/entity references resolvable.
- **P** Full tag-level parse; binary DXF supported or rejected explicitly. Text encoding from `$DWGCODEPAGE` (legacy Japanese files are expected in Shift-JIS/CP932) with detection of mis-decoding; `\U+`/`%%` control codes (e.g. `%%c` diameter) normalised with the original retained.
- **C** `$INSUNITS` and `$MEASUREMENT` recorded; **`$INSUNITS = 0` (unitless) is common and is never defaulted** — units become an explicit unresolved observation resolved by written evidence or a human (DI-05). Model space vs paper space; viewports with their scales; block inserts with nested transforms (non-uniform scale, mirroring recorded); UCS/OCS (extrusion direction) conversion to WCS.
- **L** Preserve layers, linetypes, colours, lineweights, blocks, groups, entity handles, draw order. Entities: LINE, CIRCLE, ARC, ELLIPSE, LWPOLYLINE/POLYLINE (bulges), SPLINE, TEXT/MTEXT, DIMENSION, LEADER/MLEADER, HATCH, INSERT, POINT, SOLID; others enumerated as unsupported per file.
- **DIMENSION entities (DI-06).** A dimension entity carries definition points, a *measured* value and optional *override text*. Both are extracted. If the displayed text differs from the measured value beyond rounding, a `dimension_override_conflict` is raised — an overridden ("typed-over") dimension is a classic source of drawings whose geometry and stated dimensions disagree. The displayed text is the written dimension; the measured value is geometry evidence.
- **E** Open polylines that appear closed, zero-length entities, duplicates, entities at extreme coordinates, missing blocks, unresolved external references (XREF) — reported, never ignored.
- **U** ACIS-based entities (3DSOLID, REGION, BODY), proxy/custom objects, OLE, images, fields: reported as unsupported content with their extents.
- **S** No external reference following; no embedded object execution.
- **Pr** File, space (model/paper/block path), entity handle.

#### DWG (conditional)

Subject to the unresolved licensing/technology decision ([licensing review](../research/DEPENDENCY_LICENSING_REVIEW.md) §6). Requirements are identical to DXF once entities are available. Until decided: `rejected{unsupported_format}` with guidance to supply DXF or PDF. The pipeline is defined against a neutral entity model so that a DWG reader, whichever is licensed, plugs in without change (DI-07).

#### STEP (ISO 10303-21 exchange files)

- **V** Header and schema identification (application protocol), entity reference integrity.
- **P** Through the kernel port in an isolated worker (ADR-002/003); geometry and topology, assembly structure, names, colours/layers, units, and product manufacturing information (PMI) where present.
- **C** Units from the file's unit contexts (must be present; absent → unresolved, never assumed). Model coordinates in file units; declared geometric uncertainty recorded.
- **Authority (DI-08).** A model contains geometry but usually no written dimensions. Whether the model or a drawing is the *master definition* is a job-level declaration by the customer/engineer (a Decision), never inferred. Without it, model geometry is evidence (acquisition class *vector/model-extracted*), and discrepancies with drawings are conflicts.
- **U** Unsupported entity types, invalid B-rep (reported by a validity check; healing is a recorded, reviewable operation — never silent).
- **Pr** File, entity instance id (`#n`), product/shape path.

#### Other formats (candidates, justified by market — to be confirmed with the factory)

| Format | Why considered | Status |
|---|---|---|
| IGES | Legacy exchange still in circulation | Via kernel port if adopted; low priority |
| JWW / JWC (Jw_cad) | Widely used free 2D CAD in Japan; plausible in small-manufacturer supply chains | Candidate; depends on `FR-03-2`; reader technology and licence to research (T-14) |
| SXF (SFC / P21) | Japanese public-works CAD exchange format | Candidate only if the factory receives it |
| Office documents, e-mail attachments with pasted drawings | Occur in practice | Out of scope: the contained image/PDF must be supplied directly |

### 1.3 PDF page classification (vector vs scanned — never assumed)

Each page is classified independently from its content: **vector** (drawing operators carry the linework), **raster** (one or more images cover the page; no meaningful vector linework), **hybrid** (image plus vector overlay — e.g. scanned drawing with CAD title block or markups), or **raster with hidden text**. Signals: ink coverage by vector paths vs by images, path count and structure, image area fraction, text rendering modes. Classification with its signals is recorded; uncertain pages are processed by **both** pipelines and reconciled. A multi-page PDF may mix types (DI-09).

---

## 2. Drawing pre-processing (raster and scanned inputs)

### 2.1 Transform chain (traceability)

Every operation that changes pixel positions is an explicit, invertible map with stored parameters:

`source pixels → [T1: EXIF/rotation] → [T2: deskew] → [T3: perspective] → [T4: crop/segment] → working image`

- Each $T_i$ is recorded as type + parameters (rotation by multiple of 90°; affine; homography; regular-grid warp) with the estimation residual and uncertainty.
- Operations that change only pixel values (denoising, contrast, binarisation, background removal) are recorded with parameters but have identity geometry.
- Every observation stores its location in **source pixel coordinates**, obtained by applying the inverse chain, together with the chain's identifier and the positional uncertainty contributed by each step (DI-20).
- The original image is retained unmodified; working images are caches that can be regenerated (ARCH-04).

### 2.2 Steps

| Step | Method requirements | Dimension-safety rule |
|---|---|---|
| Orientation | Coarse (0/90/180/270) from text-line and title-block evidence; ambiguous cases (drawings with text in two directions are normal) flagged | Exact transform, no resampling loss beyond a 90° rotation |
| Deskew | Angle from dominant line directions (border, long lines) with uncertainty | Rigid rotation only; angle and uncertainty recorded |
| Perspective correction | Only when a perspective source (photograph) is identified and a reference rectangle (drawing border) is detected with adequate residual | Homography recorded; positional uncertainty grows away from the reference points; photographs are a lower-trust measurement class |
| Non-rigid dewarping (folds, curved pages) | Available only as an explicit, flagged option | **Disabled for measurement by default**: local warps change measured geometry in ways that are hard to bound. If used, all raster measurements on that page are downgraded to "indicative" (DI-21) |
| Resolution assessment | Effective resolution measured from content: stroke width in pixels, character height in pixels, sharpness; compared with thresholds obtained from the benchmark (none assumed) | Below threshold → page flagged `low_resolution`; OCR numeric readings from it require human confirmation regardless of confidence |
| Scale/aspect check | Declared DPI is not trusted. Pixel-to-sheet scale and **anisotropy** (different x/y scale from copying, faxing, scanning) are estimated from known references: border/frame dimensions, written dimensions paired with measured features in both directions | Anisotropy is reported; raster measurements carry the resulting uncertainty |
| Noise reduction, contrast, binarisation, background removal | Edge-preserving methods; adaptive thresholds; parameters recorded | Thin-line and small-glyph preservation is a tested property (decimal points, minus signs, dimension-line arrowheads, "1" vs "7"); recognisers also receive the **unfiltered** image so that filtering cannot be a single point of failure (DI-22) |
| Page segmentation | Border/frame, title block, revision table, notes area, parts list, drawing field | Regions are observations with boundaries; nothing is discarded |
| View-region detection | Candidate view regions within the drawing field (§5) | — |

- **DI-23** Pre-processing never produces a "corrected drawing" that replaces the source for downstream stages, and never rescales content to match a written dimension.
- **DI-24** Stamps, handwriting, highlighter and revision marks overlapping linework are segmented as separate layers where possible and always reported; handwritten changes to dimensions are high-priority review items, not noise.

---

## 3. Text and dimension intelligence

### 3.1 Sources of text, in decreasing reliability of *reading*

1. CAD entities (DXF TEXT/MTEXT/DIMENSION) — exact strings.
2. PDF text objects with valid Unicode mapping — exact strings, subject to the render cross-check (DI-03).
3. Vector glyph outlines / stroked CAD fonts in PDF — no string exists; the region is rendered at high resolution and recognised (acquisition: OCR, but on clean synthetic rendering).
4. Raster text — OCR.
5. Handwriting — OCR/HTR with mandatory human confirmation.

### 3.2 Pipeline

`text-region detection → script/orientation classification → recognition (n-best) → normalisation → tokenisation → domain parsing → association (§5.6) → validation (§3.6)`

- **Detection** handles horizontal and vertical Japanese text, rotated dimension text (aligned with dimension lines), text inside symbols/balloons, and small text (tolerances, superscripts).
- **Recognition** returns, per token, an **n-best list** of alternatives with scores and per-character alternatives where the engine provides them. For numeric tokens the alternative list is mandatory: it is the representation of *discrete misread risk* required by Spec 02 (MATH-31).
- **Normalisation** (recorded; original string retained): full-width → half-width alphanumerics (`１２３`→`123`, `φ`/`Φ`/`⌀`/`%%c` → DIAMETER), unified minus/dash/long-vowel disambiguation (`-`, `−`, `ー`, `－`), decimal separator, degree/minute/second marks, multiplication signs (`×`, `x`, `X`). Normalisation rules are deterministic and versioned; ambiguous cases (is `ー` a minus sign?) produce alternatives, not a choice.
- **Domain parsing** by a deterministic grammar (not a language model) into typed tokens. A language/vision model may *propose* a parse for free-form notes, which is then an inferred candidate.

### 3.3 Content types and their parsed forms

| Content | Parsed form (see [schema](../architecture/drawing-intelligence/OBSERVATION_SCHEMA.md)) | Notes |
|---|---|---|
| Linear dimension | value (exact decimal), unit (explicit or sheet default), qualifiers | Decimal kept as written: `1250.5` → 12505/10 (MATH-03); trailing zeros retained as written precision |
| Diameter / radius / spherical | value + kind (`DIAMETER`, `RADIUS`, `SPHERICAL_DIAMETER`, `SPHERICAL_RADIUS`) | Symbol before the value; in Japanese practice also `R`, `SR`, `Sφ` |
| Square, chamfer, thickness prefixes | `□` (square), `C` (45° chamfer), `t` (thickness) | Prefix letters are context-dependent: `C5` may be a chamfer or a part mark — ambiguity retained |
| Angle | value in degrees/minutes/seconds as written, exact rational | Converted to radians only inside the UEM |
| Tolerance | symmetric `±a`; deviations `+a/−b`; limits (max/min); fit class (e.g. `H7`); general-tolerance class from a note | Fit classes and general-tolerance classes are **references to standard tables**, not numbers — lookup requires verified table data (research register) |
| Count/pattern | `4×φ12`, `12-φ18` → count + feature | Hyphen-as-count notation is common in Japanese drawings and collides with minus/range notation: alternatives retained |
| Reference (auxiliary) dimension | value in parentheses → flag `reference` | **Not authoritative**: used only as a consistency check, never as a hard constraint (DI-30) |
| Not-to-scale dimension | underlined value or note → flag `not_to_scale` | Suppresses written-vs-measured discrepancy for that feature |
| Theoretically exact dimension | framed value → flag `theoretically_exact` | |
| Nominal size designations | e.g. pipe nominal sizes (`100A`, `4B`), section designations, thread designations | A **designation, not a dimension**. Actual dimensions come from a standard table: provenance class *standard-derived* (§6.2), requiring verified table data and the applicable standard edition (DI-31) |
| Material | grade string + standard reference where given | Vocabulary-assisted; unknown grades reported, not corrected |
| Thickness | `t9`, `t=9`, `PL-9`, `板厚 9` → thickness value | Often in notes or parts list rather than on the view |
| Surface finish symbols | symbol class + parameters | Carried as annotations; no effect on development |
| Welding symbols | arrow side/other side, weld type, size, length/pitch, all-around, field weld, tail notes | Structured where recognised, else image region + `unparsed` status. Affects joint geometry in Spec 02 §6.5 — unparsed welding information on a joint used for a cut curve forces review |
| Revision identifiers | revision mark, table row (mark, date, description, author) | Linked to the regions/dimensions they mark (§8) |
| Title block | drawing number, title/part name, scale, projection method, units, material, quantity, sheet n of m, dates, drafter/checker/approver, company | Field layout varies per company: per-customer title-block templates are configuration; unknown layouts are parsed generically with review |
| Manufacturing / general notes | text + parsed directives where a rule matches (general tolerance, unspecified radii/chamfers, weld notes, finish, units) | Notes that set defaults (units, general tolerance, "unspecified corner R…") are sheet-level observations that qualify many dimensions |

### 3.4 Japanese support strategy

- Japanese and English are handled as first-class and **mixed within one string** (e.g. `φ216.3×t8.2 SGP 全周溶接`).
- A versioned [technical vocabulary](../architecture/drawing-intelligence/JAPANESE_DRAWING_VOCABULARY.md) maps terms to semantic tokens (inside/outside diameter, thickness, development, both sides, all-around weld, reference dimension, typical, etc.). It is a seed list for review by a native-speaking fabrication engineer; per-customer extensions are ProcessData-like configuration with provenance.
- Vocabulary is used to *interpret* recognised text and to rank n-best alternatives; it must not "auto-correct" a recognised string into a vocabulary term without recording the original and the substitution as an inferred candidate (DI-32).
- Character-level risks to be tested explicitly: full/half-width forms, visually confusable characters (`0/O/○`, `1/l/I/｜`, `5/S`, `8/B/3`, `ー/−/一`, `ロ/□/口`, `ニ/二`, `カ/力`, `エ/工`), vertical text, small furigana-size annotations, handwriting, seals/stamps over text.
- Standards (drawing practice, general tolerances, welding symbols, pipe sizes) are **not yet identified or verified**; nothing in this specification claims conformance.

### 3.5 OCR approach for local execution (architecture, not selection)

Requirements: runs fully on-premises without network; redistributable weights; deterministic inference (fixed weights, pinned runtime, no sampling); per-token alternatives; acceptable on CPU with optional GPU.

Proposed architecture (ADR-005, option 3):

1. **Two or more independent recognisers** for numeric/symbol tokens (different model families or engines). Agreement is recorded; disagreement automatically creates alternatives and a review item. Agreement between engines reduces but does not remove misread risk (common failure on degraded glyphs) and is **not** a validation by itself.
2. A **specialised dimension-token recogniser** (digits, decimal point, ±, φ, R, °, ×, parentheses, t, C) — small, trainable on synthetic renderings; candidate for in-house training on tier-S data.
3. A general Japanese/English recogniser for notes and title blocks.
4. Optional layout/vision model to propose regions, view labels and note interpretations (inferred candidates only).
5. External reading/OCR services only where the customer automated-reading policy allows; never required.

Candidate technologies, licences and limitations: [research register](../research/DRAWING_INTELLIGENCE_RESEARCH_REGISTER.md). No engine is chosen until benchmarked on authorised Japanese drawings.

### 3.6 Reading validation (how an OCR-read value becomes usable)

An OCR-read written dimension has `reading_state`:

`unvalidated → corroborated → confirmed` or `→ corrected` / `→ rejected`

| State | Meaning | Reached by |
|---|---|---|
| `unvalidated` | Recognised; alternatives present | Recognition |
| `corroborated` | Deterministic corroboration rules passed, e.g.: all recognisers agree **and** the value is consistent with the measured feature at the view's established scale within the measurement uncertainty **and** no alternative reading is also consistent; or a dimension chain closes exactly (sum of parts = written overall) | Deterministic rules, recorded with the evidence used |
| `confirmed` | A person confirmed the reading against the source image | Human action (audited) |
| `corrected` | A person replaced the reading (original retained) | Human action (audited) — acquisition class becomes *human-entered* |
| `rejected` | Not a dimension / not applicable | Human action |

- **DI-33** Only `confirmed` or `corrected` readings may be supplied to the UEM as class-H (authoritative) constraints **by default**. Whether `corroborated` suffices for specific, well-tested situations is a policy decision reserved for Specification 05 with benchmark evidence (false-accept rate). Until then: human confirmation of every class-H value used in release-critical geometry.
- **DI-34** Recogniser confidence appears in no corroboration rule as a sufficient condition. It may only order the review queue and add review items.
- Text from sources 1–2 (§3.1) starts as `corroborated` if the render cross-check passes, since the string is exact; it is still subject to reconciliation (§7).

---

## 4. Geometric feature extraction

### 4.1 Vector sources versus raster sources

| Aspect | Vector (PDF paths, DXF/DWG entities) | Raster |
|---|---|---|
| Geometry | Read exactly as stored; no detection needed | Detected: skeleton/edge extraction, line and arc fitting, vectorisation |
| Position uncertainty | File coordinate precision (plus fit residual where a primitive is recognised from Béziers/polylines) | Pixel quantisation, stroke-width, binarisation and transform-chain uncertainty; systematic scale/anisotropy terms |
| Primitive type | Native for CAD; recognised for PDF (circle from Béziers, polyline approximations of arcs from CAD plots) with residual | Fitted (orthogonal-distance regression, Spec 02 §2.4) with covariance |
| Line style | Stored width, dash pattern, colour, layer | Inferred from stroke width and gap pattern — less reliable |
| Typical faults | Duplicates, tiny gaps, exploded blocks, arcs plotted as many short segments, overlapping entities, hidden layers | Broken lines, merged close lines, text touching linework, smudges read as geometry |
| Provenance class (§6.2) | Vector-extracted geometry | Raster-measured geometry |

Vector geometry is exact **as a description of the drawing sheet**. It is still not authoritative for the part: drawings are frequently not to scale, edited by text override, or schematic. Both classes are *evidence* (Spec 02 class S); only their uncertainty differs (DI-41).

### 4.2 Features to detect and classify

Entities: lines, circles, arcs, ellipses (including circles seen obliquely), splines/free curves, polylines.
Roles (each a classification with alternatives):

| Role | Principal cues (to be validated against real drawings and the applicable standard) |
|---|---|
| Visible outline (part geometry) | Thick continuous lines |
| Hidden line | Dashed lines of medium/thin width |
| Centre line / axis / pitch circle | Thin chain (long-dash–dot) lines; crossing circle centres; symmetry |
| Dimension line | Thin continuous, terminated by arrowheads/ticks/dots, with associated text |
| Extension (projection) line | Thin continuous from a feature toward a dimension line, small gap or overrun |
| Leader | Thin line from text/balloon to a feature, with arrow or dot |
| Section boundary / cutting plane | Chain line with thick ends, arrows and letters |
| Hatching | Regular families of parallel thin lines bounded by outlines |
| Break lines, phantom/imaginary lines | Freehand/zigzag thin lines; chain double-dot lines |
| Symbols | Welding, surface finish, datum, balloons, north/flow arrows, projection symbol |
| Border, frame, zone marks, title block, tables | Sheet furniture |

- **DI-42** Role classification is an *inferred candidate* with alternatives. Geometry whose role is uncertain between "part geometry" and "annotation/reference" is kept in both candidate sets and flagged; it is never dropped and never silently included in part geometry.
- **DI-43** Line-style conventions (which width/dash pattern means what) are taken from layer names and line types where the source provides them (CAD), otherwise from a per-sheet clustering of widths and dash patterns mapped to roles by rule; the mapping is recorded so a reviewer can correct it once per sheet rather than per entity.
- Hidden lines and centre lines carry real geometric information (hidden edges, axes) and are passed on with their roles; hatching identifies cut material in sections; break lines mean the view is shortened and that **measured lengths across the break are meaningless** (flag propagated to reconciliation, DI-44).
- Arrowheads, ticks and terminators are detected as dimension-structure evidence, with their own positional observations.

### 4.3 Clean-up operations (all recorded, none silent)

Merging collinear fragments, closing gaps below a stated threshold, removing exact duplicates, recognising arcs from segment chains. Each produces a derived observation linked to its sources; the originals remain. Thresholds are sheet-relative, recorded, and produce review flags when a merge is not unambiguous (two candidate closures).

---

## 5. Drawing view intelligence

### 5.1 View detection and typing

View regions are found by clustering part-geometry candidates and their annotations; each region receives type candidates: front / top / bottom / left / right / rear (principal orthographic), isometric or other pictorial, section (with cutting-plane reference), detail (with source-region reference and its own scale), auxiliary (projected along an inclined direction), partial / broken / half views, development (flat pattern drawn by the designer — highly valuable as redundant evidence), schematic/not-to-scale sketches, and non-view regions (tables, notes).

"Front" is a designer's naming choice, not a geometric fact: the system assigns *relative* roles (view B is the projection of view A from the right) and takes names only from labels on the sheet.

### 5.2 Orthographic relationships

Two principal views related by projection are aligned on the sheet: features share coordinates along the common axis (within drawing accuracy). The relation between view placement and viewing direction depends on the projection method:

- **Third-angle:** the view seen from the right is placed on the right; from above, above.
- **First-angle:** the view seen from the right is placed on the left; from above, below.

### 5.3 Projection method identification (never assumed)

Evidence, each recorded as an observation:

1. Projection symbol in the title block (truncated-cone symbol: arrangement of its two views).
2. Explicit text (e.g. third-angle / first-angle statements in either language).
3. Geometric discrimination: for parts with asymmetric features, only one method makes the views mutually consistent (tested deterministically by the correspondence/constraint check below).
4. Customer/site default — a configured prior with provenance (`FR-03-4`), used only to order hypotheses, **not** to decide.

Outcomes:

- Evidence agrees → method established, with the evidence listed.
- Evidence conflicts (symbol says one, geometry fits the other) → conflict record, REVIEW REQUIRED.
- No discriminating evidence: if both methods yield the same part up to a proper rigid motion (e.g. a symmetric part), the ambiguity is recorded as **immaterial**; if they yield mirror-image parts, `Ambiguous{projection_method}` → REVIEW REQUIRED (DI-51). Mirror-image fabrication is a real and costly error; this check is mandatory for every multi-view job.

### 5.4 Scale detection (per view)

Evidence: title-block scale; per-view scale labels (detail and section views often differ); "NTS"/not-to-scale notes; ratio of written dimensions to measured feature lengths within the view (robust estimate across many dimensions, Spec 02 §2.4, with outliers reported). The scale of each view is an observation with uncertainty; a view whose dimension/measurement ratios are inconsistent with any single scale is flagged **not to scale** and its measured geometry is excluded from quantitative use (it remains usable for topology). Different x/y ratios indicate scan anisotropy (§2.2) or a deliberately distorted view.

### 5.5 Candidate interpretations

An interpretation hypothesis is a structured record: `{projection method, view roles and relative orientations, scale per view, feature correspondences across views, dimension associations}`. Hypotheses are generated by rules and (optionally) proposed by a model; they are **evaluated deterministically**:

1. Hard consistency: alignment of corresponding features; every written dimension associable; no geometric contradiction when the hypothesis' constraints are passed to the UEM (ALG-01: `Contradictory` eliminates a hypothesis).
2. Explanatory coverage: fraction of part-geometry candidates and dimensions explained.
3. Outcome: zero surviving hypotheses → `Uninterpretable` (review); one → proceed; several that produce **different geometry** → `Ambiguous` with the discriminating differences listed (Spec 02 MATH-12); several that produce the same geometry → recorded as equivalent.

A model's preference among surviving hypotheses is shown to the reviewer as a suggestion and is never used to discard a surviving alternative (DI-52). Hypothesis enumeration may be incomplete; this limitation is stated in the output (search bounds reached, yes/no).

### 5.6 Dimension association

Each dimension is associated with the geometry it measures through its structure: text ↔ dimension line ↔ terminators ↔ extension lines ↔ feature points (or leader ↔ feature for diameters, radii, notes). The association is an inferred relationship with alternatives. Rules:

- The dimension's *kind* (linear along a direction, aligned, angular, diameter, radius, arc length, ordinate, chain/baseline) is derived from structure, with the text prefix as corroboration; disagreement (e.g. `R` text on a linear structure) is a conflict.
- A diameter dimension on a non-circular view (a cylinder seen from the side) is associated with the pair of silhouette lines and with the circle in the adjacent view via cross-view correspondence.
- **Inside / outside / mean** reference is an attribute that is `unknown` unless stated by text, by the position of extension lines on a wall drawn with thickness, or by a declared customer convention (`FR-02-2`); `unknown` on a thickness-relevant dimension forces review (DI-53) — it changes developed lengths by multiples of the wall thickness.
- A dimension with no resolvable target is `unassociated` and reported; it is not attached to the nearest geometry by default.
- Association errors are expected to be the dominant silent failure (ADR-005). Therefore every association used by a class-H constraint is displayed for confirmation with the reading (same review action), and is checked by measurement consistency where the view is to scale.

### 5.7 Section and detail references

Cutting-plane lines and letters in a parent view ↔ section view title (`A-A`, `断面 A-A`); detail circles/letters ↔ detail view titles and scales (`A部詳細`, `DETAIL A`). Links are established only by explicit matching labels on the same sheet set; unmatched references are reported (missing sheet or misread label).

---

## 6. Engineering observation model

Full field definitions, enumerations and validation rules: [OBSERVATION_SCHEMA.md](../architecture/drawing-intelligence/OBSERVATION_SCHEMA.md). Summary:

### 6.1 Record

```
Observation {
  id                 : content hash (immutable)                      # unique identifier
  tenant, job, revision_context
  type               : enum (Dimension, Tolerance, TextNote, TitleField, GeometryEntity, LineRole,
                             ViewRegion, ViewRelation, Scale, ProjectionMethod, Association, Symbol,
                             Material, Thickness, Designation, RevisionMark, DocumentRelation,
                             UnitDeclaration, Hypothesis, …)
  value              : typed payload (exact decimal + unit for numerics; never free text for numerics)
  alternatives       : [ {value, score, source_engine} ]            # discrete reading hypotheses
  unit               : explicit | sheet_default(ref) | unresolved
  source             : { artifact_hash, page | space/block path, entity/operator id,
                         region (source coordinates), transform_chain_id }
  method             : { component, version, model/weights hash, parameters hash }
  origin             : enum of 8 (§6.2)                              # provenance classification
  authority          : written | measured | inferred | assumed      # Spec 01 class (ARCH-14)
  confidence         : { kind: none | raw_score | calibrated_probability, value, calibration_ref }
  uncertainty        : none | continuous{distribution/covariance, components} | discrete{alternatives}
  reading_state      : unvalidated | corroborated | confirmed | corrected | rejected | n/a
  validation         : [ {actor, time, action, evidence_refs, signature} ]   # human validation history
  relations          : [ {kind, target_id, role} ]                   # associates, derived_from, supersedes,
                                                                     # conflicts_with, same_feature_as, part_of
  flags              : [reference, not_to_scale, theoretically_exact, low_resolution, across_break, …]
}
```

Records are immutable; a correction is a new record that `supersedes` the old one (ARCH-13).

### 6.2 The eight origin classes and their mapping

| # | Origin class | Meaning | Spec 01 authority | Spec 02 class | Uncertainty form |
|---|---|---|---|---|---|
| 1 | Explicit written engineering information | Text content stated on the drawing, read exactly (CAD entity or valid PDF text) | written | H once validated | none on reading; tolerance carried separately |
| 2 | Vector-extracted geometry | Geometry stored in a vector PDF, DXF/DWG or model file | measured | S | continuous (file precision, fit residual) |
| 3 | OCR-extracted information | Written content read by recognition | written, **gated by `reading_state`** | H only when confirmed/corrected (DI-33); otherwise not consumable as H | discrete alternatives |
| 4 | Raster-measured geometry | Geometry detected in pixels | measured | S | continuous (larger), systematic terms |
| 5 | Cross-view derived relationship | Correspondence/alignment between views, section/detail links | inferred | constraint candidates subject to hypothesis evaluation | discrete (hypothesis) |
| 6 | Algorithmically inferred candidate | Role classification, association, vocabulary interpretation, model suggestions | inferred | never H; becomes usable only when accepted by rule or person | discrete; confidence usually uncalibrated |
| 7 | Human-entered correction | Value or relation entered by an identified person | written (if transcribing the drawing) or Decision (if an engineering decision) | H / Decision | none; audit trail |
| 8 | Manufacturing assumption | Value from a process rule or customer convention | assumed | via ProcessData / Decision only (ARCH-11) | per ProcessData |

An additional sub-class, **standard-derived** (dimension obtained from a designation via a standard table), is modelled as origin 6 with `relation: derived_from → Designation` plus a `standard_table_ref`; it is authoritative only if the table data and edition are verified and the designation's reading is validated (DI-31).

The eight origin classes **refine** the four authority classes of Spec 01; they do not replace them. This is recorded as a proposed refinement in §14 — Spec 01 is not edited.

### 6.3 Confidence and uncertainty representation by method

| Method | Confidence | Uncertainty |
|---|---|---|
| Exact text / CAD entity | `none` (not applicable) | none for the reading |
| OCR / HTR | `raw_score` unless a calibration on the benchmark exists (`calibrated_probability` with reference to the calibration dataset and its validity conditions) | discrete alternatives (mandatory for numerics) |
| Vector geometry | `none` | coordinate precision; primitive-fit residual |
| Raster geometry | fit quality | covariance from fitting + transform chain + scale/anisotropy terms (Spec 02 §8.1) |
| Classification / association / hypothesis | `raw_score` (uncalibrated by default) | discrete alternatives |
| Human entry | `none` | none; second-person check is policy (Spec 05) |

- **DI-60** A probability may be labelled *calibrated* only with a reference to a calibration study on the benchmark; otherwise it is a raw score and the UI must not present it as a probability of correctness.

### 6.4 Validation rules (excerpt)

Numeric payloads must parse to exact decimals with explicit unit state · every observation has a resolvable source region · origin/authority combinations must be one of the rows above · class 3 numerics must carry alternatives (may be a single entry only if every engine produced no other candidate) · `confirmed/corrected` require a validation entry with actor and evidence · relations must reference existing records in the same tenant · a `supersedes` chain has one head · flags `reference` and class-H use are mutually exclusive.

---

## 7. Dimension reconciliation

Reconciliation is deterministic (Z2-style logic operating on observations; it uses UEM functions for scale estimation and constraint checks). It produces **conflict records** and **gap records**; it never edits values.

### 7.1 Rules

| ID | Situation | Rule | Result |
|---|---|---|---|
| RC-01 | Written value vs measured feature at the view's scale | Compare using measurement uncertainty (Spec 02 MATH-10). Written stays authoritative | Within uncertainty → corroboration evidence. Outside → `written_vs_measured` discrepancy listing: possible misread (check alternatives), wrong association, not-to-scale, real drawing error |
| RC-02 | Discrepancy explained by an alternative reading | If an n-best alternative of the reading matches the measurement and the primary does not | High-priority review item "probable misread"; **no automatic switch** |
| RC-03 | Written diameter vs measured circle | As RC-01; also check radius/diameter confusion (factor 2) and inside/outside (difference of 2t) as named hypotheses | Discrepancy with candidate explanations |
| RC-04 | Same feature dimensioned differently in two views or sheets | Exact comparison of written values (rational) | Equal → redundancy recorded. Different → `inter_view_conflict` (Contradictory) |
| RC-05 | Dimension chain vs overall dimension | Exact sum comparison, honouring reference flags and stated tolerances | Mismatch → `chain_conflict` with the minimal set |
| RC-06 | Different scales on views | Each view has its own scale observation; no cross-view measurement comparison without scale conversion | Missing/unclear view scale → measured geometry of that view qualitative only |
| RC-07 | Missing dimension | After constraint analysis (ALG-01) reports free parameters | `missing_dimension` gap listing which parameter is undetermined and what would determine it. Never filled from measurement silently; a measured value may be **offered** and, if accepted by an engineer, enters as a Decision with provenance *measured* |
| RC-08 | Contradictory tolerances | Tolerance on a dimension vs general-tolerance note vs duplicate dimension with a different tolerance | Conflict; no "tightest wins" default — the rule is a customer policy to be recorded (`FR-05-1`) |
| RC-09 | Ambiguous inside/outside | DI-53 | Review with the effect quantified (difference in developed length) |
| RC-10 | Unit ambiguity | No unit statement and `$INSUNITS` unset; or mixed units | `unit_unresolved`; plausibility evidence listed; not defaulted |
| RC-11 | Conflicting revisions | §8.3 | Review |
| RC-12 | DXF dimension override | DI-06 | Conflict |
| RC-13 | Model vs drawing | DI-08 | Conflict unless master declared |
| RC-14 | Measurement across a break or on a not-to-scale feature | DI-44 | Measurement excluded from RC-01; recorded |
| RC-15 | Designation vs written dimension (e.g. nominal pipe size and a written outside diameter that differs from the table value) | Compare when table data are verified | Conflict |

### 7.2 Authority rule

**DI-50** Explicit written dimensions remain authoritative. They change only through a human-authorised correction (origin 7) that records: who, when, the original, the replacement, the reason category (misread / drawing error confirmed with customer / superseded by revision), and supporting evidence. A "drawing error" correction — i.e. the drawing itself is wrong — is an engineering Decision and should reference customer confirmation; the system records whether such confirmation is attached and customer policy decides whether it is mandatory (`FR-05-3`).

### 7.3 Conditions that put the drawing into REVIEW REQUIRED

Any unresolved: conflict record (RC-01…15) on geometry used by the job · gap record on a required parameter · `Ambiguous` hypothesis set · unit unresolved · projection method ambiguity that is not immaterial · class-H reading not `confirmed/corrected` (default policy) · inside/outside unknown on a thickness-relevant dimension · `extraction_unreliable` or `low_resolution` page contributing class-H readings · JBIG2/lossy risk flag on numeric readings · unsupported content intersecting the part's views · unparsed welding information on a joint that defines a cut · handwritten modification detected · accepted-with-repair file. These map onto gates G01–G04 of Spec 02 §10; none can be cleared by model confidence.

Conditions that produce **REJECTED** at ingestion: file rejected (§1.1); zero interpretable content.

---

## 8. Multi-page and multi-drawing relationships

### 8.1 Identities (kept distinct)

| Entity | Identity |
|---|---|
| File | Content hash |
| Document | Drawing number + sheet set, as stated in title blocks |
| Sheet | Document + sheet number |
| Document revision | Document + revision identifier |
| Part / item | Part number or item mark within a document's scope |
| Part revision | Part + the document revision(s) that define it |
| Job package | The set of files submitted for one job, with roles (assembly, detail, reference, specification) |

### 8.2 Establishing relationships

Relationships are observations (origin 5 or 6) built **only from explicit identifiers**: drawing numbers and sheet counters, part/item marks and balloons ↔ parts-list rows, "see drawing …" references, section/detail labels, revision marks, file-internal structure (pages of one PDF). 

- **DI-70** Geometric similarity is never sufficient to merge or relate parts. It may generate a *suggestion*.
- **DI-71** Geometry and dimensions from different documents are combined into one part's constraint system only through an accepted relationship; unrelated or reference-only documents ("for reference", `参考図`) contribute no constraints.
- Several parts on one sheet: view regions are grouped per part by part marks, titles and enclosing frames; ungrouped views are reported and not assigned by proximity alone.
- Assembly ↔ detail: assembly dimensions that restate detail dimensions are redundancy checks (RC-04); interface dimensions (positions, orientations of branches) may exist only on the assembly and are then authoritative for the relationship between parts.
- Material specifications and standards referred to by the drawing are linked as documents; their content is used only if supplied and parsed, else recorded as *referenced, not available*.
- Missing members (sheet 2 of 3 absent; referenced detail not supplied) are gap records.

### 8.3 Revisions

Revision evidence: title-block revision field, revision table rows, revision marks/clouds near changed features, file metadata (weak). Rules: a new document revision is a new SourceArtifact linked to its predecessor (Spec 01 §6.3); observations are diffed; changed/added/removed observations are listed for confirmation. Conflicts: two files claiming the same revision with different content; mixed revisions within a package (detail at rev C, assembly referencing rev B); a higher revision that is older by date; hand-marked changes without a revision entry → all REVIEW REQUIRED (RC-11). The system never assumes "latest file uploaded = current revision".

---

## 9. Automated reading and human interaction

### 9.1 Component boundaries

| Component | Zone | May produce | May not |
|---|---|---|---|
| File parsers | Z0/Z1 | Origin 1–2 observations | Interpret |
| Image processing | Z1 | Transform chains, working images, origin 4 geometry | Alter source; rescale to fit dimensions |
| OCR / HTR | Z1 | Origin 3 with alternatives | Emit a single "best" numeric without alternatives; correct text by vocabulary silently |
| Computer vision (detection/classification) | Z1 | Origin 6 candidates (regions, roles, symbols) | Remove content from consideration |
| Automated drawing reading (language/vision models) | Z1 | Origin 5–6 candidates: view classifications, candidate dimensions, annotation interpretations, relationships, ambiguity flags, explanations of alternative interpretations | See list below |
| Deterministic geometry processing (reconciliation, hypothesis evaluation) | Z2 logic | Conflict/gap records, hypothesis verdicts | Decide between surviving alternatives that differ in geometry |
| Human engineering review | Z4 | Origin 7 corrections, confirmations, Decisions | Bypass gates not resolvable by Decision (MATH-51) |

**Automated drawing reading may not:** alter an explicit dimension; supply a value for a missing measurement; override or relax a deterministic constraint or conflict; mark anything validated, confirmed or certified; set `reading_state` beyond `unvalidated`; approve or contribute to approval of release; discard an alternative; be the sole source of any class-H input. Structurally enforced: model-facing components can write only observation types with origin 5–6 and `reading_state ∈ {unvalidated, n/a}` (schema-level permission, DI-80). Free-form model text is stored only as an explanatory note attached to a structured candidate and is never parsed downstream.

Explanations generated by a model are labelled as machine-generated suggestions; they are not evidence.

### 9.2 Human review workflow

1. **Triage view** — per sheet: extraction status, unsupported content, conflicts, gaps, ambiguities, ordered by impact on the part (sensitivity from Spec 02 ALG-11 where available) and then by risk flags.
2. **Reading confirmation** — each class-H candidate shown as: source image crop (unfiltered, at native resolution), recognised value, alternatives, associated geometry highlighted, measured value at scale and its agreement. Actions: confirm / correct / reject / mark as reference / change association. Reading and association are confirmed together.
3. **Conflict resolution** — each conflict record with all involved observations and candidate explanations; resolution is a correction (misread), an association change, a flag (not to scale), or a Decision (drawing error, with reason and optional customer confirmation attachment).
4. **Ambiguity resolution** — surviving hypotheses side by side with their geometric differences; the engineer selects one (Decision) or requests clarification from the customer (job state: waiting-for-input).
5. **Gap resolution** — supply missing value with its source (customer reply, standard, engineer's decision) — recorded with provenance, never as "written".
6. **Sign-off of the observation set** — a signed record of the content hash of the accepted set; downstream reconstruction consumes only a signed set.

Traceability (DI-81): every action stores actor, time, role, before/after record ids, evidence viewed (image crop hash), free-text reason where required; nothing is overwritten. Bulk actions ("confirm all") are disabled for class-H numerics; whether a second-person check is required is customer policy (`FR-05-3`). Anti-rubber-stamping measures (e.g. requiring the reviewer to view the crop, seeded check items) are a UX research item (T-20).

Corrections are retained per tenant as potential training/evaluation data **only** under the tenant's recorded permission (§11).

---

## 10. Output contract to geometry reconstruction

The single hand-over object is a signed **ObservationSet**. The reconstruction engine (Spec 04) and UEM (Spec 02) read nothing else from this subsystem.

```
ObservationSet {
  schema_version, id (hash), tenant, job, created_by components (versions)
  sources          : [SourceArtifact refs with ingestion outcome]
  sheets           : [ {sheet id, document identity, revision, page classification, frames, transform chains,
                        sheet-level declarations: units, general tolerance, projection method (state)} ]
  observations     : [Observation]                      # §6
  parts            : [ {part identity, revision, member views, source documents} ]
  hypotheses       : [ {id, content, verdict: sole | equivalent | alternative | eliminated, search_complete: bool} ]
  conflicts        : [ {id, rule (RC-nn), observations, affected features, candidate explanations, state} ]
  gaps             : [ {id, kind, what is undetermined, what would determine it} ]
  unsupported      : [ {source ref, region/extents, kind, intersects part views: bool} ]
  review           : { required: bool, reasons: [codes with refs], sign_off: signature | none }
  master_definition: drawing | model | undeclared
}
```

**Conventions.**

- Sheet frame: mm on the sheet, origin lower-left of the sheet frame, x right, y up, counter-clockwise positive (matches the Spec 02 pattern-frame handedness). Raster-origin observations additionally keep source pixel coordinates. Model-space sources (DXF/STEP) keep file coordinates and file units with an explicit unit state.
- **No part-space coordinates are produced here.** View-to-part transforms are unknowns solved by reconstruction; this subsystem supplies view frames, scales and correspondences as observations.
- Numerics: exact decimals as written + unit state; angles as written (deg/min/sec) exact.
- Mapping to UEM inputs: validated origin 1/3/7 dimensions → class-H constraint candidates (with association, reference-surface attribute, flags); origin 2/4 geometry → class-S observations with covariance; origin 5/6 → constraints only inside an accepted hypothesis; origin 8 → never from this subsystem except as a pointer to ProcessData/Decision.
- **DI-90** The consumer must reject an ObservationSet that is unsigned, has `review.required = true` with unresolved critical reasons, or fails schema validation. Consumption for *preview/draft* geometry is allowed on an unsigned set, with every downstream record marked non-releasable.
- **DI-91** No field in the contract is free text intended for machine interpretation; notes are for humans only.
- Unsupported-feature reports identify whether unsupported content overlaps any part's views; if yes → review.

---

## 11. Security and confidentiality

(Architecture commitments from Spec 01 §8; details to Specification 07.)

- **Malicious files:** §1.1; parser fuzzing and malformed-file corpora are part of the test plan; decoders and parsers are pinned and patched as a managed supply chain (SBOM).
- **Sandboxing:** one short-lived, unprivileged, network-less process per file per stage; resource limits; output is schema-validated before it leaves the sandbox; a crash is a recorded outcome.
- **Derived renderings** shown to clients are produced by our renderer from sanitised data, never the original file streamed to a browser plug-in.
- **Local execution:** all OCR/vision/reading models needed for the baseline product run on-premises with no network. External services are used only under the tenant's automated-reading policy level, per-job overridable only downward (more restrictive), and each external call is logged with what was sent.
- **Confidentiality:** tenant isolation of files, observations, caches, vocabularies and correction data; title-block and customer names are confidential data too; logs contain identifiers, not drawing content.
- **Retention:** original files, derived images and observation sets follow the tenant's retention policy; working images are deletable caches. Retention vs reproducibility remains the open question of Spec 01.
- **Audit:** uploads, views, exports, validations, corrections, sign-offs, policy changes, external-service calls.
- **Training:** no customer drawing, crop, correction or derived data is used to train or tune any model without explicit recorded permission from the tenant (and, where drawings belong to the tenant's customers, the tenant's confirmation that it may grant it). Default: off. Tenant-specific tuning produces tenant-scoped weights.
- **Model supply chain:** weights are identified by hash, obtained from recorded sources, licence-checked for commercial redistribution (research register), and scanned as untrusted files (model formats that can execute code on load are prohibited).

---

## 12. Testing and validation

Defined in the [Drawing Intelligence Test Plan](../benchmark/DRAWING_INTELLIGENCE_TEST_PLAN.md): test classes for every item in the task list, metrics, the false-accept metric, and the ambiguous-drawing rejection test. Qualification of any recogniser or rule for production use follows the same principle as Spec 02 §12: results state corpus, version, sample size and limits; passing tests does not make an interpretation trustworthy for release — per-job validation is always required.

---

## 13. Technical research

Candidates, licences, suitability and benchmark needs: [Drawing Intelligence Research Register](../research/DRAWING_INTELLIGENCE_RESEARCH_REGISTER.md). Summary of *directions* (not selections):

| Area | Direction | Decided by |
|---|---|---|
| PDF parsing | Permissively licensed parser with full content-stream access, plus an independent renderer for the cross-check; AGPL/GPL options excluded unless commercially licensed | Capability benchmark on CAD-plotted Japanese PDFs + licence review |
| Raster processing | Established imaging library; our own recorded transform chain | Thin-feature preservation tests |
| OCR | ≥ 2 independent engines + specialised dimension recogniser; local execution | Benchmark on authorised Japanese drawings; false-accept rate |
| CAD parsing | Own neutral entity model; DXF reader to be chosen/benchmarked; DWG undecided; STEP via kernel port | Entity coverage on factory files; licensing |
| Vector extraction | Arc/circle recognition from Béziers and segment chains with residuals | Tier-S synthetic tests |
| View detection | Rule-based hypothesis generation + optional model proposals; deterministic evaluation | Hypothesis-coverage benchmark |
| Local automated-reading inference | Portable runtime with deterministic CPU execution; weights with redistribution rights | Determinism and hardware survey (`FR-07-1`) |

---

## 14. Consistency with Specifications 01 and 02

| Item | Existing | This specification | Resolution |
|---|---|---|---|
| Provenance classes | Spec 01 ARCH-14: four classes (written / measured / inferred / assumed) | Eight origin classes requested for this task | Modelled as a **refinement**: each observation carries both `origin` (8) and `authority` (4) with the fixed mapping of §6.2. **Proposed amendment** to Spec 01 §6.1 (Observation record) — not applied |
| Observation status | Spec 01: proposed / accepted / rejected / superseded | `reading_state` (unvalidated / corroborated / confirmed / corrected / rejected) + supersession by relation | Mapping: accepted = confirmed or corrected (or corroborated where Spec 05 permits). **Proposed amendment** — not applied |
| H/S classes | Spec 02 §2.3 | Consumed as defined; adds the rule that OCR-read values are H only after reading validation, and that reference dimensions are never H | Consistent; Spec 02 §2.3 should cite DI-30/DI-33 at its next revision |
| Discrete misread risk | Spec 02 MATH-31 | Implemented as mandatory n-best alternatives | Consistent |
| Release states | Spec 02 §10 | REVIEW REQUIRED triggers map to G01–G04 | Consistent; G02/G04 definitions should reference §7.3 here |
| Inside/outside attribute | Spec 02 `FR-02-2` | DI-53 | Consistent |
| Development classes (E/A/X/N) | Proposed in Spec 02 §14; **not accepted**, pending engineering review | Not used, not amended. One related observation for the reviewers: a *development view drawn by the designer* (§5.1) is evidence about the intended pattern but says nothing about its exactness class; the classification question remains entirely within Spec 02 | Recorded for review; no change |
| Exactness vs numerical evaluation; exactness vs DXF representation | Spec 02 §0.2 (E-C/E-N; $\varepsilon_{rep}$) | Analogous distinction here: exact *reading* of a string vs authority of its content; exact vector coordinates vs their authority for the part | Consistent in spirit; no change |
| Sheet frame handedness | Spec 02 §0.3 pattern frame | Same convention for sheet frame | Consistent |
| ADR-005 | Proposed | Elaborated; no decision changed | Consistent |
| Specification 02 status | Provisional until independent review | Treated as provisional: this document depends only on its interfaces (H/S classes, outcomes, gates), not on unreviewed family formulas | — |

No existing specification or ADR was modified by this task.

---

## 15. Open requirements from the factory (additions)

| ID | Need |
|---|---|
| FR-03-1…5 | As in the roadmap (samples, format mix, scan quality, projection/house standards, automated-reading restrictions) |
| FR-03-6 | Originating CAD systems of incoming drawings (affects PDF structure, fonts, DXF dialects); whether JWW or other Japanese formats arrive |
| FR-03-7 | Title-block layouts of the main customers |
| FR-03-8 | House conventions: reference dimensions, not-to-scale marking, hole/count notation, thickness notation, nominal pipe-size usage and the standards they refer to |
| FR-03-9 | How often drawings are hand-marked, stamped, faxed or photographed |
| FR-03-10 | Whether designers supply development views, and how reliable they are considered |
| FR-03-11 | Current practice when a drawing is ambiguous or wrong (who contacts the customer; how the answer is recorded) |
| FR-03-12 | Typical package structure (assembly + details? single sheets?) and revision practice |
