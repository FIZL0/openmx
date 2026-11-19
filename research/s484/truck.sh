#!/bin/bash
# S484: does a bike ridden into free09's parked monster truck strike its hull and push it? No uinput
# pad here, so a held-gas from-savestate tape (research/c15/tape.py) plays on recording2's state over
# AT-SPI (C6's recipe), and ride.py moves the bike in front of the truck while the tape's start holds
# the VM paused, then logs the truck's body a row a frame.
#   KIT_PCSX2=3 ./kit pcsx2 with -- bash research/s484/truck.sh PID X Z FRAMES OUT.csv
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6="$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback"
PID=$1 X=$2 Z=$3 N=$4 OUT=$5
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
printf 'down cross\nsleep 40000\n' > "$WORK/gas.pad"
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$WORK/gas.pad" "$WORK/play.p2m2" > /dev/null
python3 -I -c 'import shutil,sys; shutil.copy(sys.argv[1], sys.argv[2])' "$ROOT/context/recordings/recording2.p2m2_SaveState.p2s" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
mkdir -p "$(dirname "$OUT")"
cd "$ROOT"
python3 -B "$HERE/ride.py" "$PID" "$X" "$Z" "$N" "$OUT" || status=$?
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
echo "work files: $WORK"
exit ${status:-0}
