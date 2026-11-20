#!/bin/bash
# R63: play a capture's tape (research/r22/capture.sh's OUT.p2m2) from save slot STATE and save slot SLOT
# when the frame counter reaches FRAME (research/r22/play_save.sh, which wants the state beside the tape).
#
#   ./kit pcsx2 with -- bash research/r63/save_pre.sh TAPE.p2m2 STATE FRAME SLOT
set -e
TAPE=$1 STATE=$2 FRAME=$3 SLOT=$4
DIR=$(sed -n 's/^Savestates *= *//p' "$(dirname "$(./kit pcsx2 | sed -n 's/^config *//p')")"/PCSX2/inis/PCSX2.ini)
WORK=$(mktemp -d -p "${TMPDIR:-/tmp}")
cp "$TAPE" "$WORK/play.p2m2"
cp "$(printf '%s/SLUS-20767 (9866FE54).%02d.p2s' "$DIR" "$STATE")" "$WORK/play.p2m2_SaveState.p2s"
PID=$(./kit pcsx2 | sed -n 's/.*(pid \([0-9]*\)).*/\1/p')
bash research/r22/play_save.sh "$PID" "$WORK/play.p2m2" "$FRAME" "$SLOT"
