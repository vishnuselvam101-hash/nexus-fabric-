# Incident Response Plan

18 November 2026 · Draft for a development-stage product. Names, contacts and time limits are **unassigned**: an organisation must fill them in; nothing here has been exercised.

## What counts as an incident

Suspected unauthorised access to drawings or engineering records · integrity failure (audit chain broken, replay mismatch, blob authentication failure) · credential or key compromise · a result used for manufacturing without release · malware in an uploaded file · loss of data or key.

## Signals available in the software today

| Signal | Where |
|---|---|
| Failed and throttled sign-ins | Audit log actions `login_failed`, `login_throttled` |
| Audit chain broken | `/api/audit/verify`; Administration page |
| Replay mismatch; records failing identity check | Job page; job state becomes REJECTED; audit `replay_audit … MISMATCH` |
| Blob fails authentication or hash | Download error |
| Rejected uploads | Audit `upload_rejected` |
| Privileged changes | Audit `user_created`, `user_deactivated`, `member_added`, `password_changed` |

There is no alerting: someone must look. Automated alerts are not implemented.

## Steps

1. **Contain.** Deactivate affected accounts (revokes sessions). If the key or host is suspected: stop the service; preserve the data directory and key file read-only.
2. **Preserve evidence.** Copy the database, blobs and server output before any repair. Run the audit verification and record the result.
3. **Assess.** Which projects, which drawings, which jobs? Use the audit log by user and time; re-run replay audits on jobs in the affected period.
4. **Engineering impact.** Any job whose records fail verification is REJECTED; identify every file exported from it (audit `dxf_export`) and notify whoever received it. Treat exported files from the affected period as unverified.
5. **Eradicate and recover.** Rebuild from a known-good binary; rotate the storage key (procedure not yet implemented — until then, re-encrypt by export and re-import); reset passwords.
6. **Notify.** Customers whose drawings may be affected, according to contract and law — legal advice required; not defined here.
7. **Learn.** Add a regression test; update the vulnerability register and threat model.

## Gaps

No backup/restore tooling · no key rotation · no alerting · no defined contacts, severity levels or notification deadlines · plan never exercised.
