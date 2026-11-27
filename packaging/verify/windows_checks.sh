#!/usr/bin/env bash
# Windows package checks for packaging/verify_release.sh (sourced).
# These run under wine, which is an EMULATOR. Every result is labelled "emulated (wine ...)" and none is
# a Windows result. Development build. Not validated. Outputs must not be used for manufacturing.

WIN_PASSWORD="correct-horse-9"     # throw-away password for the verification administrator only
WPID=""
SESSION_JOB=""
WIN_ENGINE=""

win_how() { echo "emulated (wine $WINE_VER; not Windows)"; }

# win_boot PREFIX LOG: a fresh 64-bit prefix. Mono and Gecko are not installed (no network).
win_boot() {
  mkdir -p "$1"
  WINE_PREFIXES+=("$1")
  wine_env "$1" timeout 400 wine wineboot --init >"$2" 2>&1
}

# win_start PREFIX EXE PORT LOG: desktop mode in the background; sets WPID to the WINE PROCESS itself.
# "( exec env ... wine ... )" replaces the subshell with env, and env with wine, so $! is the wine pid and a
# SIGINT sent to it reaches the program. (A plain function call in the background would give the pid of a subshell.)
win_start() {
  wine_env_vars "$1"
  ( exec env -i "${WINE_VARS[@]}" wine "$2" --desktop --port "$3" --no-browser ) >"$4" 2>&1 &
  WPID=$!
  OWNED_PIDS+=("$WPID")
}

# win_ready PORT LOG: up to 120 s for the address line and a health answer.
win_ready() {
  local i
  for i in $(seq 1 120); do
    if grep -q "Address: http://127.0.0.1:$1/" "$2" 2>/dev/null &&
      curl -s -m 5 -H 'X-NF-Request: 1' "http://127.0.0.1:$1/api/health" | grep -q '"status":"ok"'; then
      return 0
    fi
    kill -0 "$WPID" 2>/dev/null || return 1
    sleep 1
  done
  return 1
}

# win_token PORT LOG: the one-time token from the setup link line (only if the link was printed).
win_token() {
  tr -d '\r' <"$2" | sed -n "s|.*Setup link[^:]*: http://127.0.0.1:$1/#setup=\([0-9a-f]\{64\}\).*|\1|p" | head -1
}

# json_field FILE KEY: a top-level field of a JSON object (empty if absent).
json_field() {
  python3 -c 'import json,sys
try:
    print(json.load(open(sys.argv[1]))[sys.argv[2]])
except Exception:
    pass' "$1" "$2"
}

# win_session PREFIX EXE PORT TAG LOGS [REOPEN_CALC_ID]
# Starts the installed program in desktop mode and checks it over HTTP. Without REOPEN_CALC_ID: first-run setup
# with the link token, then a project and a job (calculation). With it: checks that the job is still listed.
# Stops it with Ctrl+C (SIGINT). Records win.<TAG>.* checks.
win_session() {
  local PREFIX=$1 EXE=$2 P=$3 TAG=$4 LOGS=$5 REOPEN=${6:-}
  local HOW BASE L JAR TOKEN CODE PID CALC ok
  HOW=$(win_how)
  BASE="http://127.0.0.1:$P"
  L="$LOGS/win-$TAG-server.log"
  JAR="$WORK/$TAG.jar"
  SESSION_JOB=""

  win_start "$PREFIX" "$EXE" "$P" "$L"
  if ! win_ready "$P" "$L"; then
    rec "win.$TAG.start" "Desktop mode starts and answers /api/health ($TAG)" "$HOW" failed "no answer within 120 s" \
      "$(tr -d '\r' <"$L" | tail -1)" "$(basename "$L")"
    graceful_stop "$WPID"
    return 1
  fi
  rec "win.$TAG.start" "Desktop mode starts on 127.0.0.1 and answers /api/health ($TAG)" "$HOW" passed "" \
    "$(first_line "$L" 'Address: http')" "$(basename "$L")"

  PID=$(curl -s -m 10 -H 'X-NF-Request: 1' "$BASE/api/health")
  if printf '%s' "$PID" | grep -q '"status":"ok"'; then
    rec "win.$TAG.health" "GET /api/health answers status ok" "$HOW" passed "" "$PID" "$(basename "$L")"
  else
    rec "win.$TAG.health" "GET /api/health answers status ok" "$HOW" failed "no ok status" "$PID" "$(basename "$L")"
  fi

  if [ -z "$REOPEN" ]; then
    TOKEN=$(win_token "$P" "$L")
    if printf '%s' "$TOKEN" | grep -qE '^[0-9a-f]{64}$'; then
      rec "win.$TAG.setup_link" "First-run setup link printed with a 64-hex token in the #setup= fragment" "$HOW" passed "" \
        "$(first_line "$L" 'Setup link')" "$(basename "$L")"
    else
      rec "win.$TAG.setup_link" "First-run setup link printed with a 64-hex token in the #setup= fragment" "$HOW" failed "no link with a token" \
        "$(first_line "$L" 'Setup')" "$(basename "$L")"
    fi
    rec "win.$TAG.browser_fragment" "Browser reads and removes the fragment token" "$HOW" "not run" \
      "no browser test under wine; fragment handling is checked by tests/release_verify.js on Linux only" "" "$(basename "$L")"

    CODE=$(curl -s -m 20 -o "$WORK/$TAG-setup.json" -w '%{http_code}' -H 'X-NF-Request: 1' -H "X-NF-Setup-Token: $TOKEN" \
      -H 'Content-Type: application/json' -d "{\"username\":\"admin\",\"password\":\"$WIN_PASSWORD\"}" "$BASE/api/setup")
    if [ "$CODE" = 200 ]; then
      rec "win.$TAG.setup" "First-run setup over HTTP with the link token (POST /api/setup)" "$HOW" passed "" \
        "HTTP $CODE; $(curl -s -m 10 -H 'X-NF-Request: 1' "$BASE/api/health")" "$(basename "$L")"
    else
      rec "win.$TAG.setup" "First-run setup over HTTP with the link token (POST /api/setup)" "$HOW" failed "HTTP $CODE" "HTTP $CODE" "$(basename "$L")"
    fi
  fi

  # Log in and act on the program: project and job, or the job listed after an upgrade.
  curl -s -m 20 -c "$JAR" -o "$WORK/$TAG-login.json" -w '%{http_code}' -H 'X-NF-Request: 1' -H 'Content-Type: application/json' \
    -d "{\"username\":\"admin\",\"password\":\"$WIN_PASSWORD\"}" "$BASE/api/login" >"$WORK/$TAG-login.code"
  if [ "$(cat "$WORK/$TAG-login.code")" = 200 ]; then ok=1; else ok=0; fi
  if [ -z "$REOPEN" ]; then
    local PID_PROJ
    if [ "$ok" = 1 ]; then
      PID_PROJ=$(curl -s -m 20 -b "$JAR" -H 'X-NF-Request: 1' -H 'Content-Type: application/json' \
        -d '{"name":"Release verification","description":"release_verify"}' "$BASE/api/projects" | python3 -c 'import json,sys
try: print(json.load(sys.stdin)["id"])
except Exception: pass')
      CALC=""
      if [ -n "$PID_PROJ" ]; then
        curl -s -m 60 -b "$JAR" -o "$WORK/$TAG-calc.json" -w '%{http_code}' -H 'X-NF-Request: 1' -H 'Content-Type: application/json' \
          -d '{"family":"cylinder_shell","r_dev":500,"length":1000}' "$BASE/api/projects/$PID_PROJ/calculations" >"$WORK/$TAG-calc.code"
        CALC=$(json_field "$WORK/$TAG-calc.json" id)
        if [ "$(cat "$WORK/$TAG-calc.code")" = 200 ] && [ -n "$CALC" ]; then
          SESSION_JOB=$CALC
          rec "win.$TAG.job" "Create a project and a job (calculation) over HTTP" "$HOW" passed "" \
            "project $PID_PROJ; job $CALC state $(json_field "$WORK/$TAG-calc.json" state)" "$(basename "$L")"
        else
          rec "win.$TAG.job" "Create a project and a job (calculation) over HTTP" "$HOW" failed "HTTP $(cat "$WORK/$TAG-calc.code")" \
            "$(head -c 200 "$WORK/$TAG-calc.json")" "$(basename "$L")"
        fi
        # A refused request is also stored as a job (state REFUSED) but writes no output file. It is the job used
        # for the upgrade check when the valid job above could not be stored.
        local RCODE RID RSTATE
        RCODE=$(curl -s -m 60 -b "$JAR" -o "$WORK/$TAG-refused.json" -w '%{http_code}' -H 'X-NF-Request: 1' -H 'Content-Type: application/json' \
          -d '{"family":"cone_frustum","r1":200,"r2":199.99999,"h":300}' "$BASE/api/projects/$PID_PROJ/calculations")
        RID=$(json_field "$WORK/$TAG-refused.json" id)
        RSTATE=$(json_field "$WORK/$TAG-refused.json" state)
        if [ "$RCODE" = 422 ] && [ "$RSTATE" = REFUSED ] && [ -n "$RID" ]; then
          rec "win.$TAG.job_refused" "Create a refused job (stored as REFUSED, no output file) over HTTP" "$HOW" passed "" \
            "HTTP 422; job $RID state $RSTATE" "$(basename "$L")"
          [ -z "$SESSION_JOB" ] && SESSION_JOB=$RID
        else
          rec "win.$TAG.job_refused" "Create a refused job (stored as REFUSED, no output file) over HTTP" "$HOW" failed "HTTP $RCODE state ${RSTATE:-none}" \
            "$(head -c 200 "$WORK/$TAG-refused.json")" "$(basename "$L")"
        fi
      else
        rec "win.$TAG.job" "Create a project and a job (calculation) over HTTP" "$HOW" failed "project not created" "" "$(basename "$L")"
      fi
    else
      rec "win.$TAG.job" "Create a project and a job (calculation) over HTTP" "$HOW" failed "login HTTP $(cat "$WORK/$TAG-login.code")" "" "$(basename "$L")"
    fi
  else
    if [ "$ok" = 1 ] && curl -s -m 20 -b "$JAR" -H 'X-NF-Request: 1' "$BASE/api/calculations" |
      python3 -c 'import json,sys; ids=[c.get("id") for c in json.load(sys.stdin)]; sys.exit(0 if int(sys.argv[1]) in ids else 1)' "$REOPEN"; then
      rec "win.$TAG.job_survived" "Job $REOPEN and its project still listed after the upgrade (login as before)" "$HOW" passed "" \
        "GET /api/calculations lists job $REOPEN" "$(basename "$L")"
    else
      rec "win.$TAG.job_survived" "Job $REOPEN and its project still listed after the upgrade" "$HOW" failed "job not listed or login failed (HTTP $(cat "$WORK/$TAG-login.code"))" "" "$(basename "$L")"
    fi
  fi

  graceful_stop "$WPID"
  local rc=$RC_WAIT
  if [ "$rc" = 0 ] && grep -q 'database is closed' "$L"; then
    rec "win.$TAG.stop" "Graceful stop on SIGINT (wine delivers it as Ctrl+C): exit 0 and database closed" "$HOW" passed "" \
      "$(tr -d '\r' <"$L" | grep 'database is closed' | tail -1) (exit $rc)" "$(basename "$L")"
  else
    rec "win.$TAG.stop" "Graceful stop on SIGINT (wine delivers it as Ctrl+C): exit 0 and database closed" "$HOW" failed "exit $rc" \
      "$(tr -d '\r' <"$L" | tail -1)" "$(basename "$L")"
  fi
  return 0
}

# run_windows_checks SRC OUT WORK VERSION PREVIOUS_INSTALLER_OR_EMPTY
run_windows_checks() {
  local SRC=$1 OUT=$2 WORK=$3 VER=$4 PREV=$5
  local LOGS="$OUT/logs" SETUP="$OUT/nexus-fabric-$VER-windows-x64-setup.exe"
  local ZIP="$OUT/nexus-fabric-$VER-windows-x64-portable.zip"
  local HOW P1 U INST SM DESK DATA L rc line i P2 PDIR reg
  WINE_HOME="$WORK/home"
  mkdir -p "$WINE_HOME"
  WINE_VER=$(wine_env "$WORK/wp-version" timeout 60 wine --version 2>/dev/null | head -1)
  [ -n "$WINE_VER" ] || WINE_VER="unavailable"
  WINE_VER=${WINE_VER#wine-}
  HOW=$(win_how)
  vrow "wine (Windows emulator used here)" "wine-$WINE_VER" "tool" static "not Windows"

  P1="$WORK/wp-install"
  win_boot "$P1" "$LOGS/win-boot.log"
  rc=$?
  if [ "$rc" != 0 ] || ! U=$(wine_user_dir "$P1"); then
    # wine could not initialise: every wine-dependent check is recorded as not run, with the reason. Nothing is claimed.
    local WHY="wine could not initialise (wineboot exit $rc)"
    rec win.prefix "Fresh 64-bit wine prefix (wineboot --init)" "$HOW" "not run" "$WHY" "$(tail -1 "$LOGS/win-boot.log")" win-boot.log
    for i in win.install.exit win.install.files win.install.shortcut win.install.desktop_shortcut win.install.registry \
      win.install.version win.install.start win.install.health win.install.setup_link win.install.setup \
      win.install.job win.install.job_refused win.install.stop win.portable.version \
      win.uninstall.program win.uninstall.shortcuts win.uninstall.registry win.uninstall.data_kept win.upgrade; do
      rec "$i" "Windows check (needs wine): $i" "$HOW" "not run" "$WHY" "" win-boot.log
    done
    return 0
  fi
  rec win.prefix "Fresh 64-bit wine prefix (wineboot --init)" "$HOW" passed "" "wineboot exit 0" win-boot.log
  INST="$U/AppData/Local/Programs/NexusFabric"
  SM="$U/AppData/Roaming/Microsoft/Windows/Start Menu/Programs/NEXUS FABRIC"
  DESK="$U/Desktop"
  DATA="$U/AppData/Local/NexusFabric/data"

  # Silent install into the per-user folder (no administrator rights).
  L="$LOGS/win-install.log"
  wine_env "$P1" timeout 400 wine "$SETUP" /S >"$L" 2>&1
  rc=$?
  if [ "$rc" = 0 ]; then
    rec win.install.exit "Silent install (setup.exe /S) exits 0" "$HOW" passed "" "exit 0" win-install.log
  else
    rec win.install.exit "Silent install (setup.exe /S) exits 0" "$HOW" failed "exit $rc" "exit $rc" win-install.log
  fi
  local missing=""
  for i in nexus-fabric.exe README.txt NOTICE.txt THIRD_PARTY_NOTICES.txt uninstall.exe; do [ -f "$INST/$i" ] || missing="$missing $i"; done
  if [ -z "$missing" ]; then
    rec win.install.files "Installed files present in the per-user folder" "$HOW" passed "" "$(ls "$INST" | tr '\n' ' ')" win-install.log
  else
    rec win.install.files "Installed files present in the per-user folder" "$HOW" failed "missing:$missing" "$(ls "$INST" 2>/dev/null | tr '\n' ' ')" win-install.log
  fi
  if [ -f "$SM/NEXUS FABRIC.lnk" ] && [ -f "$SM/Uninstall NEXUS FABRIC.lnk" ]; then
    rec win.install.shortcut "Start-menu shortcuts present" "$HOW" passed "" "$(ls "$SM" | tr '\n' ' ')" win-install.log
  else
    rec win.install.shortcut "Start-menu shortcuts present" "$HOW" failed "shortcut missing" "$(ls "$SM" 2>/dev/null | tr '\n' ' ')" win-install.log
  fi
  if [ -f "$DESK/NEXUS FABRIC.lnk" ]; then
    rec win.install.desktop_shortcut "Desktop shortcut present (optional component, ticked by default)" "$HOW" passed "" "present" win-install.log
  else
    rec win.install.desktop_shortcut "Desktop shortcut present (optional component, ticked by default)" "$HOW" failed "absent" "absent" win-install.log
  fi
  reg=$(wine_env "$P1" timeout 60 wine reg query 'HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NexusFabric' /v DisplayVersion 2>&1 | tr -d '\r')
  if printf '%s' "$reg" | grep -q 'DisplayVersion'; then
    rec win.install.registry "Uninstall entry present (HKCU, DisplayVersion)" "$HOW" passed "" \
      "$(printf '%s' "$reg" | grep DisplayVersion | head -1)" win-install.log
  else
    rec win.install.registry "Uninstall entry present (HKCU, DisplayVersion)" "$HOW" failed "key not found" "$(printf '%s' "$reg" | tail -1)" win-install.log
  fi
  line=$(wine_env "$P1" timeout 60 wine "$INST/nexus-fabric.exe" --version 2>/dev/null | tr -d '\r' | head -1)
  WIN_ENGINE=$(printf '%s' "$line" | grep -oE 'nf-engine [^ )]+' | head -1)
  if [ "$(agrees "$line" "$VER")" = yes ]; then
    rec win.install.version "Installed exe --version prints $VER" "$HOW" passed "" "$line" win-install.log
  else
    rec win.install.version "Installed exe --version prints $VER" "$HOW" failed "version $VER not printed" "$line" win-install.log
  fi
  vrow "nf-server --version, Windows exe (installed copy, under wine)" "$line" "$HOW" "$(agrees "$line" "$VER")" ""

  # The program in desktop mode, over HTTP, from the installed copy.
  if pick_port; then
    P2=$NEXT_PORT
    win_session "$P1" "$INST/nexus-fabric.exe" "$P2" install "$LOGS" ""
  else
    rec win.install.session "Desktop mode session" "$HOW" "not run" "no free port in 8870-8879" "" ""
  fi
  local JOB_INSTALL=$SESSION_JOB

  # Portable zip: unpack and --version (no installer, no data folder).
  PDIR="$WORK/portable"
  mkdir -p "$PDIR"
  if python3 -m zipfile -e "$ZIP" "$PDIR" >"$LOGS/win-portable-unzip.log" 2>&1 && [ -f "$PDIR/nexus-fabric-$VER/nexus-fabric.exe" ]; then
    rec win.portable.unzip "Portable zip unpacks: exe, README, notice" "static" passed "" \
      "$(ls "$PDIR/nexus-fabric-$VER" | tr '\n' ' ')" win-portable-unzip.log
  else
    rec win.portable.unzip "Portable zip unpacks: exe, README, notice" "static" failed "unzip failed or exe missing" "" win-portable-unzip.log
  fi
  line=$(wine_env "$P1" timeout 60 wine "$PDIR/nexus-fabric-$VER/nexus-fabric.exe" --version 2>/dev/null | tr -d '\r' | head -1)
  if [ "$(agrees "$line" "$VER")" = yes ]; then
    rec win.portable.version "Portable exe --version prints $VER" "$HOW" passed "" "$line" win-install.log
  else
    rec win.portable.version "Portable exe --version prints $VER" "$HOW" failed "version $VER not printed" "$line" win-install.log
  fi
  vrow "nf-server --version, Windows exe (portable zip, under wine)" "$line" "$HOW" "$(agrees "$line" "$VER")" ""

  # Silent uninstall: program and shortcuts and the registry entry go; the data folder stays.
  L="$LOGS/win-uninstall.log"
  wine_env "$P1" timeout 300 wine "$INST/uninstall.exe" /S >"$L" 2>&1
  rc=$?
  for i in $(seq 1 120); do
    [ -e "$INST/nexus-fabric.exe" ] || [ -e "$INST/uninstall.exe" ] || break
    sleep 1
  done
  if [ ! -e "$INST/nexus-fabric.exe" ] && [ ! -e "$INST/uninstall.exe" ] && [ ! -e "$INST/README.txt" ] && [ ! -d "$INST" ]; then
    rec win.uninstall.program "Silent uninstall (uninstall.exe /S, exit $rc) removes the program folder" "$HOW" passed "" "program folder gone" win-uninstall.log
  else
    rec win.uninstall.program "Silent uninstall (uninstall.exe /S, exit $rc) removes the program folder" "$HOW" failed "files remain: $(ls "$INST" 2>/dev/null | tr '\n' ' ')" "" win-uninstall.log
  fi
  if [ ! -e "$SM/NEXUS FABRIC.lnk" ] && [ ! -e "$SM/Uninstall NEXUS FABRIC.lnk" ] && [ ! -e "$DESK/NEXUS FABRIC.lnk" ]; then
    rec win.uninstall.shortcuts "Start-menu and desktop shortcuts removed" "$HOW" passed "" "none remain" win-uninstall.log
  else
    rec win.uninstall.shortcuts "Start-menu and desktop shortcuts removed" "$HOW" failed "a shortcut remains" "" win-uninstall.log
  fi
  reg=$(wine_env "$P1" timeout 60 wine reg query 'HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\NexusFabric' 2>&1)
  if [ $? -ne 0 ]; then
    rec win.uninstall.registry "Uninstall registry entry removed" "$HOW" passed "" "reg query: key not found" win-uninstall.log
  else
    rec win.uninstall.registry "Uninstall registry entry removed" "$HOW" failed "key still present" "$(printf '%s' "$reg" | tail -1)" win-uninstall.log
  fi
  if [ -f "$DATA/nexus-fabric.sqlite" ]; then
    rec win.uninstall.data_kept "Data folder kept after uninstall (database file present)" "$HOW" passed "" \
      "$(ls -la "$DATA" | awk 'NR>1{print $NF}' | tr '\n' ' ')" win-uninstall.log
  else
    rec win.uninstall.data_kept "Data folder kept after uninstall (database file present)" "$HOW" failed "database file missing" "" win-uninstall.log
  fi

  # Upgrade: previous installer first, a job created with it, the new installer installed over it.
  if [ -z "$PREV" ]; then
    rec win.upgrade "Upgrade: previous installer, job, install new over it" "$HOW" "not run" "no --previous installer given" "" ""
    return 0
  fi
  if [ ! -f "$PREV" ]; then
    rec win.upgrade "Upgrade: previous installer, job, install new over it" "$HOW" "not run" "--previous file not found" "" ""
    return 0
  fi
  local P3="$WORK/wp-upgrade" U3 INST3 DATA3 PREV_LINE NEW_LINE JOB
  win_boot "$P3" "$LOGS/win-upgrade-boot.log" || true
  if ! U3=$(wine_user_dir "$P3"); then
    rec win.upgrade "Upgrade: previous installer, job, install new over it" "$HOW" failed "prefix not created" "" win-upgrade-boot.log
    return 0
  fi
  INST3="$U3/AppData/Local/Programs/NexusFabric"
  DATA3="$U3/AppData/Local/NexusFabric/data"
  L="$LOGS/win-upgrade-install-previous.log"
  wine_env "$P3" timeout 400 wine "$PREV" /S >"$L" 2>&1
  rc=$?
  if [ "$rc" = 0 ] && [ -f "$INST3/nexus-fabric.exe" ]; then
    rec win.upgrade.install_previous "Previous installer installs silently" "$HOW" passed "" "exit 0; $(basename "$PREV")" win-upgrade-install-previous.log
  else
    rec win.upgrade.install_previous "Previous installer installs silently" "$HOW" failed "exit $rc" "exit $rc" win-upgrade-install-previous.log
    return 0
  fi
  PREV_LINE=$(wine_env "$P3" timeout 60 wine "$INST3/nexus-fabric.exe" --version 2>/dev/null | tr -d '\r' | head -1)
  if pick_port; then
    P2=$NEXT_PORT
    win_session "$P3" "$INST3/nexus-fabric.exe" "$P2" upgrade-prev "$LOGS" ""
  fi
  JOB=$SESSION_JOB
  L="$LOGS/win-upgrade-install-new.log"
  wine_env "$P3" timeout 400 wine "$SETUP" /S >"$L" 2>&1
  rc=$?
  if [ "$rc" = 0 ] && [ -f "$DATA3/nexus-fabric.sqlite" ]; then
    rec win.upgrade.install_new "New installer installed over the previous one; data folder untouched" "$HOW" passed "" \
      "exit 0; database file present" win-upgrade-install-new.log
  else
    rec win.upgrade.install_new "New installer installed over the previous one; data folder untouched" "$HOW" failed "exit $rc or database missing" "exit $rc" win-upgrade-install-new.log
  fi
  NEW_LINE=$(wine_env "$P3" timeout 60 wine "$INST3/nexus-fabric.exe" --version 2>/dev/null | tr -d '\r' | head -1)
  if [ -n "$JOB" ] && pick_port; then
    P2=$NEXT_PORT
    win_session "$P3" "$INST3/nexus-fabric.exe" "$P2" upgrade-new "$LOGS" "$JOB"
  else
    rec win.upgrade-new.job_survived "Job still listed after the upgrade" "$HOW" "not run" "no job from the previous version to check" "" ""
  fi
  if [ "$PREV_LINE" = "$NEW_LINE" ]; then
    rec win.upgrade.version_changed "Version changed by the upgrade" "$HOW" "not run" \
      "both installers are $VER: the version number cannot change, so this cannot be shown; engine identifier compared below" "$NEW_LINE" ""
  elif [ "$(agrees "$NEW_LINE" "$VER")" = yes ]; then
    rec win.upgrade.version_changed "Version changed by the upgrade" "$HOW" passed "" "$PREV_LINE -> $NEW_LINE" ""
  else
    rec win.upgrade.version_changed "Version changed by the upgrade" "$HOW" failed "new version line does not show $VER" "$NEW_LINE" ""
  fi
  rec win.upgrade.engine "Engine identifier before and after the upgrade (recorded)" "$HOW" passed "" \
    "previous: $(printf '%s' "$PREV_LINE" | grep -oE 'nf-engine [^ )]+') ; new: $(printf '%s' "$NEW_LINE" | grep -oE 'nf-engine [^ )]+')" ""
  return 0
}
