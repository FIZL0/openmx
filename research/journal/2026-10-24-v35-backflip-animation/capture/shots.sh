#!/bin/bash
# V35: play recording2 back on the original (C6's recipe, as play.sh) and shoot it round flip 1.
#   ./kit pcsx2 with -- bash research/journal/2026-10-24-v35-backflip-animation/capture/shots.sh PID \
#       context/recordings/recording2.p2m2 context/shots/v35 [TICK ...]
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../../2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 OUT=$3
shift 3
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$HERE/shots.py" "$OUT" "$@" || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
echo "work files: $WORK"
