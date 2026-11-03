#!/bin/bash
# C18: play an S149a car tape back on the original and capture the car's particle manager with
# make_spec.py's spec. S149a's capture.sh (research/journal/2026-10-29-s149a-car-captures) with this
# spec in place of watch/car.toml. Run from the repo root, as one job:
#
#   export KIT_PCSX2=3 WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid it prints; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-11-03-c18-car-particles-capture/capture/capture.sh \
#       PID context/s149a/truck.p2m2 MGR ENTRIES KIND FIRST N context/capture/c18/truck.csv
#
# MGR/ENTRIES as make_spec.py; KIND the car's kind (3 buggy, 4 truck, 5 baja); FIRST the first
# frame_counter to write, N the rows. For the wreck, play a tape whose save state edge_state.py has
# shortened the map in (context/c18/truck-edge.p2m2: the edge throws the parked truck, reason 13).
set -e
HERE=research/journal/2026-11-03-c18-car-particles-capture/capture
C7=research/journal/2026-10-20-c7-c7-biplane-capture/playback
PID=$1 REC=$2 MGR=$3 ENTRIES=$4 KIND=$5 FIRST=$6 N=$7 OUT=$8
mkdir -p context/s149a "$(dirname "$OUT")"
WORK=$(mktemp -d -p context/s149a)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
python3 -B "$HERE/make_spec.py" 0x00605270 "$MGR" "$ENTRIES" \
    | sed -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and car_kind == $KIND\"/" \
    > "$WORK/spec.toml"
# The pad stamp as the recorder's clock, in a copy of this worktree's kit.toml (it is not touched).
sed -e 's/^vsync_addr *=.*/vsync_addr = "0x47a9d8"/' kit.toml > "$WORK/kit.toml"

python3 "$C7/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C7/resume_if_paused.py" "$PID"
python3 "$C7/press.py" "$PID" 'System/Reset'
python3 -c 'import time; time.sleep(2)'
KIT_CONFIG="$WORK/kit.toml" KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
python3 -c 'import time; time.sleep(2)'
python3 "$C7/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 "$C7/open_only.py" "$PID" "$WORK/play.p2m2"
python3 -c 'import time; time.sleep(4)'
python3 "$C7/resume_if_paused.py" "$PID"
wait $rec || true
python3 "$C7/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C7/resume_if_paused.py" "$PID" > /dev/null
python3 "$C7/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "work files: $WORK"
