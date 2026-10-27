# 1 — plane touchdown sound (FINAL)

Task P146. 2026-10-27.

## What I did

- Read the plane's audio setup, the touchdown emitter's loader and its play (`./kit fn 353a60`,
  `./kit fn 3dcd10`, `./kit fn 3dd0a0`) and the measure that calls it (`./kit fn 352960`). I checked
  the loader's key strings in the ELF and read the disc's `machine.aud` sections.
- Captured the emitter in the original, in my copy (copy 1): `biplane.p2m2` played back with
  `capture/touchdown.toml` (`capture/play.sh`, P164's script without the video). 1718 rows, frames
  10971..12688, no gaps. The data is in `context/capture/p146/touchdown.csv` in slot s1 and is not
  committed.
- Ported it to `crates/openmx/src/audio.rs`:
  - `MPH`.
  - `MachineTouchdown`: the two sections, with the loader's defaults and its FPU multiply. Its
    `level_at` gives the level.
  - `TireTouchdown`: one per seat, ticked in `Sound::frame`'s plane branch after the crash clank.
  - `Reading::plane_touchdown`, loaded with the plane's other sounds.
  - New fields on `SeatFrame` (`touchdown`), `SeatSound` (`tires`) and `Sound` (`plane_touchdown`).
  - A seat that leaves stops its touchdown, `quiet` hushes it, and a seat's pause holds it with the
    rest of that seat's voices.
- `crates/openmx/src/main.rs` `seat_sound`: a seat in the plane hands over the body velocity's length
  on the tick `measure.grounded` rises, and `None` on every other tick.
- `crates/game-sim/src/flight.rs` `Flight::post`: a touchdown draws one `rand()` from the plane's
  generator, before the crash test, as the original's pick does.
- Tests:
  - `crates/openmx/tests/audio.rs`: `a_replayed_biplane_touchdown_sounds_as_the_original`,
    `a_split_touches_down_for_its_own_plane` and `the_plane_touchdown_is_the_discs`.
  - `crates/game-sim/tests/flight_collision.rs`: `a_touchdown_draws_one_rand`.
- Nothing taken from opentennis: it has no landing sounds to copy.

## What the original does

The plane's audio setup builds a machine emitter at plane +0x67c and loads it with the prefix
"biplane". Its position is the body's position, and its velocity is the body's velocity.

The loader works through three sections:

| Section | What the loader does with it |
| --- | --- |
| `[biplaneTireParams]` | Gates the rest: without it, nothing loads. Gives MinDistance 30 and MaxDistance 150 (defaults 0 and 1). |
| `[biplaneTireTouchdown]` | `MinLevelVelocity` 60 and `MaxLevelVelocity` 150, each multiplied by 1.46666 (mph to ft/s) on the FPU, which chops: 87.99959564 and 219.99899292. `MinLevelAttenuation` 40, and `MaxLevelAttenuation` less it (60). The list: one non-streamed, non-looping `Impact\plane_tire`. |
| `[<p>Suspension]` | Not on the disc for the plane. |

The plane's measure keeps the last grounded flag, then sets grounded to "any of the three tires is
down". When that flag rises, it calls the emitter's touchdown play with a1 0. (It also hands over the
body's shock peak rotated into the body frame, which only the suspension half reads.) The play:

1. Picks from the list with one `rand()` call, even though the list has one entry. The no-repeat rule
   applies only to two or more, and the pick never writes its "last" word (it stays -1 in the capture).
2. Sets the sound's level to `int(40 + (clamp(|v|, 88.0, 220.0) - 88.0) / 132.0 * 60)`, where `|v|`
   is `sqrt(z² + y² + x²)` of the body velocity.
3. Places it in 3D and starts it.

Nothing holds the sound off: a touchdown while the last one still sounds starts the one sound object over.

## How it was proved

- **Live capture, five touchdowns.** On the five ticks the grounded flag rose, the sound's level word
  (+0x3c) took the values below. Its +0x40 word changed on every one of the five ticks. That includes
  11748, where the level stayed 40, so the sound was started there too.

  | Tick | What happened | Level |
  | --- | --- | --- |
  | 11665 | the nose-in's wreck tick | 42 |
  | 11739 | a bounce while wrecked | 40 |
  | 11748 | the put-back | 40 |
  | 12465 | a landing at 136.5 ft/s | 62 |
  | 12650 | the put-back | 40 |

  `MachineTouchdown::level_at` gives all five levels from the captured velocities. The replay test
  feeds every tick from 11600 to 12700 through `Sound::frame`. It starts a voice on exactly those five
  ticks, at exactly those gains, and on no other tick. The 11748 restart comes 9 ticks after the
  bounce, which is shorter than the 0.324 s clip, so the restart path is exercised. The emitter's
  speeds, levels and distances read in the capture (87.99959564, 219.99899292, 40, 60, 30, 150) match
  what the port reads off the disc, to the bit. An IEEE multiply gave one bit more on both speeds, so
  the loader goes through `ps2_float::F`.
- **The disc.** `the_plane_touchdown_is_the_discs` reads 30/150, the two speeds to the bit, 40/60,
  and one clip: mono, 22050 Hz, 0.3238 s, not looping.
- **The draw.** `a_touchdown_draws_one_rand` drops the port's plane 6 ft onto flat ground. On every
  touchdown tick its generator moves exactly one LCG step, and on every other tick it does not move.
  This rule comes from the decompilation (the call is unconditional when the list is not empty). The
  capture cannot show it, because the game's global `rand()` advances by many draws every tick.
- **Split.** The seat that touches down sounds; the seat flying on, a bike's frame and a paused ride
  do not.
- The existing sim tests still pass: `flight`, `biplane`, `flight_collision` (34 tests).

## Not verified / not 1:1

- **Audible timing, loudness and the 3D falloff.** No video was filmed this time. The capture proves
  the tick and the level word. It does not prove that the sound is heard on that tick, how loud level
  42 is on the chip (the port uses gain = level / 100, as P164 does), or that the mixer's near/far
  curve matches the original's 3D sound. The original also hands the sound the body's velocity; the
  port has no doppler.
- **The restart.** The original starting its one sound object over is read from the +0x40 word
  changing at 11748. I did not hear it cut and restart in the original. The port stops the old voice
  and starts a new one.
- **Which tick the port's own plane touches down.** The port fires on its own `measure.grounded` rise.
  This is proved against the original only through the replayed inputs. Whether the port's landings,
  bounces and crashes ground the wheels on the same ticks as the original is the sim's business (S161
  for crashes). One example: the port's dive (P164) hits its tires a tick before the wreck. The
  original's nose-in had a wheel down on the wreck tick itself.
- **The put-back touchdown in the port.** The original sounds when it puts the plane back on its spawn
  (11748, 12650), because the grounded flag rises there. I did not check that the port's
  put-back/respawn path makes `measure.grounded` rise on its first measure as well.
- **The `rand()` draw is proved by the decompilation, not by a capture.** The global generator moves
  2..19+ draws a tick in the original, so a capture cannot isolate this one. Of the plane's own draws,
  only the edge throw's spins come later and read the draw.
- **`seat_sound` itself.** It is not run by a test: the shell's binary has no test harness for it. The
  audio tests feed `SeatFrame`s directly. The flag it reads (`grounded && !was_grounded`) is the one
  `a_touchdown_draws_one_rand` asserts on.
- **The suspension half** (`[<p>Suspension]`, velocity tiers) is not ported. The plane has no such
  section on the disc. Only the car and the buggy would read it, and the port has neither.
- **Online not run live.** Online seats go through the same `seat_sound` and `Sound::frame` as split
  seats, but I did not run a networked ride.
- **The pick** uses the port's dice, not the game's `rand()`. With one clip, the result is the same.

## Next

Nothing for P146.
