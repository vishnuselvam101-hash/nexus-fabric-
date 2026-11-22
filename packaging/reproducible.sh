# Shared by packaging/build_all.sh, packaging/linux/build.sh and packaging/windows/build.sh (sourced, not run).
# Task 4.1, reproducible builds: two builds of one commit, from any folder, give the same SHA-256 values.
# What is fixed here, and why each one mattered:
#   - every time stored in a package (tar entry times, zip entry times, NSIS file times) is SOURCE_DATE_EPOCH,
#     the commit time, instead of the time the build ran;
#   - entries are stored in a sorted order instead of the order the file system returns them;
#   - owner and permissions are fixed instead of taken from the builder's account and umask;
#   - gzip stores no file name and no time; zip stores no extra fields (uid/gid, extended times);
#   - the compiled programs carry no folder path of the build machine (--remap-path-prefix for Rust,
#     -ffile-prefix-map for the bundled SQLite C code) and no link time (PE time stamp).
# Development build. Not validated. Outputs must not be used for manufacturing.

# Sets and exports SOURCE_DATE_EPOCH (seconds since 1970, UTC) for the build in folder $1, if it is not set:
#   1. SOURCE_DATE_EPOCH from the environment (packaging/verify_release.sh sets it from the commit);
#   2. the commit time of HEAD (git log -1 --format=%ct), when $1 is a git checkout;
#   3. otherwise the modification time of $1/Cargo.toml, with a warning: a copy made by `git archive` or a
#      downloaded source archive has the commit time there, but any other copy may not.
nf_source_date_epoch() {
  local root=$1 t=""
  if [ -n "${SOURCE_DATE_EPOCH:-}" ]; then
    case "$SOURCE_DATE_EPOCH" in
      *[!0-9]*) echo "SOURCE_DATE_EPOCH must be a whole number of seconds, got: $SOURCE_DATE_EPOCH" >&2; return 1 ;;
    esac
    export SOURCE_DATE_EPOCH
    return 0
  fi
  if command -v git >/dev/null 2>&1 && git -C "$root" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    t=$(git -C "$root" log -1 --format=%ct 2>/dev/null || true)
  fi
  if [ -z "$t" ]; then
    t=$(date -r "$root/Cargo.toml" +%s 2>/dev/null || stat -c %Y "$root/Cargo.toml" 2>/dev/null || true)
    echo "warning: not a git checkout and SOURCE_DATE_EPOCH is not set; using the time of Cargo.toml ($t)." >&2
    echo "warning: the packages are reproducible only if SOURCE_DATE_EPOCH is set to the commit time." >&2
  fi
  case "$t" in
    ''|*[!0-9]*) echo "cannot determine SOURCE_DATE_EPOCH (set it to the commit time)" >&2; return 1 ;;
  esac
  SOURCE_DATE_EPOCH=$t
  export SOURCE_DATE_EPOCH
}

# Normalises a staged package folder $1 before it is archived: owner-independent permissions
# (folders and the files named in $2... 755, every other file 644) and every time set to SOURCE_DATE_EPOCH.
nf_normalise_tree() {
  local dir=$1 f
  shift
  find "$dir" -type d -exec chmod 755 {} +
  find "$dir" -type f -exec chmod 644 {} +
  for f in "$@"; do
    chmod 755 "$dir/$f"
  done
  # TZ=UTC: `touch -d @N` is absolute, but zip stores local (DOS) time, so every step uses UTC.
  find "$dir" -exec env TZ=UTC touch -h -d "@$SOURCE_DATE_EPOCH" {} +
}

# Converts a path for the native tools: a Windows path under Git Bash / MSYS, unchanged elsewhere.
nf_native_path() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -w "$1"; else printf '%s' "$1"; fi
}

# Prints the RUSTFLAGS for a reproducible build of the workspace in $1 for the Rust target triple $2.
# The remapped names are fixed words, not paths, so the same flags work in Git Bash (no MSYS path conversion).
# Any RUSTFLAGS already set are kept, after these (the checksum then also depends on them).
nf_rustflags() {
  local root=$1 triple=$2 cargo_home flags target_dir
  cargo_home=${CARGO_HOME:-$HOME/.cargo}
  # The more general prefix first: when several prefixes match, rustc applies the last one given.
  flags="--remap-path-prefix=$(nf_native_path "$cargo_home")=cargo-home"
  flags="$flags --remap-path-prefix=$(nf_native_path "$root")=nexus-fabric"
  # The target folder (generated sources of build scripts) when it lies elsewhere: CARGO_TARGET_DIR, or a
  # target/ that is a link to a shared cache (packaging/verify_release.sh does that).
  target_dir=${CARGO_TARGET_DIR:-$root/target}
  if [ -d "$target_dir" ]; then
    target_dir=$(cd "$target_dir" && pwd -P)
    case "$target_dir" in
      "$root"/*) ;;
      *) flags="$flags --remap-path-prefix=$(nf_native_path "$target_dir")=nexus-fabric-target" ;;
    esac
  fi
  case "$triple" in
    *-windows-gnu*)
      # GNU ld writes the link time into the PE header unless told not to.
      flags="$flags -Clink-arg=-Wl,--no-insert-timestamp" ;;
    *-windows-msvc*)
      # link.exe: -Brepro replaces the time stamp and the PDB signature by a hash of the output;
      # -PDBALTPATH:%_PDB% records only the PDB file name, not the build folder. (Dash form: no MSYS path conversion.)
      flags="$flags -Clink-arg=-Brepro -Clink-arg=-PDBALTPATH:%_PDB%" ;;
  esac
  if [ -n "${RUSTFLAGS:-}" ]; then flags="$flags $RUSTFLAGS"; fi
  printf '%s' "$flags"
}

# Prints the CFLAGS for C code compiled by build scripts (the bundled SQLite of rusqlite): no build-folder paths.
nf_cflags() {
  local root=$1 cargo_home flags target_dir
  cargo_home=${CARGO_HOME:-$HOME/.cargo}
  flags="-ffile-prefix-map=$(nf_native_path "$cargo_home")=cargo-home -ffile-prefix-map=$(nf_native_path "$root")=nexus-fabric"
  target_dir=${CARGO_TARGET_DIR:-$root/target}
  if [ -d "$target_dir" ]; then
    target_dir=$(cd "$target_dir" && pwd -P)
    case "$target_dir" in
      "$root"/*) ;;
      *) flags="$flags -ffile-prefix-map=$(nf_native_path "$target_dir")=nexus-fabric-target" ;;
    esac
  fi
  if [ -n "${CFLAGS:-}" ]; then flags="$flags $CFLAGS"; fi
  printf '%s' "$flags"
}
