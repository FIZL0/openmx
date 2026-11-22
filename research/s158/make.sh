#!/bin/bash
# S158: write a two-pad tape for the 2p state from two pad scripts, with the 2p state beside it.
#
#   bash research/s158/make.sh P1.pad P2.pad NAME     -> context/recordings/split/s158/NAME.p2m2
set -e
DIR=context/recordings/split
OUT=$DIR/s158/$3.p2m2
mkdir -p "$DIR/s158"
python3 -I research/s158/tape2.py "$DIR/2p.p2m2" "$1" "$2" "$OUT"
S=_SaveState.p2s
cp "$DIR/2p.p2m2$S" "$OUT$S"
