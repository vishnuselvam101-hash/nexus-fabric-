# shellcheck shell=bash
# Sourced by packaging/sign_sums.sh and packaging/verify_sums.sh. Checks that a file is the NEXUS FABRIC release PUBLIC key:
# an Ed25519 SubjectPublicKeyInfo PEM. Returns 1 with one line on stderr for: a missing file, the repository placeholder,
# a PRIVATE key (OpenSSL also accepts one with -pubin, so this is tested first), a file that is not a PEM public key, a
# file that does not parse, and a key of another algorithm. On success prints the SHA-256 fingerprint (DER form), returns 0.
# Development build. Not validated. Outputs must not be used for manufacturing.
nf_pubkey_check() {
  local f=$1 out fp
  if [ -z "$f" ] || [ ! -f "$f" ]; then
    echo "public key check failed: file not found: ${f:-(none given)}" >&2; return 1
  fi
  if grep -q 'PLACEHOLDER' -- "$f"; then
    echo "public key check failed: $f is a PLACEHOLDER, not a key (the release key has not been issued yet)" >&2; return 1
  fi
  if grep -q 'PRIVATE KEY' -- "$f"; then
    echo "public key check failed: $f is a PRIVATE key; only the public key (nf-release-public.pem) may be used here" >&2; return 1
  fi
  if ! grep -q '^-----BEGIN PUBLIC KEY-----$' -- "$f"; then
    echo "public key check failed: $f is not a PEM public key (no BEGIN PUBLIC KEY line)" >&2; return 1
  fi
  if ! out=$(openssl pkey -pubin -in "$f" -noout -text 2>/dev/null); then
    echo "public key check failed: $f does not parse as a public key" >&2; return 1
  fi
  case "$out" in
    "ED25519 Public-Key:"*) ;;
    *) echo "public key check failed: $f is not an Ed25519 public key" >&2; return 1 ;;
  esac
  if ! fp=$(openssl pkey -pubin -in "$f" -outform DER 2>/dev/null | sha256sum); then
    echo "public key check failed: cannot compute the fingerprint of $f" >&2; return 1
  fi
  fp=${fp%% *}
  echo "Ed25519 public key fingerprint (SHA-256 of the DER public key): $fp"
}
