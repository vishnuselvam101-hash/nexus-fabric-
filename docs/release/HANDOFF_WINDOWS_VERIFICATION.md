# Hand-off: verify NEXUS FABRIC on a real Windows PC

For a CLI session running **on a Windows 10 or 11 PC** (not in the cloud). The cloud build machine is Linux and
could only try the Windows files under an emulator, which is not evidence that they work on Windows. This file is the
complete instruction; paste the block below into the CLI session on the Windows PC.

Development build. Not validated. Outputs must not be used for manufacturing.

## What to paste into the CLI session on the Windows PC

```
You are verifying the Windows package of NEXUS FABRIC on this real Windows PC. Repository:
vishnuselvam101-hash/nexus-fabric-, branch dev/nexus-fabric-strategy-8guhxz. You verify and report; you do NOT
change product code (crates/, web/). If something is broken, describe it exactly; do not fix it here.

1. Get the code: clone the repository (or `git pull` if it exists) and check out the branch. Record the full commit hash.
2. Build natively: install the Rust toolchain (stable, MSVC or GNU) and NSIS if they are missing, then build the
   packages as docs/PACKAGING.md describes (packaging/build_all.sh under Git Bash, or the commands inside it).
   Record tool versions, every build command and its exit code. If the build fails, stop and report the full error.
3. Record for each artefact: file name, size, SHA-256 (PowerShell: Get-FileHash <file> -Algorithm SHA256).
4. Version agreement: the installer file name, the workspace version in Cargo.toml, and the output of
   `nexus-fabric.exe --version` must agree. Record the engine identifier it prints. Check `--help` prints help and
   exits 0, and an unknown option exits with a non-zero code, without starting anything.
5. Install test, as a normal (non-administrator) user: run the installer; note every Windows warning shown
   (SmartScreen, antivirus) word for word; confirm the program folder, Start-menu shortcut, optional desktop
   shortcut and the entry in "Installed apps".
6. First run: start it from the Start menu. Confirm the console window shows a local address, the default browser
   opens the first-run screen, an administrator account can be created, sign-in works, and the interface switches
   between English and Japanese. Note where the data folder is (expected under %LOCALAPPDATA%\NexusFabric).
7. Work test: create a project; compute one job of each of the five part types with the pre-filled example values;
   open the pattern, the Gates and Checks tabs and the AMOEBA Control Centre; download a DXF and open it in any DXF
   viewer available; record a correction on the header-with-branch job; run the replay audit. Type `1,5` into a
   dimension field and confirm it is refused, not turned into 15.
8. Offline test: disconnect the PC from every network (Wi-Fi off, cable out), start the program again, repeat a
   computation and a DXF download. Everything must work.
9. Second copy: start the program twice; the second must refuse and say why.
10. Close test: close the console window, then start again; the data must still be there and the audit check
    (Administration) must report intact. Also stop it with Ctrl+C once.
11. Upgrade test, if an older installer is available: install the older one, create a job, install the new one over
    it; the job must still be there and `--version` must show the new build. If no older installer exists: "not run".
12. Uninstall: uninstall from "Installed apps"; confirm program files, shortcuts and the installed-apps entry are
    gone and the data folder is kept (unless "also delete my data" was ticked — test both if time allows).
13. Portable zip: unpack it elsewhere and run `nexus-fabric.exe --version` and one start.

Write the results to docs/release/WINDOWS_NATIVE_<short commit>.md: Windows edition and build number, CPU, RAM,
the commit, tool versions, the artefact table with SHA-256, then one row per step above: passed / failed / not run,
with the exact evidence (message text, screenshot file name, command output). Add screenshots under
docs/release/windows-native/. A step you could not perform is "not run" with the reason — never "passed".
Do not write "certified", "approved", "validated" or "production-ready" anywhere. Commit only files under
docs/release/ on the same branch and push. Do not open or merge a pull request.
```

## What this hand-off does not cover

- Code signing: the files are unsigned, so Windows will warn until the owner buys a code-signing certificate.
- macOS: no package exists.
- Engineering correctness: this checks that the software installs and runs on Windows, nothing more. Review by a
  fabrication engineer and the independent geometry validation are separate (docs/project/READINESS.md).

## Where the Linux-side evidence is

`packaging/verify_release.sh` builds the packages from a clean copy of a commit, records checksums and version
agreement, and runs the Linux checks natively. Its manifests are written to `docs/release/`. On the cloud machine the
Windows checks could only be attempted under an emulator and are recorded as such, or as "not run".
