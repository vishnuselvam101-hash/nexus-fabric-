# NEXUS FABRIC — readiness classifications

Owner decision, 20 Nov 2026: readiness is never stated with one label such as "production-ready". Five independent
classifications are used. Passing one never implies passing another. This file is updated only with evidence.

| Classification | Meaning | Status on 20 Nov 2026 | Evidence |
|---|---|---|---|
| Software verified | The automated software checks pass on a named commit | Holds for `ed4a5fb` (engine 0.4.2, 20 Nov): 263 unit and integration, 89 API, 66 security, 194 browser, 18 desktop-mode checks; i18n catalogue complete; clippy and fmt clean on the CI toolchain (1.100). The no-network part of offline_check is not re-run on the live host | Lead's own runs; `tests/`, `tests/offline_check.sh` |
| Security reviewed | Audit findings are fixed or explicitly accepted by the owner | **Does not hold.** The deeper audit passes 35 of 40 checks; five fail (V-44 twice, V-45, V-47 twice). Open high findings: V-21 database not encrypted, V-22 key management, V-23 no multi-factor sign-in or recovery | `docs/security/VULNERABILITY_REGISTER.md`, `tests/security_r6.sh` |
| Geometry verified | The independent geometry validation passes | **Not yet claimed.** The quick corpus now passes on engine 0.4.0 after the validation tool was corrected (155 cases: 93 agree, 62 correctly refused, 0 disagree; every planted fault detected; lead's own run on a clean packaged build, 20 Nov). Engine 0.4.2 (20 Nov): the FULL corpus (1390 cases: 972 agree, 418 correctly refused, 0 disagree, 0 wrongly accepted, 0 wrongly refused, 0 errors; every planted fault detected; 1049 s) has no disagreement; its only FAIL verdict is **2 product-check-not-PASS cases that are identical on the clean 0.4.1 engine, so pre-existing and not a regression**: `N-tangent-r-b90-g2e-06` (beta 90, |delta| + r within 2e-6 R of tangency: `opening_developed_length_equals_3d_arc_length` and `saddle_developed_length_equals_3d_arc_length` not PASS) and `N-aspect-branch-tiny` (`opening_area_equals_surface_area` not PASS). The engine fails closed in both (the gate verdict is not PASS, the job can never reach APPROVED, no pattern is released as verified); tracked with D4 / BE-038 (the near-tangent knee: an error bound near tangency, RB-005). The checks and the oracle were not weakened. The second computational review's findings B2-001..B2-007 are addressed in 0.4.2 (B2-001 as a review-required advisory, B2-002..B2-007 as fixes). Still noted: one comparison tolerance was relaxed from 1e-11 to 2e-9 relative and two scaled cases pass with under 6 % margin; the near-tangent D4 knee and the shallow-angle verifier floor (~0.003 deg, fail-closed via G10) remain open | `docs/backend/COMPUTATIONAL_VALIDATION_REPORT.md` section 10, `docs/backend/REVIEW_B2_COMPUTATIONAL.md` |
| Fabrication reviewed | A qualified fabrication engineer has reviewed the outputs and assumptions | **Does not hold.** 0 of 56 engineering statements reviewed; every algorithm is evidence level EV-1. Two items the fabrication engineer must decide because the specification has no rule and the engine states them as quantities only: the **minimum K-factor** (the neutral-axis position of the development; none is assumed, 0 < k <= 0.5 is only a consistency range) and the **minimum seam clearance in the flat pattern** (every cone and reducer reports `seam_gap`; the engine refuses a gap of at most 1e-9 mm and withholds a PASS of the profile gate below `seam_gap_resolution` (the larger of 1e-9 mm and twice the numerical and representation errors of the adjoining curves), but whether a narrow slit between the seam edges is acceptable for cutting is an engineering rule that does not exist yet) | `docs/project/MASTER_GOAL.md` |
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
