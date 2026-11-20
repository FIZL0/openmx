#!/bin/bash
# R48: research/s228/play_capture.sh with watch/r48-truck.toml and its own first-row check: play a
# from-savestate tape on a kit copy over AT-SPI and record it from frame FIRST for N rows.
#   KIT_PCSX2=2 ./kit pcsx2 with -- bash research/r48/play_capture.sh PID TAPE.p2m2 FIRST N OUT.csv
# The copy must run with QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1.
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
sed -e "s/^every       = 1/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26))\"/" \
    "${SPEC:-watch/r48-truck.toml}" > "$WORK/spec.toml"
mkdir -p "$(dirname "$OUT")"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
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
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "work files (the played copy, the spec, the logs): $WORK"
