#!/bin/bash
# P85's capture of the original's shootoff sound: PCSX2's Video Capture (its audio track is the SPU's
# output) around research/p85_edge_sound.py, which loads save state 7 on free09 and rides off the map
# edge with watch/p85-edge-sound.toml logging the sound child and the music every tick. P49's AT-SPI
# helpers press the menu item and fill the save dialog.
#
# usage (from the worktree): ./kit pcsx2 with -- bash research/journal/2026-10-23-p85-shootoff-sound/capture/run.sh PID NAME X,Z
set -e
pid=$1; name=$2; to=$3
p49=research/journal/2026-10-21-p49-engine-sound/capture
out=$(pwd)/context/capture/p85
mkdir -p "$out"
python3 -B "$p49/press.py" "$pid" 'System/Video Capture' >/dev/null 2>&1 &
press=$!
./kit pad send "release" "sleep 1500"
python3 -B "$p49/save_as.py" "$pid" 'Video Capture' "$out/$name.mp4"
kill $press 2>/dev/null || true
python3 -B research/p85_edge_sound.py "$name" --to "$to" || true
timeout 8 python3 -B "$p49/press.py" "$pid" 'System/Video Capture' || true
