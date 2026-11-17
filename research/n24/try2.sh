#!/bin/bash
# N24: one trial. Writes TAPE from two pads' segments (make_tape2.py), plays it from the state beside it
# (TAPE_SaveState.p2s) and logs both planes until the tape ends (log2.py).
# usage: KIT_PCSX2=1 research/n24/try2.sh PID EVERY TAPE P1_SEGMENT... / P2_SEGMENT...
set -e
PID=$1 EVERY=$2 T=$3; shift 3
n=$(python3 -B research/n24/make_tape2.py context/recordings/fly_and_collide.p2m2 $T "$@" | awk '{print $2}')
first=$(python3 -B research/c16/state_frame.py ${T}_SaveState.p2s)
timeout 900 python3 -B research/n24/log2.py $first $((first + n)) $EVERY 900 &
log=$!
timeout 60 ./kit pcsx2 with -- bash research/c14/play.sh $PID $T > /dev/null
wait $log
