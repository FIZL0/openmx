#!/bin/bash
# F71: C15's play.sh with research/f71/ring.toml and save state 2's vsync (28499) as the start; no bike check.
# (C6's recipe: research/journal/2026-10-20-c6-c6-recording2-capture) with the MX vs. ATV trigger:
#   KIT_CONFIG=<SLUS-21104 kit.toml> KIT_PCSX2=N ./kit pcsx2 with -- bash research/f71/play.sh PID \
#       TAPE.p2m2 STATE.p2s ROWS context/capture/f71/NAME.csv
# STATE.p2s becomes the tape's _SaveState.p2s. Rows start on vsync FIRST..FIRST+26 (no vptr trigger).
set -e
HERE=$(cd "$(dirname "$0")/../c15" && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 STATE=$3 N=$4 OUT=$5
FIRST=${FIRST:-28499}
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/^every       = 1.*/&\nstart       = \"vsync >= $FIRST and vsync <= $((FIRST + 26)) \"/" \
    "$HERE/../f71/ring.toml" > "$WORK/spec.toml"
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

echo "work files (the played copy, the spec, the logs): $WORK"
