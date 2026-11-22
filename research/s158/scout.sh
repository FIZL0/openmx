#!/bin/bash
# S158: play a two-pad tape from the 2p state and watch both riders over PINE (research/s158/watch.py)
# instead of recording: for steering the riders into each other before a capture. R22's play.sh.
#
#   ./kit pcsx2 with -- bash research/s158/scout.sh PID TAPE.p2m2 SECS [EVERY]
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 SECS=$3 EVERY=${4:-0.25}
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B research/s158/watch.py "$SECS" "$EVERY"
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause' > /dev/null
