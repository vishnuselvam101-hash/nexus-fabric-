#!/usr/bin/env bash
# Builds the Linux package from a built nf-server binary.
# Usage: packaging/linux/build.sh <nf-server> <version> <dist dir> [<nf-pdf2dxf>]
# Produces <dist dir>/nexus-fabric-<version>-linux-x86_64.tar.gz containing:
#   nexus-fabric            the program (desktop mode is started with --desktop)
#   nf-pdf2dxf              the PDF -> DXF converter (when given), beside the program,
#                           where the server looks for it (crates/nf-server/src/pdfconv.rs)
#   nexus-fabric.sh         launcher script
#   nexus-fabric.desktop    desktop entry (edit its Exec= line to the full path first)
#   README.txt, NOTICE.txt, THIRD_PARTY_NOTICES.txt (licence texts of the compiled crates)
# Reproducible (task 4.1): entries sorted by name, owner root (0:0), fixed permissions, every time
# SOURCE_DATE_EPOCH (default: the commit time), GNU tar format, gzip without name or time.
set -euo pipefail

if [ "$#" -ne 3 ] && [ "$#" -ne 4 ]; then
  echo "usage: $0 <nf-server> <version> <dist dir> [<nf-pdf2dxf>]" >&2
  exit 2
fi
CONV=""
if [ "$#" -eq 4 ]; then
  CONV=$(cd "$(dirname "$4")" && pwd)/$(basename "$4")
fi
BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
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

mkdir -p "$STAGE/$NAME"
cp "$BIN" "$STAGE/$NAME/nexus-fabric"
chmod 755 "$STAGE/$NAME/nexus-fabric"
if [ -n "$CONV" ]; then
  cp "$CONV" "$STAGE/$NAME/nf-pdf2dxf"
  chmod 755 "$STAGE/$NAME/nf-pdf2dxf"
fi
cp "$ROOT/packaging/linux/nexus-fabric.sh" "$STAGE/$NAME/nexus-fabric.sh"
cp "$ROOT/packaging/linux/nexus-fabric.desktop" "$STAGE/$NAME/nexus-fabric.desktop"
chmod 755 "$STAGE/$NAME/nexus-fabric.sh"
cp "$ROOT/packaging/README.txt" "$STAGE/$NAME/README.txt"
cp "$ROOT/packaging/NOTICE.txt" "$STAGE/$NAME/NOTICE.txt"
cp "$ROOT/packaging/THIRD_PARTY_NOTICES.txt" "$STAGE/$NAME/THIRD_PARTY_NOTICES.txt"

if [ -n "$CONV" ]; then
  nf_normalise_tree "$STAGE/$NAME" nexus-fabric nexus-fabric.sh nf-pdf2dxf
else
  nf_normalise_tree "$STAGE/$NAME" nexus-fabric nexus-fabric.sh
fi

OUT="$DIST/$NAME-linux-x86_64.tar.gz"
rm -f "$OUT"
# --sort=name: fixed order; --mtime repeats the time set by nf_normalise_tree, as a guard;
# --format=gnu: no pax headers (they would carry access and change times); gzip -n: no name, no time.
tar --sort=name --format=gnu --mtime="@$SOURCE_DATE_EPOCH" --owner=0 --group=0 --numeric-owner \
  -C "$STAGE" -cf - "$NAME" | gzip -n -9 >"$OUT"
echo "built $OUT"
