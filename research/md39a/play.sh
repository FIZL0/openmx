#!/bin/bash
# MD39a: play a tape (research/c15/tape.py) from a car's save state and capture car.toml every tick.
# C15's play.sh with the car spec:
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N ./kit pcsx2 with -- bash research/md39a/play.sh PID \
#       TAPE.p2m2 STATE.p2s CAR VPTR FIRST ROWS context/capture/md39a/NAME.csv
# CAR is the car object (hex), VPTR its vtable word (decimal), FIRST the state's own vsync. Rows start on
# FIRST once the vptr is in, so a long-booted VM cannot arm on the menus. Take ROWS a few short of the
# tape's ticks: the movie's end stops the VM, and the recorder would wait out KIT_FRAME_STALL.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 STATE=$3 CAR=$4 VPTR=$5 FIRST=$6 N=$7 OUT=$8
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/CAR/$CAR/" \
    -e "s/^every       = 1.*/&\nstart       = \"vsync >= $FIRST and vsync <= $((FIRST + 26)) and vptr == $VPTR\"/" \
    "$HERE/car.toml" > "$WORK/spec.toml"
mkdir -p "$(dirname "$OUT")"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=300 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID"
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -14
python3 -c "import csv,sys; csv.field_size_limit(1<<24); r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['vptr']==sys.argv[2] else 'first row has no car: the movie did not load')" "$OUT" "$VPTR"
echo "work files (the played copy, the spec, the logs): $WORK"
