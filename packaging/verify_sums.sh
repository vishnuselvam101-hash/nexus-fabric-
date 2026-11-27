#!/usr/bin/env bash
# Checks the signature of SHA256SUMS with the NEXUS FABRIC public key. Needs only OpenSSL (1.1.1 or later).
# Usage: packaging/verify_sums.sh <folder containing SHA256SUMS and SHA256SUMS.sig> <nf-release-public.pem>
# The public key is checked first (packaging/nf_pubkey_check.sh): it must be an Ed25519 public key, not the placeholder
# and not a private key; its SHA-256 fingerprint is printed. Exit 0 only if the key is valid and the signature is good.
# Then check the packages with: sha256sum -c SHA256SUMS
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd -P)
if ! . "$ROOT/packaging/nf_pubkey_check.sh"; then
  echo "verify_sums.sh: helper packaging/nf_pubkey_check.sh is missing" >&2; exit 2
fi
if [ "$#" -ne 2 ]; then
  echo "usage: $0 <folder with SHA256SUMS and SHA256SUMS.sig> <public key PEM>" >&2
  exit 2
fi
[ -f "$1/SHA256SUMS" ] || { echo "verify_sums.sh: SHA256SUMS not found in $1" >&2; exit 2; }
[ -f "$1/SHA256SUMS.sig" ] || { echo "verify_sums.sh: SHA256SUMS.sig not found in $1 (unsigned release or incomplete download); do not use these files" >&2; exit 1; }
if ! nf_pubkey_check "$2"; then
  echo "verify_sums.sh: SHA256SUMS NOT verified: the public key was rejected" >&2
  exit 1
fi
if openssl pkeyutl -verify -pubin -inkey "$2" -rawin -in "$1/SHA256SUMS" -sigfile "$1/SHA256SUMS.sig" >/dev/null 2>&1; then
  echo "SHA256SUMS signature: ok"
else
  echo "SHA256SUMS signature: FAILED (do not use these files)" >&2
  exit 1
fi
