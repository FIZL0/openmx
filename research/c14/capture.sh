#!/bin/bash
# C14: play a Kiowa tape and capture it with watch/c14-kiowa.toml from the state's first tick. C6's
# play_capture.sh with the rider trigger swapped for the helicopter's: System -> Reset first, so the
# state load is a forward frame jump, the recorder armed on the human's helicopter (the chain is null
# until the state is in), then Play on a writable copy and one resume.
# usage: ./kit pcsx2 with -- research/c14/capture.sh PID TAPE.p2m2 ROWS OUT.csv
set -e
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
PLAY=$ROOT/research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 N=$3 OUT=$4
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
sed -e "/^required = true/d" -e 's/^every       = 1$/&\nstart       = "heli_ctl_src == 1 and heli_class == 4573568"/' \
    "$ROOT/watch/c14-kiowa.toml" > "$WORK/spec.toml"
grep -q '^start ' "$WORK/spec.toml"
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$PLAY/resume_if_paused.py" "$PID"
python3 -B "$PLAY/press.py" "$PID" 'System/Reset' > /dev/null
timeout 2 tail -f /dev/null || true
KIT_FRAME_STALL=300 "$ROOT/kit" record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
timeout 2 tail -f /dev/null || true
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$PLAY/open_only.py" "$PID" "$WORK/play.p2m2"
timeout 4 tail -f /dev/null || true
python3 -B "$PLAY/resume_if_paused.py" "$PID"
wait $rec || true
python3 -B "$PLAY/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "work files: $WORK"
