#!/bin/sh
# Linker for the cross-built Windows programs (x86_64-pc-windows-gnu), set by packaging/build_all.sh.
# Task 4.1, reproducible builds. It runs the real linker ($NF_REAL_LINKER, default x86_64-w64-mingw32-gcc)
# with the build's target folder named "nf-target" instead of its absolute path.
#
# Why: GNU ld lays out the import tables with KEEP (SORT(*)(.idata$N)), which sorts the input files by name,
# archive path first. The import libraries of raw-dylib imports sit inside the rlibs in the target folder
# (windows-sys, getrandom) and inside the Rust standard library (in the toolchain folder), so the order of
# the DLLs in the import directory depended on whether the target folder's path sorted before or after the
# toolchain's (seen: /home/... gave kernel32.dll first, /tmp/... gave it third). With the target folder
# reached through the relative name nf-target/..., the order no longer depends on where the build ran.
# The link runs in a temporary folder that holds only the link nf-target -> the target folder.
# NF_LINK_TARGET_DIR: the target folder (set by packaging/build_all.sh). Without it, the linker runs unchanged.
# Development build. Not validated. Outputs must not be used for manufacturing.
set -eu

real=${NF_REAL_LINKER:-x86_64-w64-mingw32-gcc}
tdir=${NF_LINK_TARGET_DIR:-}
if [ -z "$tdir" ] || [ ! -d "$tdir" ]; then
  exec "$real" "$@"
fi
logical=$(cd "$tdir" && pwd)
physical=$(cd "$tdir" && pwd -P)

work=$(mktemp -d "${TMPDIR:-/tmp}/nf-link.XXXXXX")
trap 'rm -rf "$work"' EXIT
ln -s "$physical" "$work/nf-target"

# Rewrites one argument: an absolute path under the target folder, alone or after -L / -o, becomes nf-target/...
rewrite() {
  case "$1" in
    "$logical"/*) printf '%s' "nf-target/${1#"$logical"/}" ;;
    "$physical"/*) printf '%s' "nf-target/${1#"$physical"/}" ;;
    -L"$logical"/*) printf '%s' "-Lnf-target/${1#-L"$logical"/}" ;;
    -L"$physical"/*) printf '%s' "-Lnf-target/${1#-L"$physical"/}" ;;
    *) printf '%s' "$1" ;;
  esac
}

n=$#
for a in "$@"; do
  case "$a" in
    @*)
      # A response file (rustc uses one for a very long command line): one argument per line, rewritten alike.
      src=${a#@}
      dst="$work/args.$n"
      : >"$dst"
      while IFS= read -r line || [ -n "$line" ]; do
        printf '%s\n' "$(rewrite "$line")" >>"$dst"
      done <"$src"
      set -- "$@" "@$dst"
      ;;
    *)
      set -- "$@" "$(rewrite "$a")"
      ;;
  esac
done
shift "$n"

cd "$work"
"$real" "$@"
