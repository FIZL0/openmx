#!/bin/bash
# R22: play a tape from its save state (research/r22/play.sh) and save to SLOT when the frame counter
# reaches FRAME (research/r22/save_at.py), under one lock:
#
#   ./kit pcsx2 with -- bash research/r22/play_save.sh PID TAPE.p2m2 FRAME SLOT
#
# save_at starts after play.sh's reset, so the old state's counter does not trip it.
PID=$1 REC=$2 FRAME=$3 SLOT=$4
bash research/r22/play.sh "$PID" "$REC" 8 - > /dev/null 2>&1 &
play=$!
sleep 5
python3 -B research/r22/save_at.py "$FRAME" "$SLOT" 50
wait $play
