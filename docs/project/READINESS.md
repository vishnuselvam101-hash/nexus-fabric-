# NEXUS FABRIC — readiness classifications

Owner decision, 20 Nov 2026: readiness is never stated with one label such as "production-ready". Five independent
classifications are used. Passing one never implies passing another. This file is updated only with evidence.

| Classification | Meaning | Status on 20 Nov 2026 | Evidence |
|---|---|---|---|
| Software verified | The automated software checks pass on a named commit | Holds for the bug-fix baseline `cd452de`: 157 unit and integration, 89 API, 66 security, 183 browser, 18 desktop-mode checks; API, browser and desktop suites also pass with no network | Lead's own runs; `tests/`, `tests/offline_check.sh` |
| Security reviewed | Audit findings are fixed or explicitly accepted by the owner | **Does not hold.** The deeper audit passes 35 of 40 checks; five fail (V-44 twice, V-45, V-47 twice). Open high findings: V-21 database not encrypted, V-22 key management, V-23 no multi-factor sign-in or recovery | `docs/security/VULNERABILITY_REGISTER.md`, `tests/security_r6.sh` |
| Geometry verified | The independent geometry validation passes | **Not yet claimed.** The quick corpus now passes on engine 0.4.0 after the validation tool was corrected (155 cases: 93 agree, 62 correctly refused, 0 disagree; every planted fault detected; lead's own run on a clean packaged build, 20 Nov). Still open before this can hold: the full corpus has not been re-run; the second computational review (8/10) has an open high finding, B2-001 (the radius rule accepts sizes no wall can produce); one comparison tolerance was relaxed from 1e-11 to 2e-9 relative and two scaled cases pass with under 6 % margin | `docs/backend/COMPUTATIONAL_VALIDATION_REPORT.md` section 10, `docs/backend/REVIEW_B2_COMPUTATIONAL.md` |
| Fabrication reviewed | A qualified fabrication engineer has reviewed the outputs and assumptions | **Does not hold.** 0 of 56 engineering statements reviewed; every algorithm is evidence level EV-1 | `docs/project/MASTER_GOAL.md` |
| Manufacturing approved | The approval workflow exists and has been completed for a result | **Does not hold and cannot.** Engineer approval is not implemented; no result can reach APPROVED; every DXF is marked NOT RELEASED | Gate G16 |

Three approval gates follow from this and are independent: software verification, independent geometry validation,
fabrication-engineer acceptance. No wording anywhere (interface, documents, films, installers) may let the first stand
in for the other two.

## Desktop product: two release classes (owner decision, 20 Nov 2026)

| Class | Conditions |
|---|---|
| **Current security baseline — restricted deployment** | Honest statement that the program does not encrypt the database or the whole data folder; operating-system disk encryption required; individual operating-system accounts with restricted access to the data folder; documented backup and recovery; no claim of complete application-level encryption |
| **Production security target — required before unrestricted deployment** | Database encrypted at rest; encryption keys protected by the operating system; recovery key controlled by the customer organisation; defined key rotation; secure backup and restore; tests against a copied data folder |

The threat "the data folder is copied off the PC" is inside the threat model. Details and the plan:
`docs/security/DESKTOP_KEY_ASSESSMENT.md`.
