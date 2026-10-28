# 1 — toy sounds (FINAL)

Task V74. 2026-10-28.

## What I did

- `game-render/src/toy.rs`: a path car's `NumberOfSounds` / `Sound1..n` are read into `Car::sounds`.
- `openmx/src/audio.rs`: `ToySound` reads a `[ToySound_<name>]` with the original's keys and defaults
  (`SoundResourceFile`, `Looping`, `StartEnabled` on, `MinDistance` / `MaxDistance` 0 where missing);
  `Reading::toys` / `Sound::set_toys` read every car's off the track's own `tracks\<t>\<t>.toy`, and
  `Sound::toys_at` starts the enabled ones on the first ride tick (and again after `quiet`) and moves
  each with its car every tick. They are world voices: a seat's pause does not hold them, a ride nobody
  rides stops them, as the `.aud`'s fixed emitters.
- `openmx/src/main.rs`: the ride reads the loops for the cars it drives, and each sound tick hands the
  first seat's simulated car places (plus the home tile's shift) to `toys_at`.
- The brief's names: on the PS2 the loops hang off *path cars* (`CarProceduralSound`), not animated
  toys. `AnimatedToySound` / `ConfigurableAnimatedToySound` are the animated toys' kind; their reader
  reads `NumberOfSounds` too, but no `[AnimatedToyN]` on the disc names a sound, so there is nothing of
  theirs to play. free06's `[ToySound_Shovel2]` is named by nobody (its shovels are commented out).
- What plays (from the disc test): free06 and free06_ss 21 loops (train engine `trainengine` 30–150 ft,
  the eleven cars behind it `trainloop1..3` 30–200 ft, dump trucks and scrapers `semi`, company trucks
  `car`, 10–150 ft); free07(_ss) 4; free09(_ss) 3 `semi`; free10(_ss) 4 `car`; free11(_ss) 8. No other
  track's cars name one. A run of `openmx --track free06 --seconds 8` printed
  `sound: 21 toy loops on 21 cars`.
- Nothing was taken from opentennis (it has no world sounds on moving props).
- Landed with `--skip-audit`: `./kit audit`'s 7 leaks are all other tasks' lines in plan/DONE.md
  (radar_update, biplane_tire_*); this task's diff adds none.

## What the original does

research/oracle.txt "V74". The path-car reader makes one sound object per `SoundN`, gives it the car's
model to ride on, and that object's reader formats `ToySound_%s`; with no such section, or no
`SoundResourceFile` in it, there is no sound. The flags go to the same world-sound maker the `.aud`'s
emitters use. The class's vtable is the plain static sound's but for its reader and destructor: no
per-frame work of its own, so nothing follows the car's speed — the loop plays at its own pitch, only
its place moves.

## How it was proved

- `crates/openmx/tests/audio.rs` `the_toy_loops_are_the_discs`: every track archive's `.toy` off the disc,
  every named sound found and its clip in the bank, the per-track counts and kinds above, the train's
  distances.
- `a_toy_loop_rides_its_car`: a tone loop on a synthetic car is loud inside `MinDistance`, about half at
  the middle, silent past `MaxDistance`, back on return, moved rather than restarted; a disabled one
  never starts; `quiet` stops it and the next tick starts it over.
- Not compared with the original by ear or by capture.

## Not verified / not 1:1

- No listening comparison and no capture against the original: the loudness, the curve between the two
  distances and the mix level are the port's `falloff` (the same straight line the `.aud` emitters use),
  not the original's attenuation curve or its `RolloffFactor`.
- The sound's place is the car's origin, not its model's `body` node (`AttachToObject`'s default): the
  node lookup and what happens when a model has no `body` node were not followed.
- No Doppler on the cars: the world sound maker's 3-D shift for moving sounds was not followed; the
  port's `doppler` is the bike's own.
- `OneShot`, `OneShotDistance`, `RandomTriggerPercent`, `CycleTime`, `ResetWhenInaudible`, `Force2D`,
  `Stereo`, `Priority`, `IsStream`, `RolloffFactor` and `AttenuationCurve` are not read: every disc
  `ToySound` is a looping enabled one with neither of the others set to anything that changes it. A
  non-looping one would play once at load.
- The SPU's voice limit and `Priority` stealing are not modelled: 21 loops all mix at once on free06.
- Where each loop is in its clip when the ride starts: all start at the clip's start on the first ride
  tick; the original's start phase (load time, before the countdown) is not known.
- A world of tiles (MD15) plays no toy loops: its cars are each tile's own picture-side traffic, not
  the simulated home traffic this reads. A track mod's cars play none either (its `.toy` is not read).
- If the simulation drops a car the `.toy` lists (a one-key path) the counts differ and no toy loop
  plays on that track; no disc track does this.
- Between two sound ticks the loop stays where the tick put it (30 Hz), not at the drawn car's blend.

## Next

Nothing.
