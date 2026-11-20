#!/bin/bash
# R63: load slot FROM, put player 1's buggy at X Y Z turned by quaternion QX QY QZ QW (research/c34/place.py),
# let it run SECS seconds, then save to slot TO.
#
#   ./kit pcsx2 with -- bash research/r63/place_save.sh FROM TO SECS X Y Z QX QY QZ QW
set -e
C6=research/journal/2026-10-20-c6-c6-recording2-capture/playback
FROM=$1 TO=$2 SECS=$3
shift 3
PID=$(./kit pcsx2 | sed -n 's/.*(pid \([0-9]*\)).*/\1/p')
python3 -B "$C6/press.py" "$PID" 'Tools/Input Recording/Stop' > /dev/null 2>&1 || true
./kit pine load "$FROM" > /dev/null
sleep 3
python3 -B "$C6/resume_if_paused.py" "$PID" > /dev/null
python3 -B research/c34/place.py 0x017ea7e0 "$@"
sleep "$SECS"
./kit pine save "$TO" > /dev/null
sleep 3
./kit pine | grep frame
