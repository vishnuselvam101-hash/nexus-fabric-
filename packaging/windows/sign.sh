#!/usr/bin/env bash
# Signs Windows programs (Authenticode, SHA-256) with osslsigncode. Called by packaging/build_all.sh (the two
# programs), packaging/windows/build.sh (the installer) and, for the uninstaller, by the NSIS build (!uninstfinalize).
# Usage: packaging/windows/sign.sh <file.exe> [more files]      (each file is signed in place)
# Signing is requested only when NF_SIGN_PKCS11_MODULE or NF_SIGN_PFX is set. Without either, nothing is done
# and the caller prints "unsigned build". The secret values are never printed or written to a log by this script.
# Environment:
#   Production (token or cloud HSM, through PKCS#11):
#     NF_SIGN_PKCS11_MODULE  path of the PKCS#11 module of the token or cloud HSM
#     NF_SIGN_PKCS11_KEY     PKCS#11 URI of the private key, e.g. pkcs11:token=<label>;id=%01;type=private
#     NF_SIGN_PKCS11_CERT    PKCS#11 URI of the certificate on the token (optional if NF_SIGN_CERTS is set)
#     NF_SIGN_CERTS          file with the certificate chain, PEM (needed if the certificate is not on the token)
#     NF_SIGN_PASS_FILE      file holding the token PIN (first line; optional; read by osslsigncode, not on the command line)
#   Test certificates only (packaging/windows/make_test_cert.sh):
#     NF_SIGN_PFX            PKCS#12 file
#     NF_SIGN_PASS_FILE      file holding the PKCS#12 password (first line; read by osslsigncode, not on the command line)
#   Both:
#     NF_SIGN_TS             RFC 3161 timestamp URL. Unset: no timestamp is added (the signature then expires with the cert).
#     NF_SIGN_DESC           description shown by Windows (default: NEXUS FABRIC)
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

# Tells the caller whether signing was requested (exit 0) or not (exit 1). Used by build_all.sh, build.sh and verify_release.sh.
nf_sign_requested() {
  [ -n "${NF_SIGN_PKCS11_MODULE:-}" ] || [ -n "${NF_SIGN_PFX:-}" ]
}

if [ "${1:-}" = "--requested" ]; then
  nf_sign_requested && exit 0 || exit 1
fi

if [ "$#" -lt 1 ]; then
  echo "usage: $0 <file.exe> [more files]" >&2
  exit 2
fi
if ! nf_sign_requested; then
  echo "sign.sh: no signing environment set (NF_SIGN_PKCS11_MODULE or NF_SIGN_PFX); nothing signed" >&2
  exit 1
fi
if ! command -v osslsigncode >/dev/null 2>&1; then
  echo "sign.sh: osslsigncode not found" >&2
  exit 1
fi

ARGS=(-h sha2 -n "${NF_SIGN_DESC:-NEXUS FABRIC}")
if [ -n "${NF_SIGN_PFX:-}" ]; then
  [ -f "$NF_SIGN_PFX" ] || { echo "sign.sh: NF_SIGN_PFX file not found" >&2; exit 1; }
  ARGS+=(-pkcs12 "$NF_SIGN_PFX")
else
  [ -n "${NF_SIGN_PKCS11_KEY:-}" ] || { echo "sign.sh: NF_SIGN_PKCS11_KEY is not set" >&2; exit 1; }
  [ -f "$NF_SIGN_PKCS11_MODULE" ] || { echo "sign.sh: NF_SIGN_PKCS11_MODULE file not found" >&2; exit 1; }
  ARGS+=(-key "$NF_SIGN_PKCS11_KEY" -pkcs11module "$NF_SIGN_PKCS11_MODULE")
  if [ -n "${NF_SIGN_PKCS11_CERT:-}" ]; then ARGS+=(-pkcs11cert "$NF_SIGN_PKCS11_CERT"); fi
  if [ -n "${NF_SIGN_CERTS:-}" ]; then ARGS+=(-certs "$NF_SIGN_CERTS"); fi
fi
if [ -n "${NF_SIGN_PASS_FILE:-}" ]; then
  [ -f "$NF_SIGN_PASS_FILE" ] || { echo "sign.sh: NF_SIGN_PASS_FILE not found" >&2; exit 1; }
  ARGS+=(-readpass "$NF_SIGN_PASS_FILE")
fi
if [ -n "${NF_SIGN_TS:-}" ]; then
  ARGS+=(-ts "$NF_SIGN_TS")
  TS_NOTE="RFC 3161 timestamp from NF_SIGN_TS"
else
  TS_NOTE="no timestamp (NF_SIGN_TS not set)"
fi

for f in "$@"; do
  [ -f "$f" ] || { echo "sign.sh: file not found: $(basename "$f")" >&2; exit 1; }
  case "$(head -c 2 "$f")" in
    MZ) ;;
    *) echo "sign.sh: not a Windows program: $(basename "$f")" >&2; exit 1 ;;
  esac
  TMP="$f.signing.$$"
  rc=0
  osslsigncode sign "${ARGS[@]}" -in "$f" -out "$TMP" >"$TMP.log" 2>&1 || rc=$?
  if [ "$rc" -ne 0 ]; then
    LAST=$(grep -v -i 'pass' "$TMP.log" 2>/dev/null | tail -1 || true)
    rm -f "$TMP" "$TMP.log"
    echo "sign.sh: signing failed for $(basename "$f") (osslsigncode exit $rc): ${LAST:-no message}" >&2
    exit 1
  fi
  rm -f "$TMP.log"
  mv -f "$TMP" "$f"
  echo "signed $(basename "$f") (SHA-256 digest; $TS_NOTE)"
done
