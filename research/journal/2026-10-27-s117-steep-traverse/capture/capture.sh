#!/usr/bin/env bash
# S117: one held-traverse case, recorded from a save-state slot at a quarter speed with a pad script
# played over it (S106's capture.sh, writing beside S58's cases).
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-27-s117-steep-traverse/capture/capture.sh CASE SLOT ROWS
#
# CASE is a pad script beside this file. The capture goes to context/capture/s58-slip/CASE.csv, where
# S58's test reads its cases. Run S106's setup.py 0.25 with the copy stopped first, and S106's tele.py
# for the slot (README.md here).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
case=$1
slot=$2
rows=$3
mkdir -p context/capture/s58-slip
./kit pad send release
./kit record watch/s58-slip.toml --state "$slot" -n "$rows" -o "context/capture/s58-slip/$case.csv" &
rec=$!
./kit pad script "$here/$case.pad"
wait $rec
./kit pad send release
