#!/bin/bash
# S36: play the biplane recording back on the original and capture crash.toml, C6's recipe
# (research/journal/2026-10-20-c6-c6-recording2-capture/playback/play_capture.sh) with this folder's spec.
# Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-10-20-s36-s36-the-biplane-crashes/play_crash.sh \
#       PID context/recordings/biplane.p2m2 10971 1719 OUT.csv
#
# The recording's state holds frame_counter 10970, so FIRST is 10971.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$HERE/../2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
sed -e "s/^every       = 1 .*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    "$HERE/crash.toml" > "$WORK/spec.toml"

python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C6/resume_if_paused.py" "$PID"
python3 "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 "$C6/resume_if_paused.py" "$PID"
wait $rec || true
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 "$C6/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "work files: $WORK"
