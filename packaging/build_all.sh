#!/usr/bin/env bash
# Builds every package into dist/ (ignored by git; binaries are never committed).
# Usage: packaging/build_all.sh [--linux-only]
#   On Linux:   Linux tar.gz, and the Windows exe, installer and portable zip
#               (cross-compiled with mingw-w64; needs the x86_64-pc-windows-gnu
#               Rust target and x86_64-w64-mingw32-gcc). --linux-only skips Windows.
#   On Windows: the Windows exe, installer and portable zip (Git Bash, NSIS on PATH).
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
cd "$ROOT"
LINUX_ONLY=0
[ "${1:-}" = "--linux-only" ] && LINUX_ONLY=1

# The version of the workspace package (the [workspace.package] table).
VERSION=$(awk '/^\[workspace.package\]/{f=1;next} /^\[/{f=0} f && /^version *=/{gsub(/"/,"",$3); print $3; exit}' Cargo.toml)
if [ -z "$VERSION" ]; then
  echo "cannot read the workspace version from Cargo.toml" >&2
  exit 1
fi
DIST="$ROOT/dist"
mkdir -p "$DIST"
echo "NEXUS FABRIC $VERSION packages -> $DIST"

# Runs one build step and prints "STEP <name> exit=<status>" (read by packaging/verify_release.sh), then stops on failure.
step() {
  local name=$1 rc
  shift
  set +e
  "$@"
  rc=$?
  set -e
  echo "STEP $name exit=$rc"
  if [ "$rc" -ne 0 ]; then
    echo "build step $name failed (exit $rc)" >&2
    exit "$rc"
  fi
}

case "$(uname -s)" in
  Linux*)
    step cargo-linux cargo build --release -p nf-server -p nf-pdf --bin nf-server --bin nf-pdf2dxf
    step package-linux packaging/linux/build.sh target/release/nf-server "$VERSION" "$DIST" target/release/nf-pdf2dxf
    if [ "$LINUX_ONLY" -eq 0 ]; then
      if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
        echo "x86_64-w64-mingw32-gcc not found: install mingw-w64, or run with --linux-only" >&2
        exit 1
      fi
      if ! rustup target list --installed 2>/dev/null | grep -qx x86_64-pc-windows-gnu; then
        echo "Rust target x86_64-pc-windows-gnu missing: rustup target add x86_64-pc-windows-gnu (or --linux-only)" >&2
        exit 1
      fi
      step cargo-windows env CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
        cargo build --release -p nf-server -p nf-pdf --bin nf-server --bin nf-pdf2dxf --target x86_64-pc-windows-gnu
      step package-windows packaging/windows/build.sh target/x86_64-pc-windows-gnu/release/nf-server.exe "$VERSION" "$DIST" \
        target/x86_64-pc-windows-gnu/release/nf-pdf2dxf.exe
    fi
    ;;
  MINGW*|MSYS*|CYGWIN*)
    step cargo-windows-host cargo build --release -p nf-server -p nf-pdf --bin nf-server --bin nf-pdf2dxf
    step package-windows-host packaging/windows/build.sh target/release/nf-server.exe "$VERSION" "$DIST" target/release/nf-pdf2dxf.exe
    ;;
  *)
    echo "unsupported host: $(uname -s)" >&2
    exit 1
    ;;
esac

# Checksums of every package in dist/ (the file itself excluded).
(
  cd "$DIST"
  rm -f SHA256SUMS
  for f in nexus-fabric-*; do
    if [ -f "$f" ]; then sha256sum "$f"; fi
  done > SHA256SUMS
  cat SHA256SUMS
)
echo "STEP sha256sums exit=0"
echo "done: $DIST"
