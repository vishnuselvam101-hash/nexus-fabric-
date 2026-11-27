#!/usr/bin/env bash
# Builds the Windows packages from a built nexus-fabric.exe.
# Usage: packaging/windows/build.sh <nf-server.exe> <version> <dist dir> <nf-pdf2dxf.exe>
# Produces in <dist dir>:
#   nexus-fabric-<version>-windows-x64-setup.exe     per-user installer (NSIS)
#   nexus-fabric-<version>-windows-x64-portable.zip  exe + converter + README + notice + third-party notices (no installer)
# nf-pdf2dxf.exe is placed beside nexus-fabric.exe, where the server looks for it.
# Reproducible (task 4.1): every staged file gets the time SOURCE_DATE_EPOCH (default: the commit time) and
# fixed permissions before it is archived; the zip stores its entries in sorted order, in UTC, with no extra
# fields (-X); the installer records the same fixed file times. PowerShell's Compress-Archive (used only when
# zip is absent) is NOT reproducible; a warning says so.
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
. "$ROOT/packaging/reproducible.sh"
nf_source_date_epoch "$ROOT"
mkdir -p "$DIST"
DIST=$(cd "$DIST" && pwd)
NAME="nexus-fabric-$VERSION"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

# Windows tools need Windows paths when run from Git Bash or MSYS.
winpath() { nf_native_path "$1"; }

mkdir -p "$STAGE/$NAME"
cp "$EXE" "$STAGE/$NAME/nexus-fabric.exe"
cp "$CONV" "$STAGE/$NAME/nf-pdf2dxf.exe"
cp "$ROOT/packaging/README.txt" "$STAGE/$NAME/README.txt"
cp "$ROOT/packaging/NOTICE.txt" "$STAGE/$NAME/NOTICE.txt"
cp "$ROOT/packaging/THIRD_PARTY_NOTICES.txt" "$STAGE/$NAME/THIRD_PARTY_NOTICES.txt"
nf_normalise_tree "$STAGE/$NAME" nexus-fabric.exe nf-pdf2dxf.exe

# Portable zip: one folder, exe + converter + README + notice.
ZIP="$DIST/$NAME-windows-x64-portable.zip"
rm -f "$ZIP"
if command -v zip >/dev/null 2>&1; then
  # The folder entry and the files, in byte order; names read from the list (-@), so zip does not walk the folder.
  (cd "$STAGE" && find "$NAME" -print | LC_ALL=C sort | TZ=UTC zip -q -X -@ "$(winpath "$ZIP")")
else
  echo "warning: zip not found; Compress-Archive is used and the portable zip is NOT reproducible" >&2
  powershell.exe -NoProfile -Command "Compress-Archive -Path '$(winpath "$STAGE/$NAME")' -DestinationPath '$(winpath "$ZIP")' -Force"
fi
echo "built $ZIP"

# Installer (NSIS). The installer needs no administrator rights.
SETUP="$DIST/$NAME-windows-x64-setup.exe"
rm -f "$SETUP"
# Signing (optional): the uninstaller is signed inside the NSIS build (!uninstfinalize, through sign.sh), the
# installer after it is built. Without a signing environment neither step runs and the output is unchanged.
SIGN_NSIS=()
SIGN_SETUP=0
"$ROOT/packaging/windows/sign.sh" --preflight || { echo "build.sh: signing setup incomplete (see above)" >&2; exit 1; }
if "$ROOT/packaging/windows/sign.sh" --requested; then
  SIGN_NSIS=(-DNF_SIGN_UNINST="$ROOT/packaging/windows/sign.sh")
  SIGN_SETUP=1
fi
makensis -V2 \
  -DVERSION="$VERSION" \
  -DSTAGE="$(winpath "$STAGE/$NAME")" \
  -DOUTFILE="$(winpath "$SETUP")" \
  ${SIGN_NSIS[@]+"${SIGN_NSIS[@]}"} \
  "$(winpath "$ROOT/packaging/windows/installer.nsi")"
if [ "$SIGN_SETUP" = 1 ]; then
  "$ROOT/packaging/windows/sign.sh" "$SETUP"
fi
echo "built $SETUP"
