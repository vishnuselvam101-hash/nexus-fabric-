# ADR-011: Source-denominated coverage ledger

**Status: Ledger accepted with conditions · Release gate (G17) deferred** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Decision
Coverage is accounted against denominators enumerated from the source at engulfment; every unit has exactly one state.

## Conditions
One definition of "explained"; versioned applicability table; multi-level raster enumeration with attribution; separation of accounting closure, examination coverage and completeness claims — proposed in [COVERAGE_AND_SAFETY_REFINEMENT.md](amoeba/COVERAGE_AND_SAFETY_REFINEMENT.md) Part D. A distinct `excluded_by_decision` state is proposed and **not yet decided**.

## Deferred
Gate G17: decision after enumeration cost, stability and reviewer workload are measured.
