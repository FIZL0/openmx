#!/usr/bin/env bash
# S184: V70's full-lock back-up recorded from slot 8 at a quarter speed (S68's recipe and lanes).
#
#   python3 -B research/journal/2026-10-24-s68-backwards-crash/capture/setup.py 0.25   # copy stopped
#   ./kit pcsx2 start
#   ./kit pcsx2 with -- bash research/s184/capture.sh 290
#   ./kit pcsx2 stop; python3 -B research/journal/2026-10-24-s68-backwards-crash/capture/setup.py 1.0
#
# The capture goes to context/capture/s184/backup.csv (a subfolder, so the readers of context/capture/
# skip it).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
rows=$1
mkdir -p context/capture/s184
./kit pad send release
./kit record watch/s68-backwards.toml --state 8 -n "$rows" -o context/capture/s184/backup.csv &
rec=$!
./kit pad script "$here/backup.pad"
wait $rec
./kit pad send release
