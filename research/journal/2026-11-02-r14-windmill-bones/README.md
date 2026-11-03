# R14 — the windmill's bones

2026-11-02. Task: `./kit plan R14`.

S211's replay throws the off-map cannonball's vector, but at different ticks than the original
(8 against 14 on south3, 42 against 85 on apex). I logged the original's ragdoll bones through the
same kind of run. The original's ticks are an artifact of the capture, not of the windmill or the
bone search. research/s167_windmill.py teleports the bike, and the teleport leaves the ragdoll's
bone bodies behind, ~1,166 ft back. On the next frame the main bone is put on the bike. The riding
joints then drag the other bones across the gap at ~15,000 ft/s, and the main bone goes with them.
It crosses the map's margin, and the cannonball throws.

When the bones are moved with the bike, the original wrecks the same way. Its bones then follow the
wreck at ~130 ft/s, and nothing throws in 188 frames. The port's ride from that image throws nothing
either, and its main bone stays within 9.5 ft of the original's over 30 ticks.

Script: `research/r14_windmill_bones.py`. Test: `tests/replay.rs
the_windmill_wrecks_the_bones_with_the_bike`. S211's pins stay, with comments saying the original's
ticks are the teleport's.
