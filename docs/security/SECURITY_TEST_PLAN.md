# Security Test Plan

18 November 2026

## Automated today

| Suite | What it attacks | Run |
|---|---|---|
| `tests/security_smoke.sh` (52 checks) | Missing anti-forgery header; cross-origin requests; headers and cookie attributes; fifteen cross-project access attempts (read, upload, calculate, replay, correct, resolve, summaries, self-granting membership); path traversal in file and part names; SQL metacharacters; malformed and oversized bodies; executable upload under an innocent name; plaintext search of blob store and database; key location and permissions; **tampering with the audit log, engineering records and a stored blob followed by detection**; password change and session revocation; deactivation; throttling; forged cookie | `bash tests/security_smoke.sh` |
| `tests/e2e_smoke.sh` (61) | Authentication, roles, membrane, withheld files | `bash tests/e2e_smoke.sh` |
| `tests/ui_smoke.js` | Stored script-injection payloads must not execute; no blocking dialogs; content security policy produces no violations | see Build and Test Guide |
| Unit tests | Authenticated encryption round trip and tamper detection; password hashing; membrane | `cargo test --workspace` |
| `cargo audit` | Known advisories in dependencies | `cargo audit` |

All run against a temporary server with synthetic data. None may be pointed at a production system.

## Not yet automated (gaps)

| Area | Needed |
|---|---|
| Static analysis beyond the compiler | `cargo clippy` with a security-oriented lint set; `cargo deny` for licences and sources |
| Secret scanning | Repository scan in the commit routine |
| Dynamic scanning | A web scanner run against the test server |
| Fuzzing | DXF reader (`nf-dxf::read`) and request deserialisation now; every parser before it is merged |
| Load and denial-of-service | Concurrent sign-ins, large job bursts, slow requests |
| Session and time-based tests | Expiry, clock changes |
| TLS configuration, database encryption, backup restore | When those exist |
| Parser sandbox escape, worker capability, prompt-injection, lane-isolation tests | When those components exist — tests are a merge condition |

## Independent testing

No independent penetration test or audit has been performed. One is required before any deployment with confidential data. Its findings go into the vulnerability register.
