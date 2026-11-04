# B28 — knock heard on the strike tick

2026-11-03. Task: `./kit plan B28`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

Since B27 the free09 ball knock's speed is its owner's impulse, and the port's knock went silent. The
ball's probe heard the bike one tick after the strike, when its shock peak is 0 again. The cause is
order. The original's props node walks its filed list and reads the count again at every step. The
bike is first on the list, and its strike wakes a never-woken ball, which files the ball's object at
the end of the list. So the walk collides the ball's probe in the same tick, and the material
listener hears it on the bike's frame. The port's collide pass runs before the bike's strike, so a
ball that the strike files was not searched until the next tick. `PropSet::strike` now hears a
just-filed prop's probe on the ridden bikes (`Collection::hear_filed_by_strike`), sound only. A
seeded stand-in strike tick hears it, and moves nothing bit for bit. The shell's ride on F27a1's line
plays one bbhit again (NCC 0.94, gain 0.48 against the capture's 0.49). The physics side, the
just-filed probe's own contacts in the strike tick, is filed as B32. Done; details in
`1-knock-heard-on-the-strike-tick-FINAL.md`.

## Files

| file | what |
|---|---|
| `crates/game-sim/src/prop_world.rs` | `Collection::hear_filed_by_strike` |
| `crates/game-sim/src/prop_contact.rs` | `PropSet::strike` hears what it filed |
| `crates/game-sim/tests/knock_launch.rs` | `a_struck_ball_is_heard_in_its_strike_tick` |
