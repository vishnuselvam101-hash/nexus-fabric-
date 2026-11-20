# Desktop mode: where the storage key lives — assessment and plan

Date: 20 Nov 2026. Status: **assessment only; nothing in the product was changed.** Finding BH-017 of the bug hunt
(`docs/qa/BUG_HUNT_R7.md`); related open findings V-21 (database not encrypted) and V-22 (key management) in
`VULNERABILITY_REGISTER.md`. Development build. Not validated. This document is not a security certification.

## 1. What the product does today (read from the code)

- Stored files (uploaded documents and generated DXF files) are encrypted with AES-GCM using a 32-byte storage key
  (`crates/nf-server/src/storage.rs`). The same key authenticates the anchor of the audit chain.
- **The SQLite database is not encrypted** (V-21, open). Requests, results, records, user names and password hashes
  (Argon2) are in plain SQLite.
- Server mode: the key path is `NF_KEY_FILE`; if unset, `storage.key` inside the data folder. The health endpoint
  reports whether the key is outside the data folder. The deployment guide places it under `/etc`.
- Desktop mode (`nf-server --desktop`): the key is always `<data folder>/storage.key`
  (`crates/nf-server/src/desktop.rs`). The data folder is created with owner-only permissions where the operating system
  supports it; on Windows it relies on the profile's default access rules.
- If the key file is missing, damaged or replaced, the program refuses to start and creates no new key (stress level 6,
  four key-fault scenarios, all refused).

## 2. What the present arrangement protects against, and what it does not

| Situation | Protected today? |
|---|---|
| Another user account on the same PC reading the folder | Partly: file permissions only (Linux 0700; Windows profile defaults). The encryption adds nothing here, because key and data share those permissions. |
| A copy of the whole data folder leaving the PC (backup, USB stick, cloud-sync folder, stolen disk without disk encryption) | **No.** The copy contains the key next to the encrypted files, and the database is plain. |
| A copy of only the `blobs` folder | Yes: the files are unreadable without the key. |
| Malware running as the same user | No, and no key location on the same machine can fully protect against it. |
| Tampering with stored files | Detected (authenticated encryption; replay and audit verification). |

So, in desktop mode, the file encryption is close to decorative against the most likely loss: the folder being copied.
That is an accurate description of BH-017, and it is why the interface and documents must not call the stored data
"encrypted" without this qualification.

## 3. Options

| Option | What it does | Works offline | Cost and risk |
|---|---|---|---|
| A. Leave as is, document it, recommend full-disk encryption (BitLocker, FileVault, LUKS) | Nothing changes; the operating system protects a stolen disk | yes | No engineering cost. Does not help when the folder is copied from a running PC. |
| B. Wrap the key with the operating system's per-user protection: Windows DPAPI; macOS Keychain; Linux Secret Service, with a clear fallback | A copied folder cannot be decrypted on another machine or by another user | yes (all three are local services) | Moderate. Linux has no guaranteed secret service on a headless or minimal desktop, so a documented fallback is needed. **Restoring a backup on another PC needs an explicit recovery path**, or the data is lost. Cannot be proven on real Windows from this build machine (wine does not implement DPAPI faithfully). |
| C. Passphrase chosen by the user, key derived with Argon2 at start | Protects a copied folder everywhere, no operating-system dependency | yes | The user must type it at every start; a forgotten passphrase loses the stored files unless a recovery key is kept. |
| D. B or C **plus** encrypting the database (V-21) | Closes the real gap: today the database is readable even without the key | yes | Largest change: a different SQLite build or page-level encryption, migration of existing data, performance check. |

## 4. Recommendation

1. Now: option A as an honest statement. Say in the install guide and the first-run screen that desktop data is
   protected by the operating-system account and disk encryption, not by the program, and recommend disk encryption.
2. Next: option B, with a **recovery key** shown once at first run (printed or saved by the user) so that a backup can be
   restored on another machine; fallback to option C where no operating-system store exists. Not started.
3. Then: option D for the database, decided together with V-21, because protecting the key while the database stays
   plain protects little.

## 5. Conditions before any of B, C or D is built

- A written migration: existing installations keep working; the old key file is removed only after the wrapped key has
  been read back successfully; an interrupted migration can be resumed; nothing is migrated silently.
- A recovery procedure tested end to end, including "restore a backup on a new PC".
- Key rotation defined (what is re-encrypted, how long it takes on a 2 GB store, what happens if interrupted).
- Tests for: store unavailable, wrong user, moved folder, damaged wrapped key, downgrade to an older program version.
- One run on a real Windows PC and a real macOS machine by a person; this container cannot provide that evidence.

## 6. Owner decisions (20 Nov 2026)

1. **Wording: approved.** The install guide and the first-run screen state that desktop mode does not encrypt the
   application database or the whole data folder, that stored files have their own encryption but this does not make
   the desktop data set encrypted, that security depends on the computer and its operating environment, and recommend
   disk encryption, individual accounts, restricted access to the data folder and secure backups. The wording must not
   suggest that an operating-system account by itself encrypts the database. Disk encryption is not presented as a
   substitute for application-level encryption: it protects a stolen device, not a copy taken while the disk is unlocked.
2. **Threat model: a copied data folder is in scope.** Application-level database encryption, operating-system
   protected keys, a recovery mechanism, key rotation and tested backup and restore are acceptance conditions for
   unrestricted desktop deployment (see `docs/project/READINESS.md`). Until then the desktop product is "restricted
   deployment".
3. **Recovery key: controlled by the customer organisation**, held by an authorised administrator, not by an individual
   employee alone. A small site without an IT department names an owner and a second custodian. Requirements: the only
   recovery key is never stored beside the encrypted data; no recovery key is built into the program; recovery requires
   authorisation and leaves an audit entry; recovery is tested on a separate machine; the procedure covers the
   administrator leaving the company; the vendor has no access by default.

This document remains the record of the current risk. It is kept separate from any implementation; the implementation
is to be designed from a written threat model, with migration, backup and recovery defined before code is written
(section 5).
