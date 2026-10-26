#!/bin/bash
# S100: play a generated full-throttle recording from the teleported tree state and capture it. C6's
# play_capture.sh with the spec extended by extra.toml. Args: PID REC FIRST N OUT
set -e
H=research/journal/2026-10-20-c6-c6-recording2-capture/playback
S=$(dirname "$0")
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
mkdir -p context/s100; WORK=$(mktemp -d -p context/s100)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
sed -e '/^state  = /d' -e 's/^warmup = .*/warmup = 0/' -e '/^required = true/d' \
    -e "s/^every       = 1 .*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    watch/acceptance.toml > "$WORK/spec.toml"
cat "$S/extra.toml" >> "$WORK/spec.toml"

python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$H/resume_if_paused.py" "$PID"
python3 "$H/press.py" "$PID" 'System/Reset'
python3 -c 'import time; time.sleep(2)'
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
python3 -c 'import time; time.sleep(2)'
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$H/open_only.py" "$PID" "$WORK/play.p2m2"
python3 -c 'import time; time.sleep(4)'
python3 "$H/resume_if_paused.py" "$PID"
wait $rec || true
python3 "$H/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$H/resume_if_paused.py" "$PID" > /dev/null
python3 "$H/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['rider_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files: $WORK"
