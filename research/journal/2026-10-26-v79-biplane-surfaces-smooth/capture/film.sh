#!/bin/bash
# V79: film the original's biplane surfaces on a stick step. Loads the human's biplane state (copied
# to slot 8 by put_state.py), starts PCSX2's Video Capture over AT-SPI (P49's press.py/save_as.py),
# throws the stick full left, full right, centre and the rudder, stops the capture.
#
# usage (from the worktree, PCSX2 started with the Qt accessibility env):
#   ./kit pcsx2 with -- bash research/journal/<entry>/capture/film.sh PID OUT.mp4
set -e
pid=$1; out=$2
here=$(cd "$(dirname "$0")" && pwd)
p49=$(pwd)/research/journal/2026-10-21-p49-engine-sound/capture
python3 -B "$here/put_state.py"
./kit pine load 8
./kit pad send "release" "down cross" "sleep 1500"
python3 -B "$p49/press.py" "$pid" 'System/Video Capture' >/dev/null 2>&1 &
press=$!
timeout 1 tail -f /dev/null || true
python3 -B "$p49/save_as.py" "$pid" 'Video Capture' "$out"
kill $press 2>/dev/null || true
./kit pad send < "$here/throw.pad"
# The pad server plays the file's timing itself and `send` returns at once: wait the file's length out.
timeout 11 tail -f /dev/null || true
timeout 8 python3 -B "$p49/press.py" "$pid" 'System/Video Capture' || true
