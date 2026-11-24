#!/bin/bash
# render every scene (EffectCraft, 4K60) per language, join in order, add the soundtrack -> final/
cd "$(dirname "$0")"
for l in en ja; do
  : > renders/list-$l.txt
  for s in S01 S02 S03 S04 S05 S06 S07 S08 S09 S10 S11 S12; do
    /opt/artcraft/bin/effectcraft-cli render $s-$l.ecproj --comp $s --out renders/$s-$l.mp4 --format h264 --resolution full --quality best --bitrate 90000 > renders/$s-$l.log 2>&1 || echo "render $s $l FAILED"
    echo "file '$s-$l.mp4'" >> renders/list-$l.txt
  done
  ffmpeg -loglevel error -y -f concat -safe 0 -i renders/list-$l.txt -i audio/soundtrack.wav -map 0:v -map 1:a -c:v libx264 -preset slow -crf 14 -pix_fmt yuv420p -r 60 -c:a aac -b:a 320k -ar 48000 -shortest -movflags +faststart final/NEXUS-FABRIC-film-70s-$l-4k60.mp4 && echo "final $l done"
done
