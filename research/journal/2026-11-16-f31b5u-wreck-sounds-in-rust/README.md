# F31b5u — wreck sounds in rust

2026-11-16. Task: `./kit plan F31b5u`.

The rider leaf's last interpreted calls on flip-1 (rand, the wreck cue's busy / made playing, the sound's
busy and play: 1,823 instructions) now run in Rust (`handling/sound.rs`, switch `wreck sound`). The heap and
the out Fade were already at 0. An oracle over 15,000 cases (28 paths) matches the original; ride-2 runs with
0 interpreted instructions and flip-1 is down to WreckTest's 32,092 (F31b5t). Gaps: see 1-…-FINAL.md.
