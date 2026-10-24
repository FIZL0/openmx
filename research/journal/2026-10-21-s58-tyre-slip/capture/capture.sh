#!/usr/bin/env bash
# S58: one slip case, recorded from slot 8 at a quarter speed with a pad script played over it.
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-21-s58-tyre-slip/capture/capture.sh CASE ROWS
#
# CASE is a pad script beside this file (launch, turn, brake, traverse). The capture goes to
# context/capture/s58-slip/CASE.csv (a subfolder, so the readers of context/capture/ skip it).
# Run setup.py 0.25 with the copy stopped first.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
case=$1
rows=$2
mkdir -p context/capture/s58-slip
./kit pad send release
./kit record watch/s58-slip.toml --state 8 -n "$rows" -o "context/capture/s58-slip/$case.csv" &
rec=$!
./kit pad script "$here/$case.pad"
wait $rec
./kit pad send release
