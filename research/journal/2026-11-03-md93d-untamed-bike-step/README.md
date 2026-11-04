# MD93d — untamed bike step

2026-11-03. Task: `./kit plan MD93d`.

This task classified SLUS-21701's bike air state (its PostProcess) and no-hander against SLUS-21104's. MD81a
had left both unclassified.

- **No-hander:** scheduling only, so it needs no port.
- **Air state:** reworked in one branch, out of the world. There it unwrecks the bike, which puts it back
  without the ground snap, where SLUS-21104 called the put-back with the snap. It is ported as
  `game_sim::handling::untamed::air_state`, with its calls reported as `untamed::AirCall`.

`tests/untamed_reworked.rs` proves the port bit for bit over 20,000 generated states off the Untamed image.
Nothing in the ride calls it. Details are in [1-untamed-bike-step-FINAL.md](1-untamed-bike-step-FINAL.md).
