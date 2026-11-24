# ADR-012: Bounded, deterministic adaptive exploration (incorporating signals)

**Status: Provisional — benchmark required** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Decision
Adaptive exploration is designed as: probe and test catalogue; deterministic scheduling in rounds; budgets in logical work units; mandatory work independent of priority; signals as events derived at round commit (formerly proposed ADR-014, **merged here**); hypothesis-driven investigation records; pre-qualified strategy profiles with recorded switches; stopping rules with a residual-uncertainty statement.

## Proposed designs
[INVESTIGATION_MODEL.md](amoeba/INVESTIGATION_MODEL.md), [DETERMINISM_AND_APPROVAL.md](amoeba/DETERMINISM_AND_APPROVAL.md) §1–3, [COVERAGE_AND_SAFETY_REFINEMENT.md](amoeba/COVERAGE_AND_SAFETY_REFINEMENT.md) S-3.

## Remains provisional until
Replay determinism under parallel execution is demonstrated; mandatory work is shown unaffected by signals; and the benefit question is answered under ADR-009B. The record-keeping and self-consistency audits in the investigation model are reliability mechanisms kept regardless of the outcome.
