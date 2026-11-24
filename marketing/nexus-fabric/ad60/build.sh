#!/bin/bash
# build one scene project per language: ./build.sh S01 en  ->  S01-en.ecproj
set -e
s=$1; l=$2; cd "$(dirname "$0")"
(echo "var LANG = \"$l\";"; cat data/geometry.js scenes/lib.jsx scenes/$s.jsx) > _build_$s-$l.jsx
/opt/artcraft/bin/effectcraft-cli script _build_$s-$l.jsx --save-as $s-$l.ecproj --json
rm -f _build_$s-$l.jsx
