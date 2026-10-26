#!/bin/bash
# C8: play the C8 tape (make_tape.py) back on the original and capture c8.toml. V35's play.sh, which
# is S109's with another spec:
#   ./kit pcsx2 with -- bash research/journal/2026-10-26-c8-flip-capture/capture/play.sh PID \
#       context/recordings/c8/c8.p2m2 2374 440 context/capture/c8/run1.csv
# The steps and why each is there are C6's (research/journal/2026-10-20-c6-c6-recording2-capture).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../../2026-10-20-c6-c6-recording2-capture/playback" && pwd)
P49=$(cd "$HERE/../../2026-10-21-p49-engine-sound/capture" && pwd)
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    "$HERE/${SPEC:-c8}.toml" > "$WORK/spec.toml"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=300 python3 -B "$HERE/../../2026-10-23-s109-animandpump/capture/record_saving.py" "$WORK/spec.toml" "$OUT" "$N" --save "${SAVE:-}" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
if [ -n "${VIDEO:-}" ]; then
  python3 -B "$P49/press.py" "$PID" "System/Video Capture" >/dev/null 2>&1 &
  press=$!
  for i in 1 2 3 4 5 6 7 8 9 10; do python3 -B "$P49/save_as.py" "$PID" "Video Capture" "$VIDEO" && break; read -t 1 < /dev/zero || true; done
  kill $press 2>/dev/null || true
fi
python3 -B "$C6/resume_if_paused.py" "$PID"
wait $rec || true
if [ -n "${VIDEO:-}" ]; then
  timeout 8 python3 -B "$P49/press.py" "$PID" "System/Video Capture" || true
fi
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -14
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['rider_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
