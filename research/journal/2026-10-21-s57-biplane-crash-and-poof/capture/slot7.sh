#!/bin/bash
# Put a human recording's save state into this copy's slot 7 (the kit's tools never write 7), or slot 8.
# usage: slot7.sh [RECORDING [SLOT]]   default: biplane 7; S57 used `slot7.sh first_recording 8` for the bike
set -e
D=/home/rycs/repos/openmx-slots/s5-slots/pcsx2/s5/PCSX2/savestates
SRC=context/recordings/${1:-biplane}.p2m2
mkdir -p "$D"
cp "${SRC}_SaveState.p2s" "$D/SLUS-20767 (9866FE54).0${2:-7}.p2s"
ls -la "$D"
