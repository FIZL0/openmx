# S16 — the wheel

2026-10-15. Task: `./kit plan S16`.

The three steps the bike's ground frame drives each wheel through — the suspension ray that looks for
the ground under the wheel and picks between its answer and the collision sweep's, the tire forces
that come out of the contact it settled on, and the wheel's own spin and steering — ported into
`crates/game-sim/src/wheel.rs` and proved against the original over ten thousand generated wheels
each, every word of the wheel object and of the chassis under it. The ten fields `wheel_clear`
empties, which `move.rs` had to keep by offset, are named here. What is hooked rather than ported,
and the three places the original's own arithmetic reaches nothing the proof can see, are in
`1-the-wheel-FINAL.md` under **Not verified / not 1:1**.
