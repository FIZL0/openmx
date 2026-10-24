#!/usr/bin/env bash
# S68: one backwards case, recorded from slot 8 at a quarter speed with a pad script played over it.
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-24-s68-backwards-crash/capture/capture.sh CASE ROWS
#
# CASE is a pad script beside this file (flat, slope). The capture goes to
# context/capture/s68-backwards/CASE.csv (a subfolder, so the readers of context/capture/ skip it).
# Run setup.py 0.25 with the copy stopped first.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
case=$1
rows=$2
mkdir -p context/capture/s68-backwards
./kit pad send release
./kit record watch/s68-backwards.toml --state 8 -n "$rows" -o "context/capture/s68-backwards/$case.csv" &
rec=$!
./kit pad script "$here/$case.pad"
wait $rec
./kit pad send release
