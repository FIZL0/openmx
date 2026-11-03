# B22 — the windmill's wreck entry leaves at the bike's velocity

2026-11-03. Task: `./kit plan B22`.

On B15's south3 image (the windmill throw), B19's late entry put the port's bones on the original's
row, then the bones drifted apart at ~2.2 ft a tick. The cause was not the original's entry or its
first step. In one tick the throw carries the bike 10.8 ft. The port's `riding_tick` measured
`Riding::strayed` against the bike after that move, so the riding body was started afresh on the
verdict's tick. A fresh body has no drawn history, and with none `Down::enter_from` hands every bone
the bike's velocity. The stray is now measured against the bike as it stood at the top of the tick,
so only a teleport or reset between ticks starts a new body. Now the main bone leaves the entry at
(40.4, −37.3, 121.9) ft/s against the original's (41.4, −38.6, 121.6), and the bones stay within
1.61 ft through tick 11.

Tests: `tests/ragdoll.rs a_thrown_bike_keeps_its_riding_body`, and
`tests/replay.rs the_windmill_strikes_the_ports_bones_left_at_its_face` (tightened).
