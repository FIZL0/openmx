# S255 — riding ragdoll at rest

2026-11-02. Task: `./kit plan S255`.

The Deck's split screen spends a big share of sim in the riding ragdoll's solve. The original does nothing
special for a rider at rest: `rider_physics_update` steps the world whenever the body is placed, and
`dynamics_world_step` has no sleep and no early out, so every tick gets all three constraint passes. In the
port no rider is ever bit-still (the bikes creep or roll), so a skip that stays exact would never fire, and
none was built. A lossy rest check is proposed in 1-riding-ragdoll-at-rest-FINAL.md and parked as F22 for the user's ruling. What
landed is exact: ps2-float's five four-lane op bodies are now `#[inline(always)]`. LLVM had been calling
them out of line. That makes the riding step 20% faster (0.30 → 0.24 ms on `bench_riding_step`), and sim
in the 2P headless shell 7–8% faster. Checked: the 1203-tick riding replay against the original is bit
for bit, ps2-float's suites pass, and `cargo test -p game-sim --no-fail-fast` gives 1061 passed and 1
failed (the_humans_tricks, the known red on C6's capture in s1). Not measured on a Deck.
