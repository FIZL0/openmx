#!/bin/bash
# C47: play a bones_tape.py tape on the original and log it with S74's bones_watch.py --attach. S201's
# play_entry.sh (C6's recipe: Stop, unpause, Reset, Play a writable copy, resume once) with S74's watcher.
# Run from the repo root, as one job, on a copy started with accessibility on:
#
#   export WAYLAND_DISPLAY=wayland-1 XDG_SESSION_TYPE=wayland QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   KIT_PCSX2=2 ./kit pcsx2 start            # note the pid it prints; wait for ./kit pine to answer
#   KIT_PCSX2=2 ./kit pcsx2 with -- bash research/journal/2026-11-22-c47-rerecord-bones/capture/play_bones.sh \
#       PID TAPE.p2m2 FIRST context/s74/free06-ride.txt
#
# FIRST is the tape state's frame counter; the watcher logs 540 ticks (S74's 18 s) from FIRST + 75 (its 2.5 s).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../../2026-10-20-c6-c6-recording2-capture/playback" && pwd)
S74=$(cd "$HERE/../../2026-10-21-s74-putback-bones/capture" && pwd)
PID=$1 TAPE=$2 FIRST=$3 OUT=$4
WORK=$(dirname "$OUT")/work
mkdir -p "$WORK"
cp "$TAPE" "$WORK/play.p2m2"
cp "$TAPE"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
python3 -c 'import time; time.sleep(2)'
python3 -B "$S74/bones_watch.py" --state 0 --attach 60 --after $((FIRST + 75)) --frames 540 --out "$OUT" > "$WORK/watch.log" 2>&1 &
watch=$!
python3 -c 'import time; time.sleep(1)'
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
python3 -c 'import time; time.sleep(4)'
python3 -B "$C6/resume_if_paused.py" "$PID"
wait $watch || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
tail -3 "$WORK/watch.log"
