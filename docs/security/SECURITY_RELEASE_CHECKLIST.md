# Security Release Checklist

18 November 2026 · A release is **not security-ready** unless every "Required" line is true. Current state in the right column.

| # | Required for any deployment holding confidential data | Now |
|---|---|---|
| 1 | No open Critical finding; every High finding fixed or accepted in writing by a named owner with a date | ✗ four High open, unowned |
| 2 | Transport encrypted end to end (built-in TLS or mandated proxy), Secure cookies | ✗ |
| 3 | Data at rest encrypted including the database; keys separate from data; rotation procedure tested | ✗ |
| 4 | Multi-factor authentication for administrators and approvers | ✗ |
| 5 | `security_smoke`, `e2e_smoke`, `ui_smoke`, unit tests and `cargo audit` all pass on the release build | ✓ (development build) |
| 6 | Independent penetration test completed; findings registered | ✗ |
| 7 | Backup and restore tested; incident response plan exercised | ✗ |
| 8 | Third-party dependency evaluation records complete | ✗ |

| # | Merge conditions that bind future code | Now |
|---|---|---|
| M1 | **No file parser (PDF, image, CAD, STEP) is merged unless it runs in a separate, network-less, resource-limited process per file, with fuzz tests and malformed-file tests** | No parser exists |
| M2 | No automated-reading/OCR component is merged with write access to authoritative records, with tools, or with network access; document text is treated as data, never as instructions; tests include injected instructions in drawing text | None exists |
| M3 | No approval workflow is merged without per-user signing over the release-closure hash, obligation and audit gates, and separation of preparer and approver | Not built |
| M4 | No external service call is added without the tenant's recorded authorisation and an audit entry | None exists |
| M5 | Every new endpoint has a membership check and a negative test | Enforced for current endpoints |
| M6 | Every new view escapes output; no inline scripts; CSP unchanged | Holds |

Wording rule: never describe the product as hack-proof, unhackable, certified or compliant without the corresponding evidence.
