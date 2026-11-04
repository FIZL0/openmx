# MD93b — untamed bike step

2026-11-03. Task: `./kit plan MD93b`.

This task ported SLUS-21701's spin in the air (the 360) as `game_sim::handling::untamed::spin`, and Untamed
rides on a trick pack now use it (`ride.rs`, in place of `air::three_sixty`). It is SLUS-21104's 360 on the
backflip's reworked input (MD93a):

- the fourth trick button arms it, whatever else is held;
- the torque decays only with all four buttons clear;
- the lip reads the stick before the limiter;
- a bound 360 control fires it on the lip at once, at 1100 yaw and 200 pitch, turning the way the *last* 360 did;
- an end clears stunt ids over 48 (SLUS-21104: 40).

`tests/untamed_reworked.rs` proves it bit for bit over 20,000 generated states off the Untamed image. The
ride feeds it stand-ins, filed as MD93g. Details in [1-untamed-bike-step-FINAL.md](1-untamed-bike-step-FINAL.md).
