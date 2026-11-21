# AMOEBA Architecture Diagrams (updated)

Version 0.2 · 18 November 2026 · Reflects the refinement notes; proposed, not accepted in every detail (see the [Decision Register](../DECISION_REGISTER.md)). Diagrams are Mermaid and render on GitHub.

## 1. Governance layers

```mermaid
flowchart TB
  subgraph Z4["Z4 Human authority"]
    REV[Review: confirm / correct / decide]
    APP[Approval over release-closure hash]
  end
  subgraph Z3["Z3 Independent verification and audit"]
    VER[verify.*]
    AUD[audit A1-A5]
    COV[coverage / closure / gates / residual statement]
  end
  subgraph Z2["Z2 Deterministic engineering"]
    BLD[build.* via UEM]
    REC[reconcile / propagate / investigate]
  end
  subgraph Z1["Z1 Interpretation - candidates only"]
    DIG[digest.*]
    EXP[explore / relate probes]
  end
  subgraph Z0["Z0 Membrane"]
    MEM[admit / reject]
    ENG[engulf: snapshot + enumeration]
  end
  subgraph ZR["ZR Runtime"]
    SCH[Scheduler - pure function of state]
    EXE[Executor - sandboxes, safety aborts]
    GATE[Commit gate + append store]
  end
  EOS[(Engineering Organism State - production records)]
  OVL[(Lane overlays - experimental / candidate)]
  MEM --> ENG --> DIG --> EXP --> REC --> REV --> BLD --> VER --> COV --> APP
  VER -- findings --> REC
  AUD -- findings --> REC
  REC -- obligations --> BLD
  REC -- directed probes --> EXP
  SCH --> EXE --> GATE --> EOS
  EOS --> SCH
  EXE -.non-production.-> OVL
```

## 2. Round model

```mermaid
sequenceDiagram
  participant S as Scheduler
  participant W as Workers (parallel)
  participant G as Commit gate
  participant E as EOS
  participant H as Human inputs
  E->>S: frozen state v_k (primary data only)
  S->>W: work set W_k + budgets in work units
  W->>G: outputs (each computed from v_k only)
  G->>E: validate, sort canonically, append -> v_k+1
  H->>E: queued inputs merged at boundary (sequence number)
  E->>S: v_k+1, derived signals
```

## 3. Dependency propagation (header with branch, Example 1)

```mermaid
flowchart LR
  DH[Dimension D_H] --> RH[R_H contact]
  CONV[Sheet convention: surface reference] --> DH
  CONV --> DB[Dimension D_B]
  RH --> SH[Surface S_H]
  DB --> SB[Surface S_B]
  SH --> G((Intersection curve))
  SB --> G
  G --> PB[Pattern P_B] --> XB[DXF branch]
  G --> PO[Opening on P_H]
  RH --> PH[Pattern P_H shell] --> XH[DXF header]
  PO --> PH
  G -. interface I_HB .- B[Part: Branch]
  SH -. interface I_HB2 .- B2[Part: Branch 2]
  SH -. interface I_HF .- F[Part: Flange]
  G --> W[Weld prep along curve]
```

Solid arrows: derivation (downstream invalidation, mandatory). Dotted: interfaces (lateral obligations). A change at `CONV` reaches `D_B` by common cause.

## 4. Approval and release closure

```mermaid
flowchart LR
  OUT[Release outputs of part P] --> CL[Release closure C of P]
  CL --> H[release_closure_hash]
  CL --> EV[Gate evaluation E of P]
  H --> AP[Approval: hash + E + signature]
  EV --> AP
  U[Unrelated additions] -. no effect .-> AP
  CH[Change / finding / obligation on a closure member or interface-defining record] == invalidates ==> AP
```

## 5. Investigation loop

```mermaid
flowchart LR
  F[Finding] --> I[Investigation: hypotheses incl. UNKNOWN_CAUSE]
  I --> T[Select test: catalogue order or expected risk reduction per cost]
  T --> X[Record expected outcomes] --> R[Run probe] --> A[Record actual outcome and cost]
  A --> I
  I --> D{Decisive rule?}
  D -- yes --> C[Closed: explained by rule]
  D -- no --> HU[Human decision with dossier]
  I --> P[propagate.obligations]
  I --> S[Stop: fixed point / budget / low yield -> residual uncertainty statement]
```
