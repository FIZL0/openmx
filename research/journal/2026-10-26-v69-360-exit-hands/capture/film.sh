#!/bin/bash
# V69: film MX vs. ATV's 360 (M21's closed-loop driver) with PCSX2's Video Capture, in kit copy 1.
# usage (from the worktree, KIT_CONFIG at the scratch mxvatv kit.toml): ./kit pcsx2 with -- bash film.sh PID MOVE NAME
set -e
pid=$1; move=$2; name=$3
here=$(cd "$(dirname "$0")" && pwd)
cap=/home/rycs/repos/openmx-slots/s1/research/journal/2026-10-21-p49-engine-sound/capture
if [ -z "$OPEN" ]; then
./kit pad send "release"
python3 -B "$cap/press.py" "$pid" 'System/Video Capture' >/dev/null 2>&1 &
press=$!
fi
for i in 1 2 3 4 5 6 7 8 9 10; do python3 -B "$cap/save_as.py" "$pid" 'Video Capture' "$here/$name.mp4" 2>/dev/null && break; read -t 1 < /dev/zero || true; done
kill $press 2>/dev/null || true
python3 -B "$here/both.py" "$here/$name.csv" --game mxvatv --move "$move" --frames 420
timeout 8 python3 -B "$cap/press.py" "$pid" 'System/Video Capture' || true
