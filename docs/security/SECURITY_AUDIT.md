# Security Audit

18 November 2026 · Scope: the whole repository as implemented (four Rust crates, web client, tests). Method: manual code review by the implementing AI system, plus the automated tests named below. **This is a self-assessment, not an independent audit or penetration test, and no certification or compliance is claimed. NEXUS FABRIC is not and cannot be "hack-proof".**

## Summary

| | Count |
|---|---|
| Findings | 27 |
| Fixed and covered by a test this session | 14 |
| Open — critical | 0 known |
| Open — high | 4 |
| Open — medium | 6 |
| Open — low / informational | 3 |

"0 known critical" reflects what a self-review found. It is not evidence that none exist. The software is still **not suitable for confidential drawings** (see open high findings).

Evidence: `tests/security_smoke.sh` (52 checks), `tests/e2e_smoke.sh` (61), `tests/ui_smoke.js`, `cargo audit` (104 crates, 0 advisories, advisory database of 18 November 2026).

## Findings fixed this session

| ID | Finding | Severity | Fix | Test |
|---|---|---|---|---|
| V-01 | Broken access control: any signed-in user could read and modify every project, job, file and obligation | Critical | Per-project membership checked server-side on every endpoint; unauthorised objects answer 404; summaries and dashboards scoped | security_smoke "project isolation" (15 checks) |
| V-02 | No protection against cross-site request forgery beyond the cookie attribute; upload accepted any content type | High | State-changing API calls require the `X-NF-Request` header and a same-origin `Origin`; cookie stays SameSite=Strict | "anti-forgery" checks |
| V-03 | No sign-in throttling: unlimited password guessing | High | 5 failures per account per 15 minutes; 40 failures overall per minute; throttled attempts audited | "sign-in throttled", "throttle holds" |
| V-04 | No way to change a password or revoke sessions | High | Password change requires the current password and revokes all sessions of the account | 4 checks |
| V-05 | Accounts could not be disabled | Medium | Administrator can deactivate; sessions revoked; inactive accounts cannot sign in | 3 checks |
| V-06 | No security response headers; pages could be framed; no content security policy | Medium | CSP (`default-src 'none'; script-src 'self'` …), frame denial, no-sniff, no-referrer, API responses `no-store` | header checks; browser test runs with the policy and reports no violations |
| V-07 | Audit log could be altered without trace | High | Hash chain over entries; `/api/audit/verify` | "tampered audit entry detected" |
| V-08 | A panic while holding the database lock would disable the service for all users (lock poisoning) | Medium | Poisoned lock is recovered | Code review; hostile-input checks leave the server healthy |
| V-09 | Storage key always stored beside the data it protects | High | `NF_KEY_FILE` allows a separate location; status endpoint reports whether it is separate | "storage key is outside the data directory" |
| V-10 | 50 MB body limit applied to every endpoint | Low | 256 KB default; 50 MB only on upload | "oversized JSON body refused" |
| V-11 | Files of superseded or rejected jobs could be downloaded | Medium | Withheld (409) | e2e and security checks |
| V-12 | Tampering with stored engineering records was undetected until someone looked | High | Replay audit recomputes every record identity; mismatch sets the job to REJECTED and withholds files | "tampered engineering records detected" |
| V-13 | Blocking browser dialogs used for decisions; a script-injection would be indistinguishable | Low | Inline forms; browser test fails on any dialog | ui_smoke |
| V-14 | Stored script injection | — (verified absent) | All output is escaped; CSP as second line | ui_smoke stores `<img onerror>` and `<script>` payloads; nothing executes |

## Open findings

| ID | Finding | Severity | Why open | Interim mitigation |
|---|---|---|---|---|
| V-20 | No TLS in the server | High | Needs a decision between built-in TLS and a mandated proxy, plus certificate handling for offline factories | Bind to localhost (default) or run behind a TLS proxy; set `NF_COOKIE_SECURE=1` there |
| V-21 | Database not encrypted at rest (project names, file names, parameters, results, audit log) | High | Needs a decision on database encryption technology and its licence | Full-disk encryption on the host |
| V-22 | One encryption key for all blobs; no rotation; no KMS/HSM; key readable by the service account | High | Key-management design not written (Specification 07) | Separate key location; file mode 600 |
| V-23 | No multi-factor authentication; no account recovery; no single sign-on | High | Not implemented | Strong passwords; throttling |
| V-24 | Per-account throttling lets an attacker lock a known account out for 15 minutes | Medium | Trade-off accepted for now | Administrator can use another account; audit shows throttling |
| V-25 | Audit chain can be truncated or wholly rebuilt by someone with write access to the database file | Medium | Needs an external anchor (signed checkpoints, write-once store) | File permissions |
| V-26 | No parser sandbox exists | Medium now, **Critical the moment any parser is added** | Nothing is parsed yet: uploaded files are stored only | Release gate: no parser may be merged without an isolated worker (see checklist) |
| V-27 | Administrators can read every project; no separation of duties; no second-person rule | Medium | Role model is minimal | Few administrators; audit log |
| V-28 | Records and blobs are tamper-*evident*, not tamper-*proof*; hashes are not signed, so a consistent rewrite of a job's records and row is not detected by replay | Medium | Needs signed release manifests and an anchor | Replay detects partial tampering; release is impossible anyway |
| V-29 | No tenant separation (one installation = one organisation) | Medium | By current design | One installation per customer |
| V-30 | Replay subprocess inherits the server's environment and privileges | Low | No lower-privilege worker identity yet | It reads only the request on stdin |
| V-31 | Web client is embedded and unsigned; no subresource integrity needed as there are no third-party scripts | Informational | — | CSP restricts sources to self |
| V-32 | Dependencies (104 crates) are unevaluated beyond the advisory scan; no reproducible-build or vendoring policy | Low | Process not established | `cargo audit` in the test routine; `Cargo.lock` committed |

## Areas checked and found without issue (at the level of a code review)

Injection: all SQL uses bound parameters; the only formatted SQL fragments are built from integer ids and fixed strings (`scope()`), never from request text · Path traversal: blobs are addressed by their own SHA-256; file names are metadata only; the DXF part name is a lookup key, not a path · Deserialisation: typed, no polymorphic or executable formats · Server-side request forgery: the server makes no outbound requests · Secrets in logs: passwords and tokens are not logged; session tokens are stored hashed · Cryptography: library Argon2id, AES-256-GCM with random nonces, SHA-256; no custom algorithms · Unsafe code: none in the workspace crates.

## Not applicable yet (no such component exists)

Prompt injection and AI model security · malicious PDF/CAD parsing · computation-worker capabilities · container and network segmentation · backup encryption. Their requirements are in the threat model and release checklist so that they bind the code when it is written.
