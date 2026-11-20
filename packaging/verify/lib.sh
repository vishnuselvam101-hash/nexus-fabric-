#!/usr/bin/env bash
# Helpers for packaging/verify_release.sh. Sourced, not run directly.
# Development build. Not validated. Outputs must not be used for manufacturing.

RESULTS=""          # TSV: id, title, how it ran, status, reason, evidence, log file
VERSIONS=""         # TSV: label, value, how it ran, agrees, note
FACTS=""            # TSV: key, value
NEXT_PORT=""
USED_PORTS=""
OWNED_PIDS=()       # processes this run started (and only these are ever stopped)
WINE_PREFIXES=()
WORK_DIRS=()
WINE_VER="not run"

say() { printf '%s\n' "$*" >&2; }

# rec ID TITLE HOW STATUS REASON EVIDENCE LOGFILE
# STATUS is exactly one of: passed, failed, not run. HOW says how it ran.
rec() {
  local ev=${6//$'\t'/ }
  ev=${ev//$'\n'/ }
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" "$5" "$ev" "$(basename "${7:-none}")" >>"$RESULTS"
  say "[$4] $1: $2${5:+ -- $5}"
}

# vrow LABEL VALUE HOW AGREES NOTE
vrow() {
  local v=${2//$'\t'/ }; v=${v//$'\n'/ }
  printf '%s\t%s\t%s\t%s\t%s\n' "$1" "$v" "$3" "$4" "$5" >>"$VERSIONS"
}

fact() { local v=${2//$'\t'/ }; v=${v//$'\n'/ }; printf '%s\t%s\n' "$1" "$v" >>"$FACTS"; }

# first_line FILE REGEX: the first matching line, at most 300 characters.
first_line() { grep -m1 -E -- "$2" "$1" 2>/dev/null | tr -d '\r' | cut -c1-300; }

# agrees LINE VERSION: yes if VERSION appears as a whole version token in LINE.
agrees() {
  local esc=${2//./\\.}
  if printf '%s\n' "$1" | grep -qE "(^|[^0-9.])${esc}([^0-9.]|\$)"; then echo yes; else echo no; fi
}

# pick_port: sets NEXT_PORT to a free port in 8870-8879. No other port is ever used.
pick_port() {
  local p
  for p in 8870 8871 8872 8873 8874 8875 8876 8877 8878 8879; do
    case " $USED_PORTS " in *" $p "*) continue ;; esac
    if python3 -c "import socket; s = socket.socket(); s.bind(('127.0.0.1', $p))" 2>/dev/null; then
      USED_PORTS="$USED_PORTS $p"
      NEXT_PORT=$p
      return 0
    fi
  done
  NEXT_PORT=""
  return 1
}

# wait_pid PID SECONDS: waits for a process this shell started; SIGKILL after SECONDS. Sets RC_WAIT.
wait_pid() {
  local pid=$1 secs=$2 killer
  ( sleep "$secs"; kill -KILL "$pid" 2>/dev/null ) &
  killer=$!
  wait "$pid" 2>/dev/null
  RC_WAIT=$?
  kill "$killer" 2>/dev/null
  wait "$killer" 2>/dev/null
  return 0
}

# graceful_stop PID: SIGINT (Ctrl+C), then wait up to 60 s. Sets RC_WAIT (exit status).
graceful_stop() {
  kill -INT "$1" 2>/dev/null
  wait_pid "$1" 60
}

# wine_env PREFIX COMMAND...: runs COMMAND with a clean environment and this prefix.
# A clean environment is required: the inherited CCR_PRELOAD_CLAUDE preload makes wine abort
# with "free(): invalid pointer" during wineboot.
wine_env() {
  local prefix=$1
  shift
  env -i PATH="$PATH" HOME="$WINE_HOME" TMPDIR="$TMPDIR" LANG=C.UTF-8 \
    WINEARCH=win64 WINEDEBUG=-all WINEPREFIX="$prefix" WINEDLLOVERRIDES='mscoree,mshtml=d' "$@"
}

# wine_user_dir PREFIX: the profile folder of the wine user (the one folder under users/ that is not Public).
wine_user_dir() {
  local d
  for d in "$1"/drive_c/users/*; do
    [ -d "$d" ] || continue
    case "$(basename "$d")" in Public) ;; *) echo "$d"; return 0 ;; esac
  done
  return 1
}

cleanup() {
  local p pid d
  for p in ${WINE_PREFIXES[@]+"${WINE_PREFIXES[@]}"}; do
    [ -d "$p" ] && wine_env "$p" timeout 60 wineserver -k >/dev/null 2>&1
  done
  for pid in ${OWNED_PIDS[@]+"${OWNED_PIDS[@]}"}; do
    kill -KILL "$pid" 2>/dev/null
  done
  for d in ${WORK_DIRS[@]+"${WORK_DIRS[@]}"}; do
    rm -rf "$d"
  done
}
