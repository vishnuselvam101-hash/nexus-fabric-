# Desktop database encryption and key protection — design

**DESIGN — awaiting the owner's decision. No code is written until the owner approves this design.**

**Development build. Not validated. Outputs must not be used for manufacturing.**

Date: 21 Nov 2026 · Work plan task **2.5** (`docs/project/CLI_WORK_PLAN.md`, "2.5 Database encryption and key protection —
DESIGN ONLY") · Branch `claude/nexus-fabric-strategy-8guhxz`

Related records: V-21 and V-22 (`docs/security/VULNERABILITY_REGISTER.md:21-22`), BH-017 (`docs/qa/BUG_HUNT_R7.md:175`),
and `docs/security/DESKTOP_KEY_ASSESSMENT.md`, which stays the record of the current risk.

Citation conventions: a bare file name (`storage.rs`, `main.rs`, `desktop.rs`, `crypto.rs`, `tests.rs`) is in
`crates/nf-server/src/`; `Cargo.toml` alone means `crates/nf-server/Cargo.toml`; `<crate>-<version>/path:line` is the
published source of a third-party crate as unpacked in the local cargo registry. Line numbers are those of the working
tree read on 21 Nov 2026; other tasks may move them.

---

## 0. Summary

1. The desktop database is encrypted page by page by an encrypting build of SQLite compiled from source into the
   program. Recommended: **SQLite3 Multiple Ciphers in its SQLCipher-compatible mode**, but only if the gates of
   section 6.4 pass; otherwise SQLCipher with its LibTomCrypt back end. SQLCipher with OpenSSL is excluded by the
   offline check.
2. The storage key that exists today, a new database key and a new auxiliary secret (from which the audit-anchor key and
   the key for sign-in factor secrets are derived, so that no key serves two purposes) are kept only in wrapped form, in
   a key envelope in the data folder. They are unwrapped with a key the operating system protects for the signed-in user (Windows DPAPI,
   macOS Keychain, Linux Secret Service) or, where Linux has no Secret Service, with a passphrase.
3. A recovery key, held off the machine by the customer organisation, opens a second wrap. It is the only way to open a
   copied folder or a backup on another machine. Recovery and restore run on the console before the database is opened;
   the program then serves only administrator sign-in and the authorisation form until the recovery is authorised. That
   authorisation controls the program's own routes and the audit record only; it is not a cryptographic control, because
   whoever holds the recovery key and a copy can read the copy with another SQLCipher-compatible tool (A7).
4. Existing installations stay as they are until an administrator asks for encryption. The migration copies, verifies,
   then swaps — only when no WAL, WAL-index or journal file lies beside either database file — and can be resumed or
   rolled back after a stop at any step. It keeps the storage key unless the owner decides to rotate it (O-24); copies
   made before the migration hold that key in plaintext.
5. Not possible for any application design: protection against malware running as the same user, against an
   administrator of the PC, and on a running PC with the user signed in. A lost operating-system profile without the
   recovery key means the data and every backup are lost. No vendor key exists. A passphrase slot in a copied folder can
   be guessed offline, and a copy of the whole user profile is protected only by the account password.
6. Nothing in this document has been built or run. Every value that nobody has measured or taken from a standard is an
   open decision (section 18).

## 1. Scope

- **In scope:** desktop mode (`nf-server --desktop`, `desktop.rs:1-13`) as packaged for Windows (installer and portable
  zip, cross-built for `x86_64-pc-windows-gnu` with mingw) and Linux (`tar.gz`): `docs/PACKAGING.md:111-137`. macOS is not
  packaged (`docs/PACKAGING.md:232`); its part of this design can be neither built nor tested now.
- **Not in scope:** server mode. It shares `storage::open_storage` (`storage.rs:285`, reached through `build_app`,
  `main.rs:3556`, in both modes; desktop at `desktop.rs:214`), so the implementation must leave server mode's behaviour
  unchanged unless the owner decides otherwise (O-19): no key, no encryption, today's settings. The SQLite library
  itself does change for server mode too, because one program serves both modes (C7): server mode runs on the new
  library, so G-8 and every server-mode command of work-plan section B are run against it, and every desktop-only
  setting (key, `temp_store`, locking mode) is applied at the desktop open only. Its key and backup model is
  `deploy/README.md:64-72`. Also not in scope:
  TLS (V-20) and multi-factor sign-in (V-23, task 2.6).
- The design adds no AI component and no network use. Every mechanism is a local file, an operating-system service
  reached through local calls, or paper. Nothing updates itself and nothing reports anywhere.

## 2. Owner decisions in force (20 Nov 2026)

| # | Decision (paraphrased; source) | How this design uses it |
|---|---|---|
| D1 | Wording: desktop mode does not encrypt the database or the whole data folder; stored files have their own encryption, which does not make the data set encrypted; security depends on the computer; recommend disk encryption, individual accounts, restricted access to the data folder and protected backups; no suggestion that an operating-system account encrypts the database; disk encryption is not a substitute for application-level encryption. `DESKTOP_KEY_ASSESSMENT.md:64-69`; work plan task 2.4 | That wording stays in place until this design is built **and** tested; it changes only with evidence (`docs/project/READINESS.md:3-4`) |
| D2 | A copied data folder is inside the threat model. Application-level database encryption, keys protected by the operating system, a recovery mechanism, key rotation, and tested backup and restore are conditions for unrestricted desktop deployment; until then the desktop product is "restricted deployment". `DESKTOP_KEY_ASSESSMENT.md:70-73`; `READINESS.md:18-26` | Sections 4 to 11 |
| D3 | Recovery key controlled by the customer organisation: held by an authorised administrator, not by one employee alone; a small site names an owner and a second custodian; the only recovery key is never stored beside the encrypted data; no recovery key is built into the program; recovery needs authorisation and leaves an audit entry; recovery is tested on a separate machine; the procedure covers the administrator leaving; the vendor has no access by default. `DESKTOP_KEY_ASSESSMENT.md:74-78` | Section 8 |
| Cond. | Conditions the assessment sets before anything is built (its section 5 — not an owner decision; kept because D2 and D3 rely on them): a written migration (existing installations keep working; the old key file is removed only after the wrapped key has been read back; resumable; nothing migrated silently); recovery tested end to end, including a new PC; rotation defined; tests for store unavailable, wrong user, moved folder, damaged wrapped key and downgrade; one run on a real Windows PC and a real macOS machine by a person. `DESKTOP_KEY_ASSESSMENT.md:53-60` | Sections 9, 10, 11, 16, 17 |

## 3. What the product does today (read from the code)

| Item | Today | Source |
|---|---|---|
| Database | `<data>/nexus-fabric.sqlite`, opened by the bundled SQLite with no key: plaintext (V-21) | `storage.rs:290-292`; `Cargo.toml:17` |
| Journal mode | WAL required, start-up refuses otherwise; `synchronous = FULL`; foreign keys on | `storage.rs:297-302` |
| Checks at start | `PRAGMA quick_check`; the full check on demand | `storage.rs:167-183, 309-316` |
| Schema version | `user_version`; a newer version is refused | `storage.rs:15, 303-308, 388-389` |
| Installation identity | Random id in `meta` and in `blobs/.installation-marker`; a mismatch refuses start | `storage.rs:338-387` |
| Storage key | 32 raw bytes; created only when no data exists; a missing, unreadable, wrong-length, world-readable (Unix) or foreign key refuses start (BE-010) | `storage.rs:201-281` |
| Key location, desktop | Always `<data folder>/storage.key` | `desktop.rs:200` |
| Data folder | Windows `%LOCALAPPDATA%\NexusFabric\data`; elsewhere XDG or HOME; 0700 on Unix; Windows inherits the profile's access rules | `desktop.rs:74-127` |
| Stored files | AES-256-GCM, 12-byte random nonce, file = nonce, ciphertext, tag; named by the SHA-256 of the plaintext; written atomically; verified on every read | `crypto.rs:33-55`; `storage.rs:419-481` |
| Audit | Hash chain; newest hash, count and id authenticated with the storage key (HMAC-SHA256); the verification states what it cannot detect | `main.rs:280-293, 324-376, 1125-1190` |
| Single instance | Exclusive lock on `desktop.lock` | `desktop.rs:129-148` |
| Clean stop | `PRAGMA wal_checkpoint(TRUNCATE)` | `desktop.rs:274` |
| Setup token, desktop | Made in memory, handed over in the URL fragment, printed on the console only when no browser opened; only its SHA-256 is kept; never written to disk in desktop mode | `desktop.rs:150-174, 198-205, 251-259`; `main.rs:3568-3606` |
| Logs | Standard output and error only; no log file (BE-021 open) | e.g. `main.rs:126-141`; `docs/backend/DEFECT_REGISTER.md:40` |
| Backups | No tooling (BE-024 open); rusqlite's `backup` feature is not enabled | `docs/backend/DEFECT_REGISTER.md:43`; `Cargo.toml:17` |
| Status report | `/api/security/status` reports `database_encrypted_at_rest: false` and `key_rotation: false` | `main.rs:1244-1265` (fields at 1257-1260) |
| Installer | Per user, without administrator rights; the data folder is kept on uninstall unless the user ticks its deletion | `packaging/windows/installer.nsi:10, 30, 119-121` |

## 4. Threat model

The method of `docs/security/THREAT_MODEL.md` applies; this section adds desktop data at rest. Existing rows concerned:
T9 theft of disk, backup or database file; T12 key disclosure; T22 loss of data or key (`THREAT_MODEL.md:37, 40, 50`).

### 4.1 Attacker classes

| # | Attacker | In scope? | Protection after this design | What remains |
|---|---|---|---|---|
| A1 | Holds a copy of the data folder or of a backup (USB stick, cloud-sync folder, file share, backup medium), taken from a stopped or a running PC, **without** the recovery key; also a copy of the whole user profile or the whole disk (profile backup, PC-migration tool, disk image) | **Yes** (D2) | Database pages and stored files are ciphertext. The keys are in the folder only wrapped: once under a key the operating system keeps outside the folder for one user, once under the recovery key kept off the machine | The metadata of section 14. Copies made before migration stay readable **and hold `storage.key` in plaintext** (it sits in the data folder, `desktop.rs:200`): as long as SK is not rotated (O-24), that key opens every stored file of any later copy, including files written after migration — a cloud-sync folder's version history is such an older copy. A passphrase slot in the copy can be attacked by guessing the passphrase offline, limited only by the Argon2id cost (O-5) and the passphrase rules (O-25). A copy of the whole profile or disk also carries the operating-system store (on Windows the DPAPI master keys, section 7.4): the keys are then protected only by the strength of the account password, which can be guessed offline (as A3) |
| A2 | Another local account without administrator rights | Yes | Folder permissions (Unix 0700, `desktop.rs:120-125`; Windows profile rules, `desktop.rs:115-116`) **and** a user-scope operating-system wrap that the other account cannot open | With machine scope (O-4) only the folder permissions remain |
| A3 | Thief of a switched-off PC without disk encryption | Yes, partly | The operating-system wrap opens only with the account's credentials | The operating-system stores protect their secrets with the account password: an account without a password, an automatic sign-in without one, or a guessable password gives no protection against someone holding the disk. Disk encryption stays required (D1) |
| A4 | Thief of a running, sleeping or unlocked PC with the user signed in | No — out of reach | — | Keys are in memory; the store is unlocked |
| A5 | Malware or any program running as the same user | No — out of reach | — | It can ask the store for the key as the program does, read the program's memory, and record a typed passphrase or recovery key (`DESKTOP_KEY_ASSESSMENT.md:28`) |
| A6 | Administrator or root of the PC; in a Windows domain also the domain administrators | No — out of reach | — | Can run code as the user and read memory; in a domain, user DPAPI master keys are backed up to the domain controllers (Microsoft, [MS-BKRP] BackupKey Remote Protocol) |
| A7 | Holds the recovery key **and** a copy of the folder | By design can read everything | Recovery is recorded, but only in the audit chain of the copy being opened; the original installation never learns of it | Custody of the recovery key is the only control (section 8.3) |
| A8 | Alters a copied or stored folder and puts it back | Detection is in scope | Authenticated pages (chosen scheme, gate G-6) and the already authenticated stored files (`crypto.rs:45-55`, `storage.rs:435-438`) | Replacing the database, one page, the envelope or a backup with an **older authentic version of itself** (rollback) is not detected — as today for the audit chain (`main.rs:1190`). This includes putting back one authentic slot or recovery block of an older envelope generation: the associated data of a wrap (section 7.3) does not carry the generation, so such a slot still opens and yields the keys of its own generation |
| A9 | The vendor | No access by default (D3) | No vendor key, no escrow, no network | A malicious program release is a supply-chain threat (T18, `THREAT_MODEL.md:46`), outside this design |

### 4.2 Out of reach of any application design

- Malware or any program running as the same user (A5), an administrator or root account (A6), a running or sleeping PC
  with the user signed in (A4).
- Bytes already written in plaintext: the database, its copies and backups from before the migration; old blocks of a
  deleted file on an SSD; restore points or file-history copies of the data folder (whether Windows tools copy
  `%LOCALAPPDATA%` is to be checked on a real PC, W-8).
- Files a user downloads or exports from the program: they leave the data folder in plaintext.
- Anyone who holds the recovery key and a copy of the folder (A7).

### 4.3 What the design must achieve

- **P1** A copy of the data folder or of a backup, without the recovery key, reveals no record content, no stored file
  content and no key — only the metadata listed in section 14. Three limits, stated again in section 14: a copy made
  before the migration holds SK in plaintext, which opens the stored files of later copies unless SK is rotated at
  migration (O-24); a passphrase slot can be guessed offline (O-5, O-25); a copy that includes the user's profile is
  protected only by the account password.
- **P2** Another account without administrator rights on the same PC cannot unwrap the keys (user scope, O-4).
- **P3** A changed byte in an encrypted page or a stored file is detected, and nothing altered is served.
- **P4** The data can be recovered on another machine with the recovery key and an administrator of the installation,
  and the recovery is recorded in the audit chain of the recovered installation.
- **P5** Every change of format or key can be stopped at any point and then resumed or rolled back without data loss.
- **P6** No step ever creates a new key while data exists — the rule of BE-010 (`storage.rs:201-203, 225-232`).

## 5. Assets

| Asset | Where | Today | After this design |
|---|---|---|---|
| Database | `<data>/nexus-fabric.sqlite` (`storage.rs:290`) | Plaintext (V-21): user names, Argon2 hashes and salts (`storage.rs:18`), session token hashes (`:19`), projects, document metadata with file names and SHA-256 (`:21`), records (`:22`; the uploaded file name is also inside a record, `main.rs:1444`), results (`:23`), audit (`:25`), failed sign-ins with client address (`:29`, `:44-45`), `meta` (installation id, key fingerprint, audit anchor) | Every page encrypted and authenticated with the database key |
| Write-ahead log | `nexus-fabric.sqlite-wal` | Plaintext page images; emptied at a clean desktop stop (`desktop.rs:274`); present while running and after a crash | Page images encrypted. Frame headers (page numbers, commit marks, salts, checksums) are written by SQLite's WAL layer and are expected to stay readable (checked in G-4) |
| WAL index | `nexus-fabric.sqlite-shm` | Index of WAL frames, no record content (SQLite documentation "WAL-mode File Format") | Unchanged, or not created at all with exclusive locking (O-14) |
| Freed pages | Inside the database file | Rows deleted every hour (sessions and failed sign-ins, `main.rs:3776-3786`, `desktop.rs:263-272`) can stay readable in free pages | Free pages are encrypted like all pages |
| Stored files | `<data>/blobs/<sha256>.bin` (`storage.rs:419-421`) | AES-256-GCM with the storage key (`crypto.rs:33-55`); names are the SHA-256 of the plaintext | Format unchanged; the storage key is no longer in a plaintext file; names unchanged unless O-16 |
| Unfinished stored files | `blobs/.tmp-*` (`storage.rs:455-459`) | Hold sealed bytes only (`storage.rs:460-470`); removed at start (`storage.rs:484-495`) | Unchanged |
| SQLite temporary files | The operating system's temporary folder | The bundled build sets no `SQLITE_TEMP_STORE` (`libsqlite3-sys-0.40.0/build.rs:170-187`; it is set only for SQLCipher, `:192`, and Android, `:279`), so a large sort or temporary table may spill plaintext outside the data folder | Kept in memory in desktop mode (section 6.5) |
| Installation marker | `blobs/.installation-marker` (`storage.rs:142, 385-387`) | Installation id in plaintext; not secret | Unchanged |
| Storage key file | `<data>/storage.key` (`desktop.rs:200`) | 32 raw bytes beside the data (`storage.rs:223`); 0600 on Unix (`storage.rs:121-131`); Windows: profile rules only | Removed by the migration (section 9); never written again in desktop mode (section 7.6, step 6). Copies and backups made before the migration keep it (A1, O-24) |
| Key envelope (new) | `<data>/key-envelope.json` | — | Wrapped keys, fingerprints and slot data only (section 7.3). A previous generation exists only during an unfinished rewrite or rotation recorded in a journal (section 7.3) |
| Operating-system secret (new) | Windows: the user's DPAPI master key in the user profile; macOS: login keychain; Linux: Secret Service collection | — | Outside the data folder |
| Recovery key (new) | Paper or removable media, off the machine | — | Section 8 |
| Setup token | Desktop: memory, URL fragment, and a console line when no browser opened (`desktop.rs:251-255`) | Never on disk in desktop mode (`desktop.rs:205` leads to `main.rs:3570-3576`); exists only while no administrator and no data exist | Unchanged |
| Console output | Standard output and error (`main.rs:126-141` and others) | No log file (BE-021); shows the data folder path and job numbers, no keys | Rule: never key bytes, recovery key, passphrase or wrapped values; fingerprints and ids only |
| Process memory, swap or page file, hibernation file, crash dumps | The machine | Hold decrypted data and the storage key | Will hold the data keys while the program runs; out of reach (section 14) |
| Downloads and exports | Wherever the user saves them | Plaintext | Unchanged; not covered |
| Backups (new) | A folder the administrator chooses | None in desktop mode (BE-024) | Section 10 |

## 6. Encrypting the SQLite database

### 6.1 Constraints that decide between the options

- **C1 Offline check.** `tests/offline_check.sh:16` refuses crates whose names match its pattern, which names `openssl`
  and `rustls`. It reads `cargo tree` only, so a C library linked by a build script is invisible to it (section 15
  adds checks for that). It runs `cargo tree -p nf-server` without `--target` and with the default edges, which list
  the dependencies of the host platform only and include dev-dependencies (The Cargo Book, `cargo tree`, options
  `--target` and `--edges`). So a Windows-only crate (for DPAPI or a keyring) is invisible when the check runs on the
  Linux build host, and a test-only crate added under `[dev-dependencies]` (`Cargo.toml:24`) — for example a SQLCipher
  build with OpenSSL — fails the check like a shipped one. Section 15 extends the check to every target and places the
  G-6 test build outside nf-server's tree.
- **C2 Windows cross-build.** The Windows package is built on Linux for `x86_64-pc-windows-gnu` with mingw, with SQLite
  compiled in: "no SQLite DLL is needed" (`docs/PACKAGING.md:111-137`).
- **C3 WAL.** Start-up requires WAL (`storage.rs:297-300`); the clean stop relies on it (`desktop.rs:274`).
- **C4 Backups.** A consistent snapshot is needed; copying a live database with its WAL is already advised against
  (`deploy/README.md:68`).
- **C5 Licences.** Crates are limited to the list in `packaging/about.toml:6-16`. C code compiled by a build script is
  not seen by that tool and is described by hand in the template `packaging/about.hbs`, as SQLite is today (its
  paragraph, `about.hbs:14-16`). `packaging/THIRD_PARTY_NOTICES.txt` is generated from that template by
  `packaging/generate_notices.sh:11-13` and says "Do not edit it by hand" (`THIRD_PARTY_NOTICES.txt:12`): a hand edit
  of the generated file is lost at the next generation.
- **C6 Auditability.** The on-disk format must be checkable by a second, independent implementation in tests — the
  practice the project already follows for geometry (`validation/`).
- **C7 One program, two modes.** Desktop mode is the same program as server mode (`desktop.rs:1-2`); `build_app` opens
  storage for both (`main.rs:3555-3556`; desktop at `desktop.rs:214`). A library or compile-time change therefore
  reaches server mode too; only settings applied at the desktop open stay desktop-only.

### 6.2 Options compared

Speed and file size were not measured for any option; they are gate G-8. Licences are as published by each project and
must be confirmed at the pinned version (G-1).

| Option | Licence | Dependency tree and offline check | Windows cross-build | WAL and backup | Auditability | Verdict |
|---|---|---|---|---|---|---|
| **A** SQLCipher with vendored OpenSSL (`bundled-sqlcipher-vendored-openssl`) | SQLCipher: BSD-style; OpenSSL: its own | Adds the `openssl-sys` crate (`libsqlite3-sys-0.40.0/Cargo.toml:51-54, 86-88`): `offline_check.sh:16` fails | Vendored OpenSSL would also have to be cross-built for mingw; not tried | WAL supported per its vendor; backup API between databases with the same key; conversion by its export function (`sqlcipher_export`) | OpenSSL is widely examined but large | **Excluded**, unless the owner changes the offline check |
| **B** SQLCipher with the system's libcrypto (`bundled-sqlcipher`) | as A | Passes the crate-name grep, but links `crypto` outside cargo (`libsqlite3-sys-0.40.0/build.rs:258-264, 269-272`): the check cannot see it; the Linux package would depend on the distribution's libcrypto | `is_windows` requires a Windows **host** (`build.rs:200`); on the Linux build host the mingw target falls through to `dylib=crypto` (`build.rs:271`): a mingw libcrypto and a DLL beside the exe, contrary to C2. The crate says bundled SQLCipher is "available for the moment only on Unix" (`build.rs:92-95`) | as A | as A | **Excluded**: it evades the check's intent instead of meeting it |
| **C** SQLCipher with LibTomCrypt, compiled from the sources in `sqlcipher-src` (pinned `=420.0.0` by `libsqlite3-sys-0.40.0/Cargo.toml:113-115`) | SQLCipher: BSD-style; LibTomCrypt: public-domain style | No crate named by the pattern; two C code bases compiled in. libsqlite3-sys has no switch for this back end, so an own C build is linked through its `sqlcipher` linked path (`Cargo.toml:77` of that crate; `build.rs:86-96, 463-500`) | Not proven. Only a wasm32 precedent: `sqlite-wasm-rs-0.6.3/build.rs:57-80, 379-400` builds it with `-DSQLCIPHER_CRYPTO_LIBTOMCRYPT=1` and needed its own shim | as A | SQLCipher page format documented by its vendor; LibTomCrypt is a separate crypto library | **Second choice** |
| **D** SQLite3 Multiple Ciphers (SQLite3MC), compiled from the sources in `sqlite3mc-src` (pinned `=206.1.0` in `sqlite-wasm-rs-0.6.3/Cargo.toml:101-103`) | MIT | One C code base with its own cipher code; no crate named by the pattern; linked through libsqlite3-sys's linked path (`build.rs:463-500`) | Not proven. Only a wasm32 precedent: `sqlite-wasm-rs-0.6.3/build.rs:83-92` compiles it with no crypto library added (`compile: None`); its flag `-DARGON2_NO_THREADS` (`:90`) shows that the source carries its own crypto code | WAL supported per its documentation (G-4); backup and plaintext-to-encrypted export to be confirmed (G-5) | Its cipher code is the project's own and less examined than a dedicated crypto library; offset by the cross-check G-6: in SQLCipher-compatible mode SQLCipher itself must read the same file | **Recommended, conditional on G-1 … G-10** |
| **E** Encrypting chosen columns in Rust (aes-gcm is already in the tree, `Cargo.toml:7`) | No new licence | No new crate | No C change | Stock WAL and backup — but every column left in clear is in the WAL and the backups | SQL reads inside stored JSON (`CORE_EXPR`, `main.rs:1522`; migration, `storage.rs:337`), indexes (`storage.rs:52-62`), unique user names (`storage.rs:18`) and session lookups (`storage.rs:19`) need clear or deterministic values; table structure, row counts, sizes and times stay visible; every query in `main.rs` must be reviewed, and one missed column leaks | **Rejected**: partial, large, easy to get wrong |
| **F** Own encrypting VFS | — | New C or unsafe Rust against SQLite's VFS interface | — | A new page-cipher construction (nonce storage, WAL frames, torn writes) would be designed here | Nobody here can call such a construction sound without an outside cryptographic review | **Rejected** |
| **G** SQLite Encryption Extension (SEE) from the SQLite developers | Commercial; terms and cost are the owner's | — | — | — | Source and terms not available to this review | **Not evaluated** |

### 6.3 Recommendation (conditional)

Option **D**, with these choices:

- **SQLCipher-compatible page scheme** (each page authenticated with an HMAC), with the library's SQLCipher-4-compatible
  settings as they stand in the pinned source. The exact parameters are read from that source at implementation and
  fixed by a known-answer test; none is chosen here. Reason: SQLCipher itself can then read the files (an independent
  cross-check in tests, G-6), and option C could take over later without a data migration. If raw keys or WAL do not
  work in that mode (G-3, G-4), the library's ChaCha20-Poly1305 scheme is the alternative (O-2). Its older schemes
  without page authentication must never be used.
- **Raw 32-byte key** (the size of today's storage key, `storage.rs:223`), so no password-based key derivation and no
  iteration count has to be chosen.
- **Compiled from source, statically**, for both targets: no DLL, no libcrypto. It replaces the bundled SQLite for the
  whole program, server mode included (C7), with SQLite's own default `SQLITE_TEMP_STORE` (section 6.5), so that server
  mode keeps today's settings.

Why not C first: two C code bases and glue instead of one, and the only precedent needed its own shim
(`sqlite-wasm-rs-0.6.3/build.rs:379-400`). C stays the fallback if D fails a gate. If both fail, the desktop product
stays in the "restricted deployment" class (D2) and the owner is asked again.

### 6.4 Gates before the final choice

Each needs recorded evidence; "not run" is never "passed".

| Gate | What must be shown |
|---|---|
| G-1 | The licence text of the pinned source has been read and is written by hand into the template `packaging/about.hbs` (as SQLite is at `:14-16`), the notices file is regenerated (`packaging/generate_notices.sh`), and `tests/notices_check.sh` checks it |
| G-2 | It builds with the existing toolchains for `x86_64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`, statically; neither binary imports an OpenSSL library; the dynamic section of the Linux binary and the import table of the Windows exe are recorded in full, and the owner accepts those lists (O-20) — no list is predicted here; the integration route (O-3) works with every command of work-plan section B |
| G-3 | A raw 32-byte key is accepted in the chosen mode without a passphrase derivation |
| G-4 | `PRAGMA journal_mode = WAL` returns `wal` (`storage.rs:297-300`); WAL page images are ciphertext (file scan); recovery after a killed process works; closing the last connection after a checkpoint removes the `-wal` and `-shm` files, as in the stock library (`libsqlite3-sys-0.40.0/sqlite3/sqlite3.c:69473-69517`), which the migration relies on (section 9.3) |
| G-5 | The online backup API, through `Backup::new` (`rusqlite-0.43.0/src/backup.rs:194`) into a destination connection keyed with the same DBK before its first statement, then `run_to_completion` (`:294`), copies an open encrypted database into a new file; the destination is ciphertext (file scan as G-4) and opens with DBK. A documented method exports a plaintext database into an encrypted one and keeps `user_version` and every row |
| G-6 | SQLCipher (a test-only build, never shipped, kept outside nf-server's dependency tree as a separate tool beside `validation/`, with the LibTomCrypt back end so that it needs no OpenSSL; section 15) opens a database written by the product with the same raw key, and the product opens one written by SQLCipher |
| G-7 | In desktop mode temporary storage stays in memory: the chosen build's compile-time `SQLITE_TEMP_STORE` (read from the pinned source) lets `PRAGMA temp_store = MEMORY` take effect; a large sort leaves no file in the operating system's temporary folder; in server mode the same sort behaves as today |
| G-8 | Speed, file size and peak memory measured before and after on the same machine and data (the work-plan rule of task 3.4), in desktop mode and in server mode; figures recorded; the owner accepts or rejects them (O-21) |
| G-9 | `tests/offline_check.sh` passes together with the additions of section 15 |
| G-10 | The random source of the page cipher (page IVs, database salt) is identified in the pinned source for each target and shown to work there: the same page written twice gets different IVs, and two new databases get different salts. The only precedent for a cipher build had to install its own entropy hook (`sqlite-wasm-rs-0.6.3/build.rs:66-67`, `SQLITE_EXTRA_INIT`) |

### 6.5 Database settings the implementation must use

- **Key first.** The key is applied immediately after `Connection::open` (`storage.rs:291`) and before any other
  statement (`storage.rs:295-302`). It is passed as bytes through the library's API or key pragma, never inside a file
  URI, never in an environment variable (the replay child inherits the environment: `main.rs:2397-2402`), never printed.
  The same holds for every connection the program opens on an encrypted file: the migration copy (section 9.3) and the
  backup destination (section 10.1) are keyed before their first statement.
- **Temporary storage in memory, desktop mode only.** Desktop mode sets `PRAGMA temp_store = MEMORY` at open. The
  library keeps SQLite's own compile-time default, `SQLITE_TEMP_STORE` 1 (`libsqlite3-sys-0.40.0/sqlite3/sqlite3.c:15905-15909`),
  under which that pragma overrides the compile-time setting (SQLite documentation, "PRAGMA temp_store" and the
  compile-time option `SQLITE_TEMP_STORE`); G-7 confirms this for the chosen build. A build-wide `3`, as libsqlite3-sys
  uses on Android (`build.rs:279`; its SQLCipher build uses `2`, `:192`), would force memory on server mode as well
  (C7). Consequence in desktop mode: a large sort, index build or temporary table is limited by the memory of the PC;
  G-8 records peak memory.
- **WAL and `synchronous = FULL`** as today (`storage.rs:297-302`).
- **Optional, O-14:** `PRAGMA locking_mode = EXCLUSIVE` before the first access in desktop mode. SQLite then keeps the WAL
  index in process memory and creates no `-shm` file (SQLite documentation "Write-Ahead Logging", WAL without shared
  memory), and no other process can open the database while the program runs. Tests that open a second connection
  (`tests.rs:764, 2450`) would change accordingly.
- **Cipher settings and page size** stay at the library's defaults for the chosen scheme unless G-8 gives a reason; any
  change is an owner decision.

## 7. Key hierarchy

### 7.1 Keys

| Key | Size | Made | Used for | Kept |
|---|---|---|---|---|
| **SK** storage key | 32 bytes (`storage.rs:223`; `crypto.rs:34`) | Exists today (`storage.rs:233`) | Sealing stored files (`crypto.rs:33-55`) only. Today it also authenticates the audit anchor (`main.rs:289-293`); after the migration that moves to AK. Its fingerprint is in `meta` (`storage.rs:84-89, 254-279`) | Today a plaintext file; after: only wrapped, in the envelope |
| **DBK** database key | 32 bytes (the size of SK) | At migration, or at the first start of a new installation, from the operating system's random source | Raw key of the encrypting SQLite build | Only wrapped |
| **XS** auxiliary secret | 32 bytes | With DBK, from the operating system's random source | Never used directly; only input to the derivation of AK and FK below | Only wrapped |
| **AK** audit-anchor key | 32 bytes | Derived from XS at each start, never stored | HMAC of the audit anchor (replaces SK in `anchor_mac`, `main.rs:289-293`) | Memory only |
| **FK** factor-secret key | 32 bytes | Derived from XS at each start, never stored | Sealing the sign-in factor secrets of task 2.6 (`SIGNIN_HARDENING_NOTE.md`, option A), so they are not a third use of SK | Memory only |
| **KR** recovery-wrapping key | 32 bytes | New with every recovery key | Wraps SK, DBK and XS for recovery. Lets the running program wrap new data keys for recovery without the recovery key being present | Wrapped under RK, and inside **every** operating-system or passphrase slot; so a new KR rewrites every slot (section 11) |
| **KEK-OS** | 32 bytes | One per user and machine | Wraps SK, DBK, XS and KR in the operating-system slot | Protected by DPAPI (its output kept in the envelope) or held in the Keychain or the Secret Service |
| **KEK-PW** (fallback) | 32 bytes of Argon2id output (the output size of `crypto.rs:18`) | Derived at each start from the passphrase and a 16-byte random salt (the salt size used for passwords, `main.rs:732`) | Wraps SK, DBK, XS and KR in the passphrase slot | Never stored on disk; whether it stays in memory for the run, so the slot can be rewritten at a rotation, is O-23 |
| **RK** recovery key | 32 bytes | At setup or migration, and at every recovery-key rotation | Wraps KR | Off the machine only (D3) |

Every key in this table has the size of today's storage key, 32 bytes (`storage.rs:223`), the key size of the
AES-256-GCM cipher already used (`crypto.rs:34-35`), so each can serve as an AES-256-GCM key; no other key size is
introduced.

Why a separate DBK instead of using SK for the database: a separate key lets the database and the stored files be
re-keyed independently (section 11). The cost is one more value in the same envelope.

**Key separation (RC-07, RC-20, RA-003).** Today SK serves two algorithms, AES-GCM for stored files and HMAC for the
audit anchor, and task 2.6 would add a third use (sealing factor secrets). After this design every key has one purpose.
AK and FK are derived from XS with HKDF-SHA256 (RFC 5869): extract with a salt of 32 zero bytes (RFC 5869 section 2.2,
salt not provided), expand to one 32-byte block with distinct fixed `info` labels, one for AK and one for FK. Both steps
are HMAC-SHA256 with a 32-byte key, which the hand-written HMAC of `storage.rs:91-108` already computes, so no new crate
is needed; RFC 5869 Appendix A test case 3 (no salt, which means 32 zero bytes) is the known-answer test, together
with a fixed vector for each label recorded at implementation. Why one secret and two derived
keys instead of two wrapped keys: one more value in every slot instead of two; AK and FK change together at an XS
rotation (section 11), which is acceptable because both are small (one anchor, a few factor secrets).

What a separate anchor key does and does not give, stated with RC-20's verification: AK alone does not stop someone who
holds the keys from rewriting the chain, and it does not stop a rollback of the whole database (A8). What it removes is
the cross-use: a party who learns SK (for example from a pre-migration copy, A1, O-24) can no longer re-sign the anchor.
The fix that makes the anchor effective — verifying the stored anchor before every append and failing closed, instead of
taking the count from the unauthenticated `meta.audit_count` (REVIEW_C_EXTERNAL, RC-07 verification) — belongs to task
2.2 (RA-003) and must land first; this design assumes it and adds only the key. A copy of the anchor outside the
database is O-28.

**Wrapping algorithm.** AES-256-GCM through the `aes-gcm` crate already in the tree (`Cargo.toml:7`; `crypto.rs:7-8`),
with a random 12-byte nonce as today (`crypto.rs:36`; the random construction of NIST SP 800-38D section 8.2.2) and with
associated data — a variant of `crypto::seal` and `crypto::open`, which take none today (`crypto.rs:34-55`). Each
wrapping key encrypts only a handful of times, far below the invocation limit of SP 800-38D section 8.3. No new crate is
needed. AES key wrap (RFC 3394, RFC 5649) was not chosen: it would add a crate and has no associated data.

### 7.2 Picture

```
 operating system (one per user and machine)              recovery key RK (paper or removable media, off the machine)
   DPAPI user scope | Keychain | Secret Service                        |
   or passphrase -> Argon2id                                           |
        | protects KEK-OS, or derives KEK-PW                           |
        v                                                              v
 slot:  AES-256-GCM( KEK , SK | DBK | XS | KR )         recovery block: AES-256-GCM( RK , KR )
                                                                      + AES-256-GCM( KR , SK | DBK | XS )
        \___________________________ key envelope, in the data folder ___________________________/

 SK  -> stored files (AES-256-GCM, crypto.rs:33-55)
 DBK -> every database page and every WAL page image (encrypting SQLite build)
 XS  -> HKDF-SHA256 -> AK: audit anchor HMAC (main.rs:289-293)
                    -> FK: sealing of sign-in factor secrets (task 2.6)
```

### 7.3 The key envelope

- File `key-envelope.json` in the data folder, written the way stored files are: a new file, owner-only from its first
  byte, fsync, rename, directory fsync on Unix (`storage.rs:121-139, 454-475`).
- **No old generation is kept.** Every rewrite runs under a journal marker, `key-envelope.journal` (generations from and
  to, and the reason; no secret): (1) write the marker; (2) copy the current file to `key-envelope.prev.json`, written
  the same way; (3) write the new generation and rename it over `key-envelope.json`; (4) read it back and unwrap it
  through the slot of this user and machine (or the passphrase slot, O-23) and, when the recovery block changed, open
  that block with the new RK, which was typed back before step 1 (section 11); (5) delete `key-envelope.prev.json`;
  (6) delete the marker. Directory fsync on Unix after steps 3, 5 and 6. A database-key rotation instead keeps the
  previous generation under its own journal (`encryption-journal.json`, section 9.3) until its step S6, because a
  rollback before S5 needs the old DBK; the migration has no previous generation (a plaintext installation has no
  envelope).
- At start, `key-envelope.prev.json` is used **only** while such a journal exists, that is when a rewrite or rotation
  did not finish: if `key-envelope.json` holds the new generation and unwraps, steps 5 and 6 are completed; otherwise
  the previous generation is put back as `key-envelope.json` and the marker removed. Without a journal, a
  `key-envelope.prev.json` is not used, is reported and is deleted, and an unreadable `key-envelope.json` is a refusal,
  never a fall-back (section 7.6, step 3).
- So an old recovery block, an old passphrase slot or a replaced operating-system slot leaves the data folder as soon as
  the new generation has been read back. Between steps 2 and 5 the folder holds both generations, and a copy taken in
  that moment holds the old secrets too. Copies and backups made earlier keep their own envelope (section 11).
- Fields, none of them secret in clear:
  - `format` and `version`;
  - `installation_id` — the id already in `meta` and in the marker (`storage.rs:338-387`);
  - `generation` — increased by every rewrite;
  - `fingerprints` of SK, DBK and XS — the construction of `storage.rs:84-89`, each with its own fixed prefix (the
    XS fingerprint is also kept in `meta`, as SK's is, so start-up can tell that the anchor key belongs to this database);
  - `slots` — each with `kind` (`windows-dpapi-user`, `windows-dpapi-machine`, `macos-keychain`, `linux-secret-service`,
    `passphrase-argon2id`), `id`, creation time, what the kind needs (DPAPI output; keychain service and account names;
    Secret Service attributes; Argon2id salt and parameters) and the wrapped SK, DBK, XS and KR;
  - `recovery` — recovery key id (section 8.2), creation time, KR wrapped under RK, SK, DBK and XS wrapped under KR;
  - `rotation` — present only while a key rotation runs (section 11);
  - `pending_audit` — events that must be written into the audit chain at the next opening or, for a recovery or a
    restore, once an administrator has given the authorisation (sections 8.4, 13).
- Associated data of every wrap: `format`, `version`, `installation_id`, slot kind and slot id — values that never change
  for that slot. A wrapped value therefore cannot be moved to another installation or slot. One slot can be replaced
  without touching the others only when nothing but its own key-encryption key changes (a KEK-OS rotation, a new slot
  after recovery). Because every slot holds SK, DBK, XS and KR (section 7.2), a change of any of those four rewrites
  every slot and the recovery block in one generation (section 11). The generation is deliberately not in the
  associated data (a slot replaced on its own keeps its bytes); the consequence, an older authentic slot put back, is
  the rollback that A8 states as not detected.
- The file as a whole is not authenticated. Someone who can write the folder can delete a slot (the result is a refusal
  to start) or a pending event (the result is a missing audit entry in that copy; section 13).
- Never in the envelope: a key in clear, the recovery key, a passphrase.

### 7.4 Protection by the operating system

**Windows — DPAPI.**

- `CryptProtectData` and `CryptUnprotectData` from `crypt32.dll` are bound in `windows-sys`
  (`windows-sys-0.61.2/src/Windows/Win32/Security/Cryptography/mod.rs:283, 315`), a crate already in the tree
  (`Cargo.lock:1102-1103`, pulled in by tokio, mio, socket2 and others: `Cargo.lock:312, 605, 789, 927, 989`). Only one
  more feature of that crate is needed; no new crate.
- `CRYPTPROTECT_UI_FORBIDDEN` (`mod.rs:5272`) is always set: fail instead of showing a dialog. `CRYPTPROTECT_LOCAL_MACHINE`
  (`mod.rs:5252`) is not set: user scope (O-4).
- DPAPI protects KEK-OS; its output is kept in the slot. Only the same Windows user can reverse it (or, in a domain,
  holders of the domain's DPAPI backup key — A6), through the user's DPAPI master key, which Windows keeps in the user's
  profile (outside the data folder) and protects with the logon credentials (Microsoft documentation of
  `CryptProtectData`, dpapi.h).
- Outside the data folder is not outside the profile: the data folder is in `%LOCALAPPDATA%`, which the program itself
  takes as `%USERPROFILE%\AppData\Local` when the variable is missing (`desktop.rs:87-91`). A copy of the whole profile
  (profile backups, PC-migration tools) carries the data folder and the DPAPI master keys together; the keys are then
  protected only by the strength of the Windows password, which can be guessed offline on the copy (A1; tests W-1,
  W-8). Whether the same holds for a home-folder copy with the macOS login keychain or a Linux keyring is recorded in
  M-1 … M-6 and L-1 … L-4.
- DPAPI's optional entropy is not used: any program running as the user could read whatever value the program supplied.

| | User scope (recommended, O-4) | Machine scope |
|---|---|---|
| Copied folder taken off the PC | Cannot be unwrapped | Cannot be unwrapped |
| Another account on the same PC | Cannot unwrap | **Can unwrap** — any account or service on the PC can |
| Windows profile lost, Windows reinstalled, local-account password reset by an administrator | Slot unusable; recovery key needed | Survives a lost profile; lost with a reinstall |
| Fits today's per-user data folder (`desktop.rs:86-98`) and per-user installer (`installer.nsi:10`) | Yes | Not needed for one user |

Behaviour this relies on, to be confirmed on real Windows PCs because wine does not implement DPAPI faithfully
(`DESKTOP_KEY_ASSESSMENT.md:40`): a password change by the user keeps access (W-3); a reset of a local account's password
by an administrator loses it (W-4); accounts without a password, with a Microsoft account or a PIN sign-in, and
domain-joined PCs behave as stated in section 4.1 (W-5).

**macOS — Keychain (design only).**

- A generic-password item in the user's login keychain holds KEK-OS; its service name and account (the installation id)
  are in the slot. The item is marked not synchronisable, so it never leaves the machine through a cloud keychain.
- The item's access rule names the program by its code signature. No macOS build and no signing exist
  (`docs/PACKAGING.md:232, 235-241`); an unsigned program would meet prompts or refusals after each update. A signed macOS
  build is a precondition (O-18).
- No binding crate is in the local registry; any candidate's tree must pass the offline check.
- Nothing of this can be tested here (M-1 … M-6).

**Linux — Secret Service.**

- KEK-OS is stored as a secret item through the freedesktop.org Secret Service API on the session D-Bus (implemented by
  GNOME Keyring, KeePassXC and others) and found by attributes: application name, installation id, slot id.
- Candidate client: `zbus` 5.19.0 (pure Rust; in the local registry, not yet in this project). Its default features pull
  `async-io` (`zbus-5.19.0/Cargo.toml:62-78`); its `tokio` feature (`:84`) uses the runtime already in the tree. Besides
  `unix:`, zbus parses the bus-address transports `unixexec` (starts a program), `tcp`, `nonce-tcp`, `vsock` (with a
  feature), `ibus` (runs `ibus address`, `ibus.rs:64`), `launchd` (macOS) and `autolaunch` (Windows)
  (`zbus-5.19.0/src/address/transport/mod.rs:236-248`). The program must read `DBUS_SESSION_BUS_ADDRESS` itself and
  refuse every address in it that is not `unix:` before any connection is made, and a test must prove it, because a
  crate-name check cannot see this (section 15).
- Common implementations let every program of the same user read an unlocked collection (A5 unchanged; L-3 records the
  behaviour).
- No session bus, no service, or a locked collection with no way to prompt (headless machine, minimal desktop, remote
  shell, automatic sign-in that leaves the collection locked) counts as "store unavailable": section 7.5.

### 7.5 Where no operating-system store exists

| Situation | Behaviour |
|---|---|
| Windows: a DPAPI call fails | Refuse to start, show the system's error text, offer recovery. Never fall back to a plaintext key |
| macOS or Linux: store present but locked, and no prompt possible | Treated as unavailable |
| Linux without a Secret Service | The passphrase slot, if the administrator set one up (O-5); otherwise refuse, say why, and offer recovery |
| A wrapped key file kept in the data folder | **Not offered**: it protects nothing against a copied folder |
| A key file on removable media (alternative in O-5) | Protects only while the medium is not copied together with the folder |
| Unattended start with neither store nor passphrase | Not covered by this design; such an installation stays "restricted deployment" |

Passphrase slot: Argon2id (RFC 9106) through the `argon2` crate already in the tree (`Cargo.toml:8`), 16-byte random salt
(as `main.rs:732`). Its cost parameters are O-5: RFC 9106 section 4 names two recommended options; the present password
hashing uses the crate's defaults (`crypto.rs:19`, reported as "library defaults" at `main.rs:1251`), which were not
chosen for keys and are not assumed here. The passphrase is typed on the console with echo off — the Linux launcher runs
in a terminal (`packaging/linux/nexus-fabric.desktop:6`, `Terminal=true`) — and never given on the command line or in an
environment variable.

Every copy of the data folder and every backup carries the passphrase slot, so whoever holds one can guess the
passphrase offline, as fast as the Argon2id cost allows; nothing in the program can limit the attempts. The rules a
passphrase must meet are O-25. Whether a passphrase slot may also exist where an operating-system store works — a
second way in, with the same exposure — is O-26; start-up tries it only when the slot of this user and machine does not
open (section 7.6, step 4).

### 7.6 Start-up of an encrypted installation

1. Take the single-instance lock (`desktop.rs:197`).
2. Steps 2 to 5 of this list, the migration steps S1 to S4 (section 9.3), the first-start choice (section 9.8) and the
   recovery and restore modes (sections 8.4, 10.4) all run here: after the lock and **before** `build_app`
   (`desktop.rs:214`). Migration step S5 is the opening that `build_app` itself then performs (steps 6 and 7), and S6
   follows its first write.
   `build_app` opens the database (`main.rs:3556`) before any port is bound (`desktop.rs:221-226`), so no page can be
   served while the keys are unknown; everything that needs a key or a choice before that point uses the console
   (O-6). If a journal exists (`encryption-journal.json`, `key-envelope.journal`, `restore-journal.json`, a rotation in
   the envelope), that step is finished or rolled back first (sections 7.3, 9, 10.4, 11).
3. Read `key-envelope.json`. `key-envelope.prev.json` is used only under a journal, as section 7.3 says; otherwise a
   missing or unreadable envelope is a refusal, and nothing is created.
4. Unwrap through the slot of this user and machine; else through the passphrase slot (prompt on the console); else
   refuse, say why on the console and offer recovery there (section 8.4). Keys are never created here.
5. Compare the unwrapped keys with the fingerprints in the envelope.
6. Open the database and apply DBK before any other statement (section 6.5); then the existing checks, unchanged: WAL,
   schema version, quick check, schema, installation id and marker (`storage.rs:297-383, 385-391`), and the comparison of
   SK with the fingerprint in `meta` (`storage.rs:254-260`). For an encrypted installation the envelope path replaces
   `load_key` (`storage.rs:204-281`, called at `:384`), which reads `storage.key` and, where no data exists, creates a
   new plaintext key file (`storage.rs:233-246`). In desktop mode that key-creation branch is removed: a plaintext
   installation not yet migrated only reads its existing key file, a missing key file is a refusal whether or not data
   exists, and no step of desktop mode ever writes `storage.key` (test NK-1). Server mode keeps `load_key` unchanged
   (O-19).
7. Write the `pending_audit` events that need no authorisation into the audit chain, then clear them from the envelope.
   While a recovery or restore event awaits its authorisation (section 8.4), every API route refuses except
   administrator sign-in and the authorisation form — after a restart as well; the client's own files are still served
   so that the page can load.
8. Serve.

Refusals (P6): an envelope missing beside an encrypted database or stored files — refuse, create nothing (the BE-010
rule, `storage.rs:225-232`); a plaintext `storage.key` found in a migrated installation — not used for anything,
reported; `installation_id` of the envelope different from `meta` or the marker — refuse; a `key-envelope.prev.json`
without a journal — not used, reported, deleted (section 7.3); standard input not a terminal when a prompt is needed —
refuse with the reason instead of waiting.

## 8. Recovery key

### 8.1 Generation

- RK: 32 bytes (the key size of section 7.1) from the operating system's random source (the source of the setup token,
  `desktop.rs:150-156`); KR: 32 new bytes with every RK. The recovery block is AES-256-GCM(RK, KR) plus AES-256-GCM(KR, SK, DBK and XS).
- Made by the program at first-run setup (new installation), at the request to encrypt (existing installation), and at
  every recovery-key rotation. No recovery key is built into the program, and none exists at the vendor (D3).
- Shown once, in the browser page served on the loopback address (API responses carry `no-store`: `main.rs:3234, 3359`),
  with a print view and a text download meant for removable media; then cleared from the program's memory as far as the
  language allows. Never written to the data folder, the console or any log.

### 8.2 Format

- The printed sheet carries: product name; installation id (not secret); recovery key id; creation date; the key; the
  sentence "Anyone who holds this key and a copy of the data folder can read all data"; lines for the names and
  signatures of the custodians.
- Encoding: RFC 4648 section 6 Base32 (letters A–Z and digits 2–7; 0, 1, 8 and 9 do not occur, which avoids confusing
  0/O and 1/I), in groups. **Each group ends with its own check value** — a truncated SHA-256 over a fixed prefix, the
  group's position and the group's characters — so that a typing error is reported with its group before any
  decryption is tried; the position in the input also catches two groups typed in the wrong order. A key that passes
  every group check but is still wrong is refused when the recovery block does not open (AES-GCM authentication,
  `crypto.rs:45-55`). Group length, check-value length and whether confirmation needs the whole key: O-7.
- Recovery key id: SHA-256 over another fixed prefix and RK, the construction of the storage key fingerprint
  (`storage.rs:84-89`); not secret; kept in the envelope and printed on the sheet, so a sheet can be matched to an
  installation without opening anything. Printed length: O-7.
- Confirmation: the administrator types the key back; the program opens the recovery block with it. Until that has
  happened the installation shows a lasting warning, and (O-9) refuses to create projects.

### 8.3 Custody and offline escrow

- From D3: the customer organisation controls the key; an authorised administrator holds it, not one employee alone; a
  small site names an owner and a second custodian; it is never stored beside the data or a backup.
- What the program can do: never store RK; record who confirmed it and when (audit `recovery_key_set` with the recovery
  key id); show when it was last used or tested. What it cannot do: check where the sheets are kept, or who holds them.
- Ways to meet "not one employee alone" (O-8): (i) one key, two sealed copies — one with the owner, one with the second
  custodian; either can recover alone; (ii) the key split into two shares by exclusive-or, both needed; neither share
  alone reveals anything. Recommended: (i) for small sites, because with (ii) the loss of one share loses the data once
  the PC is gone.
- The organisation should rehearse a recovery on a separate machine (D3; test BK-2) and repeat the rehearsal after
  every rotation of the recovery key.

### 8.4 Using it — same machine after a lost profile, or another machine

Recovery runs on the console, before the database is opened and before any port is bound (section 7.6, step 2; O-6).
A console exists on both packaged targets: the program sets no `windows_subsystem` attribute (none in
`crates/nf-server/src`), so on Windows it is a console program and its shortcuts open a window (`installer.nsi:82, 97`);
the Linux launcher runs in a terminal (`packaging/linux/nexus-fabric.desktop:6`).

1. Start with the existing data folder (same machine) or after a restore has copied a backup in (section 10.4).
2. No slot opens. The program does not open the database and binds no port; on the console it says why and offers
   recovery.
3. RK is typed on the console with echo off; the group check values are tested; KR, then SK, DBK and XS, are
   unwrapped; fingerprints are compared. From this step on the keys are in the program's memory: steps 6 and 7 below
   restrict the program's own routes and produce the audit record, but they are not a cryptographic control — the
   holder of RK and the copy can read it without this program (A7).
4. The database is opened; full integrity check; audit verification.
5. A new operating-system slot for this user and machine is added (where no store exists, a new passphrase slot, the
   passphrase typed twice on the console), slots that cannot be opened here are removed, and the pending audit event
   `recovery_key_used`, marked as awaiting authorisation, is placed in the envelope; the envelope is written by the
   procedure of section 7.3 and read back.
6. The server starts in a restricted state: while that event is pending — after a restart as well — every API route
   refuses except administrator sign-in and the authorisation form (the client's own files are served so that the page
   can load). Other users cannot use the installation until the recovery is authorised (D3).
7. An administrator account **of that installation** signs in and gives an authorisation reference (free text: who
   authorised it, for example the custodians' names or a ticket number). The program cannot check a paper authorisation.
8. First database write: audit `recovery_key_used` (recovery key id, slots added and removed, authorisation reference,
   administrator). Then the pending event is cleared and normal service begins. If the program stops between steps 5
   and 8, the next start opens through the new slot and returns to the restricted state of step 6.
9. The program recommends rotating the recovery key, because it has now been typed on this machine.

If nobody can sign in as an administrator (forgotten password, the administrator has left): O-10 — the local
break-glass reset of task 2.6 (`SIGNIN_HARDENING_NOTE.md`, option D, V-23), never the recovery key.

### 8.5 When an administrator or custodian leaves

- Rotate RK and KR while the program runs (section 11): no data is re-encrypted; every slot and the recovery block are
  rewritten in one envelope generation, and the previous generation is deleted as soon as the new one has been read
  back (section 7.3); the new sheet is confirmed and handed to the new custodian; audit `recovery_key_rotated`.
- The old RK still opens backups made before the rotation, and every other envelope made before it: destroy those
  backups, or keep the old sheet under the same custody until they expire.
- Such an older envelope yields SK, DBK and XS, and these are the current keys as long as they have not been rotated. If the
  person who leaves ever had RK **and** could reach any copy of the envelope made before the rotation (a backup, an
  earlier copy of the folder), rotate the data keys as well — **after** the RK and KR rotation, so that the new data keys
  are never wrapped under the KR that person knows (section 11).
- If the leaving administrator was the only administrator account, a second administrator must exist first; the program
  should warn whenever only one active administrator exists (with task 2.6).

### 8.6 Alternative not proposed for the first version: a public-key recovery slot

The organisation could create a key pair on a separate offline machine and give each PC only the public key; recovery
slots would then be made with HPKE (RFC 9180, base mode; DHKEM(X25519, HKDF-SHA256), HKDF-SHA256 and AES-256-GCM from its
sections 7.1 to 7.3). Gains: the recovery secret never passes through the work PC at setup, and one organisation key can
cover several PCs. Costs: new crates (an X25519 and HPKE implementation, whose licences and trees must pass C1 and C5), a
key-generation tool, and the slot can only be checked by a test recovery. The envelope format reserves a slot kind for it;
O-11.

## 9. Migration of existing installations

### 9.1 Rules

From `DESKTOP_KEY_ASSESSMENT.md:55-57`: existing installations keep working unchanged until an administrator asks for
encryption; nothing is migrated silently; copy, then verify, then swap; the old key file and the plaintext database are
removed only after the encrypted database has been opened with keys read back through the operating-system slot; every
step is recorded in a journal, repeatable, and resumable. A stop at any point leaves either the old installation complete
and working or the new one complete and working.

### 9.2 Preconditions (checked when requested, and again before the copy)

- A signed-in administrator; the single-instance lock held (`desktop.rs:129-148`).
- Full integrity check (`storage.rs:169-183` with `quick = false`) reports no problem; audit verification is intact
  (`main.rs:1125-1190`); every stored file verifies (`storage.rs:424-440`). Any problem stops the request: a damaged
  installation is repaired or restored first, never migrated.
- The operating-system store is available, or the passphrase slot has been set up.
- RK made and confirmed (section 8.2).
- Free space as in section 9.5.
- The administrator confirms that a backup exists (the program cannot check this; the answer is recorded).
- S0 runs while the program serves. S1 to S4 run at the next start, after the lock and before `build_app`
  (`desktop.rs:197, 214`; section 7.6, step 2), so no connection of the running program is open on either file; S5 is
  the start-up that `build_app` performs on the encrypted database (`main.rs:3556`), and S6 follows its first write.

### 9.3 Steps, and what a stop at each step leaves

| Step | Action | If stopped during or after this step, the next start … |
|---|---|---|
| **S0** Request (program running; administrator) | Preconditions. Make DBK, KR, KEK-OS (or the passphrase slot) and RK; write `key-envelope.pending.json`; read it back through the store; RK typed back and used to open the recovery block; write `encryption-journal.json` (step `requested`); audit `storage_encryption_requested` in the plaintext database | Without a journal: deletes the pending envelope and its store item (found by the slot id in the pending envelope) and runs as before. With a journal: continues at S1 |
| **S1** Checkpoint (at the next start, after the lock and before `build_app`: section 9.2) | Preconditions again; open the plaintext database; `PRAGMA wal_checkpoint(TRUNCATE)`; close the connection; rule F1 (no side file); size and SHA-256 of the plaintext database file, and the absence of side files, recorded in the journal (`checkpointed`) | Repeats S1 |
| **S2** Copy | Export into `nexus-fabric.sqlite.encrypting`, keyed with DBK before its first statement (method per G-5); checkpoint the copy; close both connections; rule F1; fsync (`copied`) | Deletes the partial copy and its side files (F2), continues at S1 |
| **S3** Verify (read-only) | Open the copy with DBK; full integrity check; per table, row count and a SHA-256 over all rows in rowid order equal those of the source; `user_version` and `meta` equal; audit verification gives the same result; the copy does not start with the SQLite header string ("Database File Format", magic header string) and contains none of a set of plaintext probes taken from the source; close both connections; rule F1 (`verified`) | Repeats S3 |
| **S4** Swap | Pre-check: open the plaintext database, `PRAGMA wal_checkpoint(TRUNCATE)`, close it; rule F1 for both files; size and SHA-256 of the plaintext file equal to those recorded at S1. Any difference — a side file, a changed size or hash — deletes the copy and its side files and goes back to S1. Then, each rename written to the journal before it is made: (a) `nexus-fabric.sqlite` to `nexus-fabric.sqlite.plaintext-old`; (b) `.encrypting` to `nexus-fabric.sqlite`; (c) `key-envelope.pending.json` to `key-envelope.json`; directory fsync on Unix (`swapped`) | Completes the remaining renames, decided from the journal and the names present. A `nexus-fabric.sqlite`, or a side file beside that name, found after rename (a) and before (b) was made by something else since (for example an older version, section 9.6): it is moved aside under a name that says so and reported, never paired with the encrypted file |
| **S5** Open | Start-up of section 7.6 on the encrypted database; first write, one transaction: the audit anchor verified with SK and authenticated again with AK, the XS fingerprint written to `meta`, audit `storage_encrypted` (envelope generation, slot kinds, recovery key id) (`opened`) | Repeats S5; writes the audit entry only if it is not there yet |
| **S6** Retire plaintext | Delete `storage.key`; delete `nexus-fabric.sqlite.plaintext-old`, with any `-wal`, `-shm` or `-journal` beside **that** name (they exist only if a tool has opened the file since S4; the plaintext database's own side files cannot exist after S4, rule F3); directory fsync; delete the journal (`done`). If the owner decides to rotate SK at migration (O-24), the SK rotation of section 11 starts after S5 as its own resumable procedure | Repeats the deletions |

**Side files.** SQLite pairs a database with its write-ahead log, its WAL index and its rollback journal by file name
alone — `<name>-wal`, `<name>-shm`, `<name>-journal` (`libsqlite3-sys-0.40.0/sqlite3/sqlite3.c:63928, 63942` and, for
the Unix WAL index, `:45659`; SQLite documentation "Write-Ahead Logging" and "Temporary Files Used By SQLite").
Committed writes can sit in the `-wal` file and not yet in the main file, so a hash of the main file alone does not
show them; and a side file left beside a name belongs to whatever file carries that name next. The steps therefore
follow four rules:

- **F1** Every step that opens a database closes every connection it opened, and writes its journal marker only after
  a directory listing shows no `-wal`, `-shm` or `-journal` beside the plaintext database or the copy. In the stock
  library, closing the last connection after a successful checkpoint deletes the `-wal` and `-shm` files unless the WAL
  was made persistent (`sqlite3.c:69473-69517`); G-4 confirms this for the chosen build. A side file that remains stops
  the step: the plaintext installation stays in use, the reason is reported, and the step is tried again at the next
  start.
- **F2** The side files of the copy (`nexus-fabric.sqlite.encrypting-wal`, `-shm`, `-journal`) are deleted together with
  the copy whenever S2 is repeated or the migration is rolled back; they are never renamed onto another name.
- **F3** The side files of the plaintext database are never deleted by the migration: a `-wal` may hold committed writes,
  for example of an older version that was killed (section 9.6). They are folded into the main file only by opening the
  database and checkpointing (S1 and the S4 pre-check), which also changes the main file's hash. S4 renames only when
  none exists, so after S4 no plaintext side file can be paired with the encrypted file.
- **F4** The migration's own connections are closed before S4 (F1). On Windows a rename is expected to fail while any
  process holds the file open — SQLite opens files with read and write sharing only (`sqlite3.c:53698`), and antivirus
  or backup software may hold them too (section 9.7; W-7) — and on Unix a connection left open would go on using the
  renamed file and its old side files. A test that leaves a connection open on purpose (KM-9) must see S4 make no
  rename.

Kill points for tests use the existing pattern of fault points compiled out of the product (`main.rs:307-322`), plus
real kills of the built program (section 17, KM).

### 9.4 Rollback

- Before S4(b): delete the copy with its side files (F2) and the pending envelope, remove the store item, undo rename (a)
  if made (no plaintext side file exists then, F3), delete the journal, audit `storage_encryption_cancelled` in the
  plaintext database.
- Between S4(b) and the S5 audit entry: the same, after moving the encrypted file aside together with any `-wal`, `-shm`
  or `-journal` now beside `nexus-fabric.sqlite` — after S4 those belong to the encrypted file (F3) and must not be
  paired with the plaintext file when rename (a) is undone. No work has been written yet.
- After S5: no automatic rollback, because writes made after S5 would be lost. Going back means restoring the backup from
  before the migration (losing later work) or the export tool of O-13, if the owner wants one.
- When S6 runs — at once after S5 (recommended) or only after an administrator confirms — is O-12. Until S6 an
  unencrypted copy is on the disk, and the program says so.

### 9.5 Disk space and time

- S2 needs free space for a second copy of the database file after the checkpoint, plus the growth caused by the per-page
  encryption data (measured under G-8, not estimated here), plus a margin (O-12). Stored files are not copied, because SK
  does not change — unless the owner decides to rotate SK at migration (O-24); the per-file procedure of section 11
  then needs free space for one stored file at a time, and its time is measured like the rest.
- Free space is checked at S0 and again before S2. If it is short, the step stops and the plaintext installation stays in
  use; a full disk during S2 is handled like a stop (partial copy deleted).
- Time grows with the database size; it is measured on the test machines and not estimated here.

### 9.6 Older program versions

- An older version started on a migrated folder fails at `PRAGMA journal_mode = WAL` (`storage.rs:297`), before the key
  is read (`storage.rs:384`): nothing is written and no key is created. Its message wrongly says the database is damaged;
  older versions cannot be changed, so the release notes of the new version must say so.
- An older version started during a half-done migration (before S4) runs on the plaintext database and may write to it.
  Stopped cleanly, it checkpoints (`desktop.rs:274`); killed, it can leave its writes in `nexus-fabric.sqlite-wal`, not
  in the main file. The S4 pre-check opens and checkpoints the plaintext database first (F3), so either way the main
  file's hash differs from S1's and the migration goes back to S1: those writes are copied, not lost.
- An older version started between renames (a) and (b) of S4 finds no `nexus-fabric.sqlite`, creates one, writes its
  schema into it (`storage.rs:291, 317`) and refuses at the installation marker, which the data folder still holds
  (`storage.rs:368-372`). The resumed S4 moves that file and its side files aside (section 9.3).

### 9.7 Windows specifics

- A rename fails while another program (antivirus, indexer, backup software) holds the file open. The step is retried;
  if it still fails, it stops and resumes at the next start.
- A directory cannot be flushed on Windows (`storage.rs:133-137`); whether renames survive a power cut depends on the file
  system and is not tested (section 16).

### 9.8 New installations

**A first start creates nothing until a choice is made.** A data folder "holds no installation" when it has no database
file, no key envelope, no journal and nothing under `blobs/`; `desktop.lock`, which `lock_data_dir` creates before
anything else (`desktop.rs:131-138`, called at `:197`), does not count. At a start on such a folder the program creates
nothing — today `open_storage` would create `blobs/`, the database, the installation id and the marker at once
(`storage.rs:286-387`) — and asks on the console: **new installation**, **restore from a backup**, or stop. How the
choice is given (a console question, or command-line options added to `parse_args`, `desktop.rs:36-66`, for tests and
scripted installs) is O-27.

- **New:** created encrypted: SK, DBK, XS and KR made, the operating-system slot written (or, where no store exists, the
  passphrase slot), the database created with DBK; then first-run setup as today, protected by the setup token
  (`desktop.rs:6-11`). The recovery key is made with the first administrator during first-run setup. Whether project
  creation waits for a confirmed recovery key: O-9. A plaintext key file is never created (section 7.6, step 6).
- **Restore:** section 10.4, on the console, into the folder that still holds no installation.

## 10. Backup and restore on another machine

### 10.1 What a backup contains

| Part | How it is made | Protected by |
|---|---|---|
| Database snapshot | SQLite's online backup API from the open connection into a destination connection that the program opens itself on a new file and keys with the same DBK before its first statement (section 6.5): `Backup::new` (`rusqlite-0.43.0/src/backup.rs:194`), then `run_to_completion` (`:294`). Not `Connection::backup`, which opens the destination itself without a key (`backup.rs:71`), and not a file URI (section 6.5). rusqlite's `backup` feature (`rusqlite-0.43.0/Cargo.toml:76`) is not enabled today (`Cargo.toml:17`); G-5 | DBK; the destination is checked to be ciphertext before the backup is reported as written (G-5, BK-1) |
| Stored files and the installation marker | Copied as they are | SK (already sealed) |
| Key envelope | Copied | Its operating-system slot is useless on another machine; its recovery block needs RK. A passphrase slot in it can be attacked by guessing the passphrase offline (section 7.5; O-25, O-26) |
| Manifest | Every file with size and SHA-256; installation id; envelope generation; program version; time | Detects missing or damaged files. Deliberate replacement is detected by page and file authentication, not by the manifest |
| **Never included** | RK, the store item, a passphrase, a plaintext key file, a plaintext database, `key-envelope.prev.json`, any journal | — |

Order: the snapshot first, then the stored files. A stored file is written before the transaction that refers to it
commits (an upload: `main.rs:1439` before its record; a job: `persist_calc` writes its DXF files at `main.rs:1959-1960`
inside the caller's transaction, `main.rs:1946-1948`), the snapshot contains committed transactions only, and a stored
file never changes once written (named by its content, `storage.rs:446-453`). So every file the snapshot refers to
exists when the files are copied.

Every backup writes the audit entry `backup_written` (manifest hash, envelope generation, destination as typed by the
administrator) into the installation's own audit chain.

### 10.2 Destination

A folder the administrator chooses (removable medium or file share). The program only writes files; it has no network
protocol of its own.

### 10.3 Copies made by other software

Backup software that copies the data folder while the program runs may capture the database and its WAL at different
moments. Such a copy is still encrypted but may fail the integrity check on restore. Use the program's backup, or copy
while the program is stopped (a clean stop empties the WAL, `desktop.rs:274`). The same advice exists for server mode
(`deploy/README.md:68`).

### 10.4 Restore on another machine

1. Install the program. The data folder must hold no installation (section 9.8): a restore never overwrites an
   installation.
2. Start desktop mode and choose "restore from a backup" at the first-start choice (section 9.8; O-27). The restore runs
   on the console, after the lock and before the database is opened or any port is bound (section 7.6, step 2); the
   path of the backup folder is typed there.
3. Check the manifest (every file present, sizes and SHA-256 equal); copy into the data folder under a restore journal
   (`restore-journal.json`); check again after copying. A stop during the copy leaves the journal; the next start
   deletes what was copied and asks again.
4. Steps 3 to 9 of section 8.4: RK typed on the console, unwrap, full checks, every stored file verified
   (`storage.rs:424-440`), new slot for this machine, the pending event `restored_from_backup` (manifest hash, recovery
   key id, new slot id) awaiting authorisation, the restricted start until an administrator of the restored
   installation signs in and gives the authorisation reference, then the audit entry with that reference, and the
   recommendation to rotate the recovery key.

### 10.5 What a restore cannot detect

- An older backup restored in place of a newer one (rollback).
- The original PC still in use: two copies with the same installation id diverge. Without a network this cannot be
  detected, and the copies cannot be merged.

### 10.6 Acceptance

The restore on a separate machine is part of acceptance (D3; `DESKTOP_KEY_ASSESSMENT.md:57`) and of the test plan (BK-2,
W-12).

## 11. Key rotation

Two kinds: **re-wrapping** (a key-encryption key changes; data is not touched) and **re-keying** (a data key changes;
the data is encrypted again).

| Key | When (cryptoperiods and triggers: O-15) | What is encrypted again | Needs | Stopped half-way |
|---|---|---|---|---|
| KEK-OS (the operating-system slot) | Account change; move to another PC (part of restore); suspected exposure of the store | That slot of the envelope only | The running installation | Envelope rewrite of section 7.3; the old store item and the previous generation, with the replaced slot, are deleted only after the new slot has been read back |
| RK and KR (recovery) | A custodian or administrator leaves; after every use of RK; suspected exposure of RK **or of any slot** (every slot holds KR: a copied profile whose password was guessed, a guessed passphrase, same-user malware) | **Every slot** (each holds KR) **and** the recovery block, in one envelope generation: new RK, new KR; SK, DBK and XS unchanged | Running installation, an administrator; the new sheet confirmed by typing the new RK back **before** the envelope is written; KEK-PW for a passphrase slot (O-23) | Envelope rewrite of section 7.3: the previous generation — old recovery block, old KR in every slot — is deleted as soon as the new one has been read back |
| DBK | Suspected exposure (for example a person who could have combined RK with a copy of the envelope); cryptoperiod | The whole database, by the copy-then-swap of section 9 (old DBK to new DBK instead of plaintext to DBK); the new DBK is wrapped into every slot and the recovery block | Free space as in section 9.5; KEK-PW for a passphrase slot (O-23) | As section 9. The previous envelope generation is kept as `key-envelope.prev.json` under the rotation journal, with the old database file, until S6 deletes both (section 7.3) |
| SK | Suspected exposure; the number of files sealed under SK nearing the limit of NIST SP 800-38D section 8.3 (at most 2^32 encryptions under one key with random 96-bit nonces; the nonce is random, `crypto.rs:36`); cryptoperiod | Every value sealed under SK — after this design the stored files only (factor secrets are sealed under FK, the anchor is authenticated with AK) — one at a time, sealed again with the existing temporary-file, fsync and rename steps (`storage.rs:454-475`). At the start, in one transaction: SK fingerprint in `meta` updated (`storage.rs:254-279`), audit `storage_key_rotation_started` | Free space for one stored file at a time; KEK-PW for a passphrase slot (O-23) | Per file. While it runs the envelope holds the old and the new SK; a file is opened with the new SK, else the old one — unambiguous, because AES-GCM refuses a wrong key (`crypto.rs:45-55`). At the end: audit `storage_key_rotation_completed`, old SK removed from the envelope |
| XS (and with it AK and FK) | Suspected exposure; cryptoperiod (O-15) | In one database transaction: the audit anchor re-authenticated with the new AK (`main.rs:289-293, 367-376`, after the anchor has been verified with the old AK, task 2.2), every factor secret of task 2.6 sealed again under the new FK, the XS fingerprint in `meta` updated, audit `auxiliary_key_rotated`; the new XS wrapped into every slot and the recovery block | The running installation; KEK-PW for a passphrase slot (O-23) | The database transaction either commits or not. The envelope generation with the new XS is written under the journal of section 7.3 before the transaction and made current only after it commits; a stop between the two leaves the previous generation in use, recognised by the XS fingerprint in `meta` |

Notes:

- Every change of SK, DBK, XS or KR rewrites every slot and the recovery block in one envelope generation (section
  7.3), because every slot holds all four (section 7.2); this includes the SK rotation, at its start and at its end. A
  slot whose key-encryption key is not available here is removed, and that is reported.
- Order: when RK **or any slot** may be exposed and the data keys are to be rotated too, RK and KR are rotated
  **first**, then DBK, XS and SK, so that the new data keys are never wrapped under an exposed KR (tests RT-5, RT-7).
  Opening one slot yields KR as well as the data keys, so rotating DBK, XS or SK alone after a slot exposure protects
  nothing: the new keys would be wrapped under a KR the attacker holds, in every later envelope.
- A passphrase change or a KEK-OS rotation protects only copies made afterwards: an older copy of the envelope still
  opens the way it did when it was made, and yields the current SK, DBK, XS and KR until those are rotated.
- No count of encryptions per key is kept today (`crypto.rs:34-43`); the design adds one per SK in `meta`, so the
  SP 800-38D limit can be enforced.
- A sealed file carries no key identifier (`crypto.rs:33`). Trying the new key, then the old one, avoids a format change.
  A versioned header with a key identifier would give clearer messages but would make older versions unable to read new
  files: O-22.
- Backups keep the keys they were made with; a rotation does not reach them. To retire a key entirely, destroy or re-make
  the backups made under it. The recovery key that was current when a backup was made opens that backup.
- Durations grow with the bytes involved and are measured, not estimated. The question in
  `DESKTOP_KEY_ASSESSMENT.md:58` ("how long it takes on a 2 GB store") is answered by measurement after implementation.

## 12. Failure cases, stated plainly

| Event | Result |
|---|---|
| Operating-system profile lost (Windows reinstalled, account deleted, local-account password reset by an administrator, keychain or keyring reset); recovery key available | Recovery with RK (section 8.4) |
| The same, **without the recovery key** | **The data and every backup made under these keys cannot be recovered by anyone.** There is no vendor key and no other way in |
| Passphrase forgotten (fallback slot), no recovery key | Unrecoverable |
| Every copy of the key envelope lost or damaged (data folder and backups) | **Unrecoverable even with the recovery key**: the recovery key opens only the recovery block inside the envelope |
| Recovery key lost; the operating-system slot still works | The program keeps working; rotate the recovery key at once. Backups that need the lost key become unrecoverable once this PC is lost |
| Recovery key kept beside the data or a backup | That copy is not protected |
| A person who held the recovery key leaves | Rotate RK and KR; if that person could have combined the key with any copy of the envelope made before, rotate DBK, XS and SK too, in that order (section 11). Backups made before stay readable to that person if they hold a copy |
| A slot may have been opened by someone else (copied profile with a guessable password, guessed passphrase, same-user malware) | That party holds KR as well as the data keys: rotate RK and KR first, then DBK, XS and SK (section 11). Rotating only the data keys protects nothing |
| A copy or backup made before the migration | Readable, and it holds `storage.key` in plaintext: that key opens the stored files of any later copy unless SK was rotated (O-24) |
| A copy or backup that holds a passphrase slot | The passphrase can be guessed offline; only its strength (O-25) and the Argon2id cost (O-5) limit this |
| A copy of the whole user profile or disk | The operating-system store comes with it; the keys are protected only by the account password (section 7.4) |
| Stop, crash or power cut during migration or rotation | Resumed or rolled back from the journal (sections 9, 11). Power cuts are not tested (section 16) |
| Disk full during migration or rotation | The step stops; the previous state stays in use |
| Older program version on a migrated folder | Refuses at `storage.rs:297` with a misleading "damaged" message; writes nothing; creates no key (section 9.6) |
| No administrator can sign in after a restore | O-10 (with task 2.6) |
| The same backup restored and used on two PCs | Two diverging copies; not detectable and not mergeable |
| Store unavailable at start (Linux) | Passphrase slot if set up; otherwise refusal with the reason and the offer of recovery |

## 13. Interaction with the audit chain, closure hashes and blob store

- **Audit chain.** Entry hashes are computed over plaintext fields (`main.rs:280-286`); page encryption is below SQL and
  does not change them. The migration copies every row unchanged, which S3 verifies, so the chain continues across the
  migration. Today the anchor is authenticated with SK (`main.rs:289-293`). The migration moves it to AK: in the
  transaction of the first write at S5 (section 9.3), the anchor is verified with SK (with the verification of task 2.2,
  RA-003) and authenticated again with AK, together with the audit entry `storage_encrypted`; from then on SK is never
  used for the anchor, and an XS rotation re-authenticates it (section 11). The limitation text returned by
  verification (`main.rs:1190`) must be updated to what the new code does: a party without the keys cannot rewrite the
  chain at all (the pages are authenticated with DBK); a party with the envelope's keys can, because DBK and XS sit in
  the same envelope, so for that party this is one barrier, not two; rollback of a whole database stays undetected
  (O-28). The separate AK removes the cross-use with SK: a pre-migration copy's `storage.key` (A1, O-24) no longer
  re-signs the anchor.
- **New audit actions** (append-only; ids, fingerprints and generations only, never key material):
  `storage_encryption_requested`, `storage_encryption_cancelled`, `storage_encrypted`, `os_key_slot_added`,
  `os_key_slot_removed`, `recovery_key_set`, `recovery_key_rotated`, `recovery_key_used`, `restored_from_backup`,
  `backup_written`, `database_key_rotation_started` and `_completed`, `storage_key_rotation_started` and `_completed`, `auxiliary_key_rotated`.
- **Ordering.** The database cannot be opened before its key is recovered, so recovery and restore events wait in the
  envelope's `pending_audit` and become the first write after an administrator has given the authorisation; until
  then the program serves only administrator sign-in and the authorisation form (section 8.4).
- **Failed unlocks** cannot be written into the encrypted chain, because without a key it cannot be opened; they appear
  on the console only, unless the owner wants a separate plaintext record (O-17).
- **Closure hashes and record ids** are content hashes over plaintext (`crates/nf-core/src/lib.rs:309`); results,
  records and their ids do not change. The replay child receives only the request on standard input
  (`main.rs:2397-2410`) and never needs a key; it inherits the environment, which is why no key or passphrase may ever be
  placed there.
- **Blob store.** Format unchanged (`crypto.rs:33-55`); SK unchanged by the migration unless O-24 decides to rotate it,
  so otherwise no stored file is touched then.
  Names stay the SHA-256 of the plaintext (`storage.rs:419-421`): see section 14 and O-16. Records stay append-only: a
  rotation re-encrypts bytes, never content.

## 14. What this design does not protect or does not model

- **Metadata in a copied folder:** sizes of the database and of each stored file (a sealed file is the plaintext plus a
  fixed nonce and tag, without padding, `crypto.rs:33-43`); the number of stored files; their names, which are the
  SHA-256 of their contents, so anyone holding a candidate document can check whether it is stored; modification times;
  the installation id; WAL frame headers.
- **Rollback** of the database, a page, the envelope or a backup to an older authentic version of itself.
- **Plaintext written before the migration:** earlier copies and backups; the deleted plaintext file's blocks on an SSD;
  restore points. These copies also hold `storage.key` in plaintext (`desktop.rs:200`), and unless SK is rotated at
  migration (O-24) that key keeps opening the stored files of every later copy, including files written after the
  migration.
- **Offline guessing:** a passphrase slot travels with every copy and backup and can be attacked offline, limited only
  by the passphrase's strength (O-25) and the Argon2id cost (O-5); a copy of the whole user profile or disk carries the
  operating-system store, which is then protected only by the account password (section 7.4).
- **Downloads and exports** made by users.
- **Memory, swap or page file, hibernation file and crash dumps**, which can hold keys and decrypted pages. The design
  overwrites key buffers after use where the language allows; copies made by the operating system cannot be removed.
- **Same-user malware, administrators, unlocked PCs, keyloggers** that capture a typed recovery key or passphrase.
- **A recovery performed on a stolen copy** is recorded only in that copy.
- **Server mode, TLS, and one data folder shared by several Windows accounts** (the data folder is per user today,
  `desktop.rs:74-98`).

## 15. Consequences for the offline check, packaging, notices, status report and tests

- **Offline check** (`tests/offline_check.sh`): the static part stays. Added (O-20):
  1. the shipped binaries import no OpenSSL library — Linux: the dynamic section; Windows: the import table of the exe.
     Both lists are recorded in full at G-2 and accepted by the owner (O-20); the check then compares against the
     accepted lists. No list is predicted here: DPAPI is in `crypt32.dll` (`windows-sys-0.61.2/.../Cryptography/mod.rs:283`), and
     the C library's random-number call on Windows (G-10) may add another import;
  2. the C sources compiled into the program contain no network calls;
  3. if zbus is adopted, a test proves that every address in `DBUS_SESSION_BUS_ADDRESS` that is not `unix:` is refused
     before any connection is made — `unixexec`, `tcp`, `nonce-tcp`, `vsock`, `ibus`, `launchd`, `autolaunch` and any
     other (`zbus-5.19.0/src/address/transport/mod.rs:236-248`; section 7.4);
  4. the dynamic part runs the desktop suite in a namespace with loopback only (`offline_check.sh:34-48`), where no
     session bus exists: the suite uses a Secret Service started inside the namespace, or the passphrase slot. The shipped
     program has no plaintext "test key" mode;
  5. the dependency grep runs for every target (`cargo tree --target all`, The Cargo Book, `cargo tree`), or at least for
     both packaged targets as `tests/notices_check.sh:14, 56-58` already does, so a Windows-only crate for DPAPI or a
     keyring is checked on the Linux build host; dev-dependencies stay included (C1);
  6. the G-6 SQLCipher cross-check build lives outside `crates/` and outside nf-server's dependency tree — a separate
     tool, as `validation/` is — and uses the LibTomCrypt back end, so it needs no OpenSSL. It is also the first build of
     option C.
- **Packaging:** the library is compiled in statically, so "no SQLite DLL is needed" (`docs/PACKAGING.md:137`) stays true.
  The C build step is added to `docs/BUILD_AND_TEST_GUIDE.md`; every command of work-plan section B keeps working.
- **Notices:** the C library's licence text is written by hand into the template `packaging/about.hbs` (the SQLite
  paragraph, `about.hbs:14-16`, is the model) and the notices file is regenerated with `packaging/generate_notices.sh`;
  the generated `packaging/THIRD_PARTY_NOTICES.txt` is never edited by hand (its line 12). `tests/notices_check.sh` checks
  the result; new crates go through `packaging/about.toml:6-16`.
- **Status report:** `/api/security/status` (`main.rs:1257-1260`) must report the truth: database encryption; key
  protection kind (DPAPI user scope, Keychain, Secret Service, passphrase, or plaintext key file for a not-yet-migrated
  installation); whether a recovery key is set and confirmed and when it was last used; the last rotation of each key. No
  secret values.
- **Wording and registers:** the task 2.4 wording stays until this design is built and tested. V-21, V-22, BH-017, T9,
  T12 and T22 change only with that evidence (`READINESS.md:3-4`).
- **Tests that open the database file directly** — `tests/security_smoke.sh`, `tests/security_r6.sh`,
  `tests/desktop_smoke.js`, `tests/release_verify.js`, `stress/run.py`, `stress/nfstress.py`, and `tests.rs:764, 1119,
  2143, 2450` — get a keyed, test-only way in. None is weakened or removed (work plan, section A rule 2). The existing
  tamper tests keep modelling the insider who has the keys; section 17 adds the attacker who has none.

## 16. What cannot be tested without real Windows and macOS machines

This build machine has no Windows, no macOS and no desktop session; wine does not implement DPAPI faithfully
(`DESKTOP_KEY_ASSESSMENT.md:40`). Each item needs a person with a real machine.

| ID | Machine | Test |
|---|---|---|
| W-1 | Windows | User-scope DPAPI: wrap as user A; user B on the same PC cannot unwrap; a copied folder on another PC cannot be unwrapped; a copy of user A's whole profile restored on another PC: whether and with what it unwraps (records the dependence on the Windows password) |
| W-2 | Windows | Machine scope, if chosen: user B **can** unwrap (records the weaker protection) |
| W-3 | Windows | The user changes the password: still unwraps |
| W-4 | Windows | An administrator resets a local account's password: unwrap fails; recovery works |
| W-5 | Windows | Account without password; Microsoft account; PIN sign-in; domain-joined PC: behaviour recorded |
| W-6 | Windows | Windows reinstalled or a new profile: slot unusable; recovery works |
| W-7 | Windows | Process killed (TerminateProcess) at every migration step; renames while antivirus or the indexer holds the file; a rename while the migration's own connection is open (expected to fail, F4) |
| W-8 | Windows | Whether restore points, File History, profile backups or PC-migration tools copy `%LOCALAPPDATA%\NexusFabric\data`, and whether they copy the DPAPI master keys with it; such a profile copy restored on another PC (with W-1) |
| W-9 | Windows | Directory flush: `storage.rs:133-137` skips it on Windows while `storage.rs:474` does not; which behaviour a real PC shows decides the file steps of section 9 |
| W-10 | Windows | The new installer over an existing plaintext installation: it stays plaintext and working until an administrator asks for encryption |
| W-11 | Windows | Uninstall with "delete data" ticked removes the envelope with the folder |
| W-12 | Windows | Backup to removable media and restore on a second Windows PC (D3) |
| M-1 … M-6 | macOS | Everything about the Keychain: a signed build at all (none exists, `docs/PACKAGING.md:232`); the access rule before and after an update; the login keychain after an administrator password reset; the item never synchronised; FileVault on and off; restore on a second Mac; a copy of the home folder on another Mac: whether the keychain item comes with it and opens with the login password |
| L-1 … L-4 | Linux desktop (a non-live machine; work plan task 0.1) | GNOME Keyring and KeePassXC; a locked collection; automatic sign-in; same-user access to the item recorded; a copy of the home folder on another machine: whether the keyring comes with it and opens with the login password |
| — | Any | A real power cut during migration or rotation, and what an SSD keeps of deleted plaintext: not testable by software alone |

## 17. Test plan

Rules: every test fails before the feature exists and passes after; results are recorded with their output; "not run" is
never "passed" (work plan, section A rule 11).

| ID | Test | Runs here? |
|---|---|---|
| CF-1 | Copy the data folder of a migrated installation (program stopped) to another local account: refusal, offer of recovery, no file in the copy changed (hashes before and after) | Linux yes; Windows W-1 |
| CF-2 | The same on another machine | Linux yes (second container); Windows W-1 |
| CF-3 | Copy taken while the program runs (database and WAL): without RK refusal; with RK it opens or reports damage — never serves partial data silently | yes |
| CF-4 | Every file of a copied folder, and the temporary folder after a large-sort workload, searched for plaintext probes (user names, project names, file names, record values, the SQLite header string): found only in the documented places (marker id, stored-file names) | yes |
| CF-5 | Stock `sqlite3` on the copied database: "file is not a database" | yes |
| CF-6 | Copy plus the right RK on a separate machine: full recovery, audit entry present once | yes; W-12 |
| CF-7 | A folder not yet migrated is readable when copied (keeps the "restricted deployment" wording truthful) | yes |
| TM-1 | One byte changed in a database page: the page fails authentication; start-up refuses or the request fails; altered data is never served | yes |
| TM-2 | One byte changed in page 1: refusal | yes |
| TM-3 | One byte changed in a WAL frame: no altered data served; what happens to later transactions is recorded | yes |
| TM-4 | Two pages swapped: detected (if the chosen scheme binds the page number; G-6) | yes |
| TM-5 | Database replaced by an older copy encrypted with the same DBK: **not** detected — the test asserts the documented limitation, so any change shows | yes |
| TM-6 | Each envelope field changed in turn (installation id, slot id, kind, wrapped bytes, fingerprints): refusal; no key created | yes |
| TM-7 | Envelope of another installation: refusal | yes |
| TM-8 | Envelope missing beside an encrypted database or stored files: refusal; nothing created (BE-010 rule) | yes |
| TM-9 | Damaged DPAPI output, missing Keychain or Secret Service item: refusal and offer of recovery | W-, M-, L- |
| TM-10 | A plaintext `storage.key` put back into a migrated folder: not used; reported | yes |
| TM-11 | A stored file replaced by another authentic one: content-hash mismatch (`storage.rs:436-438`) — existing behaviour kept | yes |
| TM-12 | Existing tamper tests (`tests/security_smoke.sh`, `tests/security_r6.sh`) still pass with the keys supplied, as the insider case they model | yes |
| TM-13 | An older generation planted as `key-envelope.prev.json`: without a journal it is not used, is reported and deleted; with a forged `key-envelope.journal` as well, the program uses either the current or the planted authentic generation — the rollback that A8 documents as not detected — and creates no key | yes |
| KM-1 … KM-6 | Kill (SIGKILL) at every boundary of S0 … S6, inside S2, and between each rename of S4; then start: exactly one installation complete and working, row hashes as before, audit chain intact, no orphan store item, no plaintext key file after `done`, and never a `-wal`, `-shm` or `-journal` of one file paired with the other | Linux yes; Windows W-7 |
| KM-7 | Disk full at S2, S4 and S6; read-only data folder; store unavailable at S5 | yes (Linux) |
| KM-8 | Before S4, the plaintext database is left with un-checkpointed writes in its `-wal` (an older version killed after writing): the S4 pre-check folds them in, goes back to S1, and every one of those writes is in the encrypted database after `done` | yes |
| KM-9 | A connection of the migration deliberately left open at S4 (a fault point compiled out of the product, `main.rs:307-322`): F1 fails, no rename is made, the plaintext installation stays in use | yes; Windows W-7 |
| RT-1 | Kill during SK rotation, file by file: mixed store opens; resume completes; anchor verifies; fingerprint updated; old SK removed only at the end | yes |
| RT-2 | DBK rotation killed at each step (reuses KM) | yes |
| RT-3 | RK rotation: the old RK opens **nothing** in the current data folder — no `key-envelope.prev.json` remains and no slot holds the old KR — but still opens a backup made before (documented behaviour) | yes |
| RT-4 | Seal counter: rotation forced before the SP 800-38D section 8.3 limit (the limit lowered in a test-only build, never in the product) | yes |
| RT-5 | RK rotation, then DBK rotation, then SK rotation: the new RK recovers the installation on a separate machine; the old RK opens nothing in the current folder | yes |
| RT-6 | A passphrase slot after RK, DBK and SK rotations: still opens with the passphrase (per O-23) | yes |
| RT-7 | Slot exposure: a test takes KR from one slot, then the installation rotates RK and KR, then DBK, XS and SK; the taken KR opens nothing in the new envelope. The same sequence without the RK and KR step: the taken KR opens the new recovery block (asserts the documented reason for the order) | yes |
| RT-8 | XS rotation killed before and after its database transaction: the anchor verifies with the AK that matches the XS fingerprint in `meta`; factor secrets open; no mixed state | yes |
| AK-1 | Migration: after S5 the anchor verifies with AK and fails with SK; the old `storage.key` cannot re-sign it; HKDF known-answer test (RFC 5869 test case 3) and one fixed vector per label | yes |
| RC-1 | Wrong RK: check-value error naming the group; two groups swapped: refused | yes |
| RC-2 | RK of another installation: refusal | yes |
| RC-3 | Kill between the envelope write and the audit write of a recovery: the next start returns to the restricted state; after the authorisation the event is written exactly once | yes |
| RC-4 | Recovery without a usable administrator account: behaviour per O-10 | yes, after O-10 |
| RC-5 | While a recovery or restore awaits authorisation, before and after a restart: every API route except administrator sign-in and the authorisation form refuses, for every role; the client's files still load | yes |
| RC-6 | Recovery, restore or a passphrase prompt with standard input not a terminal: refusal with the reason; nothing created | yes |
| NK-1 | New encrypted installation with no rows, started and stopped: no `storage.key` exists at any point | yes |
| BK-1 | Backup while jobs are being written; manifest complete; the database snapshot is ciphertext (no SQLite header string); no plaintext probe and no plaintext key in the backup | yes |
| BK-2 | Restore on a separate machine: every job readable, every DXF downloadable (the BE-024 test, `docs/backend/DEFECT_REGISTER.md:43`) | yes; W-12, M- |
| BK-3 | Damaged or missing file in a backup: the manifest check refuses | yes |
| BK-4 | Restore into a data folder that holds an installation (a database file, an envelope, a journal or anything under `blobs/`; `desktop.lock` does not count): refusal | yes |
| BK-5 | First start on a folder that holds no installation: nothing is created before the choice of section 9.8; after "restore", no database, envelope or key exists before the copy | yes |
| BK-6 | Kill during the copy of a restore: the next start deletes what was copied and asks again | yes |
| DG-1 | Older release on a migrated folder: refusal; every file's hash unchanged; no key created | yes |
| DG-2 | Older release used during a half-done migration, once stopped cleanly and once killed with un-checkpointed writes in the `-wal`: the S4 pre-check sends the migration back to S1; no write lost | yes |
| DG-3 | Older release started between renames (a) and (b) of S4: its new file is moved aside at the resume; the encrypted database opens; no side file is paired with it | yes |
| OF-1 … OF-6 | The offline-check additions of section 15 | yes |
| IF-1 | Gate G-6: cross-reading with a test-only SQLCipher build (outside nf-server's tree, LibTomCrypt back end), both directions | yes |
| RN-1 | Gate G-10: the same page written twice gets different IVs; two new databases get different salts; on both targets | Linux yes; Windows build run on a real PC |
| PF-1 | Gate G-8: speed, size and peak memory before and after, desktop and server mode | yes; figures to the owner |
| SM-1 | Every server-mode command of work-plan section B passes on the new library, with server-mode behaviour unchanged (O-19) | yes |

## 18. Open decisions for the owner

| # | Decision | Options | Recommendation |
|---|---|---|---|
| O-1 | Library for the encrypted database | D SQLite3MC / C SQLCipher with LibTomCrypt / G SEE (not evaluated) / keep plaintext (stays "restricted deployment") | D, if G-1 … G-10 pass; else C |
| O-2 | Page scheme within SQLite3MC | SQLCipher-compatible / ChaCha20-Poly1305 | SQLCipher-compatible (independent cross-check, G-6). Unauthenticated schemes excluded |
| O-3 | Integration route | Link a separately built static library through libsqlite3-sys's linked path (`build.rs:463-500`) / patched libsqlite3-sys | Decided after G-2 |
| O-4 | DPAPI scope | User / machine | User |
| O-5 | Linux without a Secret Service; passphrase cost | Passphrase slot (Argon2id with one of the RFC 9106 section 4 options, or another measured on the slowest supported PC) / key file on removable media / no encryption there (restricted) | Passphrase slot; parameters chosen after measurement |
| O-6 | Where the passphrase and the recovery key are typed, and where recovery and restore run | Console, before the database is opened (sections 7.6, 8.4, 10.4) / a locked server start that serves an unlock, recovery and restore page (a larger change: `build_app`, `main.rs:3555-3660`, opens storage before any port is bound, `desktop.rs:214, 221-226`) | Console |
| O-7 | Recovery key presentation | Base32 (RFC 4648 section 6) / hexadecimal like the setup token (`desktop.rs:150-156`); group length; length of each group's check value (or one check value for the whole key, which cannot name the wrong group — RC-1 would change); printed length of the recovery key id; whole or partial re-entry | Base32, a check value per group, whole re-entry; lengths to be set by the owner |
| O-8 | Custody | Two sealed copies / two shares, both needed | Two sealed copies for small sites |
| O-9 | New installations before the recovery key is confirmed | Refuse to create projects / warn only | Refuse |
| O-10 | Recovery when no administrator can sign in | The recovery key may reset an administrator password, recorded in the audit chain / never | Never; the local break-glass reset of task 2.6 (`SIGNIN_HARDENING_NOTE.md`, option D) covers it |
| O-11 | Public-key recovery slot (section 8.6) | Later / now / never | Later |
| O-12 | Retiring the plaintext after migration; free-space margin; offering migration | Delete at S6 automatically / only after confirmation; margin; offered at upgrade / required by a date | Automatic at S6; margin and date are the owner's |
| O-13 | Tool to export back to plaintext (downgrade) | Not provided / provided with RK and an administrator | Not provided |
| O-14 | Exclusive locking in desktop mode (no `-shm`, no second reader) | Yes / no | Yes, with the test changes |
| O-15 | Cryptoperiods and triggers for RK, DBK and SK | Values for the owner; NIST SP 800-57 Part 1 Rev. 5 section 5.3 is a reference | The SP 800-38D section 8.3 limit always enforced for SK |
| O-16 | Stored-file names | Keep the SHA-256 of the contents (documented metadata) / keyed names (every file renamed at migration) | Keep, and document |
| O-17 | Failed unlock attempts | Console only / a separate plaintext record | Console only |
| O-18 | macOS | Build it, with a code-signing identity / not now | Not until a signed build is wanted |
| O-19 | Server mode | Unchanged behaviour on the new library (C7; G-8, SM-1) / adopt the encrypted database with the key file of `NF_KEY_FILE` as its key-encryption key | Unchanged in this task |
| O-20 | Offline-check additions (section 15), including the list of imports and dynamic libraries recorded at G-2 | Yes, and the owner accepts that list / no | Yes |
| O-21 | Acceptance of speed and size | After the G-8 figures | No threshold is assumed |
| O-22 | Key identifier in sealed files | None (try new key, then old, during rotation) / versioned header | None |
| O-23 | Rewriting the passphrase slot when SK, DBK or KR changes (KEK-PW is never stored on disk) | KEK-PW kept in memory for the run when the passphrase opened it / the passphrase typed again on the console / the slot removed and set up again | Kept in memory for the run. With O-26 "only where no store exists" the passphrase slot is then the only slot, every run is opened with it, and KEK-PW is always at hand |
| O-24 | Storage key at migration (copies made before the migration hold it in plaintext, A1) | Rotate SK as part of the migration (the SK procedure of section 11 after S5; every stored file re-sealed; time and disk measured) / keep SK and accept the risk stated in A1, P1 and section 14 | Rotate |
| O-25 | Passphrase rules (the slot can be guessed offline from any copy) | Minimum length, character rules, a check against a list of common passphrases, a generated passphrase instead of a chosen one; values are the owner's (NIST SP 800-63B, its requirements for memorized secrets, is a reference) | No value is proposed here |
| O-26 | A passphrase slot beside a working operating-system slot | Allowed (a second way in, with the offline-guessing exposure) / only where no store exists | Only where no store exists |
| O-28 | A copy of the audit anchor (count, newest hash, newest id, authenticated with AK) outside the database, so that a database rolled back on the same machine is detected (A8) | In the operating-system store item beside KEK-OS (rewritten at every audited write) / not now. It does not help against a copied folder opened elsewhere, and a passphrase slot has no such store | Decide after task 2.2 (RA-003) has made the anchor effective inside the database |
| O-27 | How a first start chooses between a new installation and a restore (section 9.8) | A console question at every start on a folder that holds no installation, with command-line options for tests and scripted installs / a new installation by default, restore only through a command-line option (a first start before the restore then creates an installation that blocks the restore, BK-4) | Console question |

## 19. Observations made while reading (nothing was changed)

Not part of the design; recorded here for the registers (tasks 0.2 and 2.4), where they belong.

- `deploy/README.md:70` says "The database and blobs are unreadable without `master.key`". The database is readable
  without any key today (`storage.rs:290-292`; V-21; `docs/security/SECURITY_ARCHITECTURE.md:8`). For task 0.2 or 2.4.
- `storage.rs:133-137` skips the directory flush on Windows ("Windows cannot open a directory as a file"), while
  `store_blob` flushes the directory without that condition (`storage.rs:474`). Which behaviour a real Windows PC shows is
  not known here; it is test W-9, and the file steps of sections 9 and 11 depend on the answer.
