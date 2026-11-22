#!/bin/bash
# S158: write a P2 pad script: gas from tick 30, stick X for N ticks from tick AT, then straight on.
#
#   bash research/s158/burst.sh AT N X > p2.pad
AT=$1 N=$2 X=$3
echo "# S158: P2 gas from tick 30, stick l $X for $N ticks at tick $AT, then straight"
echo release
echo "sleep 30t"
echo "down cross"
echo "sleep $((AT - 30))t"
echo "stick l $X 0"
echo "sleep ${N}t"
echo "stick l 0 0"
echo "sleep 600t"
