#!/bin/bash
# C20: play the whip tape (make_tape.py) on the original and log it with S201's entry_watch.py --attach
# (part bodies, nodes, joints, holds, the drawn bike frame, once a tick). S201's play_entry.sh with the
# attach wait raised (copy 3 runs at NominalScalar 0.25, so frame 2600 is ~35 s after the load):
#
#   export KIT_PCSX2=3 WAYLAND_DISPLAY=wayland-1 XDG_SESSION_TYPE=wayland QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start            # note the pid; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-11-03-c20-full-whip-capture/capture/play_whip.sh \
#       PID context/recordings/c20/c20.p2m2 2600 40 context/c20/whip-right.txt
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../../2026-10-20-c6-c6-recording2-capture/playback" && pwd)
S201=$(cd "$HERE/../../2026-11-02-s201-riding-entry/capture" && pwd)
PID=$1 TAPE=$2 FIRST=$3 SECONDS_=$4 OUT=$5
WORK=$(dirname "$OUT")/work
mkdir -p "$WORK"
cp "$TAPE" "$WORK/play.p2m2"
cp "$TAPE"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
python3 -c 'import time; time.sleep(2)'
python3 -B "$S201/entry_watch.py" --state 0 --attach "${ATTACH:-150}" --after "$FIRST" --seconds "$SECONDS_" --out "$OUT" > "$WORK/watch.log" 2>&1 &
watch=$!
python3 -c 'import time; time.sleep(1)'
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
python3 -c 'import time; time.sleep(4)'
python3 -B "$C6/resume_if_paused.py" "$PID"
timeout 400 tail --pid=$watch -f /dev/null || true
wait $watch || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
tail -3 "$WORK/watch.log"
