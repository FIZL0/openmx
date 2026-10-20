#!/bin/bash
# Play the biplane recording back on the original and capture it with watch/biplane.toml. C6's recipe
# (research/journal/2026-10-20-c6-c6-recording2-capture), with three changes: the spec, a `start` that
# waits for the plane instead of the rider, and the recorder waking on the pad stamp rather than
# frame_counter (S32: a frame_counter wake reads some rows mid-tick). Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid it prints; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-10-20-c7-c7-biplane-capture/playback/play_capture.sh \
#       PID context/recordings/biplane.p2m2 10971 1718 context/capture/acceptance/biplane.csv
#
# FIRST is the first frame_counter to write (the recording's state holds FIRST - 1); N the rows.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/^every       = 1 .*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and plane_type == 1\"/" \
    watch/biplane.toml > "$WORK/spec.toml"
# The pad stamp as the recorder's clock, in a copy of this worktree's kit.toml (it is not touched).
sed -e 's/^vsync_addr *=.*/vsync_addr = "0x47a9d8"/' kit.toml > "$WORK/kit.toml"

python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$HERE/resume_if_paused.py" "$PID"
python3 "$HERE/press.py" "$PID" 'System/Reset'
sleep 2
KIT_CONFIG="$WORK/kit.toml" KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$HERE/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 "$HERE/resume_if_paused.py" "$PID"
wait $rec || true
python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$HERE/resume_if_paused.py" "$PID" > /dev/null
python3 "$HERE/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['plane_type']=='1' else 'first row has no plane: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
