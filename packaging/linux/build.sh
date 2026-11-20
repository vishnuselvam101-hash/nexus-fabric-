#!/usr/bin/env bash
# Builds the Linux package from a built nf-server binary.
# Usage: packaging/linux/build.sh <nf-server> <version> <dist dir>
# Produces <dist dir>/nexus-fabric-<version>-linux-x86_64.tar.gz containing:
#   nexus-fabric            the program (desktop mode is started with --desktop)
#   nexus-fabric.sh         launcher script
#   nexus-fabric.desktop    desktop entry (edit its Exec= line to the full path first)
#   README.txt, NOTICE.txt
set -euo pipefail

if [ "$#" -ne 3 ]; then
  echo "usage: $0 <nf-server> <version> <dist dir>" >&2
  exit 2
fi
BIN=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
VERSION=$2
DIST=$3
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
mkdir -p "$DIST"
DIST=$(cd "$DIST" && pwd)
NAME="nexus-fabric-$VERSION"
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/$NAME"
cp "$BIN" "$STAGE/$NAME/nexus-fabric"
chmod 755 "$STAGE/$NAME/nexus-fabric"
cp "$ROOT/packaging/linux/nexus-fabric.sh" "$STAGE/$NAME/nexus-fabric.sh"
cp "$ROOT/packaging/linux/nexus-fabric.desktop" "$STAGE/$NAME/nexus-fabric.desktop"
chmod 755 "$STAGE/$NAME/nexus-fabric.sh"
cp "$ROOT/packaging/README.txt" "$STAGE/$NAME/README.txt"
cp "$ROOT/packaging/NOTICE.txt" "$STAGE/$NAME/NOTICE.txt"

OUT="$DIST/$NAME-linux-x86_64.tar.gz"
rm -f "$OUT"
tar --owner=0 --group=0 --numeric-owner -C "$STAGE" -czf "$OUT" "$NAME"
echo "built $OUT"
