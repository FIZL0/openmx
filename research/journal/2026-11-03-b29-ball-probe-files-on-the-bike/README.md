# B29 — the ball's probe files on the bike

2026-11-03. Task: `./kit plan B29`.

One paragraph: what this task was, and how it came out. Details in the numbered files; the newest one's
suffix says whose move it is (FINAL · PART · READY).

R27 found that the struck free09 ball leaves too low (vy 13.88 against the original's 19.31) because
its own ray probe meets the bike's frame collidable (`bikef.col`) and the port only heard that
(F27a2). The original files it as a contact: an entry on the ball's list (kind 0, naming the bike,
after the strike's mirror) and its mirror on the bike's list (kind 1, after the bike's own entry).
The props' collide pass now files it where the ridden bikes are the props' step's strikers
(`PropSet::file_ridden`), and keeps the mirror. The bike's strike (`prop_contact::strike`) puts the
mirror on the bike's list and hands the ball's body to the bike's resolve. On f27a1's image 2903 one
tick on, the ball's list, the bike's list, the ball's body and the bike's velocity are now the
original's, bit for bit. The 60-tick ride after it follows the capture to the shown decimals,
bounce included (it landed 6 ticks early before). Done; details in `1-ball-probe-files-on-the-bike-FINAL.md`.

## Files

| file | what |
|---|---|
| `crates/game-sim/src/flight_search.rs` | `bikes_probed`: `bikes_struck` with the hits |
| `crates/game-sim/src/prop_world.rs` | the collide pass files the hits (`file_on_bikes`) |
| `crates/game-sim/src/material_hit.rs` | `Ridden::file`, `Ridden::probed` |
| `crates/game-sim/src/prop_contact.rs` | `strike` files the mirrors (`probes_struck`), `Strike::probed` |
| `crates/game-sim/src/prop_set.rs` | `PropSet::file_ridden`; the pass empties `probed` |
| `crates/game-sim/src/prop_wreck.rs`, `prop_seats.rs` | who turns the filing on |
| `crates/game-sim/tests/knock_launch.rs` | `the_balls_probe_pushes_it_off_the_bike` on, lists compared |
