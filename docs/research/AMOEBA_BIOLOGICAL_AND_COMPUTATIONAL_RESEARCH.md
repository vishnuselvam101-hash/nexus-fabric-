# AMOEBA — Biological and Computational Research Note

Version 0.1 · 18 November 2026 · Companion to the [AMOEBA architecture](../specifications/AMOEBA_NATIVE_SYSTEM_ARCHITECTURE.md)

## 0. Status and limits

- **No literature was consulted for this note.** The biological summaries are textbook-level statements from the author's general knowledge, kept deliberately conservative; each must be checked by someone with the cited kind of source in hand before it is repeated in any external material. Specific papers are mentioned only as leads to verify, marked *(lead)*.
- The architecture **borrows vocabulary and, in a few places, ideas**. It does not model cells, is not validated by biology, and claims no biological or computational novelty.
- Several of the thirteen concepts are not properties of amoebae specifically; some are defined mainly for bacteria or for multicellular animals. This is stated per item so that the name "AMOEBA" does not imply otherwise.

## 1. Biological concepts — what can be said safely, and what cannot

| Concept | Conservative summary | Caveats for our use |
|---|---|---|
| **Phagocytosis** | Uptake of relatively large particles by a cell into a membrane-bound internal compartment (phagosome). A feeding mechanism in free-living amoebae; in animals carried out mainly by specialised cells | The engulfed material is subsequently degraded. Our "engulfment" preserves everything — the analogy ends at "taken in whole" |
| **Pseudopodia** | Temporary cell extensions, driven by the actin cytoskeleton, used in amoeboid movement and in surrounding particles | Our probes are scheduled tasks, not continuous shape changes; "extension and retraction" is only an image |
| **Lysosomal digestion** | Lysosomes are acidic, enzyme-containing compartments; fusion with a phagosome leads to breakdown of its contents, isolated from the rest of the cell | The useful idea is *containment of a hazardous process*. Destruction of the input is exactly what we must not do |
| **Cellular signalling** | Cells detect and respond to chemical signals from other cells and the environment. In the social amoeba *Dictyostelium discoideum*, starving cells signal with cyclic AMP during aggregation | Our signals are data events; no analogy to pathways is intended |
| **Chemotaxis** | Directed movement in response to a chemical gradient; *Dictyostelium* cells move toward cAMP during aggregation and toward folate associated with bacterial prey | Our "gradient" is a computed priority score; the hazard of chasing misleading gradients has a real counterpart in scheduling |
| **Immune response** | Immune systems are a feature of multicellular organisms. Single-celled amoebae have defences against pathogens but not an immune system in that sense. In *Dictyostelium* multicellular stages, cells with immune-like, phagocytic "sentinel" behaviour have been reported *(lead: Chen, Zhuchenko, Kuspa, Science, 2007)* | We use the term for anomaly detection and containment only. No claim of self/non-self discrimination or adaptive immunity |
| **Autophagy** | A conserved process in eukaryotic cells in which the cell's own components are enclosed and delivered to lysosomes (or vacuoles) for breakdown and recycling; associated with nutrient stress, among other roles | Our equivalent is cache eviction restricted to recomputable artefacts |
| **Cellular memory** | Not one defined mechanism. The phrase is used for persistent state changes such as epigenetic marks or bistable signalling states. Reports of habituation-like behaviour in single-celled organisms and slime moulds exist and their interpretation is debated; *Physarum polycephalum*, often cited, is a plasmodial slime mould, not a typical single amoeba | The weakest analogy of the set. Ours is ordinary versioned configuration and knowledge management |
| **Differentiation** | Cells acquiring distinct specialised states. In *Dictyostelium*, aggregated cells differentiate into stalk and spore cell types during fruiting-body formation | Our dispatch is deterministic and reversible; biological differentiation is neither in general |
| **Regeneration** | Regrowth or repair of lost structures — a concept mostly discussed at tissue and organism level. Some single cells (the ciliate *Stentor* is a classic example) can regenerate after damage; this is not specific to amoebae | Ours is recomputation from retained inputs |
| **Apoptosis** | A regulated form of cell death characterised in animals. Developmental cell death also occurs in *Dictyostelium* stalk cells, generally described as a non-apoptotic (vacuolar/autophagic) type | We mean orderly termination with a record; "apoptosis" is used loosely |
| **Homeostasis** | Maintenance of internal conditions within limits. Freshwater amoebae, for example, regulate water content with a contractile vacuole | General control-loop idea |
| **Quorum sensing** | Defined and best characterised in bacteria: population-density-dependent regulation through secreted signalling molecules. Density-sensing through secreted factors has also been described in *Dictyostelium* *(lead: "conditioned medium factor")* | In biology the members are near-identical and the signal measures density. Our use — thresholds over **independent** evidence — is close to the opposite requirement: identical sources should count once |

Sources to obtain for verification: a current cell-biology textbook (e.g. Alberts et al., *Molecular Biology of the Cell*); a review of *Dictyostelium discoideum* development and chemotaxis; a review of bacterial quorum sensing; reviews of autophagy and of programmed cell death nomenclature; the leads above.

## 2. Computational prior art — AMOEBA is a combination of known ideas

| Idea in AMOEBA | Established counterpart (to be cited properly after review) |
|---|---|
| One shared state on which independent processes opportunistically add partial results | **Blackboard architectures** (the Hearsay-II speech-understanding system is the classic example). AMOEBA's EOS + processes + scheduler is recognisably a blackboard system with strong typing, immutability and trust zones |
| Append-only records, state derived by replay | Event sourcing; append-only logs |
| Content-addressed immutable graph, snapshots as hash trees | Merkle DAGs as used in version-control systems and content-addressed stores |
| Recompute only what changed | Incremental build systems and dependency-tracking computation |
| Provenance graph | Provenance data models (e.g. the W3C PROV family) |
| Supervision, controlled termination, "let it fail" isolation | Actor/supervision-tree runtimes |
| Budgets, back-pressure, self-regulation | Control theory; "autonomic computing" proposals (monitor–analyse–plan–execute loops) |
| Priority-directed exploration | Best-first / agenda-based search; active learning (query where uncertain) |
| Anomaly detection framed immunologically | The research field of artificial immune systems |
| Thresholds over multiple sources | N-version programming and voting in fault-tolerant systems — including the well-known finding that independently developed versions fail in **correlated** ways more than independence would predict *(lead: Knight & Leveson, 1986)*, which is the basis for requiring measured independence |
| Staged promotion of algorithms | Release channels, canarying, qualification in regulated software |
| Biologically inspired computing using amoeboid organisms | Existing research on slime-mould (*Physarum*) inspired optimisation and on "amoeba-based" computing |

**Name collision.** "Amoeba" is also the name of an unrelated distributed operating system developed in academic research (Tanenbaum and colleagues). A name/trademark and prior-art check belongs in Document 09.

## 3. Hypotheses introduced by AMOEBA (unverified)

| ID | Hypothesis | Test |
|---|---|---|
| H1–H4 | Benefits of signal-directed exploration, conflict-directed investigation, priority-ordered review and tenant memory | Benchmark B-4 |
| H5 | Adaptivity can be made deterministic, bounded and safe at acceptable cost | B-1…B-3 |
| H6 | Source-denominated coverage corresponds to what engineers regard as "inspected" | B-5 |
| H7 | Useful independence credit exists between available recognisers | B-6 |
| H8 | A single EOS scales to large packages | B-7 |

## 4. Risks specific to the biological framing

1. **Anthropomorphic/biomorphic over-reading:** terms like "immune" or "memory" suggest capabilities (self-healing, learning) that the mechanisms do not have. Mitigation: plain engineering names in code and in customer-facing certificates; biological names only in architecture discussion.
2. **Metaphor-driven design:** adding a mechanism because the biology has one. Every mechanism here was required to have a purpose that stands without the metaphor; reviewers should challenge any that do not.
3. **Marketing claims:** no external statement that the product is "biologically validated", "self-evolving" or similar is supported by this work.

## 5. Open research items

| ID | Item |
|---|---|
| AR-01 | Verify every row of §1 against sources; correct or delete |
| AR-02 | Proper citations for §2; survey of blackboard-system lessons (control problem, scheduling knowledge) |
| AR-03 | Deterministic scheduling with parallel workers: design and proof obligations |
| AR-04 | Priority function design; no-starvation guarantee |
| AR-05 | Formal model of the release state machine and lanes for model checking |
| AR-06 | Ink-component enumeration: algorithms, thresholds, cost |
| AR-07 | Measuring dependence between recognisers and between verifiers; how to convert measured correlation into threshold rules |
| AR-08 | Governance and legal basis for tenant memory |
| AR-09 | Prior-art and name search (Document 09) |
