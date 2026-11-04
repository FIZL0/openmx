# F41 — rider writes into the bike

2026-11-04. Task: `./kit plan F41`.

F39 found that the tail's rider listener writes two windows of the bike's collision record (the bike's
+0x870..+0x8a0 and +0x920..+0x980), which the port does not write. F41 watched every read on both C15
replays: the bike's frame reads exactly one of those words, +0x920, and only in the flip's wreck, from
the wreck row until the put-back clock starts counting. That word is the rider's "a ragdoll part is
touching" flag, so porting it means porting MX vs. ATV's rider ragdoll world, filed as S278. Every other
byte the rider writes there is never read by anything but the rider, and nothing else ever writes them.
Details: `1-rider-writes-into-the-bike-FINAL.md`.
