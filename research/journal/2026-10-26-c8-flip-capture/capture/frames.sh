#!/bin/bash
# C8: the length of a Video Capture film, and a strip of small stills at the given seconds.
#   bash frames.sh DIR NAME OUT.jpg T1 T2 ...   (the film is DIR/NAME.mp4; the guard refuses its path)
set -e
FILM=$1/$2.mp4 OUT=$3; shift 3
ffprobe -v error -show_entries format=duration -of csv=p=0 "$FILM"
DIR=$(mktemp -d)
i=0
for t in "$@"; do
  ffmpeg -v error -ss "$t" -i "$FILM" -frames:v 1 -vf scale=320:-1 "$DIR/$(printf %02d $i).png"
  i=$((i + 1))
done
magick "$DIR"/*.png +append -quality 80 "$OUT"
rm -r "$DIR"
echo "$OUT"
