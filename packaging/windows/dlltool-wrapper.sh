#!/bin/sh
# Called by rustc (-Cdlltool, set by packaging/reproducible.sh) when it builds the import library of a
# raw-dylib import (windows-sys, getrandom) for x86_64-pc-windows-gnu. Task 4.1, reproducible builds.
#
# rustc runs dlltool inside a temporary folder with a random name (target/.../deps/rustcXXXXXX) and names the
# output library by its absolute path. dlltool derives its symbol names (_head_<path>, <path>_iname) from that
# path, and the linker orders the import tables by those names, so the programs differed with the build folder
# and between any two builds. This wrapper gives dlltool a name derived only from the content of the .def
# file (which lists the DLL and its functions) and moves the result to the path rustc asked for.
# Arguments other than -l are passed on unchanged. The real tool: $NF_DLLTOOL, else
# x86_64-w64-mingw32-dlltool, else dlltool.
# Development build. Not validated. Outputs must not be used for manufacturing.
set -eu

real=${NF_DLLTOOL:-}
if [ -z "$real" ]; then
  if command -v x86_64-w64-mingw32-dlltool >/dev/null 2>&1; then real=x86_64-w64-mingw32-dlltool; else real=dlltool; fi
fi

def="" out=""
prev=""
for a in "$@"; do
  case "$prev" in
    -d) def=$a ;;
    -l) out=$a ;;
  esac
  prev=$a
done
if [ -z "$def" ] || [ -z "$out" ] || [ ! -f "$def" ]; then
  exec "$real" "$@"
fi

hash=$(sha256sum "$def" | cut -c1-16)
base=$(basename "$out" .lib | tr -c 'A-Za-z0-9_\n' '_')
local_name="${base}_${hash}.lib"

# Rebuild the argument list with the content-derived output name.
n=$#
prev=""
for a in "$@"; do
  if [ "$prev" = "-l" ]; then
    set -- "$@" "$local_name"
  else
    set -- "$@" "$a"
  fi
  prev=$a
done
shift "$n"

"$real" "$@"
if [ "$local_name" != "$out" ]; then
  mv -f "$local_name" "$out"
fi
