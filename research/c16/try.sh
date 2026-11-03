#!/bin/bash
# C16: one trial flight. Writes context/recordings/c16/fly.p2m2 from the segments, plays it from the state
# beside it (fly.p2m2_SaveState.p2s: slot 5's, P1 in plane 0, P2 moved by tele_p2.py) and logs the plane
# until the tape ends.
# usage: KIT_PCSX2=2 research/c16/try.sh PID EVERY SEGMENT...
set -e
PID=$1 EVERY=$2; shift 2
T=context/recordings/c16/fly.p2m2
n=$(python3 -B research/c16/make_tape.py context/recordings/fly_and_collide.p2m2 $T "$@" | awk '{print $2}')
first=$(python3 -B research/c16/state_frame.py ${T}_SaveState.p2s)
timeout 900 python3 -B research/c16/fly_log.py $first $((first + n)) $EVERY 900 &
log=$!
timeout 60 ./kit pcsx2 with -- bash research/c14/play.sh $PID $T > /dev/null
wait $log
