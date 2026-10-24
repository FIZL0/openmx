#!/usr/bin/env bash
# S106: one braked-slope case, recorded from a save-state slot at a quarter speed with a pad script
# played over it (S58's capture.sh with the slot as an argument).
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-24-s106-brake-slope-slide/capture/capture.sh CASE SLOT ROWS
#
# CASE is a pad script beside this file. The capture goes to context/capture/s106-slope/CASE.csv (a
# subfolder, so the readers of context/capture/ skip it). Run setup.py 0.25 with the copy stopped
# first, and tele.py for the standing cases' slots.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
case=$1
slot=$2
rows=$3
mkdir -p context/capture/s106-slope
./kit pad send release
./kit record watch/s58-slip.toml --state "$slot" -n "$rows" -o "context/capture/s106-slope/$case.csv" &
rec=$!
./kit pad script "$here/$case.pad"
wait $rec
./kit pad send release
