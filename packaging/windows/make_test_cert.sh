#!/usr/bin/env bash
# Makes a throw-away self-signed CODE-SIGNING TEST certificate for proving the signing pipeline. It is NOT a
# release certificate: Windows does not trust it, and nothing signed with it may be shipped.
# Usage: packaging/windows/make_test_cert.sh <empty folder>
# Writes into the folder: test-signing.pfx (PKCS#12), test-signing.pass (random password, mode 600),
# test-signing.crt (public certificate, for osslsigncode verify -CAfile). The password is never printed.
# Use with sign.sh:  NF_SIGN_PFX=<folder>/test-signing.pfx NF_SIGN_PASS_FILE=<folder>/test-signing.pass
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

if [ "$#" -ne 1 ] || [ ! -d "$1" ]; then
  echo "usage: $0 <existing empty folder>" >&2
  exit 2
fi
D=$(cd "$1" && pwd)
umask 077
# On any exit, the private key is removed; on failure the other outputs are removed too.
cleanup() {
  local rc=$?
  rm -f "$D/test-signing.key"
  if [ "$rc" -ne 0 ]; then rm -f "$D/test-signing.crt" "$D/test-signing.pass" "$D/test-signing.pfx"; fi
}
trap cleanup EXIT
CN="NEXUS FABRIC TEST CERTIFICATE (not for release)"
openssl req -x509 -newkey rsa:2048 -sha256 -days 2 -nodes \
  -keyout "$D/test-signing.key" -out "$D/test-signing.crt" -subj "/CN=$CN/O=VSNexus test" \
  -addext "keyUsage=critical,digitalSignature" -addext "extendedKeyUsage=critical,codeSigning" \
  -addext "basicConstraints=critical,CA:FALSE" >/dev/null 2>&1
openssl rand -hex 24 >"$D/test-signing.pass"
openssl pkcs12 -export -inkey "$D/test-signing.key" -in "$D/test-signing.crt" -name "$CN" \
  -out "$D/test-signing.pfx" -passout "file:$D/test-signing.pass" >/dev/null 2>&1
chmod 600 "$D/test-signing.pass" "$D/test-signing.pfx"
echo "test certificate written to the folder (self-signed, NOT a release certificate)"
