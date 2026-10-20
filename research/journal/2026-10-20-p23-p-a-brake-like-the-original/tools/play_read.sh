#!/bin/bash
# Play a movie copy on copy 1 and run a PINE reader over it (C6's sequence, with a reader instead of
# the recorder). usage: play_read.sh PID MOVIE -- reader args...
set -e
PB=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2; shift 3
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 $PB/press.py "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 $PB/resume_if_paused.py "$PID" > /dev/null
python3 $PB/press.py "$PID" 'System/Reset' > /dev/null
sleep 2
"$@" > "$WORK/read.log" 2>&1 &
rd=$!
sleep 1
python3 $PB/press.py "$PID" 'Tools/Input Recording/Play' > /dev/null 2>&1 &
python3 $PB/open_only.py "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 $PB/resume_if_paused.py "$PID" > /dev/null
wait $rd || true
python3 $PB/press.py "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 $PB/resume_if_paused.py "$PID" > /dev/null
cat "$WORK/read.log"
