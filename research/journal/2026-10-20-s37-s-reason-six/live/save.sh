#!/bin/bash
# S37: play recording2 on the original and save states on the ticks before its reason-6 wreck. play.sh's
# flow (C6's recipe), with save_before.py in place of the recorder. From the repo root, one job:
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-20-s37-s-reason-six/live/save.sh PID \
#       context/recordings/recording2.p2m2 4122:8 4123:9
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
HERE=research/journal/2026-10-20-s37-s-reason-six/live
PID=$1 REC=$2
shift 2
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C6/resume_if_paused.py" "$PID"
python3 "$C6/press.py" "$PID" 'System/Reset'
sleep 2
python3 "$HERE/save_before.py" "$@" > "$WORK/save.log" 2>&1 &
saver=$!
sleep 2
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 "$C6/resume_if_paused.py" "$PID"
wait $saver || true
sleep 3
python3 "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 "$C6/press.py" "$PID" 'System/Pause'
cat "$WORK/save.log"
echo "work files: $WORK"
