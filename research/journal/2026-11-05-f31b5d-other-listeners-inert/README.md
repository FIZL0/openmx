# F31b5d — other listeners inert

2026-11-05. Task: `./kit plan F31b5d`.

Everything on state 4's sim that is not the bike's, shown not to matter to the bike on every row of C15
ride-2 and flip-1. The `+0x24` list holds the bike's record alone, the `+0x3c` list is empty, and the
world's force and post lists hold only the bike's, on every frame. So the only other listeners are the
five other riders' PreProcess and the tail's three other leaves. Only the rider leaf writes anything the
frame reads. Drop all of them, feed the rider's one flag word (`+0x920`, F41) from the capture, and both
replays stay bit-exact. The whole bike object then matches the full frame's, apart from the rider's
own record windows. Two tests in `crates/game-sim/tests/mxvatv_bike_ride.rs`. Details in
`1-other-listeners-inert-FINAL.md`.
