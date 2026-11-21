# ADR-015: Independent-evidence thresholds

**Status: Principle accepted · Numerical thresholds deferred** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Accepted principle
Evidence counts toward a threshold only to the extent of its independence; items sharing a declared dependency (preprocessing, model family, training data, inputs, implementation) count once; independence that has not been measured receives zero credit; the strongest outcome a threshold can produce is `corroborated` — never human confirmation, verification or approval.

## Deferred
Any numerical rule converting measured correlated-failure rates into credit, pending the dependence measurements (Architecture Benchmark B-6).
