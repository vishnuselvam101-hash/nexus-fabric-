# NEXUS FABRIC

Engineering Drawing-to-Manufacturing Geometry System: converts engineering drawings (PDF, images, scans, CAD) into mathematically verified, manufacturing-ready DXF, with traceable engineering decisions and measurable accuracy.

A person enters the dimensions; deterministic mathematical engines calculate; independent verification challenges the result; an authorised engineer would release it (engineer release is not implemented in this development build, so nothing is released).

## Pilot evaluation build / パイロット評価版

Pilot evaluation build. Not validated. Every DXF is NOT-RELEASED; an engineer must check every part before cutting. Not for confidential drawings. Do not run this program on a network (it has no TLS). Use desktop mode (`nexus-fabric.exe --desktop` on Windows, `./nexus-fabric.sh` on Linux): the server then listens on 127.0.0.1 only. Desktop mode does not encrypt the database. Pilot notes: [packaging/pilot/PILOT-NOTES.md](packaging/pilot/PILOT-NOTES.md) · install: [packaging/pilot/INSTALL-PILOT.md](packaging/pilot/INSTALL-PILOT.md).

パイロット評価版です。検証されていません。すべての DXF は NOT-RELEASED（未リリース）です。切断の前に、技術者がすべての部品を確認してください。機密図面には使用しないでください。ネットワーク上でこのプログラムを動かさないでください（TLS がありません）。デスクトップモード（Windows: `nexus-fabric.exe --desktop`、Linux: `./nexus-fabric.sh`）を使用してください。このモードではサーバーは 127.0.0.1 のみで待ち受けます。デスクトップモードでもデータベースは暗号化されません。

## Software

For the pilot, run desktop mode (see above). The commands below are for development from source; server mode is not part of the pilot.

A development build exists: one binary serving a browser client. Build, run and test: [Build and Test Guide](docs/BUILD_AND_TEST_GUIDE.md). What works and what does not: [Implementation Status](docs/IMPLEMENTATION_STATUS.md) · [Dependency Matrix](docs/IMPLEMENTATION_DEPENDENCY_MATRIX.md) · [Release Readiness](docs/RELEASE_READINESS.md).

**Not validated. No output may be used for manufacturing. Not for confidential drawings.**

```
cargo build --release -p nf-server && NF_DATA_DIR=./data target/release/nf-server   # then open http://127.0.0.1:8787
cargo test --workspace && bash tests/e2e_smoke.sh && bash tests/security_smoke.sh
```

Stress testing and learning from mistakes: [docs/LEARNING_AND_STRESS_TESTING.md](docs/LEARNING_AND_STRESS_TESTING.md). Security: [audit](docs/security/SECURITY_AUDIT.md) · [threat model](docs/security/THREAT_MODEL.md) · [vulnerability register](docs/security/VULNERABILITY_REGISTER.md) · [release checklist](docs/security/SECURITY_RELEASE_CHECKLIST.md). Design: [UI/UX audit](docs/design/UI_UX_AUDIT.md) · [design system](docs/design/DESIGN_SYSTEM.md). Campaign (draft stage): [marketing/nexus-fabric](marketing/nexus-fabric/README.md).

## Status

Specifications are drafts or provisional; a first development build of the software exists (see above). No engineering content is validated. PDF input is vector-PDF conversion by the separate `nf-pdf2dxf` converter: it reads one page, needs a drawing scale (1:N) or millimetres per point, and does not interpret dimensions. No geometry kernel or OCR technology has been selected.

## Documents

| Document | Purpose |
|---|---|
| [Master Plan v1.0](docs/MASTER_PLAN.md) | Product, engineering and development strategy |
| [AMOEBA-Native System Architecture](docs/specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md) | Governing architecture: lifecycle, Engineering Organism State, mechanisms, coverage, safety boundaries (draft; definition awaiting owner confirmation) |
| [AMOEBA Independent Architecture Review](docs/reviews/AMOEBA_INDEPENDENT_ARCHITECTURE_REVIEW.md) | Critical document review: weaknesses, missing mechanisms, ADR recommendations, roadmap (findings not yet applied) |
| [Decision Register](docs/architecture/DECISION_REGISTER.md) | **Start here for status:** every decision, proposal, pending owner decision and unresolved engineering question |
| [AMOEBA notes](docs/architecture/amoeba/) | State model, mechanisms, coverage model, migration of Specs 01–03; refinements: [dependency model](docs/architecture/amoeba/DEPENDENCY_MODEL.md), [investigation model](docs/architecture/amoeba/INVESTIGATION_MODEL.md), [determinism and approval](docs/architecture/amoeba/DETERMINISM_AND_APPROVAL.md), [coverage and safety](docs/architecture/amoeba/COVERAGE_AND_SAFETY_REFINEMENT.md), [record model](docs/architecture/amoeba/CONSOLIDATED_RECORD_MODEL.md), [engineering process catalogue](docs/architecture/amoeba/ENGINEERING_PROCESS_CATALOGUE.md), [diagrams](docs/architecture/amoeba/ARCHITECTURE_DIAGRAMS.md) |
| [Fabrication Engineering Review Package](docs/reviews/FABRICATION_ENGINEERING_REVIEW_PACKAGE.md) | 56 plain-language statements and rules for a qualified fabrication engineer to classify, with consequence and severity annex — **0 reviewed; all provisional** · [one-page handoff](docs/reviews/ENGINEER_REVIEW_HANDOFF.md) |
| [Reference-Case Intake Specification](docs/benchmark/reference-cases/REFERENCE_CASE_INTAKE_SPECIFICATION.md) | How an authorised factory drawing becomes a controlled reference case; six record templates; separation of truth from predictions. First case [RC-001](docs/benchmark/reference-cases/RC-001-header-oblique-branch/CASE_STATUS.md): **prepared, no material received** |
| [Evidence Levels](docs/architecture/EVIDENCE_LEVELS.md) | Five separate kinds of evidence for engineering knowledge; everything is currently EV-1 (unreviewed proposal) |
| [AMOEBA Benchmark Protocol](docs/benchmark/AMOEBA_BENCHMARK_PROTOCOL.md) | Frozen comparison protocol with arms, ablations and falsification criteria (parameters awaiting sign-off; nothing run) |
| [01 System Architecture Specification](docs/specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md) | Trust zones, services, mathematics engine, data, security, APIs, decision register |
| [02 Mathematical Engineering Specification](docs/specifications/02_MATHEMATICAL_ENGINEERING_SPECIFICATION.md) | Mathematics engine: foundations, development classes, fabrication models, precision, uncertainty, verification, release states, algorithms (**provisional** until independently reviewed) |
| [03 Drawing Intelligence and Ingestion Specification](docs/specifications/03_DRAWING_INTELLIGENCE_AND_INGESTION_SPECIFICATION.md) | Input formats, pre-processing, text and dimension reading, views, observation model, reconciliation, review workflow, output contract (draft) |
| [Specification Roadmap](docs/specifications/SPECIFICATION_ROADMAP.md) | Remaining specifications 02–10 in dependency order, with open factory requirements |
| [Architecture Decision Records](docs/architecture/) | ADR-001 to ADR-008 (*Proposed*); ADR-009A to ADR-016 with decided status |
| [Observation Schema](docs/architecture/drawing-intelligence/OBSERVATION_SCHEMA.md) · [Japanese Drawing Vocabulary](docs/architecture/drawing-intelligence/JAPANESE_DRAWING_VOCABULARY.md) | Supporting notes to Spec 03 (vocabulary is an unreviewed seed list) |
| [Dependency Licensing Review](docs/research/DEPENDENCY_LICENSING_REVIEW.md) | Initial licence research; not legal advice |
| [Mathematical Reference Cases](docs/benchmark/MATHEMATICAL_REFERENCE_CASES.md) | Computed reference values per geometry family (draft, not yet independently reproduced) |
| [Mathematical Research Register](docs/research/MATHEMATICAL_RESEARCH_REGISTER.md) | Open questions, calibration needs, sources to verify |
| [Drawing Intelligence Test Plan](docs/benchmark/DRAWING_INTELLIGENCE_TEST_PLAN.md) | Test classes, metrics and safety metrics for ingestion and interpretation |
| [Drawing Intelligence Research Register](docs/research/DRAWING_INTELLIGENCE_RESEARCH_REGISTER.md) | Technology candidates with licences, research items, standards to obtain |
| [AMOEBA Architecture Benchmark](docs/benchmark/AMOEBA_ARCHITECTURE_BENCHMARK.md) · [AMOEBA Research Note](docs/research/AMOEBA_BIOLOGICAL_AND_COMPUTATIONAL_RESEARCH.md) | Tests required before the AMOEBA ADRs can be accepted; biological caveats and computational prior art |
| [Engineering Benchmark Plan](docs/benchmark/ENGINEERING_BENCHMARK_PLAN.md) | Corpus, metrics and protocols; no data collected yet |

Next: real evidence — fabrication-engineering review (H1–H18, RT1–RT10), one authorised header-and-oblique-branch drawing taken in as reference case RC-001, factory answers, independent mathematical review. Specification 04 is not started.
