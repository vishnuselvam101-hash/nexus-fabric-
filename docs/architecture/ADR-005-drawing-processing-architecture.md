# ADR-005: Drawing processing architecture

**Status:** Proposed · 2026-11-18 · Spec: [01 §1.2, §2](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md)

## Context
Inputs are vector PDFs, scans/images and CAD files, often multi-page and multi-view, in Japanese and English. Interpretation is inherently uncertain; AI is useful but must not author geometry. Customers may forbid external AI.

## Options
1. **End-to-end model** (drawing → geometry/DXF). Fast demo; unverifiable, no provenance; violates C1.
2. **Deterministic-only extraction.** Auditable; weak on scans, handwriting, layout variety.
3. **Staged pipelines producing typed observations:** four pipelines (vector PDF, raster, CAD, multi-view interpretation); AI and heuristics propose observations with provenance class, source region and uncertainty; a reconciliation step (written vs measured vs scale) flags conflicts; only accepted observations feed Z2; ambiguity yields candidate interpretations and questions, not guesses.

## Decision (proposed)
Option 3. Models are replaceable components behind the observation schema; each deployment runs the subset permitted by the customer AI policy (external / local-only / deterministic-only).

## Advantages
Provenance by construction; models can improve or be swapped without touching the trusted core; written dimensions take precedence over measured linework by rule, with discrepancies surfaced.

## Disadvantages
More engineer interaction than a "one-click" competitor on hard drawings; observation schema must be rich enough for real drawings (tolerances, datums, weld symbols, JIS conventions).

## Risks
Dimension-to-entity association errors are the likely dominant failure and are not self-evident; OCR confusion on similar glyphs/digits; local-only models may underperform; engineers rubber-stamping proposals.

## Validation required
- Observation schema tested against a sample of authorised factory drawings (open requirement: drawings not yet supplied).
- Per-stage metrics from the benchmark plan (extraction precision/recall, association accuracy, false-accept rate).
- Parser sandboxing tested with malformed-file corpora.
- Comparison of local versus external models on JA/EN drawings.
- UI study: does the review workspace make wrong associations visible?
