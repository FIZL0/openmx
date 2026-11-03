#!/bin/bash
# MD82b: play a tape from a machine's save state and capture cam.toml every tick. MD39a's play.sh with the
# camera spec:
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N ./kit pcsx2 with -- bash research/md82b/play.sh PID \
#       TAPE.p2m2 STATE.p2s CAM FIRST ROWS context/capture/md82b/NAME.csv
# CAM is the follow camera (hex; vtable 0x003cc6f0 at +4, research/oracle.txt # MD82b), FIRST the state's
# own vsync. Rows start in FIRST..FIRST+26 once the camera's vtable is in, so a long-booted VM cannot arm
# on the menus. Take ROWS a few short of the tape's ticks: the movie's end stops the VM.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 STATE=$3 CAM=$4 FIRST=$5 N=$6 OUT=$7
VPTR=$((0x003cc6f0))
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/CAM/$CAM/" \
    -e "s/^every       = 1.*/&\nstart       = \"vsync >= $FIRST and vsync <= $((FIRST + 26)) and cvptr == $VPTR\"/" \
    "$HERE/cam.toml" > "$WORK/spec.toml"
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
python3 -c "import csv,sys; csv.field_size_limit(1<<24); r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['cvptr']==sys.argv[2] else 'first row has no camera: the movie did not load')" "$OUT" "$VPTR"
echo "work files (the played copy, the spec, the logs): $WORK"
