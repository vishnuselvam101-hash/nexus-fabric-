# Sign-in hardening: offline second factor and account recovery

**DESIGN — awaiting the owner's decision. No code is written until the owner approves this design.**

**Development build. Not validated. Outputs must not be used for manufacturing.**

21 Nov 2026 · Work plan task 2.6 (`docs/project/CLI_WORK_PLAN.md:344-346`) · V-23, High, open
(`docs/security/VULNERABILITY_REGISTER.md:23`, "register" below). `main.rs`, `desktop.rs`, `crypto.rs`, `storage.rs`
are in `crates/nf-server/src/`.

## 0. One page for the owner

Today a password is the only factor, and nobody has a way back in: a sole administrator who forgets the password is
locked out. Every option below works without a network.

| Option | What it stops | Cost (relative; no effort estimate) |
|---|---|---|
| **D.** Local break-glass: with the program stopped, a person at the machine resets one administrator's password from a terminal | Last administrator locked out | Small to medium |
| **C.** Administrator issues a one-time reset code, handed over in person | A user's forgotten password | Small to medium |
| **A.** Time-based codes from a phone app or hardware generator (RFC 6238) | A stolen or guessed password | Medium; one new crate |
| **B.** One-time recovery codes on paper | A lost code generator (used only with A or E) | Small |
| **E.** Security keys (WebAuthn) | As A, plus fake sign-in pages | Large; licence, advisory and parser questions |

Recommended order (section 6): task 2.2 first (the sign-in race RA-002 and the lockout RA-006); then D in both modes;
then C in server mode; then A with B in server mode, administrators first; E only after the owner's decisions on
HTTPS, licence and parsing. In desktop mode only D: whoever copies the data folder bypasses any sign-in factor.

The owner decides (section 7): who must use a second factor; code length, window and step-2 limit; lifetimes of
pending steps and reset codes; the number and format of recovery codes; whether C clears second factors; how D reads
the password; QR enrolment; everything about E. This note sets no values. Sections 1 to 7 are the supporting detail.

## 1. Current state (read from the code)

- **One factor, no in-between state.** `login` (`main.rs:779-869`): purge, limit check and read of the stored hash
  under the connection lock but in no transaction (`:789-815`); Argon2 outside the lock, with a dummy hash for unknown
  names so that timing stays equal (`:817-828`); then one IMMEDIATE transaction compares the hash and records either
  the failure (`:830-841`) or the session (`:843-858`). The gap between the first and last parts is RA-002. A session is
  32 random bytes stored as SHA-256, valid 12 hours (`main.rs:847-854`); cookie `HttpOnly; SameSite=Strict`, `Secure`
  only with `NF_COOKIE_SECURE=1` (`main.rs:859-863`).
- **Passwords, throttling.** Argon2 with library defaults (`crypto.rs:17-23`), at most 2 at once (`main.rs:3624`).
  5 failures per account per 900 s, 100 per client per 600 s (`main.rs:648-651`, `:794-805`). A correct password
  deletes that name's failure rows (`main.rs:843-846`). Rows of every key older than 900 s are deleted at each sign-in
  (`:790-793`) and every hour (`main.rs:3776-3786`, `:3949-3959`). Open: the limits can be raced (RA-002,
  `docs/backend/REVIEW_A_ARCHITECTURE.md:78-86`); anyone can lock out a named account, the administrator included
  (RA-006, same file `:140-152`; V-24, register `:24`).
- **Accounts.** Roles `admin`, `engineer`, `viewer` (`main.rs:606`). Changing one's own password revokes all sessions
  (`main.rs:958-1013`); its limit key `pwchange:<id>` is checked outside any transaction (`:966-967`) and a failure is
  recorded in a later one (`:991-996`), the same split as RA-002. That failure row is stored with client `NULL`
  (`main.rs:994`), so the per-client limit never counts password-change failures (REVIEW_C_EXTERNAL, RC-04
  verification). An administrator creates accounts and deactivates
  others, never themself (`main.rs:932-954`, `:1016-1060`).
- **No recovery.** No second factor, password reset or recovery code exists (routes `main.rs:3411-3443`). The client
  says recovery means "an administrator must create a new account" (`web/i18n.js:392`). A sole administrator who
  forgets the password has no supported way back in.
- **Setup token (the precedent for one-time secrets).** Held in memory only as SHA-256 (`main.rs:3606`, `:3620`); in
  plain text until used, in `NF_SETUP_TOKEN` (`main.rs:3529-3533`) or as 16 random bytes in a mode-0600 file
  (`main.rs:3568-3603`). 10 wrong tokens per 600 s, counted under the single key `setup:` for all clients together
  (`main.rs:652`, `:679`, `:690`); forgotten after use (`main.rs:752-753`). Desktop: 32 random bytes in the URL
  fragment (`desktop.rs:151-156`, `:169-174`), read once and removed by the client (`web/app.js:633-634`).
- **Desktop mode.** "One person" is the stated intent (`desktop.rs:1-2`), not a rule: desktop mode serves the same
  routes (`desktop.rs:273`), so an administrator can add accounts there (`main.rs:932-954`). 127.0.0.1 only
  (`desktop.rs:159-166`); host names `127.0.0.1`, `localhost` (`desktop.rs:203`); cookie never `Secure`
  (`desktop.rs:202`); storage key beside the data (`desktop.rs:200`); an exclusive lock on the data folder while it
  runs (`desktop.rs:129-148`, `:197`). Server mode takes no such lock, and its command line accepts only `--help`,
  `-h`, `--version` and `--desktop` (`main.rs:3803-3844`), apart from the internal `replay-check` child mode
  (`main.rs:3887-3890`).
- **Append-only.** Sign-in events go into the hash-chained audit log (`main.rs:329-376`). Failed and throttled
  sign-ins use `audit_anon` (`main.rs:803`, `:836`), which logs at most 30 a minute one by one and only counts the rest
  (`main.rs:298`, `:381-411`). The sign-in tables are mutable (deletes at `main.rs:790-793`, `:848`, `:1005`; hash
  overwritten at `:1001-1004`). This design keeps that split: credential state may change; events are appended to the
  audit log.
- **Schema version.** `SCHEMA_VERSION` is 3 (`storage.rs:15`). Columns added later (`storage.rs:35-50`) leave it
  unchanged, and it is written back at every start (`storage.rs:388`). An older build refuses a database only when
  that number is higher than its own (`storage.rs:304-307`).

## 2. Limits for every option

- No SMS, e-mail, push, cloud service or time-server client in the product. `tests/offline_check.sh:16` rejects
  outbound-capable libraries, e-mail (`lettre`) included.
- **Single sign-on is not covered.** It needs an identity provider the customer runs on site; the common flows need a
  call from the server to that provider (network) or a parser of signed XML from the browser (rule A.6, V-26). A
  front-channel token flow checked with a locally configured key is the only candidate and is not modelled here.
- No master password or vendor key in the program; every recovery leaves an audit entry (the owner's principle for key
  recovery, `docs/security/DESKTOP_KEY_ASSESSMENT.md:74-78`).
- A new crate needs a licence on `packaging/about.toml:6-16` and an entry in `packaging/THIRD_PARTY_NOTICES.txt`
  (`tests/notices_check.sh`).

## 3. Options

| Option | Protects against | Fully offline | User experience | Cost | New dependencies (licence; `offline_check.sh` grep) |
|---|---|---|---|---|---|
| **A.** Time-based one-time codes (RFC 6238) | A stolen or guessed password alone | Yes: phone app or hardware code generator | Enrol once (QR code or typed secret); a code at each sign-in | Medium | `sha1` 0.11.0 (MIT OR Apache-2.0); its dependencies `digest` 0.11, `cfg-if` and, on x86, x86_64 and aarch64, `cpufeatures` 0.3 are already in `Cargo.lock` (0.11.3, 1.0.5, 0.3.1). `hmac` 0.13.0 (MIT OR Apache-2.0) is not needed if the hand-written HMAC (RFC 2104, `storage.rs:91-108`) is generalised: today it takes only a 32-byte key and SHA-256. Optional `data-encoding` 2.11.1 (MIT, base32) and a QR encoder (none chosen or checked). No name matches the grep. |
| **B.** One-time recovery codes, stored hashed | Loss of the second-factor device | Yes | Shown once at enrolment, kept on paper; the server asks for one by its number; each works once | Small (needs A or E) | None (`sha2` is present, `crates/nf-server/Cargo.toml:20`) |
| **C.** Administrator-mediated reset | A user's forgotten password; a lost second factor only if C also clears factors (decision 5) | Yes: code handed over in person | Ask an administrator, get a one-time code, choose a new password | Small to medium | None |
| **D.** Break-glass for the last administrator | The only administrator locked out (forgotten password or lost second factor) | Yes: run on the machine itself, no network step | Stop the program; on the server machine or the PC, run it in a terminal with a recovery option and type a new password; start it again | Small to medium | None for the lock (`std`, as `desktop.rs:131-148`). Input without echo: `rpassword` 7.5.4 (Apache-2.0) with `rtoolbox` (`rpassword` asks for `0.0`; newest in the local index 0.0.6, which asks for `windows-sys` ^0.61, in `Cargo.lock` as 0.61.2). `rtoolbox` 0.0.2, the version unpacked in the local cache, is Apache-2.0, on the accepted list (`packaging/about.toml:8`); 0.0.6 is not unpacked and the index does not carry licences, so its licence is to be read at the pinned version, or own platform calls (decision 6). No name matches the grep. |
| **E.** Security keys (W3C Web Authentication) | As A, plus fake sign-in pages: the signed answer names the origin (WebAuthn Level 2 §7.2). No clock involved | Yes for USB/NFC keys and built-in authenticators. No for synchronised passkeys (keys copied through the vendor's cloud) and for signing in with a phone over the `hybrid` transport (FIDO CTAP 2.2), which goes through an internet relay | Touch the key or confirm with the computer's prompt | Large | `webauthn-rs` 0.6.2, `webauthn-rs-core` 0.6.2, `crypto-glue` 0.2.2: MPL-2.0, **not on the accepted list**; CBOR, DER, X.509 parsers. `crypto-glue` alone lists 38 direct dependencies, among them `rsa` 0.10.0-rc.18, a pre-release that advisory RUSTSEC-2023-0071 probably still covers (see E; `cargo audit` not run). Direct dependencies do not match the grep; `rustls` is only a test dependency of `crypto-glue`. Own verifier on `p256` 0.14.0 (Apache-2.0 OR MIT) is still a parser. |

Dependency facts are from manifests and the index in the local crate cache; `cargo tree`, `cargo audit` and
`offline_check.sh` were not run.

**Rules shared by the options.** *Two steps (A, B, E):* a correct password yields a pending step, not a session: a
short-lived, single-use token, hashed like a session token (`main.rs:847-856`) and tied to its account. A wrong
password is answered as today, factor or not (`main.rs:831-841`; timing `:817-828`). Step 1 no longer deletes the
account's failure rows (today `main.rs:843-846`); they and the step-2 failures are deleted only when step 2 succeeds.
Step 2 has its own limit key `totp:<id>`, like `pwchange:<id>` (`main.rs:966`), but checks the limit, verifies and
records a failure in one transaction. An HMAC or SHA-256 check allows that, having no deliberate cost (Argon2 runs
outside the lock for that reason, V-37, `main.rs:757`), so RA-002 does not recur there; and as only holders of the
password reach step 2, strangers cannot use this limit to lock the account (RA-006). New limit windows stay within the
900 s the purges keep (`main.rs:790-793`, `:3776-3786`), or the purge becomes per key. *Changing a factor:* enrolling,
replacing or removing a factor, or printing new recovery codes, needs the password and the current factor (or a
recovery code), not only a session, so a stolen 12-hour cookie (`main.rs:854`) cannot plant an attacker's
authenticator; a first enrolment needs the password and a code from the new authenticator. Each change revokes all
the account's sessions, as a password change does (`main.rs:1005`). *Storage:* new tables for factor secrets (sealed),
recovery codes (account, number, hash, time of use), pending steps (hash, account, expiry) and reset codes (hash,
account, expiry, issuer, time of use), and per factor the last accepted time step and the drift; a used code keeps its
row. `SCHEMA_VERSION` is raised with each schema change of this design, so an older build refuses the database
(`storage.rs:304-307`) instead of signing everyone in with the password alone; going back to an older build then means
restoring the backup made before the upgrade. *Audit:* sign-ins, factor changes, issue and use of codes, and recoveries
use `audit`; failures at step 2 and at reset-code entry come from people not signed in and use `audit_anon`, so beyond
30 a minute they are counted, not logged one by one (`main.rs:298`).

**A.** HOTP with HMAC-SHA-1 (RFC 4226 §5.3) over a 30-second time step (RFC 6238 §4.1, §5.2), at least 6 digits
(RFC 4226 §5.3), 160-bit secret (RFC 4226 §4 R6; RFC 6238 §5.1); test values in RFC 6238 Appendix B. A code is
accepted once (RFC 6238 §5.2): the last accepted step is stored per factor. The server must read the secret, so it is
sealed (`crypto.rs:33-55`), not hashed. It is sealed with its own key, not with the storage key, which already seals
stored files and today also authenticates the audit anchor: the task 2.5 design derives a factor-secret key FK for
this purpose (`docs/security/DESKTOP_ENCRYPTION_DESIGN.md`, section 7.1, "Key separation"). In server mode, and until
task 2.5 is built, FK must be derived the same way from a secret kept with the storage key; in desktop mode that
secret is beside the data until task 2.5 is built, so a copied folder yields the factor secrets. `otpauth://` in the usual QR code is a de-facto app format, not an
RFC. Sites that forbid phones need hardware generators or E.
*Guessing bound:* RFC 4226 §6 gives Sec = s·v / 10^Digit: s is, for time-based codes, the number of time steps
accepted at once (decision 2); v the attempts an attacker gets, set by the limit of decision 7 over the period
considered; Digit the code length. RFC 4226 §7.3 asks for this throttling at the server, per device and across
sessions. This note sets no values.
*Clock drift offline:* without a time source the server clock drifts by an amount this note cannot state. Accept a
bounded number of steps either side and record each user's drift on success (RFC 6238 §6; §5.2 recommends at most one
step for network delay); record drift at enrolment; refuse all codes while `now()` returns 0, which it does for a
clock before 1970 (`main.rs:87-92`); show the server clock on the administrator's status page (`main.rs:1244`).

**B.** 16 random bytes (128 bits) per code, as the server setup token (`main.rs:3578`). From 112 bits, NIST SP 800-63B
(revision 3) §5.1.2.2 allows a one-way hash: SHA-256. A code is looked up by the account of the pending step together
with its hash, not by hash alone as session tokens are (`main.rs:466-468`), so a code can finish only its own account's
sign-in. §5.1.2.2 has the verifier ask for the next secret or a specific numbered one: the server asks for the
lowest-numbered unused code by its number (accepting any unused code would be a deviation: decision 4). Each code works
once (§5.1.2.2); new codes replace all old ones.

**C.** An administrator asks the server for a one-time reset code for one account: 16 random bytes (128 bits, as
`main.rs:3578`), stored as SHA-256 with its expiry (decision 3) and the issuing administrator, and handed over in
person. The user enters it with a new password under the current policy (`main.rs:570-587`), at a step that needs no
sign-in and, in server mode, faces the internet. That step is limited per client only (the `client` column, as
`failures_from`, `main.rs:662-668`): no per-account key and no limit shared by all clients. Its failure rows must
carry the client address; the password-change path stores `NULL` there today (`main.rs:994`) and must not be copied. Against 128 bits a limit
protects server resources, not the code, and a per-account key would let anyone block a reset (RA-006). Clients behind
one address share a bucket (`docs/backend/REVIEW_A_ARCHITECTURE.md:146`). On success, in one transaction: the code is
checked again and marked used, the new password set, all sessions revoked (as `main.rs:1005`), and second factors
cleared only under decision 5. The administrator never learns the password; the audit entry names both accounts.

**D.** A local command, not a web step: there is no token, so no limit anyone could exhaust (copying the setup pattern
would bring one key for all clients, `main.rs:679`, `:690`). With the program stopped, someone with the service user's
access (server mode: its `NF_*` settings, data folder and key file) or the person at the PC (desktop mode) runs it with
a new recovery option naming one administrator account; `invocation` gains that option (`main.rs:3803-3844`) and, like
the `replay-check` child mode (`main.rs:3887`), it serves nothing. It opens the data as start-up does (`build_app`,
`main.rs:3555`), reads a new password twice from the terminal, applies the policy (`main.rs:570-587`) and in one
transaction sets salt and hash (as `main.rs:1001-1004`), clears the account's second factors and failure rows, revokes
all its sessions (as `main.rs:1005`) and writes an `audit` entry (not `audit_anon`). "Stopped" is checked, not
assumed: desktop mode already locks its data folder while it runs (`desktop.rs:129-148`, `:197`); D adds that lock to
server mode, so recovery never opens, and perhaps migrates (`storage.rs:317-337`), a database a running server holds,
and a second server on the same folder is refused. Stopping and starting the service is part of the procedure. D gives
its users nothing new: the data folder already exposes the unencrypted database (V-21) and, in desktop mode, the key
(`docs/security/DESKTOP_KEY_ASSESSMENT.md:26`). It does not stop RA-006.
D is also this note's answer to decision O-10 of the task 2.5 design ("recovery when no administrator can sign in",
`docs/security/DESKTOP_ENCRYPTION_DESIGN.md`, section 18): after a recovery or a restore, the last administrator's
password is reset with D. The recovery key never resets a password; it only opens the keys.

**E.** Browsers offer it only in a "secure context" (W3C Secure Contexts: HTTPS, or loopback and `localhost`), and
the specification refuses an IP-address host (W3C Web Authentication Level 2, §5.1.3). Desktop mode opens
`http://127.0.0.1:<port>` (`desktop.rs:169-174`), so it would have to open `http://localhost:<port>`; not tested in any
browser. Server mode needs HTTPS under a host name (V-20); an offline site must issue its own certificates and make
every client trust them. Device-model lists are published online (FIDO Metadata Service), so the make of a key would
not be checked. The browser's answers are binary (CBOR, COSE) and must be parsed: rule A.6 and V-26 apply unless the
owner rules otherwise. `crypto-glue` 0.2.2 asks for `rsa` 0.10.0-rc.18. RUSTSEC-2023-0071 (private-key recovery
through timing) is patched only from 0.10.0 (local advisory database, 19 Nov 2026); the local index has no 0.10.0
release (newest 0.10.0-rc.22), and a pre-release ranks below its release (SemVer 2.0.0 §11). So `cargo audit` (not
run) would probably report it, and the clean scan recorded for V-32 (register `:31`) would no longer hold.

## 4. Desktop mode and server mode

| | Desktop mode | Server mode |
|---|---|---|
| Who reaches sign-in | Programs and other OS accounts on the same computer (`desktop.rs:159-166`) | Anyone who reaches the listener; internet-facing (register `:23`) |
| Value of a second factor | None against whoever holds the data folder: they bypass sign-in (V-21; `docs/security/DESKTOP_KEY_ASSESSMENT.md:26`). Against another OS account that can reach the port but not read the folder, the same as in server mode. On Linux the folder is 0700 (`storage.rs:288`, `desktop.rs:123`); on Windows only the profile defaults protect it, because `owner_only` does nothing there (`storage.rs:116-117`; `DESKTOP_KEY_ASSESSMENT.md:25`) | High: the sign-in is the only barrier for everyone who reaches it |
| Accounts | Meant for one person (`desktop.rs:1-2`); several possible (`main.rs:932-954`) | Several |
| Recovery | D; also C if the owner wants it where several accounts exist | C for users; D for the last administrator |
| Security keys | Only if opened at `localhost` | Only behind HTTPS with a host name |
| Clock | The PC's clock | Server clock, possibly with no time source |

## 5. Risks

- A second factor enforced before C and D exist locks people out; a wrong clock refuses every code (fail closed).
- A fake sign-in page can relay codes in real time; only E resists this. Over plain HTTP codes and cookies travel in
  clear (RFC 6238 §5.1 asks for a secure channel; V-20).
- C lets an administrator take over any account; administrators already see every project (`main.rs:494-503`), with
  no separation of duties (V-27). Talking an administrator into a reset is the obvious attack.
- Desktop code secrets are only as protected as the key material in the data folder (V-21, V-22) until task 2.5 is
  built.
- Restoring an older copy of the database brings back used or replaced recovery codes, outstanding reset codes,
  removed factors and each factor's last accepted step. Neither V-25 (register `:25`) nor the task 2.5 design
  (`docs/security/DESKTOP_ENCRYPTION_DESIGN.md`, section 10.5) detects a rollback of the whole database. After any restore: new
  recovery codes for every account, every enrolment checked, every outstanding reset code expired.
- Factor secrets are sealed under FK, not under the storage key (section 3, A). A rotation that does not re-seal them
  disables every enrolled factor at once, and start-up checks only key fingerprints (`storage.rs:254-259`). In the task
  2.5 design the rotation that changes FK is the XS rotation, which re-seals every factor secret in one transaction
  (`docs/security/DESKTOP_ENCRYPTION_DESIGN.md`, section 11, row XS); the SK rotation does not touch them. Recovery
  codes are hashed, not sealed, and are not affected.
- With RA-002 open the password step stays raceable; a second factor must not add a way to lock out the administrator
  (RA-006).
- New sign-in code is new attack surface: tests that fail before and pass after, concurrency included.

## 6. Recommended combination and order

1. Task 2.2 first: RA-002 and RA-006 fixed (preconditions, not part of this design).
2. D in both modes: smallest change, no network step, independent of the other options.
3. C in server mode.
4. A with B (never A alone) in server mode, enforced for administrators first.
5. E only after the owner decides on HTTPS-only deployment, the licence, the advisory and the parser rule.

Desktop mode: D only, as a recommendation. The likely loss there is a copied data folder
(`docs/security/DESKTOP_KEY_ASSESSMENT.md:26`, `:31`), against which no sign-in factor helps; A would only guard
against other OS accounts that cannot read the folder, and C only matters where several accounts exist.

## 7. Open decisions for the owner

1. Who must use a second factor (administrators, everyone, optional), per mode; whether desktop mode gets A or C.
2. Code length (at least 6 digits, RFC 4226 §5.3), window s and drift handling; whether a clock running backwards
   refuses codes or only warns. With decision 7 this sets the odds Sec = s·v / 10^Digit (RFC 4226 §6).
3. Lifetimes of the pending second-factor step and of reset codes.
4. Number and printed format of recovery codes; whether any unused code may be accepted instead of the numbered one
   (a deviation from NIST SP 800-63B revision 3 §5.1.2.2).
5. Whether C also clears second factors; whether one administrator may reset another.
6. Whether D may name any administrator account or only the last active one. How D reads the password: without echo
   through `rpassword`, through own platform calls (which would be the first `unsafe` code in `crates/nf-server/src`),
   or with echo (nothing new; the password shows on screen). With `rpassword`: the licence of the `rtoolbox` version
   actually pinned is read and recorded first (section 3, D).
7. The value v of the step-2 limit (own key `totp:<id>`; window within 900 s, or a per-key purge); see 2.
8. QR enrolment (a new dependency) or typed secret only.
9. For E: HTTPS-only deployment; MPL-2.0 against `LicenseRef-Proprietary` (`Cargo.toml:8`), a question for a lawyer;
   whether its parsing falls under rule A.6; whether synchronised passkeys are accepted; `rsa` 0.10.0-rc.18 under
   RUSTSEC-2023-0071 and the 38 direct dependencies of `crypto-glue` (`cargo audit` not run).
