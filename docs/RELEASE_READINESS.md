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
| No unresolved mandatory scrutiny obligations (G18) | Mechanism implemented for parameter corrections; gate never passes because the rule table is provisional and unreviewed |
| Self-consistency audits (G19) | Separate-process replay (same machine and build); A1–A4 not implemented |
| Deterministic replay before release | Separate process, same machine and build |
| Authorised engineer approval (G16) | Not implemented |
| Explicit uncertainty and calibration handling | Missing calibration forces REVIEW REQUIRED (gate G06); no uncertainty analysis |
| Automated-reading confidence never used as approval | Holds trivially: no automated-reading component exists; gate function has no such input |

## Security gaps that block use with confidential data

Full list: [Vulnerability Register](security/VULNERABILITY_REGISTER.md); conditions: [Security Release Checklist](security/SECURITY_RELEASE_CHECKLIST.md). Fourteen findings were fixed and tested in session 3. Still open:

| Gap | Risk |
|---|---|
| No TLS in the server (V-20) | Credentials and drawings in clear text unless a proxy is added |
| Database not encrypted at rest (V-21) | Project names, file names, parameters, results and audit log readable from the file |
| One key for all blobs, no rotation, no key service (V-22) | Key compromise exposes all stored files |
| No multi-factor authentication, recovery or single sign-on (V-23) | Account takeover by password alone |
| Audit chain not externally anchored; hashes unsigned (V-25, V-28) | A file-level insider can rewrite consistently |
| No parser sandbox (V-26) | Acceptable only while nothing is parsed; blocking condition for any parser |
| No backup/restore tooling; no alerting | Data loss; incidents noticed late |
| No independent security review or penetration test | Unknown defects — this is a self-assessment |

## Engineering gaps that block use on real work

No drawing interpretation; parameters are typed in and unlinked to any source · reference-surface radii must be supplied by the user; thickness and allowances are not modelled · error figures are estimates · four part families only, with several refused sub-cases · DXF acceptance by CAM untested; units not machine-declared · no kerf, bend lines, marking, inner profiles · no plate division.

## Path to internal validation

1. Engineer review (H1–H18, RT1–RT10) and reference case RC-001.
2. Independent review of the implemented Spec 02 models; second-person recomputation of reference values.
3. Factory answers: CAM system and DXF requirements; dimensioning convention; process parameters.
4. Close the security gaps above.
5. Implement obligations, audits, independent replay, then approval.

Only after these can an internal validation campaign begin; customer beta follows that.
