#!/bin/bash
# Is a PCSX2 save -> load round trip lossless for the 2p ride? (C12) The user's session recorded on from
# the live VM; every playback starts from a load of the .p2s. Play the tape, save a state mid-ride and
# load it straight back (the movie plays on from the loaded frame), then capture N rows from a second
# after the load. Compare against an uninterrupted playback (research/c10/compare.py 2p c12/<OUT>).
# Run from the repo root, as one job:
#
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1
#   ./kit pcsx2 with -- bash research/c12/roundtrip.sh PID TAPE WAIT N OUT
#
# WAIT: seconds of playback before the save. LOADS=0 saves without loading (the control).
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
PID=$1 TAPE=$2 WAIT=$3 N=$4 OUT=$5
WORK=$(mktemp -d)
cp "$TAPE" "$WORK/play.p2m2"
cp context/recordings/split/2p.p2m2_SaveState.p2s "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*
mkdir -p "$(dirname "$OUT")"

python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' || true
python3 -B "$C6/resume_if_paused.py" "$PID"
python3 -B "$C6/press.py" "$PID" 'System/Reset'
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2"
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID"
sleep "$WAIT"
./kit pine read @frame_counter
./kit pine save 9
[ "${LOADS:-1}" = 1 ] && ./kit pine load 9
F=$(./kit pine read @frame_counter | awk '{print $2}' | tail -1)
echo "after the round trip: frame_counter $F"
START=$((F + 60))
sed -e "s/^every       = 1/&\nstart       = \"frame_counter >= $START and frame_counter <= $((START + 26)) and p1_type == 2\"/" \
    research/c10/check.toml > "$WORK/spec.toml"
KIT_FRAME_STALL=120 ./kit record "$WORK/spec.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop'
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Pause'
grep -v 'null pointer' "$WORK/rec.log" | tail -3
echo "work files: $WORK"
