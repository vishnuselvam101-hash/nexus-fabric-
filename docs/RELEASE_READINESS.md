# Release Readiness

Updated 18 November 2026

## Verdict

**Not ready for any use on real engineering work. Not ready for customer beta. Suitable only for internal development and demonstration with non-confidential data.**

| Question | Answer |
|---|---|
| Can the application be launched and used? | Yes — single binary, browser client |
| Can it produce a manufacturing release? | No. `APPROVED` is unreachable by design in this build |
| Has any engineering content been validated? | No. 0/56 dependency statements reviewed; no mathematical model independently reviewed; no process parameter calibrated; no factory drawing received |
| Is it safe for confidential drawings? | No — see security gaps |

## Mandatory release conditions (from the decision register) and their state

| Condition | State |
|---|---|
| Independent engineering verification (G10) | Partial: independent-formulation checks only; no separate implementation, no kernel cross-check |
| No unresolved mandatory scrutiny obligations (G18) | Not implemented → gate reports NOT_IMPLEMENTED |
| Self-consistency audits (G19) | Same-process replay only; A1–A4 and independent instance not implemented |
| Deterministic replay before release | Same-process only |
| Authorised engineer approval (G16) | Not implemented |
| Explicit uncertainty and calibration handling | Missing calibration forces REVIEW REQUIRED (gate G06); no uncertainty analysis |
| AI confidence never used as approval | Holds trivially: no AI component exists; gate function has no such input |

## Security gaps that block use with confidential data

| Gap | Risk |
|---|---|
| No TLS in the server | Credentials and drawings in clear text unless a proxy is added; session cookie is not marked Secure |
| Database not encrypted at rest | Project names, file names, parameters, results and audit log readable from the file |
| Single local key file, no rotation, no KMS | Key compromise exposes all blobs |
| No login rate limiting, lockout, MFA or password change | Password guessing; no recovery path |
| No per-project permissions, single tenant | Every user can read every project |
| Audit log not tamper-evident | An administrator with file access can alter it |
| No backup/restore tooling | Data loss |
| No sandbox | Acceptable only because nothing is parsed; becomes a blocker the moment a parser is added |
| No independent security review or penetration test | Unknown defects |

## Engineering gaps that block use on real work

No drawing interpretation; parameters are typed in and unlinked to any source · reference-surface radii must be supplied by the user; thickness and allowances are not modelled · error figures are estimates · four part families only, with several refused sub-cases · DXF acceptance by CAM untested; units not machine-declared · no kerf, bend lines, marking, inner profiles · no plate division.

## Path to internal validation

1. Engineer review (H1–H18, RT1–RT10) and reference case RC-001.
2. Independent review of the implemented Spec 02 models; second-person recomputation of reference values.
3. Factory answers: CAM system and DXF requirements; dimensioning convention; process parameters.
4. Close the security gaps above.
5. Implement obligations, audits, independent replay, then approval.

Only after these can an internal validation campaign begin; customer beta follows that.
