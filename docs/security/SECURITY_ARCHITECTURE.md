# Security Architecture (as implemented)

18 November 2026 · Describes the running code; planned controls are in the specifications and the threat model.

## Components and boundaries

```
Browser ──HTTP──► nf-server (one process) ──► SQLite file (plaintext)
  cookie: HttpOnly, SameSite=Strict             ├─► blobs/<sha256>.bin  (AES-256-GCM)
  header: X-NF-Request                          ├─► storage key file    (mode 600; NF_KEY_FILE)
                                                └─► child: nf-server replay-check (stdin request → stdout hashes)
```

## Controls

| Area | Implementation |
|---|---|
| Authentication | Username + password; Argon2id with per-user 16-byte random salt; constant-time comparison; unknown users take the same code path |
| Sessions | 32 random bytes; only the SHA-256 is stored; 12-hour lifetime; revoked on logout, password change and deactivation |
| Throttling | Failed sign-ins recorded; 5 per account / 15 min, 40 overall / min |
| Authorisation | Roles (admin, engineer, viewer) checked per action; project membership checked per object; non-members receive 404 |
| Anti-forgery | Middleware on every state-changing `/api/` request: required header, same-origin check |
| Response headers | Content-Security-Policy, X-Frame-Options, X-Content-Type-Options, Referrer-Policy, Permissions-Policy, COOP; `no-store` on API |
| Input | Typed JSON deserialisation; numeric range checks in the maths engine; body limits (256 KB / 50 MB upload) |
| Upload membrane | Type from content signature; size limit; file name treated as untrusted metadata; no parsing, no execution |
| Encryption at rest | Blobs: AES-256-GCM, random 96-bit nonce per blob, content hash verified after decryption. Database: none |
| Key handling | One 256-bit key generated on first start; location configurable; never logged or returned by any endpoint |
| Engineering integrity | Records are immutable and content-addressed; corrections create new jobs; release closure hash; replay in a separate process recomputes hashes and re-verifies stored records; mismatch → REJECTED and files withheld |
| Audit | Append-only table with hash chain; verification endpoint; no passwords, tokens or file content |
| Availability | Lock-poison recovery; bounded computations; no outbound network calls |
| Supply chain | `Cargo.lock`; `cargo audit`; no third-party front-end code |

## Deliberate limits of this build

No TLS, database encryption, MFA, key rotation, tenant separation, sandbox or backup tooling (see the vulnerability register). `APPROVED` is unreachable, which removes release-forgery as a present risk and makes it the main risk of future work.

## Configuration

| Variable | Security meaning |
|---|---|
| `NF_LISTEN` | Default `127.0.0.1:8787`. Binding to another address without a TLS proxy exposes credentials |
| `NF_KEY_FILE` | Put the key on separate, access-controlled storage |
| `NF_COOKIE_SECURE=1` | Set when served through TLS |
| `NF_DATA_DIR` | Restrict to the service account |
