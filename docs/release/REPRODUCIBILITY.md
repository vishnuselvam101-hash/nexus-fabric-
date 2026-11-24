# Reproducible builds (work plan task 4.1)

Date: 2026-11-22. Cloud session, branch `dev/nf-repro-ci-4.1-4.3` (from `dev/nexus-fabric-strategy-8guhxz` at
`4d2d00e`). Development build. Not validated. Outputs must not be used for manufacturing.

**Result:** on a Linux host, three builds of commit `8a86d54` in three different folders give bit-identical
SHA-256 values for all three packages and all four programs. Before this task, two builds of one commit gave
different values for every package (`docs/release/MANIFEST-cc89368.md`, section "Reproducibility").
A checksum therefore identifies a commit, **for the same tool versions** (listed below). The native Windows
build (MSVC, Windows runner) and its packages have not been checked: there is no Windows host here.

## Causes found and what removes each one

| # | Cause | Affected | Fix | Where |
|---|---|---|---|---|
| 1 | Entry times = the time the files were copied | tar.gz, zip, installer (NSIS `File` stores each file's time) | every staged file is given the time `SOURCE_DATE_EPOCH` (the commit time) before it is archived | `packaging/reproducible.sh` (`nf_normalise_tree`) |
| 2 | Entry order = the order the file system returns | tar.gz, zip | `tar --sort=name`; zip reads a byte-order sorted list (`-@`) | `packaging/linux/build.sh`, `packaging/windows/build.sh` |
| 3 | Owner and permissions from the builder's account and umask | tar.gz, zip | owner `0:0` (`--numeric-owner`); folders and programs 755, other files 644 | same |
| 4 | gzip header with a time | tar.gz | `gzip -n` (no name, time 0); GNU tar format, so no pax headers with access or change times | `packaging/linux/build.sh` |
| 5 | zip extra fields (Unix uid/gid, extended UTC times) and local-time DOS stamps | zip | `zip -X`; zip runs with `TZ=UTC` | `packaging/windows/build.sh` |
| 6 | Build-folder and cargo-home paths in the programs (panic locations, file names) | all four programs | `--remap-path-prefix` for the workspace (`nexus-fabric`), cargo home (`cargo-home`) and a target folder outside the workspace; `-ffile-prefix-map` for the bundled SQLite C code | `packaging/reproducible.sh` (`nf_rustflags`, `nf_cflags`), used by `packaging/build_all.sh` only |
| 7 | Link time in the PE header | Windows programs | `-Wl,--no-insert-timestamp` (mingw ld); `-Brepro` for a native MSVC build (not checked) | `nf_rustflags` |
| 8 | COFF symbol table naming rustc's randomly named temporary folders (`deps/rustcXXXXXX`, raw-dylib import libraries of windows-sys and getrandom): different on **every** build, even in the same folder | Windows programs (cross-built) | `-Cstrip=symbols` for `x86_64-pc-windows-gnu`; the table is used only by debuggers (a panic backtrace on Windows shows addresses instead of names) | `nf_rustflags` |
| 9 | Import directory order depends on the build folder: the PE linker script sorts `.idata$N` input files by name, archive path first, so the import libraries in the target folder's rlibs came before or after those in the standard library depending on how the build path sorts against the toolchain path (`/home/...` before `/root/.rustup`, `/tmp/...` after) | Windows programs (cross-built) | the linker runs from a temporary folder in which the target folder is the relative link `nf-target` | `packaging/windows/link-wrapper.sh` (set as the linker by `packaging/build_all.sh`) |
| 10 | Lock file could be updated during a build | all programs | `cargo build --locked` | `packaging/build_all.sh` |

`SOURCE_DATE_EPOCH`: taken from the environment if set; otherwise the commit time of `HEAD`
(`git log -1 --format=%ct`); in a copy without `.git` the build warns and uses the time of `Cargo.toml` (a copy
made by `git archive` or GitHub's source download has the commit time there). `packaging/verify_release.sh`
sets it from the pinned commit.

Linux programs: the linker's build-id is a hash of the content, so it is reproducible once the content is; no
further change was needed. No committed `.cargo/config` was added: developer builds (`cargo build`) are unchanged;
the flags apply only to `packaging/build_all.sh`.

## Proof: three builds of `8a86d54`

Tools: rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0, x86_64-w64-mingw32-gcc 13-win32, GNU ld 2.41.90.20240122
(mingw), makensis v3.09-4, Zip 3.0, GNU tar 1.35, gzip 1.12; host Linux 6.18.44 x86_64, 4 CPUs.

| Build | Folder | Source | `SOURCE_DATE_EPOCH` |
|---|---|---|---|
| W | `/home/user/nexus-fabric-/.agent/worktrees/agent-…` | git checkout, target folder reused from earlier builds | from git: 1795363516 |
| A | `/home/user/nf41-copy-a` | `git archive 8a86d54`, empty target folder | given: 1795363516 |
| B | `<scratch>/nf41/copy-two/with-a-much-longer-folder-name/b` (under `/tmp`) | `git archive 8a86d54`, empty target folder | not set: warning, time of `Cargo.toml` = 1795363516 |

| Artefact | Size (bytes) | SHA-256 (W = A = B) | W, A, B |
|---|---|---|---|
| `nexus-fabric-0.1.0-linux-x86_64.tar.gz` | 3419196 | `43dd226354edfcd8a48dc67a9912ec131449817efe026acca9c777f79cbb874e` | identical |
| `nexus-fabric-0.1.0-windows-x64-portable.zip` | 3683857 | `84d9f39f592fa08513bb70387cdec5a84457ceda4c5bbaf3989ea7a580508c33` | identical |
| `nexus-fabric-0.1.0-windows-x64-setup.exe` | 2677871 | `bb65bb879debf031f2127117dbe648d76446239607b97a33915212bffd1d9f3a` | identical |
| `SHA256SUMS` | 322 | `042a35b0ac3c3fe7a99fc291fa2686146f95be3b988c47e9e2fa68f69bea61f6` | identical |
| `nf-server` (Linux; `nexus-fabric` in the tar.gz) | 7644664 | `97418d17b7bb91d037db3e6eaa14971704b935efccded9374d2c2aa819279a50` | identical |
| `nf-pdf2dxf` (Linux) | 1143600 | `586dbfb7c959faea0bc3788d57108cc33a5691f605a98fea7d9e0bb38a1223fb` | identical |
| `nf-server.exe` (`nexus-fabric.exe` in the zip) | 6818816 | `306d14f932256b09bac7e5ce0313dd6954fc729b2439faa9826d2da9998b9229` | identical |
| `nf-pdf2dxf.exe` | 1356800 | `702219f64d1bb417cfb776d7b6a5a6dbe739a09718f59ad258ec7f5110aa2ba7` | identical |

The programs unpacked from A's tar.gz and zip have the same SHA-256 values as the programs in the table. The
package contents are unchanged: Linux `nexus-fabric`, `nf-pdf2dxf`, `nexus-fabric.sh`, `nexus-fabric.desktop`,
`README.txt`, `NOTICE.txt`, `THIRD_PARTY_NOTICES.txt`; Windows `nexus-fabric.exe`, `nf-pdf2dxf.exe` and the three
text files. These SHA-256 values hold for `8a86d54` only: a later commit has a later commit time, which is stored
in every package.

How the causes were found (each round built the packages again from fresh copies):

| Round | Commit | Linux programs and tar.gz | Windows programs, zip, installer | Cause found |
|---|---|---|---|---|
| 1 | `64ac3ce` | identical (2 copies) | different | 8: random temporary-folder names in the COFF symbol table |
| 2 | `2dd37ef` | identical (3 builds) | identical in the 2 copies under `/tmp`, different in the worktree | 9: import directory order follows the build path |
| 3 | `5a988b3` | identical | as round 2 (a dlltool wrapper did not help; removed in `8a86d54`) | 9 confirmed: the order follows the rlib archive path |
| 4 | `8a86d54` | identical (3 builds) | identical (3 builds) | none |

## What this does not cover

- **Same tools only.** A different rustc, mingw-w64 (gcc, binutils), NSIS, zip, tar or gzip version can give
  different bytes; `packaging/verify_release.sh` records the tool versions in its manifest.
- **NSIS stub time stamp.** The installer's PE header carries the time stamp of the NSIS stub shipped with makensis
  (1712802157, 2024-04-11, for v3.09-4). It is fixed for a given NSIS build, not the build time, so it does not
  break reproducibility; it was not removed (that would need a patched NSIS).
- **Native Windows build not checked.** `-Brepro` and `-PDBALTPATH:%_PDB%` for MSVC (the Windows runner of
  `.github/workflows/release.yml`), the Windows-runner packages, and the linker wrapper's absence on a Windows GNU
  host are untested. On Windows without `zip`, the portable zip is made by `Compress-Archive` and is not
  reproducible (the build prints a warning).
- **Paths with spaces.** `RUSTFLAGS` is split at spaces, so the workspace and cargo home paths must not contain
  spaces.
- **Not run here:** `packaging/verify_release.sh` (it also runs wine, which crashes in this container). Its second
  build now uses the same `SOURCE_DATE_EPOCH`; its reproducibility row should read identical on the next run.
- **No other user, umask or cargo home was tried.** Permissions are set explicitly and the cargo home is remapped,
  so neither should matter, but that was not run.
