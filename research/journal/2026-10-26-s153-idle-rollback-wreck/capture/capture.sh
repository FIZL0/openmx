#!/usr/bin/env bash
# S153: one idle roll-back case, recorded from slot 8 at a quarter speed with a pad script played over
# it (S68's capture.sh, its lanes, another folder).
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-26-s153-idle-rollback-wreck/capture/capture.sh CASE ROWS
#
# CASE is a pad script beside this file. The capture goes to context/capture/s153-idle/CASE.csv (a
# subfolder, so the readers of context/capture/ skip it). Run setup.py 0.25 with the copy stopped first.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
case=$1
rows=$2
mkdir -p context/capture/s153-idle
./kit pad send release
./kit record watch/s68-backwards.toml --state 8 -n "$rows" -o "context/capture/s153-idle/$case.csv" &
rec=$!
./kit pad script "$here/$case.pad"
wait $rec
./kit pad send release
