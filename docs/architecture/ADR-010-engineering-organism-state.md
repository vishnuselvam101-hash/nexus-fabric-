# ADR-010: Engineering Organism State

**Status: Accepted with conditions** · 18 November 2026 · Decided by the project owner; recorded in the [Decision Register](DECISION_REGISTER.md). Background: [review](../reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) §11, [original proposal](amoeba/PROPOSED_ADRS.md).

## Decision
One versioned, append-only EOS holds the whole project; processes work on views with boundary references.

## Conditions (each must be met before the ADR is treated as fully accepted)
1. Typed, versioned dependency edges and Interface records — proposed in [DEPENDENCY_MODEL.md](amoeba/DEPENDENCY_MODEL.md).
2. State identity over primary data only; immutable Findings — [DETERMINISM_AND_APPROVAL.md](amoeba/DETERMINISM_AND_APPROVAL.md) §2.
3. Approval bound to a release-closure hash, with dependency-aware invalidation — same note §4.
4. One coherent record model with separate provenance, authority, reading, validation and lifecycle axes — [CONSOLIDATED_RECORD_MODEL.md](amoeba/CONSOLIDATED_RECORD_MODEL.md).
5. A domain contract for extension beyond pipe and plate — to be written with Specification 04.
6. Rule for global dependencies (sheet- and project-level declarations) in views — covered by closure rule item 4; to be confirmed in review.

Conditions 1–4 have proposed designs awaiting review; 5 is open.
