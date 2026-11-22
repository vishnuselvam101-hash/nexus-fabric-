#!/usr/bin/env bash
# Regenerates packaging/THIRD_PARTY_NOTICES.txt from the dependency graph of
# the shipped programs (nf-server and the PDF converter nf-pdf2dxf: the whole
# workspace, whose five crates are exactly these two programs and their libraries), using cargo-about (cargo install cargo-about --locked --features cli).
# Usage: packaging/generate_notices.sh [cargo-about-command]
# The default command is `cargo about`. On this machine use `nf-cargo about`.
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
CMD=${1:-cargo about}
cd "$ROOT"
# shellcheck disable=SC2086
$CMD generate --manifest-path Cargo.toml --workspace \
  -c packaging/about.toml packaging/about.hbs \
  -o packaging/THIRD_PARTY_NOTICES.txt
echo "wrote packaging/THIRD_PARTY_NOTICES.txt ($(grep -c '^Crate: ' packaging/THIRD_PARTY_NOTICES.txt) crate entries)"
