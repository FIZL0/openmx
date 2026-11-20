#!/bin/bash
# C43: fly a generated tape from C14's Kiowa state and record research/c43/c43.toml from its first tick.
# research/c14/capture.sh's flow (Stop, resume, System -> Reset, the recorder armed on the human's
# helicopter, Play a writable copy, resume), with the tape built here from segments by
# research/c14/make_tape.py (FIELDS[:BUTTONS[:LX,LY]], two fields a tick) and the VM kept running
# after the movie ends, as research/c42/run.sh does.
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1 KIT_PCSX2=1
#   ./kit pcsx2 start
#   ./kit pcsx2 with -- bash research/c43/fly.sh PID NAME ROWS SEGMENT...
# Writes context/recordings/c43/NAME.p2m2 (+ _SaveState.p2s, C14's state) and context/capture/c43/NAME.csv.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 NAME=$2 N=$3
shift 3
C14=$ROOT/context/recordings/c14/flight.p2m2
TAPE=$ROOT/context/recordings/c43/$NAME.p2m2
OUT=$ROOT/context/capture/c43/$NAME.csv
mkdir -p "$(dirname "$TAPE")" "$(dirname "$OUT")"
python3 -I "$ROOT/research/c14/make_tape.py" "$C14" "$TAPE" "$@"
cp "$C14"_SaveState.p2s "$TAPE"_SaveState.p2s
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$TAPE"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "/^required = true/d" -e 's/^every       = 1$/&\nstart       = "heli_ctl_src == 1 and heli_class == 4573568"/' \
    "$HERE/c43.toml" > "$WORK/spec.toml"
grep -q '^start ' "$WORK/spec.toml"
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null
timeout 2 tail -f /dev/null || true
cd "$ROOT"
if [ -n "$C43_AT" ]; then      # frames to save slots at (strike.py), comma-separated
    KIT_FRAME_STALL=300 python3 -B "$HERE/strike.py" "$WORK/spec.toml" "$OUT" "$N" "$NAME" "$C43_AT" > "$WORK/rec.log" 2>&1 &
else
    KIT_FRAME_STALL=300 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
fi
rec=$!
timeout 2 tail -f /dev/null || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
timeout 4 tail -f /dev/null || true
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
# the movie pauses the VM when it ends; keep it running until the recorder has its rows (10 min at most)
end=$((SECONDS + 600))
while kill -0 $rec 2> /dev/null && [ $SECONDS -lt $end ]; do
    python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
    timeout 2 tail -f /dev/null || true
done
kill $rec 2> /dev/null || true
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
grep -v 'null pointer' "$WORK/rec.log" | tail -12
echo "csv $OUT  work $WORK"
