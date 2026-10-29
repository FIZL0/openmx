#!/bin/bash
# S149b (from S149a's capture.sh, with this folder's car_tire.toml): play a generated car tape back on the original and capture it with car_tire.toml. C7's
# play_capture.sh (research/journal/2026-10-20-c7-c7-biplane-capture/playback) with the car spec, its
# address rewritten to CAR, and a `start` that waits for the car's kind. Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid it prints; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-10-29-s149b-car-tire-model/capture.sh \
#       PID context/s149a/baja.p2m2 CAR KIND FIRST N context/capture/s149b/baja.csv
#
# The tape's save state is beside it as <tape>_SaveState.p2s. FIRST is the first frame_counter to write
# (the state holds FIRST - 1); N the rows. The csv goes in a subfolder so the readers of
# context/capture/ do not take it for a bike ride.
set -e
C7=research/journal/2026-10-20-c7-c7-biplane-capture/playback
PID=$1 REC=$2 CAR=$3 KIND=$4 FIRST=$5 N=$6 OUT=$7
mkdir -p context/s149b "$(dirname "$OUT")"
WORK=$(mktemp -d -p context/s149b)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/0x00605270/$CAR/g" \
    -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and car_kind == $KIND\"/" \
    "$(dirname "$0")/car_tire.toml" > "$WORK/spec.toml"
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
