# S343 — truck pose against the original

2026-11-17. Task: `./kit plan S343`.

The monster truck's pose step decoded (`./kit fn 0x003e07b0`) and written as `truck()` in
`crates/game-render/src/car.rs`, as S264 did the buggy's. Every node of first_recording's parked truck now
matches the original's own matrices within 0.0005 in a turn row and 0.002 ft in place (before: drive lines off
by up to 0.056, lower swingarms by 0.046). Test: `crates/game-render/tests/car.rs`
`against_the_original::the_trucks_pose_is_the_originals`.
