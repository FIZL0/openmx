#!/bin/bash
# S342: C28's tape.sh with C34's System/Reset before Play (without it the tape's presses never landed).
#   ./kit pcsx2 with -- bash research/s342/play.sh PID TAPE.p2m2 STATE.p2s SECONDS SHOT.png
set -e
C7=research/journal/2026-10-20-c7-c7-biplane-capture/playback
PID=$1 REC=$2 STATE=$3 SECS=$4 SHOT=$5
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C7/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C7/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
python3 -B "$C7/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C7/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null
sleep "$SECS"
python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null
sleep 1
python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null
python3 -B research/c28/shot.py "$PID" "$SHOT"
