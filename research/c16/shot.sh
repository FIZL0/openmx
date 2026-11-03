#!/bin/bash
# C16: a screenshot of kit copy $KIT_PCSX2 over AT-SPI (System -> Screenshot; no uinput pad here), shrunk
# to a jpg. PCSX2 only writes it on a presented frame, so a paused VM is resumed first.
# usage: research/c16/shot.sh PID OUT.jpg
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
SNAPS=../s2-slots/pcsx2/s${KIT_PCSX2:-2}/PCSX2/snaps
before=$(ls -t "$SNAPS"/*.png 2>/dev/null | head -1)
timeout 20 python3 -B $C6/resume_if_paused.py "$1" > /dev/null
timeout 20 python3 -B $C6/press.py "$1" 'System/Screenshot' > /dev/null
for _ in 1 2 3 4 5 6 7 8 9 10; do
    f=$(ls -t "$SNAPS"/*.png | head -1)
    [ "$f" != "$before" ] && break
    timeout 1 tail -f /dev/null
done
ffmpeg -loglevel error -y -i "$f" -vf scale=480:-1 -q:v 4 "$2" && echo "$2"
