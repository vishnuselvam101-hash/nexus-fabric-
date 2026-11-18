# NEXUS FABRIC

Engineering Drawing-to-Manufacturing Geometry System: converts engineering drawings (PDF, images, scans, CAD) into mathematically verified, manufacturing-ready DXF, with traceable engineering decisions and measurable accuracy.

AI interprets the drawing; deterministic mathematical engines calculate; independent verification challenges the result; an authorised engineer releases it.

## Status

Specification phase. No application code yet, and no technology or library is committed.

## Documents

| Document | Purpose |
|---|---|
| [Master Plan v1.0](docs/MASTER_PLAN.md) | Product, engineering and development strategy |
| [01 System Architecture Specification](docs/specifications/01_SYSTEM_ARCHITECTURE_SPECIFICATION.md) | Trust zones, services, mathematics engine, data, security, APIs, decision register |
| [02 Mathematical Engineering Specification](docs/specifications/02_MATHEMATICAL_ENGINEERING_SPECIFICATION.md) | Mathematics engine: foundations, development classes, fabrication models, precision, uncertainty, verification, release states, algorithms (**provisional** until independently reviewed) |
| [03 Drawing Intelligence and Ingestion Specification](docs/specifications/03_DRAWING_INTELLIGENCE_AND_INGESTION_SPECIFICATION.md) | Input formats, pre-processing, text and dimension reading, views, observation model, reconciliation, review workflow, output contract (draft) |
| [Specification Roadmap](docs/specifications/SPECIFICATION_ROADMAP.md) | Remaining specifications 02–10 in dependency order, with open factory requirements |
| [Architecture Decision Records](docs/architecture/) | ADR-001 to ADR-008 (all *Proposed*) |
| [Observation Schema](docs/architecture/drawing-intelligence/OBSERVATION_SCHEMA.md) · [Japanese Drawing Vocabulary](docs/architecture/drawing-intelligence/JAPANESE_DRAWING_VOCABULARY.md) | Supporting notes to Spec 03 (vocabulary is an unreviewed seed list) |
| [Dependency Licensing Review](docs/research/DEPENDENCY_LICENSING_REVIEW.md) | Initial licence research; not legal advice |
| [Mathematical Reference Cases](docs/benchmark/MATHEMATICAL_REFERENCE_CASES.md) | Computed reference values per geometry family (draft, not yet independently reproduced) |
| [Mathematical Research Register](docs/research/MATHEMATICAL_RESEARCH_REGISTER.md) | Open questions, calibration needs, sources to verify |
| [Drawing Intelligence Test Plan](docs/benchmark/DRAWING_INTELLIGENCE_TEST_PLAN.md) | Test classes, metrics and safety metrics for ingestion and interpretation |
| [Drawing Intelligence Research Register](docs/research/DRAWING_INTELLIGENCE_RESEARCH_REGISTER.md) | Technology candidates with licences, research items, standards to obtain |
| [Engineering Benchmark Plan](docs/benchmark/ENGINEERING_BENCHMARK_PLAN.md) | Corpus, metrics and protocols; no data collected yet |

Next: independent review of Specification 02; then 04 Geometry Reconstruction and Fabrication Specification.
