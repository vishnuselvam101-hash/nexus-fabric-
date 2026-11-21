#!/bin/sh
# Starts NEXUS FABRIC in desktop mode from the folder this script is in.
# The console shows the address; the browser opens on the first-run setup page.
# Development build. Not validated. Outputs must not be used for manufacturing.
DIR=$(cd "$(dirname "$0")" && pwd)
exec "$DIR/nexus-fabric" --desktop "$@"
