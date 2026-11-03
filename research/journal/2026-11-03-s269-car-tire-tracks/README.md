# S269 — car tire tracks

2026-11-03. Task: `./kit plan S269`.

A driven car (buggy, monster truck, baja) now lays tire tracks. Each car's particle list has four
`TireTrack` entries, one per tire. The bike's tire tracks (`tire_tracks.rs`, V40) lay them, reading the
car's tires through the same records the original's car loader sets up for its manager (S248). The marks
use the bike's tread and go into the bike's tracks' glow, so the GPU and software paths both draw them.
New tests in `crates/openmx/tests/car_tire_tracks.rs` cover the lists and the tire mapping, drive each
car up a slope, and ride the shell headless in the buggy. The work was not compared to a capture of the
original. Details in the numbered file.
