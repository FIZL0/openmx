#!/bin/bash
# S57's capture of the original's crash, wreck and L1/R1 reset: load the biplane state (slot 7, slot7.sh),
# start PCSX2's video capture, log poof.toml every frame, fly flight.pad on the kit pad, stop the capture.
#
# usage (from the worktree): ./kit pcsx2 with -- bash research/journal/2026-10-21-s57-biplane-crash-and-poof/capture/run.sh PID NAME [SLOT SPEC PAD FRAMES]
# The bike's reset: run.sh PID bike1 8 bike.toml bike.pad 900
set -e
pid=$1; name=$2; slot=${3:-7}; spec=${4:-poof.toml}; pad=${5:-flight.pad}; frames=${6:-4800}
here=$(cd "$(dirname "$0")" && pwd)
p49=$here/../../2026-10-21-p49-engine-sound/capture
out=$(pwd)/context/s57
mkdir -p "$out"
./kit pine load "$slot"
./kit pad send "release" "sleep 1000"
python3 -B "$p49/press.py" "$pid" 'System/Video Capture' >/dev/null 2>&1 &
press=$!
./kit pad send "sleep 1500"
python3 -B "$p49/save_as.py" "$pid" 'Video Capture' "$out/$name.mp4"
kill $press 2>/dev/null || true
./kit record "$here/$spec" --frames "$frames" --out "$out/$name.csv" &
log=$!
./kit pad send < "$here/$pad"
wait $log || true
timeout 8 python3 -B "$p49/press.py" "$pid" 'System/Video Capture' || true
