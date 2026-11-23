# ADR-008: Cloud versus on-premises deployment

**Status:** Proposed · 2026-11-18 · Spec: [01 §5, §8](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md)

## Context
Target customers hold confidential third-party drawings and may prohibit external processing or internet connectivity. Others will prefer a managed service. Distributing binaries on-premises triggers licence obligations that hosted-only operation may not.

## Options
1. **Cloud-first, on-premises later.** Fastest iteration; later port typically painful; excludes strict customers at the start.
2. **On-premises only.** Fits strict customers; slow updates; no managed offering.
3. **One build, three topologies** (single-node, on-premises cluster, private cloud), with **single-node offline as the baseline** every feature must work in; cloud adds scale and managed operations, not capability. Z1 capability may vary by automated-reading policy.

## Decision (proposed)
Option 3.

## Advantages
Addresses the first market's confidentiality constraints from day one; forces minimal external dependencies; shadow pilots can run inside the customer's network.

## Disadvantages
No reliance on managed cloud services in the core; offline update, licensing/activation, support and telemetry need their own designs; GPU availability on customer hardware varies.

## Risks
Version fragmentation across customer sites versus reproducibility duties; local reading models on modest hardware; distribution makes every third-party licence obligation concrete (LGPL relinking, notices, any GPL/AGPL exposure).

## Validation required
- Hardware and network survey of pilot factories (open requirement).
- Signed offline update and rollback mechanism prototype.
- Licence review per deployment mode ([licensing review](../research/DEPENDENCY_LICENSING_REVIEW.md)) by counsel.
- Minimum hardware profile derived from benchmarks, not assumed.
