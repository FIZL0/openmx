#!/bin/bash
# C31: play a tape (research/c15/tape.py) from STATE.p2s and record every score card at its fixed address
# (spec.py's BASE/STRIDE mode) into context/capture/c31/NAME.csv. C28's record.sh flow: Stop, resume,
# System -> Reset (so the state load is a forward frame jump), start the recorder, Play a writable copy.
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1; ./kit pcsx2 start
#   ./kit pcsx2 with -- bash research/c31/tape.sh PID STATE.p2s NAME PAD [FRAMES [BASE STRIDE]]
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 STATE=$2 NAME=$3 PAD=$4 N=${5:-12000} BASE=${6:-0x8fc760} STRIDE=${7:-0x1e0}
OUT=$ROOT/context/capture/c31/$NAME.csv
TAPE=$ROOT/context/recordings/c31/$NAME.p2m2
mkdir -p "$(dirname "$OUT")" "$(dirname "$TAPE")"
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$PAD" "$TAPE"
python3 -I "$HERE/spec.py" 4 "$BASE" "$STRIDE" > "$ROOT/context/capture/c31/$NAME.toml"
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
cd "$ROOT"
KIT_FRAME_STALL=300 ./kit record "$ROOT/context/capture/c31/$NAME.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
# the movie pauses the VM when it ends; keep it running until the recorder has its rows
while kill -0 $rec 2> /dev/null; do
    python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
    sleep 2
done
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
grep -v 'null pointer' "$WORK/rec.log" | tail -6
