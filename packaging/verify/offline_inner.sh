#!/usr/bin/env bash
# Runs inside a new network namespace (started by: unshare -n bash offline_inner.sh SRC LAUNCHER PORT).
# Brings up loopback only, shows which interfaces exist, checks that the internet is unreachable,
# then runs the desktop-mode browser test against the shipped launcher. Native Linux.
# Development build. Not validated. Outputs must not be used for manufacturing.
SRC=$1
LAUNCHER=$2
PORT=$3

python3 - <<'PY' || { echo "loopback: could not bring up lo"; exit 9; }
import socket, fcntl, struct
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
flags = struct.unpack("16sH", fcntl.ioctl(s, 0x8913, struct.pack("16sH", b"lo", 0) + b"\0" * 14)[:18])[1]
fcntl.ioctl(s, 0x8914, struct.pack("16sH", b"lo", flags | 1) + b"\0" * 14)
PY

echo "interfaces: $(awk -F: 'NR>2{gsub(/ /,"",$1); printf "%s ", $1}' /proc/net/dev)"
if curl -s -m 5 -o /dev/null https://example.com; then
  echo "NET reachable: the internet answered, so this run does not prove offline use"
else
  echo "NET none: curl https://example.com failed (no route out of the namespace)"
fi

cd "$SRC" || exit 1
NODE_PATH="$NF_NODE_PATH" node tests/release_verify.js "$LAUNCHER" "$PORT"
