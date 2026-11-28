#!/usr/bin/env bash
# Fails if the Linux package does not ship THIRD_PARTY_NOTICES.txt with the
# licence texts of the compiled crates, or if the notices list does not match
# the dependency graph of the shipped programs (nf-server and the PDF converter
# nf-pdf2dxf from nf-pdf) for both packaged targets.
# Usage: bash tests/notices_check.sh [path to nf-server binary] [cargo command]
# Default binary: target/release/nf-server (build it first: nf-cargo build --release).
# Default cargo command: nf-cargo (CI passes plain `cargo`).
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN=${1:-$ROOT/target/release/nf-server}
CARGO=${2:-nf-cargo}
VERSION=0.0.0-notices-check
TARGETS="x86_64-unknown-linux-gnu x86_64-pc-windows-gnu"
FIRST_PARTY="nf-core nf-digest nf-dxf nf-jobout nf-licence nf-pdf nf-pdf-winjob nf-server uem"
fail=0

check() { # check <description> <command...>
  local desc=$1; shift
  if "$@" >/dev/null 2>&1; then
    echo "ok   $desc"
  else
    echo "FAIL $desc"
    fail=1
  fi
}

if [ ! -x "$BIN" ]; then
  echo "FAIL no executable binary at $BIN (run: nf-cargo build --release)"
  exit 1
fi

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/dist" "$TMP/unpack"

# Build the real Linux package into a temp dir. A missing notices file in
# packaging/ makes build.sh fail here.
if ! bash "$ROOT/packaging/linux/build.sh" "$BIN" "$VERSION" "$TMP/dist" >"$TMP/build.log" 2>&1; then
  echo "FAIL packaging/linux/build.sh failed"
  cat "$TMP/build.log"
  exit 1
fi
echo "ok   packaging/linux/build.sh built the package"

PKG="$TMP/dist/nexus-fabric-$VERSION-linux-x86_64.tar.gz"
tar -xzf "$PKG" -C "$TMP/unpack"
NOTICES="$TMP/unpack/nexus-fabric-$VERSION/THIRD_PARTY_NOTICES.txt"

# The exact-build check: the crates named in the notices file must be the crates
# cargo resolves for nf-server and nf-pdf (normal and build dependencies, both packaged
# targets), without the first-party workspace crates. A stale file fails here.
notices_match_graph() { # notices_match_graph <THIRD_PARTY_NOTICES.txt>
  local notices=$1 t
  : > "$TMP/graph.txt"
  for t in $TARGETS; do
    if ! (cd "$ROOT" && $CARGO tree --locked -p nf-server -p nf-pdf -e normal,build \
          --target "$t" --prefix none -f '{p}' > "$TMP/tree-$t.txt"); then
      echo "cargo tree failed for $t (command: $CARGO)"
      return 1
    fi
    awk -v fp="$FIRST_PARTY" '
      BEGIN { n = split(fp, a, " "); for (i = 1; i <= n; i++) skip[a[i]] = 1 }
      NF >= 2 && !($1 in skip) { sub(/^v/, "", $2); print $1, $2 }' \
      "$TMP/tree-$t.txt" >> "$TMP/graph.txt"
  done
  sort -u "$TMP/graph.txt" -o "$TMP/graph.txt"
  sed -n 's/^Crate: //p' "$notices" | sort -u > "$TMP/notices-set.txt"
  if ! diff "$TMP/graph.txt" "$TMP/notices-set.txt" > "$TMP/set.diff"; then
    echo "  graph has $(wc -l < "$TMP/graph.txt") name+version pairs, notices list $(wc -l < "$TMP/notices-set.txt")"
    head -20 "$TMP/set.diff" | sed 's/^/  /'
    return 1
  fi
  return 0
}

# Every web/vendor/*/LICENSE (the licence of a bundled browser library) must appear
# verbatim in the notices file, so a vendored library cannot ship without its licence text.
vendor_licences_listed() { # vendor_licences_listed <THIRD_PARTY_NOTICES.txt>
  local notices_text lic n=0 text
  notices_text=$(cat "$1")
  for lic in "$ROOT"/web/vendor/*/LICENSE; do
    [ -f "$lic" ] || continue
    n=$((n+1))
    text=$(cat "$lic")
    case "$notices_text" in
      *"$text"*) ;;
      *) echo "  not listed verbatim: ${lic#"$ROOT"/}"; return 1 ;;
    esac
  done
  [ "$n" -gt 0 ] || { echo "  no web/vendor/*/LICENSE found"; return 1; }
  return 0
}

check "THIRD_PARTY_NOTICES.txt is in the package" test -f "$NOTICES"
if [ -f "$NOTICES" ]; then
  check "notices list at least 90 crate entries" \
    test "$(grep -c '^Crate: ' "$NOTICES")" -ge 90
  check "notices contain the entry and version of serde" \
    grep -q '^Crate: serde [0-9]' "$NOTICES"
  check "notices contain the MIT License text" \
    grep -q 'MIT License' "$NOTICES"
  check "notices contain the Apache License 2.0 text" \
    grep -q 'Apache License' "$NOTICES"
  check "notices contain the MIT permission grant (full licence text)" \
    grep -q 'Permission is hereby granted, free of charge' "$NOTICES"
  check "notices name+version set equals cargo tree -e normal,build of nf-server and nf-pdf for both targets" \
    notices_match_graph "$NOTICES"
  check "every web/vendor/*/LICENSE is listed verbatim in the notices" \
    vendor_licences_listed "$NOTICES"
fi

if [ "$fail" -ne 0 ]; then
  echo "notices check FAILED"
  exit 1
fi
echo "notices check passed"
