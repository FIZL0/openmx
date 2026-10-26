#!/usr/bin/env bash
# P72: the original backing up, shot while it does (no capture): slot 8 loaded, back.pad played.
#
#   ./kit pcsx2 with -- bash research/journal/2026-10-25-p72-reverse-l3/capture/shots.sh
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p context/shots/p72
./kit pad send release
./kit pine load 8
./kit pad script "$here/back.pad" &
pad=$!
sleep 12
./kit shot context/shots/p72/orig-back-a.png
sleep 6
./kit shot context/shots/p72/orig-back-b.png
wait $pad
./kit shot context/shots/p72/orig-released.png
./kit pad send release
