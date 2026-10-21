#!/bin/bash
# P49's capture of the original's engine: load the human's first_recording state (copied to slot 7),
# start PCSX2's video capture (its audio track is the SPU's output, the copy's speaker mute aside), log
# the note's hands every frame, ride ride.pad on the kit pad, stop the capture.
#
# usage (from the worktree): ./kit pcsx2 with -- bash research/journal/2026-10-21-p49-engine-sound/capture/run.sh PID NAME
set -e
pid=$1; name=$2
here=$(cd "$(dirname "$0")" && pwd)
out=$(pwd)/context/p49
mkdir -p "$out"
./kit pine load 7
./kit pad send "release" "sleep 1500"
python3 -B "$here/press.py" "$pid" 'System/Video Capture' >/dev/null 2>&1 &
press=$!
./kit pad send "sleep 1500"
python3 -B "$here/save_as.py" "$pid" 'Video Capture' "$out/$name.mp4"
kill $press 2>/dev/null || true
python3 -B "$here/hands.py" "$out/$name.csv" 40 &
log=$!
./kit pad send < "$here/ride.pad"
wait $log
timeout 8 python3 -B "$here/press.py" "$pid" 'System/Video Capture' || true
