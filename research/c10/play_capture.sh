#!/bin/bash
# Play the user's 2p recording back on the original and capture it with watch/2p.toml (C10): S93's
# research/s93/play_capture.sh with C10's spec. C6's recipe (research/journal/2026-10-20-c6-c6-
# recording2-capture/playback/play_capture.sh, whose AT-SPI helpers this calls).
# Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/c10/play_capture.sh PID REC FIRST N OUT
#
# FIRST is the first frame_counter to write (the recording's state holds FIRST - 1); N the rows.
# SPEC=<file> plays another spec (research/c10/check.toml: a few words, for comparing builds).
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/^every       = 1/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and p1_type == 2\"/" \
    "${SPEC:-watch/2p.toml}" > "$WORK/spec.toml"
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
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['p1_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
