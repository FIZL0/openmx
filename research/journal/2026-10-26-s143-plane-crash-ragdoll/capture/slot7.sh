#!/bin/bash
# Put the biplane recording's save state into this copy's slot 7 (S57's slot7.sh, for copy 1 / slot s1).
set -e
D=/home/rycs/repos/openmx-slots/s1-slots/pcsx2/s1/PCSX2/savestates
cp context/recordings/biplane.p2m2_SaveState.p2s "$D/SLUS-20767 (9866FE54).07.p2s"
