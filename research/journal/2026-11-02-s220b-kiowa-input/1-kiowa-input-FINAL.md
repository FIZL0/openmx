# 1 — kiowa input (FINAL)

Task S220b. 2026-11-02.

## What I did

- `crates/game-sim/src/kiowa.rs`:
  - `kiowa::command`: the four helicopter commands (`CLIMB` 0x1a Cross, `DESCEND` 0x1b Square,
    `PEDAL_LEFT` 0x1c L1, `PEDAL_RIGHT` 0x1d R1). The d-pad uses `Command::LEFT/RIGHT/UP/DOWN`.
  - `Pad::from_sticks(stick, value)`: the input step. `Pad::from_pad(pad)` is the same through the
    shipped bindings and `AxisRange::PLAYER`, as the biplane's `PlaneInput::from_pad` is.
  - The `Pad` docs: lateral is the stick left (+), longitudinal forward (+). Both are the stick
    negated.
  - The frame: `Ride` (body, measure, crash, flown, gravity scale, throw scale, wrapper gravity,
    area, touchdown sound present, rotor angle and hidden), the `Surroundings` trait, a private
    `Lists` implementing `biplane::Dynamics`, and `Kiowa::tick(ride, world, pad, curve, acos, rng,
    dt, around) -> Crashed`.
- `research/oracle.txt`: an S220b block. It lists the writers of the pad words and the state's world
  layout, notes the touchdown rand and the stale-register finding, and names `kiowa_input` and
  `kiowa_mx_present_a/b/c` (S220a's three presentation copies, now hooked by name).
- `crates/game-sim/tests/kiowa.rs`: `the_input_step_is_the_originals` and `the_frame_is_the_originals`.

## What the original does

- **Input step**, `kiowa_input` (vtable +0x20 in both the base and MX tables). Arguments: the heli,
  the control table (stride 0x18, value at +4), the pad index, and the stick x/y in f12/f13. It reads
  commands through `pad_control_index`. In order:
  - +0x334 = command 0x1a's value, +0x338 = 0x1b's (raw values, not 0/1).
  - +0x340 = −x, +0x344 = −y. Plain `neg.s`, not cubed as the biplane's are.
  - +0x348 = +1 if 0x1c is held, else −1 if 0x1d, else 0. "Held" is value ≠ 0.0, so −0 is not held
    and a negative value is.
  - Then the d-pad overrides: 0 held → +0x340 = +1, else 1 → −1; 2 → +0x344 = +1, else 3 → −1.
  - It writes nothing else (+0x33c is the controller's).
- **Other writers** of +0x334..+0x348 (store scan of the ELF):
  - The ctor and the base unwreck zero them.
  - The controller writes only +0x33c.
  - Three MX functions on a table no player path reaches copy them from a scripted pilot at +0x99c,
    or set descend = 1 and the rest 0. Not ported.
- **The frame**. The heli's elements in the state's dynamics world (iterations 1, rate 115):
  - bodies: the heli body.
  - force: a forwarder to heli+0x9c → `kiowa_forces`.
  - preforce: heli+0x90 → `kiowa_preforce`.
  - post: heli+0x94 → `kiowa_measure_mx`.

  `kiowa_forces` calls `body_wrapper_force(heli+0x50)`. If the body is enabled, that adds the
  gravity at heli+0x140 through body vt+0x64, then calls the collision record's vt+0x80 response.
  `kiowa_measure_mx` calls `kiowa_measure`, which runs, in order:
  - `body_wrapper_post` (the pose only)
  - the figures
  - the crash test (vt+0x1b4)
  - the touchdown sound, then the blade spin and the three presentation copies

  The touchdown's `sound_trigger` with a1 0 draws one `rand()` when the sound's clip count +0x200 ≠ 0,
  after the crash test's draws. A blade throw's draws (inside the crash test's debris slot) come
  first, hence `Surroundings::crashed` sits inside post.
- **Stale register.** When +0x458 (the rotor's collision object) is null, `kiowa_wreck_check` branches
  past the `move $20, $zero` that clears its rotor-in-the-ground flag. It then wrecks on whatever s4
  its caller left. Under the world step that is the last body pointer or the constraint pass's flag,
  so in a frame it wrecks (reason 1) out of nothing. The ctor always stores the object (null only if
  a 0x180-byte allocation fails). The port keeps "no object, no strike". S220a's crash test passed
  its no-object cases only because the oracle's call starts with s4 = 0.

## How it was proved

Function oracle on S148's state (`context/states/s148-kiowa.ee.bin`), `crates/game-sim/tests/kiowa.rs`:

| Test | Cases | What it varies and checks |
| --- | --- | --- |
| `the_input_step_is_the_originals` | 10,000 | Both pads with the shipped bindings. The sticks are what a report byte gives through the ride's range, ±0, and values past ±1. The 16-entry control table holds 0, 1, pressures, negative axis halves and −0. It asserts the input slot is `kiowa_input`. It checks the five pad words bitwise, and that +0x33c is untouched. 7479 d-pad overrides, 7563 pedals. |
| `the_frame_is_the_originals` | 10,000 | The original runs `kiowa_input` (9 in 10 flown frames), then `dynamics_world_step` on a scratch world holding the state's own heli elements (found and asserted in the state's world). It varies the pad and table, flown, the hold height/timer, filter and integral, and the body (disabled 1 in 12, position near and past the terrain's edges, velocity, at rest 1 in 10, orientation, rates). It also varies the measure's last speed, alpha, beta (a quarter near ±pi), airspeed, pitch, roll and last touching flag, and the crash state (reason 0/1/13/2, down, WreckTime, clock, throw scale). Then touching, the surface and struck flags against the live masks, the contact bit, 0–3 entries, the ground height, the sound (none / no clips / one clip), the rand seed, the rotor hidden and its angle, and dt (0, random, 1/60, 1/30). Checks: the pad words; the controller's commands and loop; the rotor and aero states; every body word (21 quadwords, 4 lanes but the accelerations' xyz, plus enabled and inverse mass); the measure's speed, alpha, beta and all nine rows and the touching flag; what the crash test asked for in order; every ground point; reason, down, clock, throw scale, rand state, rotor angle; and the number of collision responses. 8498 pads read, 3392 thrown, 1713 crashed, 210 touchdown draws, 984 at rest. |

Hooked in the original and answered alike for the port:
- the collision record's response (counted)
- `body_wrapper_post`
- the wreck slot (it writes the reason and down, as in S220a), the debris and reset slots (recorded)
- `ground_query` (the case's height)
- the rotor collision's update and `collide_against_search`
- the three presentation copies and the model-node functions
- `sound_3d` (returns 0) and the fake clip's two slots

The touchdown's `touchdown_sound` and `sound_trigger` run for real on a fake one-clip list, so the
rand draw is the game's own.

- `./kit check -- -p game-sim --test kiowa`: 11 passed. Both new tests' counts were seen through a
  temporary panic, so neither skipped.
- Mutations that make the tests fail:
  - x not negated
  - held as `> 0` (red once negative values were added)
  - Right tested before Left
  - no touchdown draw
  - the crash answer not handed on
  - `wrecked` not taken from the reason (red once the test's start value was made stale)
  - no collision response
  - no rotor spin
  - the pad dropped
- `./kit audit`: only the existing `plan/DONE.md:519/:533` leaks.

## Not verified / not 1:1

- `Pad::from_pad` (the shipped table and `AxisRange::PLAYER` on a live `input::Pad`) is not run
  against the original. The oracle covers `from_sticks` with the table's values handed in, as the
  biplane's test does. Who calls the input slot, and whether the original skips it on unflown
  frames, was not traced: `tick` takes the pad as an `Option` and leaves the choice to the caller.
- The three MX scripted-pilot writers of the pad words are not ported. No player path reaches them,
  and what drives them is not known.
- The collision record's response (vt+0x80, `0x00270dc0`) is the caller's (`Surroundings::resolve`).
  The test only counts it.
- The wreck, unwreck, blade-throw and put-back effects on the body are the caller's
  (`Surroundings::crashed`; S220a1). The test hooks them in the original, so a real blade throw's
  rand draws and body changes are not in the comparison.
- `body_wrapper_post` (the pose to the transform) and the three presentation copies are not ported.
  None of them moves a simulated figure.
- The touchdown sound is only its rand draw. Its clip choice, `sound_3d` and the clip slots are
  hooked or faked.
- The stale-register wreck of a helicopter without a rotor collision object is not reproduced (see
  above). The frame test always has the object.
- The scratch world holds only the helicopter's elements. The state's second body and listeners
  (another machine) are left out, so their order relative to the heli's is not checked.
- No ride or capture is replayed: the frame is proved one frame at a time from generated states, not
  over a flight.

## Next

Nothing for S220b. S220a1 (the wreck and put-back effects) remains, and a ride replay once a Kiowa
recording exists.
