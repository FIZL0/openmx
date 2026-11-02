#!/bin/bash
# S235: play a generated tape from a save state on copy 1 and capture climb.toml, filming it. P21's
# flat_capture.sh (C6's playback recipe) with P49's Video Capture press. Run from the repo root, with
# the copy started under the accessibility env (KIT_PCSX2=1 WAYLAND_DISPLAY=wayland-1
# QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1), as one job:
#
#   ./kit pcsx2 with -- bash research/journal/2026-11-01-s235-rider-push-uphill/capture/capture.sh \
#       PID TAPE SLOT FIRST N OUT.csv VIDEO.mp4 WORKDIR
set -e
H=research/journal/2026-10-20-c6-c6-recording2-capture/playback
V=research/journal/2026-10-21-p49-engine-sound/capture
S=$(dirname "$0")
PID=$1 TAPE=$2 SLOT=$3 FIRST=$4 N=$5 OUT=$6 VIDEO=$7 WORK=$8
mkdir -p "$WORK" "$(dirname "$OUT")"
python3 -B "$S/prepare.py" "$TAPE" "$SLOT" "$FIRST" "$WORK"

python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$H/resume_if_paused.py" "$PID"
python3 "$H/press.py" "$PID" 'System/Reset'
python3 -c 'import time; time.sleep(2)'
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
python3 -c 'import time; time.sleep(2)'
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$H/open_only.py" "$PID" "$WORK/play.p2m2"
python3 -c 'import time; time.sleep(4)'
python3 "$H/resume_if_paused.py" "$PID"
python3 -B "$V/press.py" "$PID" "System/Video Capture" > /dev/null 2>&1 &
press=$!
for i in 1 2 3 4 5 6 7 8 9 10; do python3 -B "$V/save_as.py" "$PID" "Video Capture" "$VIDEO" && break; python3 -c 'import time; time.sleep(1)'; done
kill $press 2>/dev/null || true
wait $rec || true
timeout 8 python3 -B "$V/press.py" "$PID" "System/Video Capture" || true
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$H/resume_if_paused.py" "$PID" > /dev/null
python3 "$H/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
