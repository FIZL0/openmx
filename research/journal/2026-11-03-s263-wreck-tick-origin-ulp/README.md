# S263 — wreck tick origin ulp

2026-11-03. Task: `./kit plan S263`.

On the wreck tick the chassis transform's origin parted by one ulp in one lane. The writer is the wreck
body's place: the wreck part keeps the bike model's node, the same node the chassis draws from, and placing
the wreck body rewrites that node's rows and origin. Ported (`Ride::place_wreck`, and the wreck body's quat
from `spawn_turn` of the transform's rows); all five wreck ticks now agree to the bit and every S207/S191/
S162/S219 ratchet moved one tick on (truck 106, rails 99). Details in 1-wreck-place-writes-the-node-FINAL.md.
