# ADR-013: Lanes, quarantine and retirement

**Status: Accepted with conditions** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Decision
Every algorithm, model, rule, template and knowledge item is in the experimental, candidate or production lane; quarantine and retirement are records; nothing is promoted automatically.

## Conditions
Overlay isolation of non-production output from production scheduling; taint rule over the release closure; quarantine scope rules; explicit list of what may be machine-proposed and what is human-authored only — proposed in [COVERAGE_AND_SAFETY_REFINEMENT.md](amoeba/COVERAGE_AND_SAFETY_REFINEMENT.md) S-1, S-5. Awaiting review.
