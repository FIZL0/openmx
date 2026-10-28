# S162 — backward impacts vs original

2026-10-28. Task: `./kit plan S162`.

S153's sweep found two free06 spots where the port's idle bike wrecks, and this task captured the
original at both, from teleported and turned states of S89's 250:
- **The bump.** At 4000,4000 the bike rolls back into the bump at −39 ft/s. The original bails Back (4)
  at tick 206 and the port Back (4) at 205.
- **The prop corner.** At 2500,3100 the original rolls past it at 7 ft/s with no wreck, and the port
  matches it to a thousandth of a foot for 449 ticks.

`idle_roll_offs_on_free06_wreck_as_the_original_does` (tests/replay.rs) holds both. The bump replay's
drift from tick 2 is S204.
