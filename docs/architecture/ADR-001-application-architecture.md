# ADR-001: Application architecture

**Status:** Proposed · 2026-11-18 · Spec: [01 §1–2, §5](../specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md)

## Context
The product must run as a single factory server with no internet, as an on-premises cluster and in a private cloud, with identical engineering results. It processes hostile files and calls native geometry code that can crash. Trust zones (Z0–Z4) must be enforceable in the structure, not only by convention.

## Options
1. **Monolith** — one process. Simple to ship on-premises; no isolation of parsers/native code; trust zones are only module boundaries.
2. **Microservices** — one deployable per service. Good isolation and scaling; heavy to operate on a single factory machine; distributed-systems failure modes from day one.
3. **Modular core + isolated workers** — one backend deployable containing orchestration, data access and APIs, with contract-defined modules; untrusted parsing, AI inference and native geometry run as separate sandboxed worker processes pulled from a durable job queue. The same workers can be spread across machines in larger deployments.

## Decision (proposed)
Option 3.

## Advantages
Single-node install stays simple; crash and security isolation where it matters; trust zones map to process boundaries; scales out without changing contracts.

## Disadvantages
Worker protocol and durable job graph must be built or adopted early; two execution modes (co-located and distributed) to test.

## Risks
Module boundaries eroding inside the core; queue technology unsuitable for single-node (e.g. requires a separate broker).

## Validation required
- Prototype: one job through ingest → extract → record, surviving a forced worker crash, on a single node with no network.
- Choose internal protocol/schema language (open question 8) with a Rust↔Python↔client code-generation spike.
- Confirm the job store can be the relational database itself for single-node deployments.
