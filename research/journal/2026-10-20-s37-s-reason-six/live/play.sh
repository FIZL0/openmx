#!/bin/bash
# S37: play recording2 on the original and record scenery.toml over its reason-6 wreck. C6's
# play_capture.sh with this task's spec instead of the acceptance one; run from the repo root, as one
# job, with the accessibility env set (see that script):
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-20-s37-s-reason-six/live/play.sh PID context/recordings/recording2.p2m2 80 OUT.csv
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 N=$3 OUT=$4
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C6/resume_if_paused.py" "$PID"
python3 "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=120 ./kit record research/journal/2026-10-20-s37-s-reason-six/live/scenery.toml -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
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
