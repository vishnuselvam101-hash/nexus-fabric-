# Packaging NEXUS FABRIC as a desktop program

Development build. Not validated. Outputs must not be used for manufacturing.
All rights reserved. Evaluation use only — terms to be provided by the owner.

This document covers how the downloadable packages are built, what each file is,
what was and was not tested, and what the owner still has to decide. End-user
instructions are in `packaging/INSTALL.md` (English and Japanese).

## What is built

One binary, `nf-server`, serves the web client and the API. It has two modes:

- **Server mode** (default, unchanged): configured with `NF_*` environment variables,
  listens on `NF_LISTEN` (default `127.0.0.1:8787`), data in `NF_DATA_DIR`.
- **Desktop mode** (`nf-server --desktop`, opt-in): for one person on their own
  computer. Implemented in `crates/nf-server/src/desktop.rs`.

The Windows package names the binary `nexus-fabric.exe`; the Linux package names it
`nexus-fabric`. Both are the same program.

### Desktop mode

- Data folder: Windows `%LOCALAPPDATA%\NexusFabric\data`; elsewhere
  `$XDG_DATA_HOME/nexus-fabric` (or `~/.local/share/nexus-fabric`). The folder is
  created owner-only (0700) on Unix. On Windows it inherits the per-user ACL of
  `%LOCALAPPDATA%`; no extra ACL is set.
- Listens on `127.0.0.1` only, on port 8787 (`--port N` changes the preferred port).
  If that port is busy, a free loopback port is taken and the console says so.
  Allowed host names are `127.0.0.1` and `localhost`.
- A second copy on the same data folder is refused. An exclusive lock on
  `desktop.lock` in the data folder is held for the life of the process; the OS
  releases it if the process dies.
- Opens the default browser (`explorer.exe` on Windows, `xdg-open` on Linux).
  `--no-browser` prints the address instead.
- Ctrl+C, SIGTERM, SIGINT and SIGHUP (closed terminal) on Unix; Ctrl+C, Ctrl+Break and
  console close on Windows stop the server gracefully, then the database write-ahead
  log is checkpointed and the process ends.
- Makes no outgoing network calls, has no telemetry and no updater.

### How the first-run setup token is handed over

The server needs a one-time setup token to create the first administrator. In
server mode the token comes from `NF_SETUP_TOKEN` or a file in the data folder.
Desktop mode generates its own token and passes it to the browser in the **URL
fragment**:

    http://127.0.0.1:8787/#setup=<64 hex characters>

Why this is the least risky choice here:

- The browser never sends the fragment to the server, so it is not in the HTTP
  request, the server's access log, or any proxy log.
- The client (`web/app.js`) reads it once, keeps it in memory only, and removes it
  from the address bar with `history.replaceState`, so it is not left in history.
  It is sent only in the setup request body, as in server mode.
- The token is 32 bytes from the operating system random source (`getrandom`),
  hex-encoded. The server stores only its SHA-256 hash and forgets it when the first
  administrator is created. A new token is made at each start while no administrator
  exists.
- The setup protection itself did not change: the setup endpoint still requires the
  token, still throttles wrong tokens, and still refuses once an administrator exists.
- The console prints the setup link only when no browser could be opened (or with
  `--no-browser`), because the browser cannot receive it otherwise. Ordinary starts
  print the address without the token.

Server mode's token handling (`NF_SETUP_TOKEN`, the `setup-token` file) is unchanged.
The only server-mode change is the wording of one start-up log line, which now also
mentions the desktop setup link.

### Other source changes

- `crates/nf-server/src/main.rs`: `--desktop` dispatch; the stop-signal helper now has
  Windows handling (server mode gets the same SIGTERM/SIGINT behaviour on Unix).
- `crates/nf-server/src/storage.rs`: the directory fsync after creating the storage
  key is done on Unix only. On Windows opening a directory as a file fails with
  "Access denied", which made key creation fail; found by running the Windows build
  under wine. Unix behaviour is unchanged.
- `web/app.js`: reads the `#setup=` fragment (see above) and hides the token field
  when a token came from the link.

## Files

| Path | What it is |
|---|---|
| `packaging/build_all.sh` | Builds everything into `dist/`. `--linux-only` skips the Windows packages. |
| `packaging/linux/build.sh` | Builds `nexus-fabric-<ver>-linux-x86_64.tar.gz`. |
| `packaging/linux/nexus-fabric.sh` | Launcher: runs `nexus-fabric --desktop`. |
| `packaging/linux/nexus-fabric.desktop` | Desktop entry template (edit `Exec=`). |
| `packaging/windows/build.sh` | Builds the portable zip and the installer from an exe. |
| `packaging/windows/installer.nsi` | NSIS per-user installer and uninstaller script. |
| `packaging/README.txt` | README shipped in every package. |
| `packaging/NOTICE.txt` | Notice shipped in every package. |
| `packaging/THIRD_PARTY_NOTICES.txt` | Licence texts of the compiled Rust crates, shipped in every package. Regenerate with `packaging/generate_notices.sh` (cargo-about, config `packaging/about.toml`, template `packaging/about.hbs`); `tests/notices_check.sh` builds the Linux package and fails if it lacks the file, or if the notices list does not match `cargo tree -e normal,build` of `nf-server` for both packaged targets (it runs in the `quality-gate.yml` software job as `bash tests/notices_check.sh target/release/nf-server cargo`). It does not check the installed Windows file: `packaging/verify/windows_checks.sh` checks that `THIRD_PARTY_NOTICES.txt` is present in the installed Windows folder. |
| `packaging/INSTALL.md` | Install guide, English and Japanese. |
| `.github/workflows/release.yml` | Builds the packages on a manual run only (Linux runner: all packages; Windows runner: native Windows packages); uploads workflow artefacts only. |
| `packaging/reproducible.sh` | Shared by the build scripts: fixed times (`SOURCE_DATE_EPOCH`), order, owner and permissions in the archives; Rust and C flags that keep build-folder paths and link times out of the programs (task 4.1; `docs/release/REPRODUCIBILITY.md`). |
| `tests/desktop_smoke.js` | Browser test of desktop mode (see below). |

The installer:
- installs to `%LOCALAPPDATA%\Programs\NexusFabric` and needs no administrator rights
  (`RequestExecutionLevel user`);
- offers a Start-menu shortcut (required) and a desktop shortcut (optional, ticked by
  default);
- shows the development-build notice on its welcome page;
- registers an uninstaller under the current user's uninstall list;
- on uninstall keeps the data folder unless "Also delete my data" is ticked;
- refuses to finish uninstalling while `nexus-fabric.exe` is still running.

The version comes from `[workspace.package] version` in `Cargo.toml`.

## Building

Prerequisites:

- Rust (stable), the `x86_64-pc-windows-gnu` target and `mingw-w64` for the Windows
  packages built on Linux:
  `rustup target add x86_64-pc-windows-gnu && apt-get install -y mingw-w64`
- `nsis` (`makensis`) for the installer; `zip` for the portable archive (on Windows,
  PowerShell's `Compress-Archive` is used if `zip` is absent).

Commands:

    packaging/build_all.sh               # Linux host: Linux + Windows packages
    packaging/build_all.sh --linux-only  # Linux packages only
    # Windows host (Git Bash, NSIS on PATH): packaging/build_all.sh

Output in `dist/` (ignored by git, as are `*.exe` and binaries in general):

- `nexus-fabric-<ver>-windows-x64-setup.exe`
- `nexus-fabric-<ver>-windows-x64-portable.zip`
- `nexus-fabric-<ver>-linux-x86_64.tar.gz`
- `SHA256SUMS`

Check: `cd dist && sha256sum -c SHA256SUMS`.

Cross-compilation note: the Windows exe is built with `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc`
(set by `build_all.sh`). `rusqlite` uses its bundled SQLite, so no SQLite DLL is needed.

## Release workflow

`.github/workflows/release.yml` runs on a manual run only (`workflow_dispatch`); it has no tag
trigger. The `linux` job (`ubuntu-24.04`) installs mingw-w64, NSIS and zip, runs
`cargo test --workspace --locked` and `packaging/build_all.sh`: the Linux package and the
cross-built Windows exe, portable zip and installer. The `windows` job (`windows-2025`, Git
Bash) runs the same tests, installs NSIS (and zip) if missing and builds the Windows packages
natively (`x86_64-pc-windows-msvc`). Each job checks `SHA256SUMS`, records the SHA-256 of the
programs in `BINARIES.SHA256SUMS` and uploads its files as a workflow artefact. **It does not
create a GitHub release or a tag and does not publish anything**; the token is read-only, and
the owner decides whether and when to publish. Not run yet (task 4.3).

## Reproducible builds

Two builds of one commit give the same SHA-256 values for the three packages and the four
programs, whatever folder they are built in (task 4.1, shown on a Linux host; see
`docs/release/REPRODUCIBILITY.md` for the causes removed, the two-build comparison and what
is not covered). Every time stored is `SOURCE_DATE_EPOCH`: by default the commit time of
`HEAD`; in a copy without `.git`, set it to the commit time
(`SOURCE_DATE_EPOCH=$(git log -1 --format=%ct)`), otherwise the build warns and uses the time
of `Cargo.toml`.

## Release verification

`packaging/verify_release.sh` builds the packages for one commit and records what was
run. It is repeatable and does not change the working tree.

    packaging/verify_release.sh [--previous <older setup.exe>] [--out <dir>]

- It pins `HEAD` at the start. Uncommitted changes in the working tree are reported, not
  fatal, and are not used: the build never reads the working tree.
- It builds with `packaging/build_all.sh` in a fresh copy made by `git archive` of that
  commit. Each build step prints `STEP <name> exit=<rc>` and its exit status is recorded.
  Cargo runs offline (`CARGO_NET_OFFLINE`): nothing is downloaded.
- Reproducibility: the packages are built a second time from a second fresh copy. The
  SHA-256 values of both builds are recorded and the manifest says whether they are
  bit-for-bit identical.
- Dynamic checks, each recorded as passed, failed or not run, with the mode it ran in:
  - **Linux: native.** Unpack, `--version`, desktop mode through the shipped launcher with
    first-run setup in a browser (`tests/release_verify.js`, a copy of `desktop_smoke.js`
    on ports 8870-8879), and an offline first launch in a network namespace that has only
    loopback (`unshare -n`).
  - **Windows: emulator only.** Everything runs under wine (`WINEARCH=win64`, fresh prefix,
    clean environment). Silent install, shortcuts, registry, `--version`, desktop mode over
    HTTP with the setup-link token, graceful SIGINT stop, the portable zip, silent uninstall
    (data folder kept). The upgrade check runs only with `--previous`; otherwise it is
    recorded as not run. A result from wine is never described as a Windows result.
- Outputs: `docs/release/manifest-<short commit>.json` and `docs/release/MANIFEST-<short
  commit>.md`, with artefact sizes and SHA-256, version agreement, tool and build
  environment, the resolved dependency list, each check's result and one line of evidence,
  and a fixed "What this does NOT show" section. The artefacts, logs and manifest copies
  go to `--out` (default `dist/verify-<short commit>`, which git ignores).
- Exit status is 0 only if every build step passed and no check failed.

Rehearsal example (wine and the mingw cross-compiler in the build container):

    packaging/verify_release.sh --previous /path/to/nexus-fabric-0.1.0-windows-x64-setup.exe \
      --out /path/to/outputs

The offline check needs root and `unshare`; without them it is recorded as not run. The
Windows checks need wine and the build needs the mingw cross-compiler; if the build fails,
every dynamic check is recorded as not run with the reason.

## Testing: what was run and what was not

Run here (Linux, Debian/Ubuntu container, no real Windows machine):

- Desktop mode, Linux, end to end: `node tests/desktop_smoke.js target/debug/nf-server 8860`
  (18 of 18 on the final build; an earlier run on 8862 also passed): it covers the loopback
  address, owner-only data folder, host-name refusal, the second-copy refusal, first-run
  setup through the browser with the token only in the fragment, removal of the
  fragment from the address bar, the setup link being dead after setup, Ctrl+C with exit
  0, the write-ahead log checkpointed, and the port closed afterwards.
- Windows exe under **wine 9.0** (x86_64, cross-built here): starts with `--desktop`,
  reports the data folder `C:\users\root\AppData\Local\NexusFabric\data`, answers
  `/api/health`, completes first-run setup with the fragment token over HTTP, and stops
  gracefully on SIGINT (wine delivers it as Ctrl+C) with the database closed.
  wine is not Windows: console, SmartScreen, Windows ACLs and the installer's behaviour
  on a real machine are not tested by this.
- Installer under wine (the 64-bit `setup.exe`, silent mode): the install puts the
  program, README, notice and uninstaller in `%LOCALAPPDATA%\Programs\NexusFabric`, and
  creates the Start-menu and desktop shortcuts and the uninstall registry entry. The
  installed copy starts with `--desktop` and answers `/api/health`. The silent uninstall
  removes the program folder, shortcuts and registry entries and keeps the data folder.
  wine's menu integration left a Linux `.desktop` entry for the desktop shortcut; a Windows
  machine does not produce that file. Silent mode skips the uninstaller's
  "Also delete my data" page, so that deletion path was not exercised.
- Existing suites: `cargo test --workspace` (108 passed: the 98 existing tests plus 10
  new desktop tests), `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, `bash tests/e2e_smoke.sh` (89), `bash tests/security_smoke.sh`
  (66), `tests/ui_smoke.js` (151). The first browser run gave 150 of 151: one viewer-role
  check failed on a page-wait race inside the test; a rerun on a fresh server passed
  151 of 151. The test was not changed.
- Unit tests in `desktop.rs`: data-folder rules (Windows and XDG/HOME), argument
  parsing, the bind plan (preferred port, then a free loopback port), token format and
  freshness, the fragment-only URL, and the second-copy lock.

Not tested:

- **The `.exe` and the installer have NOT been run on real Windows.**
- The port fallback to a free port was not exercised end to end; doing so would need
  a server on an arbitrary port, which the test rules here do not allow. The candidate
  list is unit-tested.
- Windows console Ctrl+Break and console-window close, the browser opening through
  `explorer.exe`, SmartScreen, and the uninstaller's data-deletion checkbox are not
  exercised. (Ctrl+C was exercised under wine as SIGINT.)
- macOS is not packaged.
- The GitHub workflow has not been run on GitHub.

## Code signing

The Windows files are **not code-signed**. Windows will show a warning such as
"Windows protected your PC" (SmartScreen) until the owner obtains a code-signing
certificate and signs `nexus-fabric.exe` and the installer. Signing is a decision for
the owner; the build scripts do not sign anything. Users are told to check the
downloads against `SHA256SUMS`.

## Open items for the owner

- Licence terms: the repository has no LICENSE file. The packages say "All rights
  reserved. Evaluation use only — terms to be provided by the owner." The owner must
  supply the terms.
- Code signing for Windows (see above).
- Whether to publish releases (the workflow only uploads artefacts).
- Windows: the data folder relies on the user profile's default ACL; no explicit ACL is set.
- No automatic update exists, by design; nothing checks for new versions.
