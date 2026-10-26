# 1 — roof landing rider collision (FINAL)

Task P147. 2026-10-26.

## What I did

- **Found the original's rider collision while riding.** It is not the ragdoll's bones or a rider
  box. The bike's own body rays (the eleven in the bike's collision files; the head ray is one of
  them) search the terrain, but only while the bike is tipped or upside down. When the bike lands on
  its roof the head ray ends under the ground, and the wreck test's hit clause throws the rider off
  with reason 6 (scenery). This does not depend on the airborne attitude clause, so it wrecks a roof
  landing under Air Wrecks On and Off alike.
- **Ported it** (`crates/game-sim/src/rider_contact.rs`: `GROUND_SEARCH_UP`, `searches_ground`,
  `ground`, `ground_hits`; `crates/game-sim/src/rider.rs` `frame_bumped`). Each riding tick, before
  the bike moves, a bike whose transform's up.y is not above 0.25 runs `contact_search::collide`'s
  terrain branch with its body rays:
  - The contacts go at the head of the bump's list, as the original's terrain entries come before its
    actor query's. They push the bike out as any contact does.
  - The same contacts head the wreck test's hit list, so the head ray's contact gives `Wreck::Scenery`.
- **Off decision: no roof clause of its own.** The original already wrecks a roof landing through the
  head ray, under both settings. Off still skips only the airborne attitude clause (P138), so nothing
  Off-specific was added.
- **Captured a roof landing in the original** (`research/p147`):
  - `make_tape.py` patches recording2. It lifts the trick buttons from field FROM and pushes the stick
    full forward to field TO, which bleeds the flip torque (C8's try 1). After that the stick is centred.
  - `roof.toml` is S138's whole-bike spec plus the flip flags, the bike's up.y and the collision
    record's +0x30/+0x34/+0x40/+0x48/+0x100 and its first entry.
  - `play.sh` is C8's play script with this spec. `start.sh` starts copy 3 with the a11y bus. `show.py`
    prints a try, and `keep.py` files one under a case name.
  - The captures are in `context/capture/p147` (git-ignored):
    - `roof`: `--push 298:320`. The torque is gone at 2536, up.y holds at −0.927 with flipping still
      1, and at 2593 the bike lands on its roof with no wheel down: reason 6. The record's first entry
      is the head ray, (−1.74e-05, 4.0637, −0.0968) → (2.98e-08, 6.6743, 0.1712), which is `air::HEAD`
      to the float.
    - `nose`: `--push 298:420`. The held stick turns the bike back past vertical, and it lands on a
      wheel at 2594 with reason 0xb.
    - Each case has states at 2530 and 2580 (`.p2s`, plus `.ee.bin` from `./kit state ram`).
  - Rebuild: `python3 -B research/p147/make_tape.py context/recordings/recording2.p2m2 context/recordings/p147/b.p2m2 --push 298:320`,
    copy recording2's `_SaveState.p2s` beside it, then
    `SAVE=2530:5,2580:6 ./kit pcsx2 with -- bash research/p147/play.sh <pcsx2 pid> context/recordings/p147/b.p2m2 2374 300 context/capture/p147/b.csv`
    with the a11y env and `WAYLAND_DISPLAY=wayland-1` exported, then `python3 -B research/p147/keep.py b roof`.
- **Tests:**
  - `tests/replay.rs`: `roof_landings_wreck_as_the_original_does_air_wrecks_on_and_off`, plus
    `SaveState::seed_flip`, which seeds the flip flags, the torque and the authority ramp that
    `seed_ride` leaves at their defaults.
  - `tests/roof_landing.rs` (new): the 0.25 threshold; Off wrecks a roof landing on the head the tick
    after the rays reach the floor; a flip onto the roof wrecks on the same tick On and Off; an upright
    landing rides out under both settings, and its body rays never search the floor.

## What the original does

- `./kit fn 0x00257a00` (the ground step) ends with
  `if (0.25 < *(float*)(*(bike+0x18)+0x94)) rec+0x30 = 0; else rec+0x30 = 1;`, where
  `rec = *(bike+0x7d0)` and +0x94 is the bike transform's up.y. Record +0x30 gates the terrain branch
  of its collide (`./kit fn 0x002708a0`, ported in S43 as `contact_search::collide`). For each ray:
  - it must point down against the transform's up;
  - its world end must be inside the terrain bounds and under the terrain point query;
  - if so, +0x40 = 1, +0x48 = 1, and a kind-0 entry with other 0 is added.
- `./kit fn 0x002522c0`'s hit loop takes the first own entry whose +0x24 (the ray end's model y) is
  above 6. Only the head ray (6.674) reaches that high. Other 0 is scenery, so reason 6.
- Upright, +0x30 is 0. Two RAM images (first_recording, front-2600) and every upright row of the
  capture show it, and the capture's rows turn to 1 from the first one with up.y under 0.25.
- A backflip cut short stays "flipping" (+0x4f8) until a wheel is down or the bike is the right way
  up. So in the original the airborne attitude clause never judges such a roof landing; the head ray
  does.
- Addresses and the capture's numbers are in `research/oracle.txt` under P147.

## How it was proved

- **Save-state capture, replayed** (`roof_landings_wreck_as_the_original_does_air_wrecks_on_and_off`):
  each case runs from the original's 2580 state with the original's pad, on free09 with the disc's
  body rays.
  - `roof`: reason 6 at 2593 under On and under Off, the original's tick exactly. up.y matches the
    capture's to six places on every tick.
  - `nose`: reason 0xb at 2594 under both, also the exact tick. Before `seed_flip` seeded the
    authority ramp (+0x4e8), the flight drifted from the first tick.
  - With the ground search switched off for one run, the roof case reproduced the report: On wrecked
    late with the wrong reason (2598, reason 8), and Off rode it out.
- **Port tests on the disc bike** (`tests/roof_landing.rs`, 4 tests): race125p.phy and its `[Wrecks]`
  limits, with the disc's body rays, over a flat floor.
- The full gate (`./kit check --all`) is the regression check for the S138 walls and nose landings,
  the S78 rider-vs-rider tests and every smooth-landing replay. It was green except for
  `one_fps` (a software-render frame budget, 130.8 ms with load average ~35, of which the sim is not
  a stage) and two lobby load flakes the gate reran green; `one_fps` passed alone (58.0 ms).

## Not verified / not 1:1

- **The flag is derived, not stored.** The original writes +0x30 at the end of the ground step from
  the transform at bike+0x18, and its collide reads it on the next tick. The port works out the same
  test from `ride.transform()` at collide time. The two agree when the transform the port collides
  with is the one the last ground step ended with, which holds on the captured ticks. On a teleport,
  a put-back or the first tick after a respawn they could differ by a tick. Not captured.
- **The ground list has its own capacity.** The original has one list (capacity +0x104) shared by
  terrain and actor entries. The port's terrain search has its own record with `CAPACITY`, and its
  entries are spliced in front of the bump's. If the terrain and the actors together overflow the
  original's list, the port keeps entries the original would drop. Not seen in any capture.
- **The rider-vs-rider gather record still has `search_ground` false.** Ground contacts reach the
  bump through `frame_bumped`, not through `rider_contact::among`'s records. Split-screen with a
  tipped bike is not captured.
- **A props-less world skips the stand-in probe.** In `ride::contacts`, when there are no props but
  there are ground contacts, the velocity-bundle stand-in is skipped (`scenery` is `Some(true)`). That
  is a port-only path; the original has no stand-in.
- **What the ground contacts do to the motion is proved only up to the wreck.** That covers the
  roof's one tick. A tipped bike that is still riding (lying on its side, its shoulder rays in the
  ground but not its head) is pushed by the same contacts, as the original's list would push it.
  There is no capture of that.
- **One roof capture and one control.** Both are on free09, from recording2's flip 1. A roll-over
  roof landing (the bike rolled rather than pitched) and a slow tip-over onto the head are not
  captured.
- **The 2530 states were saved but not replayed** (63 ticks of flight). The replay starts 13 ticks
  before touchdown, from the 2580 state.
- **The 50cc reach (`HIT_REACH_FIFTY`) is not exercised.**
- **The disc-bike tests run on a flat floor, not on the track's terrain.** The case on a disc track's
  terrain is the free09 replay.
- **One P138 doc comment is now narrower than it reads.** `tests/air_wrecks.rs`'s module doc says a
  roof landing on its flat floor is ridden out. That is still true there, because its stock bike
  carries no body rays. With the bike's collision files (every disc ride) the roof landing wrecks.
  I left P138's text as it was.

## Next

- Nothing for this task. Follow-ups could capture a rolled (not pitched) roof landing and a bike
  lying on its side still riding, to prove the ground contacts' push and not only the wreck.
