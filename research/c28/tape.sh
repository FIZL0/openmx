#!/bin/bash
# C28: play a tape (research/c15/tape.py) from a save state with no capture, then screenshot over AT-SPI.
# /dev/uinput is root-only, so menus and rides are driven by from-savestate .p2m2 movies (C15's way).
#   KIT_CONFIG=<SLUS-21104 kit.toml> ./kit pcsx2 with -- bash research/c28/tape.sh PID TAPE.p2m2 STATE.p2s SECONDS SHOT.png
# Waits SECONDS after Play (the tape's length plus whatever should run on after it), then saves a
# screenshot to SHOT.png. The VM is left running, so `./kit pine save N` can keep where it stopped.
set -e
HERE=$(cd "$(dirname "$0")" && pwd)
C6=$(cd "$HERE/../journal/2026-10-20-c6-c6-recording2-capture/playback" && pwd)
PID=$1 REC=$2 STATE=$3 SECS=$4 SHOT=$5
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$REC" "$WORK/play.p2m2"
cp "$STATE" "$WORK/play.p2m2_SaveState.p2s"
chmod u+w "$WORK"/*            # PCSX2 opens the movie for writing: a read-only copy never plays
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Play' > "$WORK/press_play.log" 2>&1 &
python3 -B "$C6/open_only.py" "$PID" "$WORK/play.p2m2" > /dev/null
sleep 4
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
sleep "$SECS"
# PCSX2 pauses when the movie ends: resume twice, a second apart, in case the end comes in between
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
sleep 1
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B "$HERE/shot.py" "$PID" "$SHOT"
