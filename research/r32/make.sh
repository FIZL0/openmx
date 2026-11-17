#!/bin/bash
# R32: rebuild context/states/s89-along.eeMemory.bin (S89's free06 bike on the railway bed).
#   ./kit pcsx2 with -- bash research/r32/make.sh PID SLOT
# PCSX2 copy started with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 (AT-SPI). Plays
# switch.pad's tape from biplane.p2m2's state (C28's way, research/c28/tape.sh), lets along.py teleport
# and save SLOT; then `./kit state ram` of that slot's .p2s gives the RAM image.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6="$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback"
PID=$1 SLOT=$2
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$HERE/switch.pad" "$WORK/play.p2m2"
python3 -I -c 'import shutil,sys; shutil.copy(sys.argv[1], sys.argv[2])' \
    "$ROOT/context/recordings/biplane.p2m2_SaveState.p2s" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$HERE/along.py" "$PID" "$SLOT"
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
echo "work $WORK"
