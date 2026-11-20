#!/bin/bash
# S494: does a bike ridden into a parked flier strike it and push it? research/s484/truck.sh with a
# kit save state for the start (research/r52/boot.py, then settle.py SLOT FRAMES) instead of
# recording2's: a held-gas tape plays from it over AT-SPI, and ride.py moves the bike DIST ft short
# of the flier on its own heading YAW, then logs the flier's body a row a frame.
#   KIT_PCSX2=2 ./kit pcsx2 with -- bash research/s494/flier.sh PID STATE.p2s MACHINE DIST FRAMES OUT.csv YAW LIFT
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6="$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback"
PID=$1 STATE=$2 MACHINE=$3 DIST=$4 N=$5 OUT=$6 YAW=$7 LIFT=$8
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
printf 'down cross\nsleep 40000\n' > "$WORK/gas.pad"
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$WORK/gas.pad" "$WORK/play.p2m2" > /dev/null
python3 -I -c 'import shutil,sys; shutil.copy(sys.argv[1], sys.argv[2])' "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
mkdir -p "$(dirname "$OUT")"
cd "$ROOT"
python3 -B "$HERE/ride.py" "$PID" "$MACHINE" "$DIST" "$N" "$OUT" "$YAW" "$LIFT" || status=$?
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
echo "work files: $WORK"
exit ${status:-0}
