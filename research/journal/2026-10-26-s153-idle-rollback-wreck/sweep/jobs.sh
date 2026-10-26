#!/usr/bin/env bash
# S153's idle-drop sweep: every 300 ft over x, z 1000..6000 on free09 and free06, four headings, 450 idle
# ticks from a standstill. 2312 runs; the wrecks land in wrecks.txt (lines 1-2 are an earlier format).
#
#   cargo build --release -p openmx && bash jobs.sh > wrecks.txt
here=$(cd "$(dirname "$0")" && pwd)
for t in free09 free06; do for x in $(seq 1000 300 6000); do for z in $(seq 1000 300 6000); do
  for d in 0 90 180 270; do echo "$t $x $z $d 450"; done; done; done; done \
  | xargs -P 12 -L 1 bash "$here/sweep.sh"
