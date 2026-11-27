#!/usr/bin/env bash
# Signs SHA256SUMS with the NEXUS FABRIC release key (Ed25519, OpenSSL) and writes SHA256SUMS.sig beside it.
# Usage: packaging/sign_sums.sh <folder containing SHA256SUMS>
# Environment:
#   NF_RELEASE_KEY  path of the Ed25519 PRIVATE key (PEM). It is kept offline and never in the repository: the script
#                   refuses a key inside the repository folder. Its passphrase, if any, is asked by OpenSSL (not read here).
# Key set-up (once, offline; test keys: make them in a temporary folder and delete them after use):
#   openssl genpkey -algorithm ED25519 -out nf-release-private.pem
#   openssl pkey -in nf-release-private.pem -pubout -out nf-release-public.pem
# The public key (nf-release-public.pem, SubjectPublicKeyInfo PEM) is published with the release.
# Check (what a customer runs; see packaging/INSTALL.md): packaging/verify_sums.sh <folder> <nf-release-public.pem>
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
if [ "$#" -ne 1 ] || [ ! -f "$1/SHA256SUMS" ]; then
  echo "usage: $0 <folder containing SHA256SUMS>" >&2
  exit 2
fi
if [ -z "${NF_RELEASE_KEY:-}" ] || [ ! -f "$NF_RELEASE_KEY" ]; then
  echo "sign_sums.sh: NF_RELEASE_KEY must name the Ed25519 private key file" >&2
  exit 2
fi
KEY_DIR=$(cd "$(dirname "$NF_RELEASE_KEY")" && pwd)
case "$KEY_DIR/" in
  "$ROOT/"*) echo "sign_sums.sh: refusing a release key inside the repository" >&2; exit 2 ;;
esac
DIR=$(cd "$1" && pwd)
openssl pkeyutl -sign -inkey "$NF_RELEASE_KEY" -rawin -in "$DIR/SHA256SUMS" -out "$DIR/SHA256SUMS.sig"
echo "signed $(basename "$DIR")/SHA256SUMS -> SHA256SUMS.sig (Ed25519)"
