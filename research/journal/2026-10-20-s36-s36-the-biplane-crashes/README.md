# S36 — s36 the biplane crashes

2026-10-20. Task: `./kit plan S36`.

The biplane's crash test, its wreck, the reset chord and the respawn are ported as a new module,
`crates/game-sim/src/biplane.rs`, on a `Plane` struct that carries only what those functions touch,
because S34's flight is not on main yet. The ported test applied to a capture I recorded (the human's
biplane recording played back, 1719 ticks, two crashes) wrecks on the same two ticks as the original
(11664, 12498) and on no others. Every tick it matches the original's wreck word, its wreck clock and its
gravity force bit for bit. It unwrecks the waited-out crash on the same tick (151 ticks in) and puts the
plane at the spawn with the spawn's facing, standing still, bit for bit. The map-edge crash, the
`VEHICLE` exception and the upside-down rule never happen in the capture, so they are ported from the
decompilation and proved only by unit tests. Details and what S34 has to wire in: `1-s36-the-biplane-crashes-FINAL.md`.
