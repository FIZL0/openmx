#!/bin/bash
# V114: the original's Kiowa on video, picture and sound. Plays C14's flight tape (rest, climb, hover,
# forward, rest, rudder, left, rest, descend, rest; context/recordings/c14/flight.p2m2 and its slot-2
# state) under PCSX2's System → Video Capture, whose AAC track is the SPU's output (P49).
#
# usage (from the worktree, the copy started with the AT-SPI env and NominalScalar 1.0):
#   ./kit pcsx2 with -- bash research/journal/2026-11-02-v114-kiowa-sound/capture/run.sh PID OUT.mp4
set -e
pid=$1; out=$2
root=$(pwd)
p49="$root/research/journal/2026-10-21-p49-engine-sound/capture"
play="$root/research/journal/2026-10-20-c6-c6-recording2-capture/playback"
python3 -B "$play/resume_if_paused.py" "$pid"
python3 -B "$p49/press.py" "$pid" 'System/Video Capture' >/dev/null 2>&1 &
press=$!
timeout 2 tail -f /dev/null || true
python3 -B "$p49/save_as.py" "$pid" 'Video Capture' "$out"
kill $press 2>/dev/null || true
bash "$root/research/c14/play.sh" "$pid" "$root/context/recordings/c14/flight.p2m2"
# 854 ticks of tape, about 29 s at full speed, and a little after.
timeout 36 tail -f /dev/null || true
timeout 8 python3 -B "$p49/press.py" "$pid" 'System/Video Capture' || true
echo "captured $out"
