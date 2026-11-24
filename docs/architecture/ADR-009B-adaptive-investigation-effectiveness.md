# ADR-009B: Effectiveness of adaptive investigation

**Status: Provisional — to be demonstrated experimentally** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Claim under test
AMOEBA's adaptive, signal- and dependency-directed investigation detects more engineering defects, especially critical ones, at equal computational budget than conventional fixed or uniform investigation.

## Decision
The claim is neither accepted nor rejected. It is tested under the frozen [AMOEBA Benchmark Protocol](../benchmark/AMOEBA_BENCHMARK_PROTOCOL.md) v1.0, whose falsification criteria and consequences were fixed before any tuning or data. The original vision is not weakened in the meantime: the adaptive mechanisms are designed and specified in full; only claims about their benefit are withheld.

## Until demonstrated
No external or internal statement of superiority. Adaptive selection runs with zero-knowledge defaults (catalogue order) where its tables are unmeasured. Safety never depends on discretionary investigation.
