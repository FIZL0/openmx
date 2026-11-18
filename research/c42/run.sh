#!/bin/bash
# C42: play make_pad.py's tape from the C31 race start, with the race cut to one lap, and record c42.toml
# into context/capture/c42/NAME.csv. C31's tape.sh flow (Stop, resume, System -> Reset, Play a writable
# copy), with the lap poke and a points read between the load and the recorder.
#   export WAYLAND_DISPLAY=wayland-1 QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1; ./kit pcsx2 start
#   ./kit pcsx2 with -- bash research/c42/run.sh PID NAME [ROWS]
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 NAME=$2 N=${3:-16000}
STATE=$ROOT/context/states/c31-sx01-race-start.p2s
OUT=$ROOT/context/capture/c42/$NAME.csv
TAPE=$ROOT/context/recordings/c42/$NAME.p2m2
mkdir -p "$(dirname "$OUT")" "$(dirname "$TAPE")"
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
python3 -I "$HERE/make_pad.py" > "$WORK/trick.pad"
python3 -I "$ROOT/research/c15/tape.py" "$ROOT/context/recordings/animandpump.p2m2" "$WORK/trick.pad" "$TAPE"
cp "$TAPE" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'System/Reset' > /dev/null
sleep 2
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
# Play leaves the VM paused at the state's frame: one lap ends the race (the lap line's laps >= mgr+0x94)
cd "$ROOT"
python3 -B "$HERE/poke.py" laps 1
KIT_FRAME_STALL=600 ./kit record "$HERE/c42.toml" -n "$N" -o "$OUT" > "$WORK/rec.log" 2>&1 &
rec=$!
sleep 3
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
# the movie pauses the VM when it ends; keep it running until the recorder has its rows
while kill -0 $rec 2> /dev/null; do
    python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null || true
    sleep 2
done
wait $rec || true
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$HERE/poke.py" read
grep -v 'null pointer' "$WORK/rec.log" | tail -6
