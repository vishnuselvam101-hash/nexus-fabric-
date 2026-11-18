# Observation Schema (supporting note to Spec 03)

Version 0.1 · 18 November 2026 · Normative detail for [Spec 03 §6 and §10](../../specifications/03_DRAWING_INTELLIGENCE_AND_INGESTION_SPECIFICATION.md). Draft; field names are proposals until the internal schema language is chosen (Spec 01 open question 8).

## 1. Common types

| Type | Definition |
|---|---|
| `Hash` | SHA-256 of canonical serialisation (canonical encoding: ADR-006 validation item) |
| `Decimal` | Exact decimal: integer mantissa + base-10 exponent + `as_written` string (keeps trailing zeros and original glyphs) |
| `Unit` | `mm | cm | m | inch | ft | deg | dms | none` |
| `UnitState` | `explicit(Unit) | sheet_default(obs_ref) | unresolved` |
| `Region` | `PdfRegion{page, bbox_sheet_mm, operator_ids[]}` · `PixelRegion{image_ref, polygon_px}` · `CadRef{space, block_path[], handle}` · `StepRef{instance_id, path[]}` |
| `SourceRef` | `{artifact: Hash, region: Region, transform_chain: Hash?}` |
| `Method` | `{component, version, weights: Hash?, params: Hash}` |
| `Confidence` | `{kind: none | raw_score | calibrated_probability, value: float?, calibration_ref: Hash?}` |
| `Uncertainty` | `none` · `continuous{components[]: {name, distribution: normal|rectangular|triangular, parameter}, covariance_ref?}` · `discrete{alternatives[]}` |
| `Alternative<T>` | `{value: T, score: float?, engine: Method}` |

## 2. Enumerations

**origin** (8): `written_exact`, `vector_geometry`, `ocr`, `raster_geometry`, `cross_view`, `inferred_candidate`, `human_entered`, `manufacturing_assumption`.

**authority** (Spec 01): `written`, `measured`, `inferred`, `assumed`.

Allowed pairs: written_exact→written · vector_geometry→measured · ocr→written · raster_geometry→measured · cross_view→inferred · inferred_candidate→inferred · human_entered→written (transcription) or `decision_ref` required (engineering decision) · manufacturing_assumption→assumed (must carry `process_data_ref` or `decision_ref`).

**reading_state**: `n/a`, `unvalidated`, `corroborated`, `confirmed`, `corrected`, `rejected`.

**flags**: `reference`, `not_to_scale`, `theoretically_exact`, `typical`, `across_break`, `low_resolution`, `lossy_glyph_risk`, `extraction_unreliable`, `handwritten`, `overlapped`, `repaired_source`.

**surface_reference**: `inside`, `outside`, `mean`, `not_applicable`, `unknown`.

## 3. Payloads by observation type

| Type | Payload |
|---|---|
| `Dimension` | `{kind: linear|aligned|angular|diameter|radius|spherical_diameter|spherical_radius|arc_length|ordinate|chamfer|thickness|square, value: Decimal, unit: UnitState, count: int?, surface_reference, tolerance_ref?, structure: {text_ref, dim_line_ref?, terminators[], extension_refs[], leader_ref?}}` |
| `Tolerance` | `{form: symmetric|deviations|limits|fit_class|general_class, upper: Decimal?, lower: Decimal?, class: string?, standard_ref?: string, applies_to: obs_ref | sheet}` |
| `TextNote` | `{text, language_tags[], orientation, parsed_directives[]: obs_refs}` |
| `TitleField` | `{field: drawing_number|title|scale|projection|unit|material|quantity|sheet|revision|date|drafter|checker|approver|company|other, value}` |
| `GeometryEntity` | `{primitive: line|circle|arc|ellipse|spline|polyline|bezier, parameters (sheet mm or file units), style: {width, dash, colour, layer}, fit: {residual, source_entities[]}?}` |
| `LineRole` | `{entity_ref, role: outline|hidden|centre|dimension|extension|leader|section_plane|hatch|break|phantom|border|symbol_part|unknown}` |
| `Symbol` | `{class: weld|surface|datum|balloon|projection|section_arrow|detail_marker|other, parsed?: structured, region}` |
| `ViewRegion` | `{boundary, type_candidates, label?: string, scale_ref?, part_ref?}` |
| `ViewRelation` | `{from, to, relation: projection_right|left|above|below|rear|section_of|detail_of|auxiliary_of|development_of, reference_label?}` |
| `Scale` | `{applies_to: sheet|view, ratio: Decimal pair | not_to_scale, evidence[]}` |
| `ProjectionMethod` | `{value: first_angle|third_angle|undetermined, evidence[]: {kind: symbol|text|geometry|site_default, ref}, materiality: material|immaterial|unknown}` |
| `Association` | `{dimension_ref, targets[]: {entity_ref, feature: endpoint|centre|edge|silhouette|axis|…}, basis: structure|leader|proximity}` |
| `Material` | `{designation: string, standard?: string}` |
| `Thickness` | `{value: Decimal, unit, applies_to}` |
| `Designation` | `{system: nominal_pipe|section|thread|other, text, standard_table_ref?: string}` |
| `RevisionMark` | `{mark, table_row?: {date, description, author}, regions[]}` |
| `DocumentRelation` | `{from_doc, to_doc, relation: sheet_of|detail_of|assembly_of|references|supersedes|specification_for, basis: identifier_match|explicit_reference|human}` |
| `UnitDeclaration` | `{scope: sheet|view|file, unit: UnitState, evidence[]}` |
| `Hypothesis` | `{projection_ref, view_relations[], scales[], correspondences[], associations[], verdict, search_complete}` |

## 4. Validation rules

1. `id == hash(canonical(record without id))`.
2. Numeric values are `Decimal`; floats are permitted only inside `GeometryEntity.parameters`, `Confidence` and `Uncertainty`.
3. `source.region` resolves within the referenced artifact; pixel regions lie inside the image; a `transform_chain` is present for any observation located on a working image.
4. `(origin, authority)` is an allowed pair.
5. `origin = ocr` and numeric payload ⇒ `uncertainty.discrete` present.
6. `reading_state ∈ {confirmed, corrected}` ⇒ a `validation` entry with actor, role, time, evidence hash and signature; `corrected` ⇒ `supersedes` the original record.
7. `flags ∋ reference` ⇒ may not be mapped to a class-H constraint.
8. `Confidence.kind = calibrated_probability` ⇒ `calibration_ref` present and valid for this `Method`.
9. Writers: components registered as model-based may write only `origin ∈ {cross_view, inferred_candidate}` (and `ocr` for recognisers) with `reading_state ∈ {unvalidated, n/a}`.
10. `Dimension.surface_reference = unknown` on a dimension whose targets belong to a walled part ⇒ review reason `SURFACE_REFERENCE_UNKNOWN`.
11. `UnitState = unresolved` on any dimension used by a part ⇒ review reason `UNIT_UNRESOLVED`.
12. Relations reference records of the same tenant; `supersedes` chains are linear with a single head.
13. `manufacturing_assumption` records are not created by ingestion components.

## 5. Review reason codes (ObservationSet.review.reasons)

`CONFLICT(rule)`, `GAP(kind)`, `AMBIGUOUS(hypothesis|projection|association|reading)`, `UNIT_UNRESOLVED`, `SURFACE_REFERENCE_UNKNOWN`, `READING_NOT_CONFIRMED`, `EXTRACTION_UNRELIABLE`, `LOW_RESOLUTION`, `LOSSY_GLYPH_RISK`, `UNSUPPORTED_IN_VIEW`, `WELD_INFO_UNPARSED`, `HANDWRITTEN_CHANGE`, `REPAIRED_SOURCE`, `REVISION_CONFLICT`, `MISSING_DOCUMENT`, `MASTER_UNDECLARED`, `SEARCH_INCOMPLETE`.
