#!/usr/bin/env bash
# Release verification for NEXUS FABRIC. For the commit at HEAD (pinned at the start):
#  1. reports uncommitted changes in the working tree (not fatal: the build never uses the working tree);
#  2. builds the packages (packaging/build_all.sh) twice, each time in a fresh copy made with `git archive` of that commit;
#  3. records artefact names, sizes and SHA-256; checks that the versions agree (Cargo.toml, file names,
#     nf-server --version on Linux, exe --version under wine, engine identifier);
#  4. runs the dynamic checks, each recorded as passed / failed / not run, with its real output kept in a log:
#     Linux natively (unpack, --version, desktop mode in a browser, offline first launch in a loopback-only
#     network namespace); Windows under wine only (silent install, shortcuts, registry, --version, desktop mode
#     over HTTP, graceful stop, portable zip, upgrade if --previous is given, silent uninstall);
#  5. writes docs/release/manifest-<short commit>.json and MANIFEST-<short commit>.md, and copies the artefacts,
#     logs and manifests to --out.
# Usage: packaging/verify_release.sh [--previous <older setup.exe>] [--out <dir>]
#   --previous  older Windows installer for the upgrade check; without it the upgrade check is "not run"
#   --out       output folder (default dist/verify-<short commit>; dist/ is git-ignored)
# Cargo runs offline (CARGO_NET_OFFLINE): nothing is downloaded. Reproducibility: the packages are built a second time.
# Exit status: 0 only if every build step succeeded and no check failed ("not run" is not a failure, but is reported).
# Only ports 8870-8879 are used. Processes this run started are stopped; the wine prefixes and temporary folders are removed.
# Development build. Not validated. Outputs must not be used for manufacturing.
set -uo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT" || exit 1
. "$ROOT/packaging/verify/lib.sh"
. "$ROOT/packaging/verify/linux_checks.sh"
. "$ROOT/packaging/verify/windows_checks.sh"

PREVIOUS=""
OUT=""
while [ $# -gt 0 ]; do
  case "$1" in
    --previous) PREVIOUS=${2:-}; shift 2 ;;
    --out) OUT=${2:-}; shift 2 ;;
    *) say "usage: $0 [--previous <setup.exe>] [--out <dir>]"; exit 2 ;;
  esac
done

# 1. Pin the commit and check the tree.
COMMIT=$(git rev-parse HEAD) || { say "not a git repository"; exit 1; }
SHORT=$(git rev-parse --short HEAD)
CORE=$(git status --porcelain --untracked-files=all -- crates web packaging)
[ -z "$CORE" ] || say "note: the working tree has uncommitted changes in crates/, web/ or packaging/; they are NOT used (the build uses the commit only)"
OTHER=$(git status --porcelain --untracked-files=all -- . ':(exclude)crates' ':(exclude)web' ':(exclude)packaging' ':(exclude)docs/release' ':(exclude)dist')
if [ -n "$PREVIOUS" ] && [ ! -f "$PREVIOUS" ]; then say "--previous: file not found: $PREVIOUS"; exit 2; fi
[ -n "$OUT" ] || OUT="$ROOT/dist/verify-$SHORT"
mkdir -p "$OUT/logs" "$ROOT/docs/release" || exit 1
RESULTS="$OUT/logs/results.tsv"; : >"$RESULTS"
VERSIONS="$OUT/logs/versions.tsv"; : >"$VERSIONS"
FACTS="$OUT/logs/facts.tsv"; : >"$FACTS"
LOGS="$OUT/logs"

WORK=$(mktemp -d "${TMPDIR:-/tmp}/nf-verify.XXXXXX") || exit 1
WORK_DIRS+=("$WORK")
trap cleanup EXIT
trap 'exit 130' INT TERM
export TMPDIR="$WORK/tmp"
mkdir -p "$TMPDIR"
WINE_HOME="$WORK/home"
mkdir -p "$WINE_HOME"
export WINE_HOME
export CARGO_NET_OFFLINE=true   # no downloads: cargo may use only what is already in the local cache

fact commit "$COMMIT"
fact short_commit "$SHORT"
fact commit_date "$(git show -s --format=%cI "$COMMIT")"
fact subject "$(git show -s --format=%s "$COMMIT")"
fact tree_sha "$(git rev-parse "$COMMIT^{tree}")"
if [ -n "$CORE" ]; then
  fact core_tree "working tree NOT clean in crates/, web/, packaging/ ($(printf '%s\n' "$CORE" | wc -l) entries); not used by the build"
else
  fact core_tree "working tree clean in crates/, web/, packaging/ (not used by the build either way)"
fi
fact other_changes "$(printf '%s' "$OTHER" | tr '\n' ';' | sed 's/;$//')"
if [ -n "$PREVIOUS" ]; then
  fact previous_installer "$(basename "$PREVIOUS")"
  fact previous_installer_sha256 "$(sha256sum "$PREVIOUS" | cut -d' ' -f1)"
else
  fact previous_installer "none given (upgrade check not run)"
fi
fact os "$(. /etc/os-release 2>/dev/null; echo "${PRETTY_NAME:-unknown}")"
fact kernel "$(uname -srm)"
fact rustc "$(rustc -V 2>/dev/null)"
fact rustc_host "$(rustc -vV 2>/dev/null | sed -n 's/^host: //p')"
fact cargo "$(cargo -V 2>/dev/null)"
fact rustup_targets "$(rustup target list --installed 2>/dev/null | tr '\n' ' ' | sed 's/ $//' || true)"
fact mingw_gcc "$(x86_64-w64-mingw32-gcc --version 2>/dev/null | head -1)"
fact makensis "$(makensis -VERSION 2>/dev/null | head -1)"
fact zip "$(zip -v 2>/dev/null | sed -n 2p)"
fact git "$(git --version)"
fact python "$(python3 --version 2>&1)"
fact node "$(node --version 2>/dev/null)"
fact curl "$(curl --version 2>/dev/null | head -1)"
fact unshare "$(unshare --version 2>/dev/null)"
PW_PATH=$(node -p "require('path').resolve(require.resolve('playwright/package.json'),'..','..')" 2>/dev/null || true)
PW_VER=$(node -p "require(require('path').resolve(require.resolve('playwright/package.json'),'..','package.json')).version" 2>/dev/null || true)
fact playwright "${PW_VER:-not found} (module dir: ${PW_PATH:-not found})"
fact target_triples "x86_64-unknown-linux-gnu (host) and x86_64-pc-windows-gnu (cross-build)"

# 2. Build from a copy of the commit.
SRC="$WORK/src"
mkdir -p "$SRC"
git archive --format=tar "$COMMIT" | tar -x -C "$SRC" || { say "git archive failed"; exit 1; }
CACHE="${NF_VERIFY_CACHE:-$HOME/.cache/nf-verify-target}"
mkdir -p "$CACHE"
ln -s "$CACHE" "$SRC/target"
fact cargo_target_cache "shared cache of compiled dependencies only; workspace crates are rebuilt from the copy"
fact cargo_lock_sha256 "$(sha256sum "$SRC/Cargo.lock" | cut -d' ' -f1)"
VERSION=$(awk '/^\[workspace.package\]/{f=1;next} /^\[/{f=0} f && /^version *=/{gsub(/"/,"",$3); print $3; exit}' "$SRC/Cargo.toml")
fact version "$VERSION"
vrow "Cargo.toml [workspace.package] version" "$VERSION" "static" "$( [ -n "$VERSION" ] && echo yes || echo no)" ""
if [ -z "$VERSION" ]; then say "cannot read the version from Cargo.toml"; exit 1; fi

# Resolved dependencies of the server crate (normal dependencies, as the lock file resolves them).
(cd "$SRC" && timeout 300 cargo tree --locked -p nf-server -e normal --prefix none --format '{p}') >"$WORK/deps.raw" 2>"$LOGS/deps.err"
DEPS_RC=$?
sort -u "$WORK/deps.raw" >"$LOGS/deps.txt"
if [ "$DEPS_RC" -eq 0 ]; then
  rec static.dependencies "Resolved dependency list of nf-server (cargo tree --locked)" "static" passed "" \
    "$(wc -l <"$LOGS/deps.txt") distinct entries; first: $(head -1 "$LOGS/deps.txt")" deps.txt
else
  rec static.dependencies "Resolved dependency list of nf-server (cargo tree --locked)" "static" failed "cargo tree exit $DEPS_RC" "$(tail -1 "$LOGS/deps.err")" deps.err
fi

WINE_VER=$(wine_env "$WORK/wp-version" timeout 60 wine --version 2>/dev/null | head -1)
fact wine "${WINE_VER:-not found}"
WINE_VER=${WINE_VER#wine-}
[ -n "$WINE_VER" ] || WINE_VER="unavailable"

BUILD_LOG="$LOGS/build.log"
(cd "$SRC" && timeout 5400 packaging/build_all.sh) >"$BUILD_LOG" 2>&1
BUILD_RC=$?
if [ "$BUILD_RC" -eq 0 ]; then
  rec build.all "packaging/build_all.sh (all build steps)" "build (Linux host)" passed "" "exit 0; $(tail -1 "$BUILD_LOG")" build.log
else
  rec build.all "packaging/build_all.sh (all build steps)" "build (Linux host)" failed "exit $BUILD_RC" "$(grep -v '^STEP ' "$BUILD_LOG" | tail -1)" build.log
fi
while IFS= read -r step; do
  name=$(printf '%s' "$step" | sed -E 's/^STEP ([^ ]+) exit=([0-9]+).*/\1/')
  ex=$(printf '%s' "$step" | sed -E 's/^STEP ([^ ]+) exit=([0-9]+).*/\2/')
  if [ "$ex" = 0 ]; then st=passed; else st=failed; fi
  rec "build.step.$name" "Build step: $name" "build (Linux host)" "$st" "" "$step" build.log
done < <(grep '^STEP ' "$BUILD_LOG")

DIST="$SRC/dist"
TAR_NAME="nexus-fabric-$VERSION-linux-x86_64.tar.gz"
SETUP_NAME="nexus-fabric-$VERSION-windows-x64-setup.exe"
ZIP_NAME="nexus-fabric-$VERSION-windows-x64-portable.zip"
if [ "$BUILD_RC" -eq 0 ] && [ -f "$DIST/$TAR_NAME" ] && [ -f "$DIST/$SETUP_NAME" ] && [ -f "$DIST/$ZIP_NAME" ] && [ -f "$DIST/SHA256SUMS" ]; then
  cp "$DIST/$TAR_NAME" "$DIST/$SETUP_NAME" "$DIST/$ZIP_NAME" "$DIST/SHA256SUMS" "$OUT/"
  if (cd "$OUT" && sha256sum -c SHA256SUMS) >"$LOGS/sha256sum-check.log" 2>&1; then
    rec static.sha256sums "SHA256SUMS matches the three packages" "static" passed "" "$(tr '\n' ' ' <"$LOGS/sha256sum-check.log")" sha256sum-check.log
  else
    rec static.sha256sums "SHA256SUMS matches the three packages" "static" failed "mismatch" "$(tr '\n' ' ' <"$LOGS/sha256sum-check.log")" sha256sum-check.log
  fi
  for f in "$TAR_NAME" "$SETUP_NAME" "$ZIP_NAME"; do
    nv=$(printf '%s' "$f" | sed -E 's/^nexus-fabric-(.+)-(linux-x86_64\.tar\.gz|windows-x64-setup\.exe|windows-x64-portable\.zip)$/\1/')
    vrow "file name version: $f" "$nv" "static" "$( [ "$nv" = "$VERSION" ] && echo yes || echo no)" ""
  done
  DO_DYNAMIC=1
else
  say "build failed or artefacts missing: dynamic checks are not run"
  DO_DYNAMIC=0
  for id in linux.unpack linux.version linux.desktop_browser linux.offline_first_launch win.install.exit win.portable.version win.uninstall.program win.upgrade; do
    rec "$id" "Not run: build did not produce the packages" "n/a" "not run" "build failed" "" build.log
  done
fi

# 2b. Second build, from a second fresh copy of the commit, to test bit-for-bit reproducibility.
: >"$LOGS/repro.tsv"
if [ "$DO_DYNAMIC" = 1 ]; then
  SRC2="$WORK/src2"
  mkdir -p "$SRC2"
  if git archive --format=tar "$COMMIT" | tar -x -C "$SRC2" && ln -s "$CACHE" "$SRC2/target"; then
    (cd "$SRC2" && timeout 5400 packaging/build_all.sh) >"$LOGS/build-second.log" 2>&1
    RC2=$?
  else
    RC2=1
  fi
  if [ "$RC2" -eq 0 ]; then
    rec build.second "Second build from a second fresh copy of the commit (different folder path)" "build (Linux host)" passed "" "exit 0" build-second.log
    for f in "$TAR_NAME" "$SETUP_NAME" "$ZIP_NAME"; do
      printf '%s\t%s\t%s\n' "$f" "$(sha256sum "$OUT/$f" | cut -d' ' -f1)" "$(sha256sum "$SRC2/dist/$f" 2>/dev/null | cut -d' ' -f1)" >>"$LOGS/repro.tsv"
    done
    if awk -F'\t' '$2!=$3{bad=1} END{exit bad}' "$LOGS/repro.tsv"; then
      fact reproducibility "bit-for-bit identical: yes (two builds, three packages)"
    else
      fact reproducibility "bit-for-bit identical: NO (both sets of SHA-256 values are recorded)"
    fi
  else
    rec build.second "Second build from a second fresh copy of the commit (different folder path)" "build (Linux host)" failed "exit $RC2" \
      "$(grep -v '^STEP ' "$LOGS/build-second.log" | tail -1)" build-second.log
    fact reproducibility "not established: second build exit $RC2"
  fi
else
  fact reproducibility "not run: the first build did not produce the packages"
fi

if [ "$DO_DYNAMIC" = 1 ]; then
  # 4a. Linux, native.
  run_linux_checks "$SRC" "$OUT" "$WORK" "$VERSION" "$PW_PATH"
  # 4b. Windows, under wine (emulator).
  run_windows_checks "$SRC" "$OUT" "$WORK" "$VERSION" "$PREVIOUS"
  # Engine identifier must be the same on both platforms.
  if [ -n "$LINUX_ENGINE" ] && [ "$LINUX_ENGINE" = "$WIN_ENGINE" ]; then
    rec static.engine "Engine identifier agrees between the Linux binary and the Windows exe" "static" passed "" "$LINUX_ENGINE" ""
  elif [ -z "$WIN_ENGINE" ]; then
    rec static.engine "Engine identifier agrees between the Linux binary and the Windows exe" "static" "not run" \
      "the Windows exe could not be run (wine did not start); linux: ${LINUX_ENGINE:-none}" "" ""
  else
    rec static.engine "Engine identifier agrees between the Linux binary and the Windows exe" "static" failed "linux: ${LINUX_ENGINE:-none}; windows: ${WIN_ENGINE}" "" ""
  fi
  vrow "engine identifier (Linux binary)" "${LINUX_ENGINE:-none}" "native Linux" "$( [ -n "$LINUX_ENGINE" ] && echo yes || echo no)" ""
  vrow "engine identifier (Windows exe under wine)" "${WIN_ENGINE:-none}" "emulated (wine $WINE_VER; not Windows)" "$( [ -n "$WIN_ENGINE" ] && [ "$WIN_ENGINE" = "$LINUX_ENGINE" ] && echo yes || echo no)" ""
fi

# 5. Manifests.
python3 "$ROOT/packaging/verify/make_manifest.py" --out "$OUT" --docs "$ROOT/docs/release" --short "$SHORT" >"$LOGS/manifest.log" 2>&1
MRC=$?
cat "$LOGS/manifest.log" >&2
[ "$MRC" -eq 0 ] || { say "manifest generation failed (exit $MRC)"; exit 1; }

FAILED=$(awk -F'\t' '$4=="failed"{n++} END{print n+0}' "$RESULTS")
NOTRUN=$(awk -F'\t' '$4=="not run"{n++} END{print n+0}' "$RESULTS")
PASSED=$(awk -F'\t' '$4=="passed"{n++} END{print n+0}' "$RESULTS")
say "summary: passed $PASSED, failed $FAILED, not run $NOTRUN. Output: $OUT"
[ "$BUILD_RC" -eq 0 ] && [ "$FAILED" -eq 0 ]
