#!/bin/bash
# S246: play one of S149a's car tapes back on the original and capture the car's camera with
# watch/car-camera.toml. S149a's capture.sh with the camera spec, its camera and viewport addresses
# rewritten for the car. Run from the repo root, as one job, on a copy whose PINE is on:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   KIT_PCSX2=2 ./kit pcsx2 start              # note the pid; wait for ./kit pine to answer
#   KIT_PCSX2=2 ./kit pcsx2 with -- bash research/journal/2026-11-02-s246-car-camera/capture.sh \
#       PID baja 401 650 context/capture/s246/baja.csv
#
# The tapes and their save states are S149a's (context/s149a/<car>.p2m2 and <car>.p2m2_SaveState.p2s,
# from the s1 worktree). FIRST is the first frame_counter to write: baja 401, buggy 467, truck 459.
set -e
C7=research/journal/2026-10-20-c7-c7-biplane-capture/playback
PID=$1 CAR=$2 FIRST=$3 N=$4 OUT=$5
case $CAR in
  baja)  KIND=5 CAM=0x14311a0 VP=0x150a6d0 ;;
  buggy) KIND=3 CAM=0x1411450 VP=0x14dd310 ;;
  truck) KIND=4 CAM=0x14514e0 VP=0x152bc70 ;;
  *) echo "no such car: $CAR"; exit 2 ;;
esac
REC=context/s149a/$CAR.p2m2
mkdir -p context/s246 "$(dirname "$OUT")"
WORK=$(mktemp -d -p context/s246)
cp "$REC" "$WORK/play.p2m2"
cp "$REC"_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
sed -e "s/0x14311a0/$CAM/g" -e "s/0x150a6d0/$VP/g" \
    -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST and frame_counter <= $((FIRST + 26)) and car_kind == $KIND\"/" \
    watch/car-camera.toml > "$WORK/spec.toml"
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
python3 "$C7/open_only.py" "$PID" "$(realpath "$WORK/play.p2m2")"
python3 -c 'import time; time.sleep(4)'
python3 "$C7/resume_if_paused.py" "$PID"
timeout 1200 tail --pid=$rec -f /dev/null || true
wait $rec || true
python3 "$C7/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 "$C7/resume_if_paused.py" "$PID" > /dev/null
python3 "$C7/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -5
echo "work files: $WORK"
