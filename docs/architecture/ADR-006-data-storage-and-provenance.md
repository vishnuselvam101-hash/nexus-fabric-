# ADR-006: Data storage and provenance

**Status:** Proposed · 2026-11-18 · Spec: [01 §6](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md)

## Context
Every output entity must trace to source regions and decisions; releases must be reproducible; revisions must trigger impact analysis; storage must work on a single on-premises node and in the cloud; tenants must be isolated.

## Options
1. **Mutable relational model with audit tables.** Familiar; provenance is bolted on and can drift from reality.
2. **Event sourcing.** Full history; derived state and replay complexity; provenance between artefacts still needs explicit modelling.
3. **Immutable, content-addressed record graph (Engineering Truth Graph)** — records keyed by hash of content + inputs; derivation edges explicit; mutable state limited to small "head" pointers; metadata in a relational database, blobs in object storage; append-only hash-chained audit log.
4. **Dedicated graph database.** Natural queries; extra operational component on-premises; maturity/licensing to assess.

## Decision (proposed)
Option 3 on PostgreSQL-class relational storage plus S3-compatible object storage (both have single-node forms). Graph database not adopted unless traversal benchmarks demand it.

## Advantages
Provenance and caching fall out of the same mechanism; tamper evidence; simple synchronisation (union of immutable records); natural impact analysis.

## Disadvantages
Storage growth; schema evolution of immutable records needs versioned readers; deletion requests conflict with immutability.

## Risks
Canonical serialisation for hashing must be stable across versions and platforms (floats especially); retention/deletion versus reproducibility unresolved; deep-graph traversal performance in SQL.

## Validation required
- Canonical encoding specification and cross-platform hash tests.
- Traversal benchmark (DXF entity → sources) at realistic graph sizes.
- Deletion design (e.g. per-tenant key destruction) reviewed against customer contracts and applicable law — legal input required.
- Tenant isolation model decided in Specification 07.
