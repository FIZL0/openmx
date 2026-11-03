# MD93 — untamed bike step

2026-11-03. Task: `./kit plan MD93`.

The task was all six of SLUS-21701's reworked bike functions plus two classifications, which is more than
one session, so I split it. This task did three of them:

- It adds `Handling::Untamed` (`--handling untamed`, the Handling row's Untamed; off by default). This is
  MX vs. ATV's handling with SLUS-21701's reworked functions wherever they are ported.
- It ports Untamed's preload, wreck entry and put-back as `game_sim::handling::untamed`.
- Each is proved bit for bit by `crates/game-sim/tests/untamed_reworked.rs` off the Untamed image. The
  test checks every word of the bike and its body, and every call out in order.
- Only the preload is in the ride. Neither SLUS-21104's wreck entry nor its put-back is either, so those
  two are proved functions only.

The rest is filed as MD93a (backflip), MD93b (spin in the air), MD93c (vehicle update), MD93d (air state
and no-hander) and MD93e (binding the analogue preload). Details in
[1-untamed-bike-step-FINAL.md](1-untamed-bike-step-FINAL.md).
