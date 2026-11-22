# Wine verification of the Windows build (work plan 4.2, Windows part; open item 3.1 "Windows not run")

Date: 2026-11-22. Branch `wt/wine` (worktree `/root/nf-wt-sec`), base `363d7de`.
Development build. Not validated. Outputs must not be used for manufacturing.

**What this is:** the Windows binaries built with the MinGW cross-compiler and run under Wine on the Linux build
host, inside the `nf-sandbox` (private network, private `/tmp`). Wine is evidence that the code runs and where it
diverges. **It is not proof of Windows behaviour.** A real Windows PC run is still required (see
`docs/release/HANDOFF_WINDOWS_VERIFICATION.md`).

## Environment

| Item | Value |
|---|---|
| Wine | wine-10.0 (Ubuntu 10.0~repack-12ubuntu1), `/usr/bin/wine64`, prefix `/tmp/wine-prefix` (win64, private to each sandbox call) |
| Cross compiler | x86_64-w64-mingw32-gcc (GCC) 13-win32 |
| Rust toolchain | `stable` = rustc 1.100.0 (0ef1bc386 2026-11-09), cargo 1.100.0 (3acc79c0c 2026-11-05); target `x86_64-pc-windows-gnu` added to `stable` for this run (it was present only on 1.95) |
| Host | Linux 7.0.0-38-generic, 8 CPUs, 31 GiB RAM |
| Sandbox | `nf-sandbox` (systemd transient unit: private network, 8 GiB, 4 CPUs, private `/tmp`) |

Target binaries (not committed; under `target/`): `target/x86_64-pc-windows-gnu/release/nf-server.exe` (PE32+ console),
`target/x86_64-pc-windows-gnu/release/nf-pdf2dxf.exe`.

## Commands

1. Target added (outside the sandbox, one command): `rustup target add --toolchain stable x86_64-pc-windows-gnu`.
2. Cross build, in the sandbox, with the linker and runner given by environment variables only (no `.cargo/config` written):
   `nf-sandbox bash -c 'export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER=wine64; cargo build --release --workspace --target x86_64-pc-windows-gnu'`
   Result: finished, exit 0 (after the fix below).
3. Workspace tests under Wine, in the sandbox:
   `WINEPREFIX=/tmp/wine-prefix WINEARCH=win64 WINEDEBUG=-all cargo test --workspace --no-fail-fast --target x86_64-pc-windows-gnu`
   (runner `wine64` from the environment). Result: exit 101 (4 failures, listed below).
4. Smoke suites against the Windows server, in one sandbox call (shared private loopback): the Windows
   `nf-server.exe` started under `wine64` by the suites' own scripts, with the `cargo build` line and the
   `target/debug/nf-server` path replaced, and `NF_DATA_DIR` / `NF_KEY_FILE` given as `Z:` paths (Wine maps `Z:` to `/`).
   No expectation was edited. Ports: e2e 8791, security 8792 (`NF_TEST_PORT`), ui 8793. `node tests/ui_smoke.js` with `NF_BASE`.
5. Probes (sandbox): a native and a Windows `rustc` build of a math probe and of a file-mode probe; `node tests/desktop_smoke.js`
   with a `wine64` wrapper as the binary.

## Results

### Workspace tests (`cargo test --workspace --target x86_64-pc-windows-gnu`, run under Wine)

| Crate / test binary | Passed | Failed |
|---|---:|---:|
| nf-core (lib 35, compat 1 of 2, dxf_statement 3, engine_id 2, refusal_codes 15, stress 6) | 62 | 1 |
| nf-dxf (lib) | 22 | 0 |
| nf-pdf (lib 18, nf-pdf2dxf bin 2, fuzz 1, geometry 21, isolation 0, proptest_fuzz 4, refusals_limits 37, regressions 2, reportlab 3, roundtrip_cli 13) | 101 | 0 |
| nf-pdf-winjob (lib; 0 tests) | 0 | 0 |
| nf-server (bin 70 of 73, cli 8) | 78 | 3 |
| uem (lib 47, shallow_angle 3) | 50 | 0 |
| Doc-tests (5 crates) | 0 | 0 |
| **Total** | **313** | **4** |

Before the fix in section "Product fix" the nf-server bin had 36 failures; the 33 that the fix resolved all came from one cause.

### Smoke suites against the Windows server under Wine

| Suite | Result |
|---|---|
| `tests/e2e_smoke.sh` | 89 passed, 0 failed |
| `tests/security_smoke.sh` | 65 passed, 1 failed (`storage key file is owner-only`, see W-3) |
| `tests/ui_smoke.js` (includes the PDF-to-DXF conversion checks, en and ja) | 223 passed, 0 failed |
| `tests/desktop_smoke.js` | 3 passed, then aborted at check 4 (see W-4); the remaining checks were not evaluated |
| `validation/run_all.py` | not run (not requested) |

## Product fix (real Windows bug, not a Wine artefact)

- `crates/nf-server/src/storage.rs`, `store_blob`, directory flush after the rename (was lines 478-479):
  - old: `std::fs::File::open(&dir)?.sync_all()` on every platform;
  - new: the same call under `#[cfg(unix)]`, with a comment, and `Ok(())`.
- Evidence: every blob store on Windows failed with `could not store file <sha>: Access denied. (os error 5)`, so each
  document upload returned HTTP 500 ("internal error; nothing was changed"). Rust's `File::open` on a directory fails with
  access denied on Windows. The same pattern was already excluded in `create_private` (storage.rs, comment
  "Windows cannot open a directory as a file"). Wine reproduces the Windows API behaviour here, but the bug is in the
  code path that Windows runs too.
- Behaviour on Unix is unchanged (the flush still runs). Durability on Windows: the rename is still performed; the directory
  flush is not, as in `create_private`.
- No test expectation was changed. `cargo fmt --all -- --check` passes; `cargo check -p nf-server` on Linux passes.

## Failures under Wine, with classification

| ID | Test | Evidence | Classification | Status |
|---|---|---|---|---|
| W-1 | `nf-core/tests/compat.rs` `compat_corpus_reproduces_the_0_4_0_snapshot` (line 403): case 5 (`eccentric_reducer`) `patterns_sha256` differs from the fixture (`31902be1…` vs `fe6b3f08…`). | Math probe (20000 inputs each): the Wine libm differs from glibc on `asin` in 17817 and `acos` in 17792 of 20000 inputs, `sinh` 1756, `tan` 777, `sin` 133, `cos` 136, `atan` 122, `powf` 34; `sqrt` and `powi` never. `crates/uem/src/develop.rs:135` already states that the engine depends on the platform library for sin, cos and asin. | Platform math-library difference. Whether the real Windows UCRT matches the Linux fixture or Wine is **not known**. | Open. Not fixed: the fixture is an expectation (not changed), and a change of the math is forbidden for this task (plan A2, A8). Needs a real Windows run; a decision on cross-platform determinism is needed. |
| W-2 | `nf-server` bin: `be008_replay_infrastructure_failure_is_not_an_integrity_failure`, `rc22_replay_reports_the_check_against_the_records` (tests.rs:3316), `rc01_stored_patterns_and_dxf_references_are_checked_against_the_records` (tests.rs:3196) | The test helper (`tests.rs:155-166`) writes a `#!/bin/sh` file and uses it as the replay executable. Windows cannot execute it: `cannot start the replay process: Bad EXE format for %1 (os error 193)`. The product answers 503 "infrastructure problem, state unchanged", which is the intended refusal. | Test-harness assumption (Unix shebang). Not a Wine artefact: real Windows also cannot run a shell script this way. The product path behaves as designed. | Open. Not changed: a Windows-native replay fixture is needed; the tests were not skipped or loosened. |
| W-3 | `tests/security_smoke.sh`: `storage key file is owner-only` (`stat -c %a` = 600 expected) | Probe: a file created by a Windows program under Wine has mode 644. The key file gets no explicit ACL on Windows; `docs/PACKAGING.md:26` and `desktop.rs:115` say Windows relies on the per-user ACL of `%LOCALAPPDATA%`. | POSIX mode check does not apply to Windows; the Windows protection is an ACL that Wine does not reproduce. | Open. Needs `icacls` evidence on a real Windows PC. The expectation was not changed. |
| W-4 | `tests/desktop_smoke.js` check 4 "the data folder exists" (aborted with ENOENT) | The Windows binary keeps its data in `%LOCALAPPDATA%\NexusFabric\data`; the harness expects the Linux XDG layout (`nexus-fabric`). The harness uses `process.platform` and cannot test the Windows layout. | Harness platform assumption. | Open. Remaining desktop checks (second copy refused, token fragment, Ctrl+C) were not run on Windows. |

Wine noise, not failures: `error: XDG_RUNTIME_DIR is invalid or not set` (Wine, printed by the test runner).

## What Wine cannot prove

- DPAPI and any Windows key protection; the ACLs of the data folder and of the key file (W-3).
- The installer (NSIS), install and uninstall, the Start-menu and desktop shortcuts, "Installed apps" entry, SmartScreen and antivirus behaviour.
- Real NTFS semantics: sharing and locking of open files, rename over an open file, antivirus scanning, case rules, path length limits, and the Job Object isolation of the converter (Wine implements the call, but the isolation result was not checked against Windows).
- Windows math results: the Windows UCRT (W-1) is not the Wine libm.
- Browser behaviour of the desktop start-up (the browser opener `explorer.exe`) and console close handling.
- The `nf-pdf-winjob` crate has no tests; the conversion path was exercised only through the ui smoke suite.

## Open items

1. A real Windows PC run is still required: `docs/release/HANDOFF_WINDOWS_VERIFICATION.md`.
2. W-1: decide the cross-platform determinism rule for the math library; check the snapshot on Windows.
3. W-2: a Windows-native replay fixture for three tests.
4. W-3: confirm the key-file ACL on Windows.
5. W-4: make the desktop smoke aware of the Windows data folder layout (a test change, to be reviewed separately).

Nothing was committed. No tag, no push, no pull request.
