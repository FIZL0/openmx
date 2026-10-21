#!/bin/bash
# S57: three save states around the bike's L1+R1 hand reset, to find what draws the rider see-through
# for a second after it: 4 before the chord, 5 just after, 6 three seconds after.
# usage (from the worktree): ./kit pcsx2 with -- bash research/journal/2026-10-21-s57-biplane-crash-and-poof/capture/ghost_states.sh
set -e
./kit pine load 8
./kit pad send "release" "sleep 2500"
./kit pine save 4
./kit pine
./kit pad send "down l1" "down r1" "sleep 150" "up l1" "up r1"
./kit pine save 5
./kit pine
./kit pad send "sleep 3000"
./kit pine save 6
./kit pine
