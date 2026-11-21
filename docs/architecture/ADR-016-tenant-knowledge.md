# ADR-016: Tenant knowledge governance

**Status: Deferred pending legal and security review** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Interim rule (in force for design work)
Tenant-scoped; propose-only (never supplies an authoritative value or confirms a reading); no learning from customer data without recorded permission; new items start in the experimental lane; exclusion templates are controlled and their use is counted in coverage statements.

## Needed for decision
Legal review of permissions (including drawings owned by a tenant's customers); security review of isolation; bias tests (Architecture Benchmark B-8).
