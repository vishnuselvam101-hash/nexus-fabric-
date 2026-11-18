# Proposed ADRs for the AMOEBA architecture

> **Superseded as a status record (18 November 2026).** Decisions on these proposals are recorded in the [Decision Register](../DECISION_REGISTER.md) and in ADR-009A, 009B and 010–016. The original text below is preserved unchanged.

Version 0.1 · 18 November 2026 · All **Proposed — not accepted**. Listed here rather than filed as ADR-009…014 until the owner confirms the AMOEBA definition and the independent architecture review has taken place.

| # | Title | Decision proposed | Main alternatives | Evidence required before acceptance |
|---|---|---|---|---|
| ADR-009 | AMOEBA as the governing architecture | All engineering work runs as AMOEBA processes over one EOS; the stage pipeline becomes a typical path | Keep the fixed pipeline of Specs 01–03 and add coverage accounting only; adopt AMOEBA for drawing intelligence only | Benchmark B-1…B-4 (determinism, safety invariants, benefit of adaptivity at equal budget); architecture review |
| ADR-010 | Engineering Organism State | EOS tuple with versioning, views and boundary references | Truth graph only (Spec 01); separate stores per subsystem | Performance of views on large packages; canonical hashing |
| ADR-011 | Source-denominated coverage and coverage gate | Coverage ledger from engulfment enumeration; five states; gate G17 | Coverage over detected observations only; no gate | Ink-enumeration cost and stability; reviewer workload for exclusions |
| ADR-012 | Bounded, deterministic adaptive exploration | Probe catalogue, budgets, deterministic priority scheduling, mandatory baseline independent of priority | Fixed exhaustive passes; unbounded agentic exploration (rejected: not observable or reproducible) | Replay determinism; no-starvation proof or test; benefit measurement |
| ADR-013 | Lanes, quarantine and retirement | experimental / candidate / production for algorithms, models, rules, templates and memory; quarantine and tombstone records | Version flags only | Demonstration that non-production output cannot reach release evidence |
| ADR-014 | Signals as derived events | Typed signals derived from EOS appends; replayable log; no direct process-to-process calls | Direct orchestration graph; external message broker as source of truth | Order-independence of results under parallel execution |
| ADR-015 | Independent-evidence thresholds | Thresholds count declared and *measured* independence; shared dependencies count once; maximum outcome `corroborated` | Simple agreement voting | Correlated-failure benchmark for recogniser pairs and verifier pairs |
| ADR-016 | Tenant knowledge ("memory") governance | Versioned, tenant-scoped, permissioned, propose-only | No learning; global learning | Legal review of permissions; bias tests |

Naming note for counsel: "Amoeba" is the name of an existing, unrelated distributed operating system from academic research, and amoeba-/slime-mould-inspired computing is an existing research field; a name and prior-art check belongs in Document 09.
