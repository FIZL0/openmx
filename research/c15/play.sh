#!/bin/bash
# C15: play a tape (tape.py) from SLUS-21104 save state 4 and capture bike.toml every tick. C8's play.sh
# (C6's recipe: research/journal/2026-10-20-c6-c6-recording2-capture) with the MX vs. ATV trigger:
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N ./kit pcsx2 with -- bash research/c15/play.sh PID \
#       TAPE.p2m2 STATE.p2s ROWS context/capture/c15/NAME.csv
# STATE.p2s becomes the tape's _SaveState.p2s. Rows start on the state's own vsync (0x3333 in state 4)
# once the bike's vptr is in, so a long-booted VM cannot arm on the menus.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 STATE=$3 N=$4 OUT=$5
FIRST=${FIRST:-13107}
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/^every       = 1.*/&\nstart       = \"vsync >= $FIRST and vsync <= $((FIRST + 26)) and vptr == 3998800\"/" \
    "$HERE/bike.toml" > "$WORK/spec.toml"
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
python3 -c "import csv,sys; csv.field_size_limit(1<<24); r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['vptr']=='3998800' else 'first row has no bike: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
