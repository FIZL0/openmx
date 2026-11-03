# MD93a — untamed bike step

2026-11-03. Task: `./kit plan MD93a`.

This task ported SLUS-21701's backflip as `game_sim::handling::untamed::backflip`, and Untamed rides now use
it (`ride.rs`, beside the Untamed preload). It is SLUS-21104's flip on a reworked input:

- the fourth trick button arms it, whatever else is held;
- a bound backflip control, on a device that has one, fires it on the lip at once;
- the lip reads the stick before the limiter;
- a flip's end clears stunt ids over 48 (SLUS-21104: 40).

`tests/untamed_reworked.rs` proves it bit for bit over 20,000 generated states off the Untamed image. The
ride feeds it stand-ins for the input it has no control for yet, filed as MD93f. Details in
[1-untamed-bike-step-FINAL.md](1-untamed-bike-step-FINAL.md).
