# Threat Model

18 November 2026 · Method: STRIDE per trust boundary · Covers what exists and, marked *(future)*, components planned in the specifications.

## Assets

| Asset | Why it matters |
|---|---|
| A1 Customer drawings and CAD files | Confidential third-party intellectual property |
| A2 Engineering records (dimensions, parameters, calculations, evidence) | Silent alteration leads to wrong parts |
| A3 Released manufacturing files (DXF) and approvals *(approval is future)* | Direct manufacturing consequence |
| A4 Credentials, sessions, storage key | Gateway to everything else |
| A5 Audit history | Accountability; detection |
| A6 Availability of the service | Factory work stops |
| A7 Algorithms, rule tables, models *(models are future)* | Integrity of every result; proprietary |

## Trust boundaries

B1 browser ↔ server · B2 server ↔ storage (database, blobs, key file) · B3 server ↔ replay child process · B4 uploaded file ↔ any parser *(future)* · B5 trusted core ↔ AI/OCR workers *(future)* · B6 experimental ↔ production lanes *(future)* · B7 installation ↔ outside network (updates, optional external AI) *(future)*

## Actors

External attacker (network) · malicious authenticated user · compromised user account · insider with file-system access · administrator (malicious or mistaken) · supplier of a malicious file · compromised dependency or model · compromised worker.

## Threats and mitigations

| # | Boundary | STRIDE | Threat | Mitigation in place | Residual risk |
|---|---|---|---|---|---|
| T1 | B1 | S | Password guessing, credential stuffing | Argon2id; throttling; audit | No MFA (V-23); account lockout nuisance (V-24) |
| T2 | B1 | S | Session theft | HttpOnly, SameSite=Strict cookie; token stored hashed; 12 h expiry; revocation on password change | No TLS built in (V-20): on an untrusted network the token can be captured |
| T3 | B1 | T | Cross-site request forgery | Required header + same-origin check + SameSite | — |
| T4 | B1 | T/E | Script injection through stored text (project names, file names, reasons) | Output escaping; CSP `script-src 'self'`; tested | New views must keep escaping; reviewed per change |
| T5 | B1 | I/E | Access to another project's data (IDOR) | Membership check on every endpoint; 404 for non-members; scoped summaries; tested | Administrators see all (V-27) |
| T6 | B1 | E | Role escalation (viewer → engineer → admin) | Role checked server-side per action; client role is display only; tested | No separation of duties |
| T7 | B1 | D | Resource exhaustion by large or many requests | Body limits; fixed-size numerical loops; bounded list sizes | No per-user request rate limit; Argon2 cost can be abused within the throttle window |
| T8 | B1 | R | User denies an action | Hash-chained audit with user and time | Chain can be rebuilt by a file-level insider (V-25) |
| T9 | B2 | I | Theft of disk, backup or database file | Blobs encrypted; key may be stored separately | Database plaintext (V-21) |
| T10 | B2 | T | Insider edits records, results or audit in the database | Record identity = content hash, recomputed by replay; audit chain; tampered job becomes REJECTED | Consistent rewrite not detected (V-28) |
| T11 | B2 | T | Blob replaced or corrupted | AES-GCM authentication; content hash re-checked on read | — |
| T12 | B2 | I | Key disclosure | File mode 600; separable location | Single key, no rotation (V-22) |
| T13 | B3 | E/T | Replay child influenced by environment | Receives only the request; result compared, never trusted to repair | Same privileges as the server (V-30) |
| T14 | B4 *(future)* | E/D | Malicious PDF/CAD exploits a parser or exhausts resources | **Today: nothing is parsed** — membership check, type sniffing, size limit, encrypted storage only | Becomes the top risk when parsing starts (V-26): requires an isolated, resource-limited, network-less worker per file and fuzzing before merge |
| T15 | B5 *(future)* | T/I | Text inside a drawing instructs an AI component (prompt injection); model output leaks data or triggers actions | Architectural rule: AI output is typed candidate data only, has no tools, no write access to authoritative records, no network (Spec 03 §9; AMOEBA safety rules) | To be enforced in code and tested when the components exist |
| T16 | B5/B7 *(future)* | T | Compromised model weights or runtime | Weights identified by hash; no code-executing model formats; licence and source recorded | Not implemented |
| T17 | B6 *(future)* | E | Experimental algorithm or rule reaches production or steers it | Lane overlays; promotion only by signed human decision | Not implemented; today every algorithm is marked not production-qualified and no result can be approved |
| T18 | B7 | T | Compromised dependency (supply chain) | Lock file committed; `cargo audit` | No vendoring, review or reproducible builds (V-32) |
| T19 | — | T | Engineering value altered between entry and output | Corrections are attestations creating new jobs; old jobs superseded and files withheld; obligations for everything dependent | Rule table unreviewed |
| T20 | — | E | Manufacturing release without verification or approval | `APPROVED` unreachable in this build; gates recomputed at read time | Approval workflow, when built, becomes a prime target: needs signing and separation of duties |
| T21 | B1 | I | Confidential data in logs or error messages | Errors are generic; audit stores identifiers and reasons, not file content | Free-text reasons could contain confidential text |
| T22 | — | D | Loss of data or key | — | No backup/restore tooling |

## The three highest priorities named by the owner

1. **Malicious PDF/CAD protection (T14).** Currently satisfied only because no file is opened. The release checklist forbids adding any parser without the isolation controls.
2. **Engineering-data integrity (T10, T19; finding V-28).** Content-addressed records, supersession and replay give tamper *evidence*; signing and an external anchor are needed for stronger guarantees.
3. **AMOEBA isolation (T15, T17).** Defined in the architecture; not yet exercised because no AI worker or experimental lane exists in code.
