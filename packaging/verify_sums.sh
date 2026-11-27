#!/usr/bin/env bash
# Checks the signature of SHA256SUMS with the NEXUS FABRIC public key. Needs only OpenSSL (1.1.1 or later).
# Usage: packaging/verify_sums.sh <folder containing SHA256SUMS and SHA256SUMS.sig> <nf-release-public.pem>
# Exit 0 only if the signature is good. Then check the packages with: sha256sum -c SHA256SUMS
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

if [ "$#" -ne 2 ] || [ ! -f "$1/SHA256SUMS" ] || [ ! -f "$1/SHA256SUMS.sig" ] || [ ! -f "$2" ]; then
  echo "usage: $0 <folder with SHA256SUMS and SHA256SUMS.sig> <public key PEM>" >&2
  exit 2
fi
if openssl pkeyutl -verify -pubin -inkey "$2" -rawin -in "$1/SHA256SUMS" -sigfile "$1/SHA256SUMS.sig" >/dev/null 2>&1; then
  echo "SHA256SUMS signature: ok"
else
  echo "SHA256SUMS signature: FAILED (do not use these files)" >&2
  exit 1
fi
