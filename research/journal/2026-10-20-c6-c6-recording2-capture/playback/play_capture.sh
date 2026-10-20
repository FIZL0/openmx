#!/bin/bash
# Play a human recording back on the original and capture it with watch/acceptance.toml, no pause.
# C6's recipe (see the entry beside this folder). Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid it prints; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-10-20-c6-c6-recording2-capture/playback/play_capture.sh \
#       PID context/recordings/recording2.p2m2 2374 1894 context/capture/acceptance/recording2-freeride.csv
#
# FIRST is the first frame_counter to write (the recording's state holds FIRST - 1); N the rows.
# What it does: Stop any movie, unpause, System -> Reset (so the counters restart far below FIRST and
# the load is a *forward* jump — the recorder's frame loop cannot follow a backward one), start the
# recorder on a copy of the spec with no state load, no warmup and `start` armed on FIRST, press
# Tools -> Input Recording -> Play on a *copy* of the recording (PCSX2 records over the file it plays),
# then resume once if Pause is checked: the playback loads its state and leaves the VM paused, and
# nothing else resumes it. `start` also needs the rider, which is null until the state is in: without
# that, a boot that runs long enough counts frame_counter past FIRST by itself and arms on the menus.
# KIT_FRAME_STALL covers the seconds the file dialog and the load hold the counter still.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e '/^state  = /d' -e 's/^warmup = .*/warmup = 0/' -e '/^required = true/d' \
    -e "s/^every       = 1 .*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    watch/acceptance.toml > "$WORK/spec.toml"

python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$HERE/resume_if_paused.py" "$PID"
python3 "$HERE/press.py" "$PID" 'System/Reset'
sleep 2
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
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
# The rider is null in the boot menus, and the trigger needs it, but say so if a row slipped through.
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['rider_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
