# MD93c — untamed bike step

2026-11-03. Task: `./kit plan MD93c`.

This task ported SLUS-21701's bike vehicle update as `game_sim::handling::untamed::vehicle_update`. It
reports its calls as `untamed::UpdateCall`, in the `Call` style of `quad::update`. It is SLUS-21104's update
on moved fields and slots, with one addition: a remote bike (vehicle state 3 or 4) has its zeroed stick's
angle and length worked out again, off the arc cosine table.

`tests/untamed_reworked.rs` proves it bit for bit over 20,000 generated states off the Untamed image. All
nine callees are hooked, and the hooks change the state so the order of reads is checked too. Nothing in the
ride calls it yet; SLUS-21104's update is not ported either. Details are in
[1-untamed-bike-step-FINAL.md](1-untamed-bike-step-FINAL.md).
