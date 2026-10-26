#!/usr/bin/env bash
# P72: the original backing up, recorded from slot 8 at a quarter speed with back.pad played over it,
# and the emulator's picture shot twice while it backs up.
#
#   python3 -B research/journal/2026-10-25-p72-reverse-l3/capture/setup.py 0.25   # the copy stopped
#   ./kit pcsx2 start      # then wait for `./kit pine` to answer
#   ./kit pcsx2 with -- bash research/journal/2026-10-25-p72-reverse-l3/capture/capture.sh
#
# The capture goes to context/capture/p72/reverse.csv (a subfolder, so the readers of
# context/capture/ skip it), the shots to context/shots/p72/.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p context/capture/p72 context/shots/p72
./kit pad send release
./kit record watch/p72-reverse.toml --state 8 -n 300 -o context/capture/p72/reverse.csv &
rec=$!
./kit pad script "$here/back.pad" &
pad=$!
sleep 14
./kit shot context/shots/p72/orig-back-a.png
sleep 8
./kit shot context/shots/p72/orig-back-b.png
wait $pad
wait $rec
./kit pad send release
