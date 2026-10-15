# S11 — s11 tricks

2026-10-15. Task: `./kit plan S11`.

Tricks: the 57-stunt table the disc ships, each player's 24-slot gesture map, the stick-and-button
gesture that names one, the clocks that decide whether a trick counts, and the trick's own half of the
landing test — ported into `crates/game-sim/src/trick.rs` and proved in `crates/game-sim/tests/trick.rs`
against the disc and the function oracle. Two things worth knowing before reading the code: no stunt
applies any rotation (the rotating moves are separate trick ids, driven by the airborne task's
torques), and the engine's shared 30,001-entry arc-cosine table needs a *double* `acos` chopped to a
single, which is why `ps2-float` gained one. The one behavioural bug the proofs caught is worth
knowing too: the throw-away wreck compares trick *ids*, and the id the name search leaves behind when
no throw-away is mapped is the backflip's own. What is still not 1:1 — the hold's wobble, which needs
the engine's shared noise sampler, and the remote-rider latch — is in the numbered file's last
section.

Details in the numbered files; the newest one's suffix says whose move it is (FINAL · PART · READY).
