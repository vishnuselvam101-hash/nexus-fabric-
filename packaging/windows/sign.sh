#!/usr/bin/env bash
# Signs Windows programs (Authenticode, SHA-256) with osslsigncode. Called by packaging/build_all.sh (the two
# programs), packaging/windows/build.sh (the installer) and, for the uninstaller, by the NSIS build (!uninstfinalize).
# Usage: packaging/windows/sign.sh <file.exe> [more files]      (each file is signed in place; non-zero exit = failed)
#        packaging/windows/sign.sh --preflight    (exit 0 if the signing setup is complete, or unset and not required)
#        packaging/windows/sign.sh --requested    (exit 0 if signing is requested, 1 if not; no checks)
# Signing is requested only when NF_SIGN_PKCS11_MODULE or NF_SIGN_PFX is set. Without either, nothing is done
# and the caller prints "unsigned build". Any other NF_SIGN_* variable set without a signing request is an error
# (an incomplete setup is never silently treated as unsigned). NF_SIGN_REQUIRED=1 makes a build without signing fail.
# The secret values are never printed or written to a log by this script.
# Environment:
#   Production (token or cloud HSM, through PKCS#11):
#     NF_SIGN_PKCS11_MODULE  path of the PKCS#11 module of the token or cloud HSM
#     NF_SIGN_PKCS11_KEY     PKCS#11 URI of the private key, e.g. pkcs11:token=<label>;id=%01;type=private
#                            (never put the PIN in the URI: use NF_SIGN_PASS_FILE)
#     NF_SIGN_PKCS11_CERT    PKCS#11 URI of the certificate on the token (needed unless NF_SIGN_CERTS is set)
#     NF_SIGN_CERTS          file with the certificate chain, PEM (needed if the certificate is not on the token)
#     NF_SIGN_PASS_FILE      file holding the token PIN (first line; optional; read by osslsigncode, not on the command line)
#   Test certificates only (packaging/windows/make_test_cert.sh):
#     NF_SIGN_PFX            PKCS#12 file
#     NF_SIGN_PASS_FILE      file holding the PKCS#12 password (first line; required; read by osslsigncode, not on the command line)
#   Both:
#     NF_SIGN_TS             RFC 3161 timestamp URL. Unset: no timestamp is added (the signature then expires with the cert).
#     NF_SIGN_DESC           description shown by Windows (default: NEXUS FABRIC)
#     NF_SIGN_REQUIRED       1: the build fails unless it is signed
# Development build. Not validated. Outputs must not be used for manufacturing.
set -euo pipefail

# Tells the caller whether signing was requested (exit 0) or not (exit 1). Used by build_all.sh, build.sh and verify_release.sh.
nf_sign_requested() {
  [ -n "${NF_SIGN_PKCS11_MODULE:-}" ] || [ -n "${NF_SIGN_PFX:-}" ]
}

# Checks the signing setup. Exit 0: complete, or nothing set and not required. Exit 1: incomplete, with the reason.
nf_sign_preflight() {
  local v
  if [ -n "${NF_SIGN_PFX:-}" ] && [ -n "${NF_SIGN_PKCS11_MODULE:-}" ]; then
    echo "sign.sh: set NF_SIGN_PFX (test certificate) or NF_SIGN_PKCS11_MODULE (production), not both" >&2; return 1
  fi
  if nf_sign_requested; then
    if [ -n "${NF_SIGN_PFX:-}" ]; then
      [ -f "$NF_SIGN_PFX" ] || { echo "sign.sh: NF_SIGN_PFX file not found" >&2; return 1; }
      [ -n "${NF_SIGN_PASS_FILE:-}" ] || { echo "sign.sh: NF_SIGN_PFX needs NF_SIGN_PASS_FILE" >&2; return 1; }
    else
      [ -f "$NF_SIGN_PKCS11_MODULE" ] || { echo "sign.sh: NF_SIGN_PKCS11_MODULE file not found" >&2; return 1; }
      [ -n "${NF_SIGN_PKCS11_KEY:-}" ] || { echo "sign.sh: NF_SIGN_PKCS11_KEY is not set" >&2; return 1; }
      [ -n "${NF_SIGN_PKCS11_CERT:-}${NF_SIGN_CERTS:-}" ] || { echo "sign.sh: set NF_SIGN_PKCS11_CERT or NF_SIGN_CERTS" >&2; return 1; }
    fi
    if [ -n "${NF_SIGN_PASS_FILE:-}" ] && [ ! -f "$NF_SIGN_PASS_FILE" ]; then
      echo "sign.sh: NF_SIGN_PASS_FILE not found" >&2; return 1
    fi
    case "${NF_SIGN_PKCS11_KEY:-}${NF_SIGN_PKCS11_CERT:-}" in
      *pin-value*) echo "sign.sh: the PIN must not be in a PKCS#11 URI; use NF_SIGN_PASS_FILE" >&2; return 1 ;;
    esac
    return 0
  fi
  for v in NF_SIGN_PKCS11_KEY NF_SIGN_PKCS11_CERT NF_SIGN_CERTS NF_SIGN_PASS_FILE NF_SIGN_TS NF_SIGN_DESC; do
    if [ -n "${!v:-}" ]; then
      echo "sign.sh: incomplete signing setup: $v is set but NF_SIGN_PKCS11_MODULE or NF_SIGN_PFX is not" >&2
      return 1
    fi
  done
  if [ "${NF_SIGN_REQUIRED:-0}" = 1 ]; then
    echo "sign.sh: NF_SIGN_REQUIRED=1 but no signing environment (NF_SIGN_PKCS11_MODULE or NF_SIGN_PFX)" >&2
    return 1
  fi
  return 0
}

if [ "${1:-}" = "--requested" ]; then
  nf_sign_requested && exit 0 || exit 1
fi
if [ "${1:-}" = "--preflight" ]; then
  nf_sign_preflight && exit 0 || exit 1
fi

if [ "$#" -lt 1 ]; then
  echo "usage: $0 <file.exe> [more files]" >&2
  exit 2
fi
nf_sign_preflight || exit 1
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
  ARGS+=(-pkcs12 "$NF_SIGN_PFX")
else
  ARGS+=(-key "$NF_SIGN_PKCS11_KEY" -pkcs11module "$NF_SIGN_PKCS11_MODULE")
  if [ -n "${NF_SIGN_PKCS11_CERT:-}" ]; then ARGS+=(-pkcs11cert "$NF_SIGN_PKCS11_CERT"); fi
  if [ -n "${NF_SIGN_CERTS:-}" ]; then ARGS+=(-certs "$NF_SIGN_CERTS"); fi
fi
if [ -n "${NF_SIGN_PASS_FILE:-}" ]; then
  ARGS+=(-readpass "$NF_SIGN_PASS_FILE")
fi
if [ -n "${NF_SIGN_TS:-}" ]; then
  ARGS+=(-ts "$NF_SIGN_TS")
  TS_NOTE="RFC 3161 timestamp from NF_SIGN_TS"
else
  TS_NOTE="no timestamp (NF_SIGN_TS not set)"
fi

# A partial output (.signing.<pid>) is removed on any exit, including an interrupt.
TMP=""
cleanup() {
  if [ -n "$TMP" ]; then rm -f "$TMP" "$TMP.log"; fi
}
trap cleanup EXIT
trap 'echo "sign.sh: interrupted; the file being signed is left unchanged" >&2; exit 130' INT TERM

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
    echo "sign.sh: signing failed for $(basename "$f") (osslsigncode exit $rc): ${LAST:-no message}" >&2
    exit 1
  fi
  rm -f "$TMP.log"
  mv -f "$TMP" "$f" || { echo "sign.sh: cannot replace $(basename "$f") with its signed copy (file not writable?)" >&2; exit 1; }
  TMP=""
  echo "signed $(basename "$f") (SHA-256 digest; $TS_NOTE)"
done
