#!/bin/bash
# S158: P2's pad script for a rear strike: gas from tick 30, stick X (default full right) for STEER ticks at tick 280
# (lines P2 up on P1), off the gas and on the brake (Square) for BRAKE ticks from tick AT, then gas at
# pressure GAS (0..1) to the end.
#
#   bash research/s158/rear.sh AT BRAKE GAS [STEER [X]] > p2.pad
AT=$1 BRAKE=$2 GAS=$3 STEER=${4:-12} X=${5:-1}
echo "# S158: P2 rear strike: steer $X for $STEER ticks at 280, brake $BRAKE ticks at $AT, then gas $GAS"
echo release
echo "sleep 30t"
echo "down cross"
echo "sleep 250t"
echo "stick l $X 0"
echo "sleep ${STEER}t"
echo "stick l 0 0"
echo "sleep $((AT - 280 - STEER))t"
echo "up cross"
echo "down square"
echo "sleep ${BRAKE}t"
echo "up square"
echo "down cross $GAS"
echo "sleep 600t"
