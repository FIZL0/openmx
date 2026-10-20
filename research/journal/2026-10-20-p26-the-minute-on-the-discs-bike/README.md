# P26 — the minute on the discs bike

2026-10-20. Task: `./kit plan P26`.

A second minute in `crates/game-sim/tests/ride.rs`, section (11): the disc's own `race125p.phy` on the
`125_faisst`'s wheels, over a flat floor at the captures' 30 Hz tick, judged against P15's bands for the
original in the capture's own units. It skips without the disc and ships `#[ignore]`d, because it fails
today. The launch and the brake's first 0.4 s fall inside the original's bands. The per-band drive, top
speed, coast and the speed a turn keeps do not. Details in `1-…-FINAL.md`.
