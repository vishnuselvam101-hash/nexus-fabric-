#!/usr/bin/env bash
# Builds the Windows packages from a built nexus-fabric.exe.
# Usage: packaging/windows/build.sh <nf-server.exe> <version> <dist dir> <nf-pdf2dxf.exe>
# Produces in <dist dir>:
#   nexus-fabric-<version>-windows-x64-setup.exe     per-user installer (NSIS)
#   nexus-fabric-<version>-windows-x64-portable.zip  exe + converter + README + notice + third-party notices (no installer)
# nf-pdf2dxf.exe is placed beside nexus-fabric.exe, where the server looks for it.
set -euo pipefail

if [ "$#" -ne 4 ]; then
  echo "usage: $0 <nf-server.exe> <version> <dist dir> <nf-pdf2dxf.exe>" >&2
  exit 2
fi
EXE=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
CONV=$(cd "$(dirname "$4")" && pwd)/$(basename "$4")
VERSION=$2
DIST=$3
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$DIST"
DIST=$(cd "$DIST" && pwd)
NAME="nexus-fabric-$VERSION"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

# Windows tools need Windows paths when run from Git Bash or MSYS.
winpath() { if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi; }

mkdir -p "$STAGE/$NAME"
cp "$EXE" "$STAGE/$NAME/nexus-fabric.exe"
cp "$CONV" "$STAGE/$NAME/nf-pdf2dxf.exe"
cp "$ROOT/packaging/README.txt" "$STAGE/$NAME/README.txt"
cp "$ROOT/packaging/NOTICE.txt" "$STAGE/$NAME/NOTICE.txt"
cp "$ROOT/packaging/THIRD_PARTY_NOTICES.txt" "$STAGE/$NAME/THIRD_PARTY_NOTICES.txt"

# Portable zip: one folder, exe + converter + README + notice.
ZIP="$DIST/$NAME-windows-x64-portable.zip"
rm -f "$ZIP"
if command -v zip >/dev/null 2>&1; then
  (cd "$STAGE" && zip -q -r -X "$(winpath "$ZIP")" "$NAME")
else
  powershell.exe -NoProfile -Command "Compress-Archive -Path '$(winpath "$STAGE/$NAME")' -DestinationPath '$(winpath "$ZIP")' -Force"
fi
echo "built $ZIP"

# Installer (NSIS). The installer needs no administrator rights.
SETUP="$DIST/$NAME-windows-x64-setup.exe"
rm -f "$SETUP"
makensis -V2 \
  -DVERSION="$VERSION" \
  -DSTAGE="$(winpath "$STAGE/$NAME")" \
  -DOUTFILE="$(winpath "$SETUP")" \
  "$(winpath "$ROOT/packaging/windows/installer.nsi")"
echo "built $SETUP"
