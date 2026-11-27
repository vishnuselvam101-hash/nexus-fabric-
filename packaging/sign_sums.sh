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
# This script checks the public half (packaging/nf_pubkey_check.sh: Ed25519, not a placeholder) and prints its
# fingerprint: compare it with packaging/INSTALL.md. Every failure prints one line saying why, and exits non-zero.
# Check (what a customer runs; see packaging/INSTALL.md): packaging/verify_sums.sh <folder> <nf-release-public.pem>
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
die() { echo "sign_sums.sh: $*" >&2; exit 1; }
PUB=""
SIG_TMP=""
cleanup() {
  if [ -n "$PUB" ]; then rm -f "$PUB"; fi
  if [ -n "$SIG_TMP" ]; then rm -f "$SIG_TMP"; fi
}
trap cleanup EXIT
trap 'echo "sign_sums.sh: interrupted; no signature was written" >&2; exit 130' INT TERM
. "$ROOT/packaging/nf_pubkey_check.sh" || die "helper packaging/nf_pubkey_check.sh is missing"

if [ "$#" -ne 1 ] || [ ! -f "$1/SHA256SUMS" ]; then
  echo "usage: $0 <folder containing SHA256SUMS>" >&2
  exit 2
fi
if [ -z "${NF_RELEASE_KEY:-}" ] || [ ! -f "$NF_RELEASE_KEY" ]; then
  echo "sign_sums.sh: NF_RELEASE_KEY must name the Ed25519 private key file (not set, or file not found)" >&2
  exit 2
fi
KEY_REAL=$(readlink -f "$NF_RELEASE_KEY") || die "cannot resolve the path of NF_RELEASE_KEY"
[ -n "$KEY_REAL" ] || die "cannot resolve the path of NF_RELEASE_KEY"
grep -q 'PRIVATE KEY' -- "$NF_RELEASE_KEY" || die "NF_RELEASE_KEY is not a PRIVATE key (PEM); refusing it"
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
DIR=$(cd "$1" && pwd) || die "cannot enter folder $1"
PUB=$(mktemp "${TMPDIR:-/tmp}/nf-release-pub.XXXXXX") || die "cannot create a temporary file in ${TMPDIR:-/tmp}"
if ! openssl pkey -in "$NF_RELEASE_KEY" -pubout -out "$PUB" 2>/dev/null; then
  die "cannot read the release key (not a valid PEM private key, wrong passphrase, or unreadable)"
fi
nf_pubkey_check "$PUB" || die "the release key's public half failed the Ed25519 check (see above)"
SIG_TMP=$(mktemp "$DIR/.SHA256SUMS.sig.XXXXXX") || die "cannot create a file in $DIR (read-only folder?)"
if ! openssl pkeyutl -sign -inkey "$NF_RELEASE_KEY" -rawin -in "$DIR/SHA256SUMS" -out "$SIG_TMP" 2>/dev/null; then
  die "Ed25519 signing failed (wrong key or passphrase, or the key is not Ed25519)"
fi
chmod 644 "$SIG_TMP" || die "cannot set the mode of the signature file"
mv -f "$SIG_TMP" "$DIR/SHA256SUMS.sig" || die "cannot write $DIR/SHA256SUMS.sig"
SIG_TMP=""
echo "signed $(basename "$DIR")/SHA256SUMS -> SHA256SUMS.sig (Ed25519)"
