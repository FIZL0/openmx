#!/bin/bash
# S437: V40's tape (research/v40/hold.pad) played on the original from first_recording's save state and
# recorded every tick with watch/s58-slip.toml, for the port's replay from the same state
# (replay.rs, the_v40_tape_replays_from_first_recordings_state). C6's play_capture.sh with S58's spec.
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start        # note the pid
#   ./kit pcsx2 with -- bash research/s437/capture.sh PID [ROWS]
#
# Writes context/capture/s437/hold.csv and context/recordings/s437/hold.p2m2 (+ .ticks.csv).
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 N=${2:-292}
FIRST=2774                     # first_recording's state holds frame_counter 2773
OUT=$ROOT/context/capture/s437/hold.csv
TAPE=$ROOT/context/recordings/s437/hold.p2m2
REC=$ROOT/context/recordings/first_recording.p2m2
mkdir -p "$(dirname "$OUT")" "$(dirname "$TAPE")"
python3 -I "$ROOT/research/c15/tape.py" "$REC" "$ROOT/research/v40/hold.pad" "$TAPE" --game 'MX Unleashed'
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e '/^state   = /d' -e 's/^warmup  = .*/warmup = 0/' -e '/^required = true/d' \
    -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    "$ROOT/watch/s58-slip.toml" > "$WORK/spec.toml"
grep -q '^start ' "$WORK/spec.toml" && ! grep -q '^state ' "$WORK/spec.toml" || { echo "s58-slip.toml changed: the spec rewrite missed"; exit 1; }
cd "$ROOT"
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
KIT_FRAME_STALL=300 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
# the movie pauses the VM when it ends; keep it running until the recorder has its rows
deadline=$((SECONDS + 1500))
while kill -0 $rec 2> /dev/null && [ $SECONDS -lt $deadline ]; do
    python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
    sleep 2
done
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
grep -v 'null pointer' "$WORK/rec.log" | tail -6
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['rider_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files: $WORK"
