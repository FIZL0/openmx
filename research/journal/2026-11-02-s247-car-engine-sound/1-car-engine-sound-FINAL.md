# 1 — car engine sound (FINAL)

Task S247. 2026-11-02.

## What I did

- **`crates/openmx/src/audio.rs`, "the cars' engine (S247)":**
  - `CarEngine` reads a profile and its `<name>Loop1..n`, with the reader's own defaults.
  - `CarEngine::revs` and `CarEngine::key` are the frame's revs and its per-loop key lookup.
  - `CarGas` is the throttle's volume ramp.
  - `CarNote` holds one looped voice per loop, follows the car and applies the existing `doppler`.
  - `CarFrame` is what a seat hands it. `SeatFrame.car`, `SeatSound.car` and `Sound.cars` are new,
    and `Reading::cars` reads `carengine.aud`'s `[buggy]`, `[baja]` and `[monstertruck]`.
  - While a seat is in a car, the bike note, the drive-on loop, the rumble, the plane and the
    helicopter stay silent. Leaving the car stops its voices.
- **`crates/openmx/src/vehicle.rs`:**
  - `CarRide` keeps `noise: Option<SmoothNoise>` and `rng`. `CarRide::step` hands the car's step
    the sampler `noise.get_or_insert_with(|| SmoothNoise::build(rng)).at(t)` in place of `|_| 0`,
    the same pattern `Flight` uses.
  - `CarRide::engine_sound` / `EngineSound` give the voice's inputs.
- **`crates/openmx/src/main.rs`:** `seat_sound` fills `car` from `garage.car_up()`.
- **`research/oracle.txt`:** the voice's functions and words.

Taken from opentennis: nothing. It has no engine sound. The pattern is this repo's own `PlaneNote`
and `HeliNote`.

## What the original does

The car's sound setup (`./kit fn 0x003990f0`) builds a keyed-loop voice (`./kit fn 0x003a5230`) and
initialises it (`./kit fn 0x003a53d0`). The init takes five inputs:
- the engine's top revs: engine +0x130, the torque table's last rpm, which is `Drivetrain::redline`;
- a pointer to the engine's +0x138, the sound pitch in revs (`Drivetrain::pitch`);
- a pointer to the car's +0x3ec, the throttle after its rate limit;
- the body's place and velocity.

The init then reads the settings (`./kit fn 0x003a5930`) and the loops (`./kit fn 0x003a5ab0`).

Each key's rate is stored as the sound chip's pitch word, `(OriginalSampleRatePS2 + rate) × 4096 /
48000`, truncated. 4096 is 48 kHz.

The voice's frame is vtable `0x0045d420` slot 14 (`./kit fn 0x003a54c0`). Each frame it does this:
1. **The ramp.** It is on the gas when `throttle ≥ ThrottleThreshold`. On the frame that changes,
   it restarts the ramp at the on or the off ramp time.
2. **The gain.** On the gas: `1 − cut × left / on_ramp`. Off it: `1 − cut + cut × left / on_ramp`.
   `cut` is `1 − OffThrottleVolume`. Both divide by the *on* ramp.
3. **The revs.** `MaxRPM × pitch / top`, held to `[MinRPM, MaxRPM]`.
4. **The loops.** A loop is silent outside its keys (only its volume is set) and plays a key's own
   values on a key. Otherwise it interpolates between the last key under the revs and the first key
   over them: volume `(int)((lo + (int)(t·Δvol)) × gain)`, pitch `lo + (int)(t·Δpitch)`. The volume
   and the pitch word go to the loop's sound object.
5. **The tail.** The ramp runs down by dt, never below 0. Then the base sound object's frame
   (`./kit fn 0x001b6f30`) handles the 3-D placement.

On the disc every profile has `OffThrottleVolume=1.0`, so `cut` is 0. The throttle therefore
changes nothing you can hear, though the ramp is ported.

## How it was proved

`cargo test --release -p openmx --test car_engine_sound`: 16 passed (3 new, plus the mounted
modules' own tests). `--test audio --test machines`: 50 passed.

- **`the_profiles_keys_are_read_as_the_original_reads_them`**, `the_voice_keys_its_loops_on_the_cars_revs`
  and `the_gas_ramps_the_volume_as_the_original_does` run on a hand-made profile. Each pitch word,
  interpolated volume and pitch, clamp and ramp value is worked by hand in the test.
- **`a_driven_cars_voice_follows_its_revs`** drives each of the disc's three cars through
  `CarRide::step` on a flat floor with the shipped pad preset: 1 s at idle, 6 s on Cross, 2 s off.
  It hands the voice the frame `seat_sound` builds. Every tick and every loop it checks:
  - the voice's revs are `MaxRPM × pitch / top` of the car's own drivetrain;
  - the loop sounds exactly when its keys give it a volume;
  - its mixer pitch is its pitch word's rate over the sample's own.

  Across the run the revs go from 555 / 611 / 483 at idle (buggy / truck / baja) to 5082 / 5508 /
  4695 through the gear changes, then back down off the gas.
- **`the_cars_sound_pitch_samples_the_smooth_noise`** drives two copies of each car on the same pad.
  One uses the shell's step. The other has noise tables with every gradient 0, which is what the
  step was handed before. The checks:
  - body position and rpm match to the bit every tick, so the noise touches only the sound;
  - the pitches differ on 37 / 35 / 34 ticks, and only with the revs in the bottom ninth;
  - the first copy's tables were built off its generator.

## Not verified / not 1:1

- **Not compared against the original running.** No PINE capture of the voice's volumes and pitch
  words, and no recording of the original's audio. The port follows the decompiled frame. Sound is
  presentation (AGENT.md), so it is matched by reading, not bit for bit. Host `f32` is used.
- **Not heard.** I did not listen to it in the app: no session with ears here. The test checks the
  mixer's gains and pitches, not the speakers.
- **The noise's tables use the port's own seed** (`CAR_NOISE_SEED`), and each car keeps its own.
  The original builds one table set for the whole session off `rand()` wherever the session's
  generator stands, and a freeride seeds that from the wall clock (see `game_sim::noise`), so no
  seed would match it. The plane's `Flight` keeps its own tables the same way.
- **The volume scale is ours.** The voice's volume percent is mapped through the shell's
  `ENGINE_GAIN` (0.7), as the helicopter's loop is. The original's percent-to-SPU volume and its
  `AttenuationCurve=Under` / `RolloffFactor` (monster truck) are not ported: the mixer's straight
  near/far falloff is used.
- **The Doppler shift is the shell's existing `doppler()`**, the bike's and helicopter's. The base
  object's own frame (`./kit fn 0x001b6f30`) was not read.
- **Parked cars are silent.** Only a driven car is stepped, so only a driven car is heard. Whether
  the original's parked cars idle audibly was not checked.
- **The wreck does not stop the car's voice.** Whether the original silences it on a car wreck was
  not checked.
- **A zero `OnThrottleVolumeRampTime`** would divide by zero in the original. The port reads that
  as no ramp left. No profile on the disc has one.
- **The shift sound** (`car_shift_sound`) is still not played. In every profile on the disc it is
  commented out (`;[buggyShiftSound]`).

## Next

none
