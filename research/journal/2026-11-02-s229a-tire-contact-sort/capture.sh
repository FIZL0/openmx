#!/bin/bash
# S229a (S149b's capture.sh, with this folder's sort.toml): play a generated car tape (make_tape.py) back on
# the original and capture it, the contact sort's probe and hit list beside S149b's blocks. C7's
# play_capture.sh with the car spec, its address rewritten to CAR, and a `start` that waits for the car's
# kind. Run from the repo root, as one job, in this slot's emulator copy:
#
#   export KIT_PCSX2=2 WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-11-02-s229a-tire-contact-sort/capture.sh \
#       PID context/s229a/garage.p2m2 0x00605270 5 401 N context/capture/s229a/baja-garage.csv
#
# The tape's save state is beside it as <tape>_SaveState.p2s (S149a's baja state, frame_counter 400).
# FIRST is the first frame_counter to write; N the rows. The csv goes in a subfolder so the readers of
# context/capture/ do not take it for a bike ride.
set -e
C7=research/journal/2026-10-20-c7-c7-biplane-capture/playback
PID=$1 REC=$2 CAR=$3 KIND=$4 FIRST=$5 N=$6 OUT=$7
mkdir -p context/s229a "$(dirname "$OUT")"
WORK=$(mktemp -d -p context/s229a)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/0x00605270/$CAR/g" \
    -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and car_kind == $KIND\"/" \
    "$(dirname "$0")/sort.toml" > "$WORK/spec.toml"
# The pad stamp as the recorder's clock, in a copy of this worktree's kit.toml (it is not touched).
# rand_state (the game's rng, research/oracle.txt) is missing from some slots' kit.toml.
sed -e 's/^vsync_addr *=.*/vsync_addr = "0x47a9d8"/' kit.toml > "$WORK/kit.toml"
grep -q '^rand_state *=' "$WORK/kit.toml" || sed -i 's/^\[symbols\]$/&\nrand_state = "0x423b30"/' "$WORK/kit.toml"

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
