#!/bin/bash
# S475: play TAPE.pad from STATE.p2s over AT-SPI and log the props per frame into OUT.csv.
#   ./kit pcsx2 with -- bash research/s475/run.sh PID PAD STATE OUT FRAMES
# PCSX2 must run with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 (the Play press is AT-SPI).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
WORKDIR=${TMPDIR:-/tmp}
C6=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 PAD=$2 STATE=$3 OUT=$4 N=$5 POKE=${6:-}
WORK=$(mktemp -d -p "$WORKDIR")
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$PAD" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 3
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
timeout 300 python3 -B "$HERE/log.py" "$OUT" "$N" $POKE
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
