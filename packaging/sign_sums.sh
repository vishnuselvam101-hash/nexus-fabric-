#!/usr/bin/env bash
# Signs SHA256SUMS with the NEXUS FABRIC release key (Ed25519, OpenSSL) and writes SHA256SUMS.sig beside it.
# Usage: packaging/sign_sums.sh <folder containing SHA256SUMS>
# Environment:
#   NF_RELEASE_KEY  path of the Ed25519 PRIVATE key (PEM). It is kept offline and never in the repository. The path is
#                   resolved (symlinks followed) before the check: a key that lies inside ANY git worktree of this
#                   repository is refused, whatever the link or folder it is named by. Its passphrase, if any, is asked
#                   by OpenSSL (not read here).
# Key set-up (once, offline; test keys: make them in a temporary folder and delete them after use):
#   openssl genpkey -algorithm ED25519 -out nf-release-private.pem
#   openssl pkey -in nf-release-private.pem -pubout -out nf-release-public.pem
# The public key (nf-release-public.pem, SubjectPublicKeyInfo PEM) is published with the release and is also kept in
# the source repository at packaging/nf-release-public.pem (a placeholder until the real key exists).
# This script prints the key fingerprint (SHA-256 of the public key in DER form): compare it with packaging/INSTALL.md.
# Check (what a customer runs; see packaging/INSTALL.md): packaging/verify_sums.sh <folder> <nf-release-public.pem>
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
if [ "$#" -ne 1 ] || [ ! -f "$1/SHA256SUMS" ]; then
  echo "usage: $0 <folder containing SHA256SUMS>" >&2
  exit 2
fi
if [ -z "${NF_RELEASE_KEY:-}" ] || [ ! -f "$NF_RELEASE_KEY" ]; then
  echo "sign_sums.sh: NF_RELEASE_KEY must name the Ed25519 private key file" >&2
  exit 2
fi
KEY_REAL=$(readlink -f "$NF_RELEASE_KEY")
# Every worktree of this repository (the main one included), as real paths.
WORKTREES=$ROOT
if command -v git >/dev/null 2>&1; then
  WORKTREES="$WORKTREES
$(git -C "$ROOT" worktree list --porcelain 2>/dev/null | sed -n 's/^worktree //p' | while IFS= read -r w; do
    if [ -d "$w" ]; then (cd "$w" && pwd -P); fi
  done)"
fi
while IFS= read -r wt; do
  [ -n "$wt" ] || continue
  case "$KEY_REAL" in
    "$wt"/*) echo "sign_sums.sh: refusing a release key inside a repository worktree (the key path resolves inside the repository)" >&2; exit 2 ;;
  esac
done <<EOF_WT
$WORKTREES
EOF_WT
DIR=$(cd "$1" && pwd)
PUB=$(mktemp "${TMPDIR:-/tmp}/nf-release-pub.XXXXXX")
trap 'rm -f "$PUB"' EXIT
openssl pkey -in "$NF_RELEASE_KEY" -pubout -out "$PUB"
FP=$(openssl pkey -pubin -in "$PUB" -outform DER | sha256sum | cut -d' ' -f1)
openssl pkeyutl -sign -inkey "$NF_RELEASE_KEY" -rawin -in "$DIR/SHA256SUMS" -out "$DIR/SHA256SUMS.sig"
echo "signed $(basename "$DIR")/SHA256SUMS -> SHA256SUMS.sig (Ed25519)"
echo "release key fingerprint (SHA-256 of the public key, DER): $FP"
