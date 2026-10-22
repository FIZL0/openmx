#!/bin/bash
# S103: V29's play_camera.sh (../2026-10-21-v29-biplane-camera) with bike-camera.toml and a `start`
# that waits for the rider. Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 start                         # note the pid it prints; wait for ./kit pine to answer
#   ./kit pcsx2 with -- bash research/journal/2026-10-22-s103-camera-out-of-the-ground/play_camera.sh \
#       PID context/recordings/first_recording.p2m2 2774 1811 context/capture/s103/bike-camera.csv [RAISE]
#
# RAISE (feet), if given, is written to the camera's floor (+0x1a0) once Play has loaded the state and
# left the VM paused, before the recorder takes the one PINE connection: the human's ride never brings
# the eye within the disc's 4.8 ft of the ground, so the floor only shows with a higher one.
set -e
SPEC=$(cd "$(dirname "$0")" && pwd)/bike-camera.toml
HERE=$(cd "$(dirname "$0")/../2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 FIRST=$3 N=$4 OUT=$5 RAISE=${6:-}
mkdir -p "$(dirname "$OUT")"
WORK=$(mktemp -d)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
sed -e "s/^every       = 1 .*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and rider_type == 2\"/" \
    "$SPEC" > "$WORK/spec.toml"
sed -e 's/^vsync_addr *=.*/vsync_addr = "0x47a9d8"/' kit.toml > "$WORK/kit.toml"

python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$HERE/resume_if_paused.py" "$PID"
python3 "$HERE/press.py" "$PID" 'System/Reset'
sleep 2
if [ -n "$RAISE" ]; then
    python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
    python3 "$HERE/open_only.py" "$PID" "$WORK/play.p2m2"
    sleep 4
    ./kit pine write 0x174d950 "$RAISE" --as f32
    ./kit pine read 0x174d950 --as f32
fi
KIT_CONFIG="$WORK/kit.toml" KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
if [ -z "$RAISE" ]; then
    python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
    python3 "$HERE/open_only.py" "$PID" "$WORK/play.p2m2"
    sleep 4
fi
python3 "$HERE/resume_if_paused.py" "$PID"
wait $rec || true
python3 "$HERE/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$HERE/resume_if_paused.py" "$PID" > /dev/null
python3 "$HERE/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
python3 -c "import csv,sys; r=next(csv.DictReader(open(sys.argv[1]))); sys.exit(0 if r['rider_type']=='2' else 'first row has no rider: the movie did not load')" "$OUT"
echo "work files (the played copy, the spec, the logs): $WORK"
