#!/usr/bin/env bash
# Linux package checks for packaging/verify_release.sh (sourced). Every check here runs natively on Linux.
# Development build. Not validated. Outputs must not be used for manufacturing.

LINUX_ENGINE=""
LINUX_HOW="native Linux"

# summary_result ID TITLE HOW RC LOG: passed only if the test exited 0 and its summary says failed: 0.
summary_result() {
  local sum status ev
  sum=$(grep -E '^passed: [0-9]+ +failed: [0-9]+' "$5" | tail -1)
  if [ "$4" -eq 0 ] && [ -n "$sum" ] && printf '%s' "$sum" | grep -qE 'failed: 0$'; then
    status=passed
    ev="$(first_line "$5" '^PASS  program prints')  |  $sum"
    rec "$1" "$2" "$3" passed "" "$ev" "$(basename "$5")"
  else
    status=failed
    ev="$(first_line "$5" '^FAIL')  |  ${sum:-no summary line}"
    rec "$1" "$2" "$3" failed "exit $4" "$ev" "$(basename "$5")"
  fi
}

# run_linux_checks SRC OUT WORK VERSION PW_PATH
run_linux_checks() {
  local SRC=$1 OUT=$2 WORK=$3 VER=$4 PW_PATH=$5
  local LOGS="$OUT/logs" TAR="$OUT/nexus-fabric-$VER-linux-x86_64.tar.gz"
  local U="$WORK/linux" NAME="nexus-fabric-$VER"
  local BIN="$U/$NAME/nexus-fabric" LAUNCH="$U/$NAME/nexus-fabric.sh"
  local rc line P P2 L3 L4 engine
  mkdir -p "$U"

  # L1: unpack the shipped tar.gz (the copy in the output folder, whose checksum was verified).
  if timeout 120 tar -xzf "$TAR" -C "$U" >"$LOGS/linux-unpack.log" 2>&1 && [ -x "$BIN" ] && [ -x "$LAUNCH" ]; then
    rec linux.unpack "Linux package: unpack tar.gz; program and launcher present and executable" "$LINUX_HOW" passed "" \
      "contents: $(ls "$U/$NAME" | tr '\n' ' ')" linux-unpack.log
  else
    rec linux.unpack "Linux package: unpack tar.gz" "$LINUX_HOW" failed "tar failed or files missing or not executable" \
      "$(tail -1 "$LOGS/linux-unpack.log")" linux-unpack.log
    return 0
  fi

  # L2: --version of the shipped binary (nf-server, packaged as nexus-fabric).
  timeout 60 "$BIN" --version >"$LOGS/linux-version.log" 2>&1
  rc=$?
  line=$(head -1 "$LOGS/linux-version.log" | tr -d '\r')
  engine=$(printf '%s' "$line" | grep -oE 'nf-engine [^ )]+' | head -1)
  LINUX_ENGINE="$engine"
  if [ "$rc" -eq 0 ] && [ "$(agrees "$line" "$VER")" = yes ]; then
    rec linux.version "nexus-fabric --version (Linux binary, nf-server)" "$LINUX_HOW" passed "" "$line" linux-version.log
  else
    rec linux.version "nexus-fabric --version (Linux binary, nf-server)" "$LINUX_HOW" failed "exit $rc or version $VER not printed" "$line" linux-version.log
  fi
  vrow "nf-server --version, Linux binary" "$line" "$LINUX_HOW" "$(agrees "$line" "$VER")" ""

  # L3: desktop mode through the shipped launcher: health, first-run setup via the link, duplicate refused,
  # fragment handling in the browser, graceful Ctrl+C stop. Browser test = tests/release_verify.js.
  L3="$LOGS/linux-desktop-browser.log"
  if [ -z "$PW_PATH" ]; then
    rec linux.desktop_browser "Desktop mode through the launcher, first-run setup in a browser, clean stop" "$LINUX_HOW" "not run" \
      "playwright not found (NODE_PATH); browser check cannot run" "" "$(basename "$L3")"
  elif pick_port; then
    P=$NEXT_PORT
    NODE_PATH="$PW_PATH" timeout 900 node "$SRC/tests/release_verify.js" "$LAUNCH" "$P" >"$L3" 2>&1
    rc=$?
    summary_result linux.desktop_browser "Desktop mode through the launcher, first-run setup in a browser, clean stop (port $P)" "$LINUX_HOW" "$rc" "$L3"
  else
    rec linux.desktop_browser "Desktop mode through the launcher" "$LINUX_HOW" "not run" "no free port in 8870-8879" "" "$(basename "$L3")"
  fi

  # L4: offline first launch: the same test inside a network namespace that has only loopback.
  L4="$LOGS/linux-offline-first-launch.log"
  if [ "$(id -u)" -ne 0 ] || ! command -v unshare >/dev/null 2>&1; then
    rec linux.offline_first_launch "Offline first launch (loopback-only network namespace)" "$LINUX_HOW" "not run" \
      "needs root and unshare" "" "$(basename "$L4")"
  elif [ -z "$PW_PATH" ]; then
    rec linux.offline_first_launch "Offline first launch (loopback-only network namespace)" "$LINUX_HOW" "not run" \
      "playwright not found (NODE_PATH)" "" "$(basename "$L4")"
  elif pick_port; then
    P2=$NEXT_PORT
    NF_NODE_PATH="$PW_PATH" timeout 900 unshare -n bash "$SRC/packaging/verify/offline_inner.sh" "$SRC" "$LAUNCH" "$P2" >"$L4" 2>&1
    rc=$?
    if grep -q '^NET none' "$L4" && grep -q '^interfaces: lo $' "$L4"; then
      summary_result linux.offline_first_launch "Offline first launch: loopback-only namespace, desktop mode, first-run setup, clean stop (port $P2)" "$LINUX_HOW" "$rc" "$L4"
      rec linux.offline_netns "Network namespace has only loopback and no route out" "$LINUX_HOW" passed "" \
        "$(first_line "$L4" '^interfaces:')  |  $(first_line "$L4" '^NET ')" "$(basename "$L4")"
    else
      rec linux.offline_netns "Network namespace has only loopback and no route out" "$LINUX_HOW" failed \
        "namespace not loopback-only, or network reachable" "$(first_line "$L4" '^interfaces:')  |  $(first_line "$L4" '^NET ')" "$(basename "$L4")"
      rec linux.offline_first_launch "Offline first launch (loopback-only network namespace)" "$LINUX_HOW" failed "namespace check failed" "" "$(basename "$L4")"
    fi
  else
    rec linux.offline_first_launch "Offline first launch" "$LINUX_HOW" "not run" "no free port in 8870-8879" "" "$(basename "$L4")"
  fi
}
