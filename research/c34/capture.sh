#!/bin/bash
# C34: play a car tape (research/c15/tape.py from a .pad) from a save state and capture it with S149b's
# car_tire.toml, the spec the car_drive.rs replays read. S229a's capture.sh with C28's tape route (no
# uinput pad here) and its keep-resuming loop. Run from the repo root, as one job, in this slot's copy:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 with -- bash research/c34/capture.sh PID STATE.p2s PAD CAR KIND N OUT.csv [EXTRA.toml]
#
# EXTRA (research/c34/balls.toml) is appended to the spec: more fields per row.
# FIRST (the first frame_counter written) is the state's own frame_counter + 1. The tape is written
# beside OUT as OUT.p2m2 (+ .ticks.csv).
set -e
C7=research/journal/2026-10-20-c7-c7-biplane-capture/playback
PID=$1 STATE=$2 PAD=$3 CAR=$4 KIND=$5 N=$6 OUT=$7 EXTRA=$8
mkdir -p "$(dirname "$OUT")"
TAPE=${OUT%.csv}.p2m2
GAME=$(python3 -I -c "print(open('context/recordings/first_recording.p2m2','rb').read()[306:561].rstrip(b'\0').decode())")
python3 -I research/c15/tape.py context/recordings/first_recording.p2m2 "$PAD" "$TAPE" --game "$GAME"
FIRST=$(( $(python3 -I -c "import sys, zipfile, struct; print(struct.unpack_from('<I', zipfile.ZipFile(sys.argv[1]).read('eeMemory.bin'), 0x479d04)[0])" "$STATE") + 1 ))
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/0x00605270/$CAR/g" \
    -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and car_kind == $KIND\"/" \
    research/journal/2026-10-29-s149b-car-tire-model/car_tire.toml > "$WORK/spec.toml"
if [ -n "$EXTRA" ]; then { echo; cat "$EXTRA"; } >> "$WORK/spec.toml"; fi
grep -q '^start ' "$WORK/spec.toml" || { echo 'car_tire.toml changed: the start line missed'; exit 1; }
# The pad stamp as the recorder's clock, in a copy of this worktree's kit.toml (it is not touched).
sed -e 's/^vsync_addr *=.*/vsync_addr = "0x47a9d8"/' kit.toml > "$WORK/kit.toml"
grep -q '^rand_state *=' "$WORK/kit.toml" || sed -i 's/^\[symbols\]$/&\nrand_state = "0x423b30"/' "$WORK/kit.toml"

python3 -B "$C7/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C7/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
KIT_CONFIG="$WORK/kit.toml" KIT_FRAME_STALL=300 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C7/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C7/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null
# the movie pauses the VM when it ends; keep it running until the recorder has its rows
while kill -0 $rec 2> /dev/null; do
    python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null || true
    sleep 2
done
wait $rec || true
python3 -B "$C7/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C7/resume_if_paused.py" "$PID" > /dev/null
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "first $FIRST, work files: $WORK"
