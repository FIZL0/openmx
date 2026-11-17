# B34 — plane struck prop searches in its strike tick

2026-11-17. Task: `./kit plan B34`.

B33 made a prop that the riding bike's strike wakes search in the same collide pass. B34 does the
same for the plane: a ball the plane's rays wake now searches at once, so its ground entries land
on its list behind the plane's mirror in the strike tick, as in the original. The S241 images were
gone from every context, so I remade the plane-into-ball run on copy 4 with a new `--at` on
research/s241/strike.py: image b34at-11065 has ball 0 asleep, b34at-11066 is the strike tick, and
the rerun is word for word the first run from 11067 on. One `Sim::step` from 11065 now gives ball
0's whole list as in 11066 (every searched field); before, the ground entry was missing. Items (1)
and (2) of the brief (the woken prop's rays on other props, and on wreck hulls), the shared-seat and
fleet plane lanes, and the car's strike are still heard only, filed as follow-ups. Done; details in
`1-plane-struck-prop-searches-in-its-strike-tick-FINAL.md`.

## Files

| file | what |
|---|---|
| `crates/game-sim/src/flight.rs` | `Flight::frame_props_in`; `knocked` puts the strike on with `PropSet::strike_in` when given the pass |
| `crates/game-sim/src/lib.rs` | `Sim::step`'s plane lane passes the props' pass movers |
| `crates/openmx/src/vehicle.rs` | `Plane::step_traffic` (non-shared lane) does the same |
| `crates/game-sim/tests/prop_plane.rs` | `a_ball_the_plane_wakes_searches_in_its_strike_tick` |
| `research/s241/strike.py`, `slots.py` | `--at FRAME` (slots FRAME, FRAME+1); the copy's config dir from `ps2kit.emu` |
