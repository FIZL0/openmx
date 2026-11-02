#!/bin/bash
# C14: play a tape (.p2m2 with its _SaveState.p2s beside it) on a kit copy with no recorder, for
# making states. Run under `./kit pcsx2 with --`. C6's playback steps: Stop any movie, unpause, Play a
# writable copy, unpause the paused VM the Play leaves behind.
# usage: research/c14/play.sh PID TAPE.p2m2
set -e
PLAY=$(cd "$(dirname "$0")/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$PLAY/resume_if_paused.py" "$PID"
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$PLAY/open_only.py" "$PID" "$WORK/play.p2m2"
timeout 4 tail -f /dev/null || true
python3 -B "$PLAY/resume_if_paused.py" "$PID"
echo "playing $WORK/play.p2m2"
