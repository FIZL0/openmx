#!/bin/bash
# R22: play a two-pad tape (research/r22/tape2.py) from a save state and capture it with SPEC
# (watch/r22-split-car.toml). C34's capture.sh with the split tape. Run from the repo root in this
# slot's copy:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 with -- bash research/r22/capture.sh PID STATE.p2s P1.pad P2.pad SPEC N OUT.csv
#
# FIRST (the first frame_counter written) is the state's own frame_counter + 1. The tape is written
# beside OUT as OUT.p2m2 (+ .ticks.csv).
set -e
C7=research/journal/2026-10-20-c7-c7-biplane-capture/playback
PID=$1 STATE=$2 P1=$3 P2=$4 SPEC=$5 N=$6 OUT=$7
mkdir -p "$(dirname "$OUT")"
TAPE=${OUT%.csv}.p2m2
python3 -I research/r22/tape2.py context/recordings/split/splitscreen2_shortcolision.p2m2 "$P1" "$P2" "$TAPE"
FIRST=$(( $(python3 -I -c "import sys, zipfile, struct; print(struct.unpack_from('<I', zipfile.ZipFile(sys.argv[1]).read('eeMemory.bin'), 0x479d04)[0])" "$STATE") + 1 ))
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
sed -e "s/^every       = 1.*/&\nstart       = \"frame_counter >= $FIRST\"/" "$SPEC" > "$WORK/spec.toml"
grep -q '^start ' "$WORK/spec.toml" || { echo 'the spec has no "every = 1" line: the start line missed'; exit 1; }
# The pad stamp as the recorder's clock, in a copy of this worktree's kit.toml (it is not touched).
sed -e 's/^vsync_addr *=.*/vsync_addr = "0x47a9d8"/' kit.toml > "$WORK/kit.toml"

python3 -B "$C7/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null || true
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
