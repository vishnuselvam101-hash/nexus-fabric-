#!/usr/bin/env bash
# Failure-path tests for the signing scripts (packaging/sign_sums.sh, verify_sums.sh, windows/sign.sh, make_test_cert.sh).
# Every case runs a script with a bad input or a forced failure and checks the exit code and the one-line reason.
# Throw-away keys, a test certificate and fake tools live in a mktemp folder, removed on exit. Nothing is written to the repo
# (the worktree-key case creates a file inside the repo for a moment and removes it). No secret is printed.
# Usage: packaging/tests/signing_failures.sh [sums|verify|sign|cert|all]
# Development build. Not validated. Outputs must not be used for manufacturing.
set -uo pipefail

PKG=$(cd "$(dirname "$0")/.." && pwd -P)
REPO=$(cd "$PKG/.." && pwd -P)
GROUP=${1:-all}
W=$(mktemp -d "${TMPDIR:-/tmp}/nf-sign-tests.XXXXXX") || { echo "cannot create a temporary folder" >&2; exit 2; }
WT_KEY=""
cleanup() {
  if [ -n "$WT_KEY" ]; then rm -f "$WT_KEY"; fi
  if [ -n "$W" ]; then rm -rf -- "$W"; fi
}
trap cleanup EXIT
PASS=0; FAIL=0

# case <name> <exit wanted> <text wanted in output, or ""> <command...>   (stdin from /dev/null: no prompts)
case_run() {
  local name=$1 want=$2 sub=$3 out rc=0
  shift 3
  out=$("$@" </dev/null 2>&1) || rc=$?
  echo "CASE $name (exit $rc, want $want)"
  printf '%s\n' "$out" | sed 's/^/  | /'
  if [ "$rc" -eq "$want" ] && { [ -z "$sub" ] || grep -qF -- "$sub" <<<"$out"; }; then
    PASS=$((PASS + 1)); echo "  PASS"
  else
    FAIL=$((FAIL + 1)); echo "  FAIL"
  fi
}

# Keys and test material (throw-away, in $W).
mkkeys() {
  openssl genpkey -algorithm ED25519 -out "$W/ed-priv.pem" 2>/dev/null
  openssl pkey -in "$W/ed-priv.pem" -pubout -out "$W/ed-pub.pem" 2>/dev/null
  openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$W/rsa-priv.pem" 2>/dev/null
  openssl pkey -in "$W/rsa-priv.pem" -pubout -out "$W/rsa-pub.pem" 2>/dev/null
  printf -- '-----BEGIN PRIVATE KEY-----\nnot-a-key\n-----END PRIVATE KEY-----\n' >"$W/broken-priv.pem"
  printf 'hello\n' >"$W/notakey.pem"
  mkdir -p "$W/rel" && printf 'abc123  nf-server.exe\n' >"$W/rel/SHA256SUMS"
}

sums_tests() {
  case_run "sign_sums: no arguments" 2 "usage:" "$PKG/sign_sums.sh"
  case_run "sign_sums: NF_RELEASE_KEY not set" 2 "NF_RELEASE_KEY must name" env -u NF_RELEASE_KEY "$PKG/sign_sums.sh" "$W/rel"
  case_run "sign_sums: key file not found" 2 "NF_RELEASE_KEY must name" env NF_RELEASE_KEY="$W/none.pem" "$PKG/sign_sums.sh" "$W/rel"
  WT_KEY="$REPO/.dist-r2-key-test.$$.pem"
  cp "$W/ed-priv.pem" "$WT_KEY"
  case_run "sign_sums: key inside a repository worktree" 2 "inside a repository worktree" env NF_RELEASE_KEY="$WT_KEY" "$PKG/sign_sums.sh" "$W/rel"
  rm -f "$WT_KEY"; WT_KEY=""
  case_run "sign_sums: key is a public key, not a private key" 1 "is not a PRIVATE key" env NF_RELEASE_KEY="$W/ed-pub.pem" "$PKG/sign_sums.sh" "$W/rel"
  case_run "sign_sums: not a PEM key" 1 "is not a PRIVATE key" env NF_RELEASE_KEY="$W/notakey.pem" "$PKG/sign_sums.sh" "$W/rel"
  case_run "sign_sums: broken private key" 1 "cannot read the release key" env NF_RELEASE_KEY="$W/broken-priv.pem" "$PKG/sign_sums.sh" "$W/rel"
  case_run "sign_sums: RSA key (not Ed25519)" 1 "is not an Ed25519 public key" env NF_RELEASE_KEY="$W/rsa-priv.pem" "$PKG/sign_sums.sh" "$W/rel"
  case_run "sign_sums: temporary file cannot be created" 1 "cannot create a temporary file" env TMPDIR="$W/no-such-dir" NF_RELEASE_KEY="$W/ed-priv.pem" "$PKG/sign_sums.sh" "$W/rel"
  case_run "sign_sums: good Ed25519 key" 0 "Ed25519 public key fingerprint" env NF_RELEASE_KEY="$W/ed-priv.pem" "$PKG/sign_sums.sh" "$W/rel"
  mkdir -p "$W/rel2" "$W/fakesign"
  printf 'abc123  x\n' >"$W/rel2/SHA256SUMS"
  cat >"$W/fakesign/openssl" <<FAKE
#!/usr/bin/env bash
# Test double: fails the Ed25519 signing step only; everything else is the real openssl.
case "\$*" in *pkeyutl*-sign*) exit 1 ;; esac
exec "$(command -v openssl)" "\$@"
FAKE
  chmod +x "$W/fakesign/openssl"
  case_run "sign_sums: Ed25519 signing step fails" 1 "Ed25519 signing failed" env PATH="$W/fakesign:$PATH" NF_RELEASE_KEY="$W/ed-priv.pem" "$PKG/sign_sums.sh" "$W/rel2"
  left=$(find "$W/rel2" -mindepth 1 | wc -l)
  echo "  files left in folder after failed signing: $left (want 1: SHA256SUMS only)"
  if [ "$left" -ne 1 ]; then FAIL=$((FAIL + 1)); echo "  FAIL (temporary signature not removed)"; fi
}

verify_tests() {
  env NF_RELEASE_KEY="$W/ed-priv.pem" "$PKG/sign_sums.sh" "$W/rel" >/dev/null 2>&1
  case_run "verify_sums: no arguments" 2 "usage:" "$PKG/verify_sums.sh"
  mkdir -p "$W/unsigned" && printf 'abc123  nf-server.exe\n' >"$W/unsigned/SHA256SUMS"
  case_run "verify_sums: SHA256SUMS.sig missing" 1 "SHA256SUMS.sig not found" "$PKG/verify_sums.sh" "$W/unsigned" "$W/ed-pub.pem"
  case_run "verify_sums: public key file missing" 1 "file not found" "$PKG/verify_sums.sh" "$W/rel" "$W/none.pem"
  case_run "verify_sums: repository placeholder key" 1 "is a PLACEHOLDER" "$PKG/verify_sums.sh" "$W/rel" "$PKG/nf-release-public.pem"
  case_run "verify_sums: private key given as public key" 1 "is a PRIVATE key" "$PKG/verify_sums.sh" "$W/rel" "$W/ed-priv.pem"
  case_run "verify_sums: RSA public key" 1 "is not an Ed25519 public key" "$PKG/verify_sums.sh" "$W/rel" "$W/rsa-pub.pem"
  case_run "verify_sums: not a PEM public key" 1 "is not a PEM public key" "$PKG/verify_sums.sh" "$W/rel" "$W/notakey.pem"
  case_run "verify_sums: good signature" 0 "signature: ok" "$PKG/verify_sums.sh" "$W/rel" "$W/ed-pub.pem"
  cp -r "$W/rel" "$W/tampered"
  sed -i 's/abc123/abc124/' "$W/tampered/SHA256SUMS"
  case_run "verify_sums: one byte of SHA256SUMS changed" 1 "signature: FAILED" "$PKG/verify_sums.sh" "$W/tampered" "$W/ed-pub.pem"
}

sign_tests() {
  mkdir -p "$W/cert" && "$PKG/windows/make_test_cert.sh" "$W/cert" >/dev/null 2>&1
  # Signing setup checks (sign.sh --preflight path through the real signing call).
  case_run "sign.sh: incomplete setup (timestamp set, no certificate)" 1 "incomplete signing setup" env -u NF_SIGN_PFX -u NF_SIGN_PKCS11_MODULE NF_SIGN_TS=http://x.invalid "$PKG/windows/sign.sh" --preflight
  case_run "sign.sh: NF_SIGN_PFX file not found" 1 "NF_SIGN_PFX file not found" env NF_SIGN_PFX="$W/none.pfx" "$PKG/windows/sign.sh" --preflight
  case_run "sign.sh: NF_SIGN_REQUIRED=1 without signing" 1 "NF_SIGN_REQUIRED=1" env -u NF_SIGN_PFX -u NF_SIGN_PKCS11_MODULE NF_SIGN_REQUIRED=1 "$PKG/windows/sign.sh" --preflight
  printf 'MZ-not-really' >"$W/prog.exe"
  printf 'plain text\n' >"$W/notexe.exe"
  case_run "sign.sh: no signing environment" 1 "no signing environment set" env -u NF_SIGN_PFX -u NF_SIGN_PKCS11_MODULE "$PKG/windows/sign.sh" "$W/prog.exe"
  case_run "sign.sh: file is not a Windows program" 1 "not a Windows program" env -u NF_SIGN_PKCS11_MODULE NF_SIGN_PFX="$W/cert/test-signing.pfx" NF_SIGN_PASS_FILE="$W/cert/test-signing.pass" "$PKG/windows/sign.sh" "$W/notexe.exe"
  mkdir -p "$W/fakebin"
  cat >"$W/fakebin/osslsigncode" <<'FAKE'
#!/usr/bin/env bash
# Test double for osslsigncode: fails with a message (fail mode) or writes partial output and waits (slow mode).
out=""; while [ $# -gt 0 ]; do if [ "$1" = "-out" ]; then out=$2; fi; shift; done
if [ "${FAKE_MODE:-fail}" = slow ]; then printf 'partial' >"$out"; sleep 3; printf 'complete' >"$out"; exit 0; fi
echo "error: certificate chain could not be verified" >&2; exit 1
FAKE
  chmod +x "$W/fakebin/osslsigncode"
  local sign_env=(env PATH="$W/fakebin:$PATH" NF_SIGN_PFX="$W/cert/test-signing.pfx" NF_SIGN_PASS_FILE="$W/cert/test-signing.pass")
  case_run "sign.sh: osslsigncode fails (message shown)" 1 "signing failed for prog.exe (osslsigncode exit 1): error: certificate chain" "${sign_env[@]}" FAKE_MODE=fail "$PKG/windows/sign.sh" "$W/prog.exe"
  cp "$W/prog.exe" "$W/prog-before.exe"
  # Interrupt: the trap must remove the partial output, leave the file unchanged and exit 130.
  env --default-signal=INT "${sign_env[@]}" FAKE_MODE=slow "$PKG/windows/sign.sh" "$W/prog.exe" >"$W/int.log" 2>&1 &
  local pid=$! rc=0
  sleep 1
  kill -INT "$pid" 2>/dev/null || true
  wait "$pid" || rc=$?
  echo "CASE sign.sh: SIGINT during signing (exit $rc, want 130)"
  sed 's/^/  | /' "$W/int.log"
  local left; left=$(find "$W" -maxdepth 1 -name 'prog.exe.signing.*' | wc -l)
  if [ "$rc" -eq 130 ] && [ "$left" -eq 0 ] && cmp -s "$W/prog.exe" "$W/prog-before.exe" && grep -qF 'interrupted' "$W/int.log"; then
    PASS=$((PASS + 1)); echo "  leftover partial files: $left; file unchanged: yes"; echo "  PASS"
  else
    FAIL=$((FAIL + 1)); echo "  leftover partial files: $left"; echo "  FAIL"
  fi
}

cert_tests() {
  case_run "make_test_cert: no arguments" 2 "usage:" "$PKG/windows/make_test_cert.sh"
  case_run "make_test_cert: folder missing" 2 "usage:" "$PKG/windows/make_test_cert.sh" "$W/no-folder"
  mkdir -p "$W/certfail" "$W/fakessl"
  cat >"$W/fakessl/openssl" <<FAKE
#!/usr/bin/env bash
# Test double: forces the PKCS#12 step to fail, everything else is the real openssl.
case " \$* " in *" pkcs12 "*) exit 1 ;; esac
exec "$(command -v openssl)" "\$@"
FAKE
  chmod +x "$W/fakessl/openssl"
  case_run "make_test_cert: pkcs12 step fails (message, outputs removed)" 1 "openssl pkcs12 failed" env PATH="$W/fakessl:$PATH" "$PKG/windows/make_test_cert.sh" "$W/certfail"
  local left; left=$(find "$W/certfail" -mindepth 1 | wc -l)
  echo "  files left in folder after failure: $left"
  if [ "$left" -ne 0 ]; then FAIL=$((FAIL + 1)); echo "  FAIL (cleanup)"; fi
  mkdir -p "$W/certok"
  case_run "make_test_cert: good run" 0 "test certificate written" "$PKG/windows/make_test_cert.sh" "$W/certok"
  left=$(find "$W/certok" -name 'test-signing.key' | wc -l)
  echo "  private key left in folder after success: $left"
  if [ "$left" -ne 0 ]; then FAIL=$((FAIL + 1)); echo "  FAIL (key not removed)"; fi
}

mkkeys
case "$GROUP" in
  sums) sums_tests ;;
  verify) verify_tests ;;
  sign) sign_tests ;;
  cert) cert_tests ;;
  all) sums_tests; verify_tests; sign_tests; cert_tests ;;
  *) echo "usage: $0 [sums|verify|sign|cert|all]" >&2; exit 2 ;;
esac
echo "RESULT ($GROUP): $PASS passed, $FAIL failed"
[ "$FAIL" -eq 0 ]
