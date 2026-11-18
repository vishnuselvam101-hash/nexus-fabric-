# ADR-003: CAD kernel selection strategy

**Status:** Proposed (strategy only — **no kernel is selected**) · 2026-11-18 · Spec: [01 §4](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md) · Licensing: [review](../research/DEPENDENCY_LICENSING_REVIEW.md)

## Context
The master plan named OCCT and CGAL as starting candidates. Neither is committed. OCCT is LGPL-2.1 with an exception; most CGAL algorithm packages are GPL-3+ with a commercial alternative. No candidate offers exact B-rep modelling. Initial geometry (pipes, cones, branches, plates) is largely analytic.

## Options
1. **Commit to OCCT + CGAL now.** Fast start; licensing and robustness unproven.
2. **Commercial kernel** (Parasolid/ACIS/CGM). Robustness reputation; cost and redistribution terms unknown; vendor lock-in.
3. **Rust-native kernel** (`truck`, Fornjot). Clean integration; maturity unproven.
4. **Layered strategy with deferred selection:**
   a. own analytic layer for developable primitives and their intersections (closed-form + interval bounds);
   b. a robust 2D profile layer for final cut geometry;
   c. a B-rep kernel behind `uem-kernel-port` for general modelling, STEP exchange and as an independent cross-check;
   d. select (b) and (c) by benchmark and licence review.

## Decision (proposed)
Option 4. Run the evaluation protocol in Spec 01 §4.3 on OCCT, at least one commercial kernel (if a trial is obtainable) and one Rust-native kernel; and on CGAL (commercial-licence assumption) versus permissively licensed 2D libraries for layer (b).

## Advantages
Critical maths for the first market is ours, analytic and verifiable; kernel becomes replaceable; a second geometry source strengthens verification independence; licence exposure is minimised and decided on evidence.

## Disadvantages
Delays a decision the team may want early; port abstraction costs effort and can leak; two geometry representations to keep consistent.

## Risks
Analytic layer scope creep into "a kernel from scratch" (forbidden by C7); lowest-common-denominator port; benchmark corpus too easy to discriminate.

## Validation required
Evaluation report with: correctness vs analytic references, degenerate-case behaviour, silent-wrong/explicit-fail/crash rates, determinism, performance, FFI effort, STEP round-trip, maintenance signals, counsel-reviewed licence obligations per deployment mode. Acceptance of this ADR's selection follow-up requires that report.
