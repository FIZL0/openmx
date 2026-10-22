#!/bin/bash
# Play a split recording back (as play_capture.sh does) and run at_ticks.py on it instead of the
# recorder: save states at the tick before each contact, screenshots around them (S93).
#
#   ./kit pcsx2 with -- bash research/s93/play_at.sh PID REC ACTION...
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2
shift 2
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B research/s93/at_ticks.py "$@"
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
