# 1 — steep traverse (FINAL)

Task S117. 2026-10-27.

## What I did

- **Two held traverses of a steep side face, recorded in the original** with `watch/s58-slip.toml` at a
  quarter speed (scripts in `capture/`):
  - **steep**: S106's `tele.py 95 -2.7 -10 20 2` (S106's `side` spot: the 46° face to the right of
    first_recording's start, the bike side on, the face falling to its right). The state is saved with
    the brake held. `steep.pad`: a second's wait, then full gas with the stick 0.3 left (uphill), held
    for the rest of the capture. 250 rows.
  - **steep2**: the same with `95 -2.7 -30`, 20 ft further back along the face (48° there), into
    slot 3. Same pad (`steep2.pad`). 250 rows.
  - The captures are in `context/capture/s58-slip/`, as `steep` and `steep2` (`.csv`). The RAM images
    are `context/states/s117-steep{,2}.eeMemory.bin` (`./kit state ram 2` and `3`).
  - `capture/capture.sh` is S106's, writing beside S58's cases. `capture/slope.py` prints each wheel's
    ground slope and how much of it falls across the bike, every Nth row. `capture/same.py` lists the
    lanes that differ between two rows. `capture/pose.py` prints the bike's node rows live.
- **What the rides are** (`slope.py`). steep starts at 46° with the normal 0.72 across the bike. It
  holds the face for 72 ticks, up to 39 ft/s, as the face curves away under it (38° and 0.15 across at
  the end). Then it drops off the face, flies 30 ticks and lands at 60 ft/s. steep2 holds the 48°
  face for 95 ticks, up to 42 ft/s. Through both, the rear is spinning under the gas and the side force
  is up to 1.2 times the load at the front and 1.8 times at the rear (`*_side_per_load`). Neither
  wheel ever locks or slides on the face.
- `crates/game-sim/tests/replay.rs`:
  - `S58_CASES` gains `steep` and `steep2`.
  - `S58_OWN_STATES` names the RAM image each own-state case is replayed from. The S58 test routes
    those through S106's `replay_a_save_state_case`, because a teleported state needs its levers and
    engine state seeded. first_recording's four cases run as before.
  - `SaveState::held_controls` reads the pad's control state (`@pad0_control_state`, 16 × 0x18,
    `held` first) off the image.
  - `save_state_inputs` takes the state's own controls (`own`) for the **first tick's** buttons. S106's
    helper passes them. S58's first_recording path passes `None`, so it is unchanged.
- Filed S177 for what parts first (below).

## What the original does

- **The first tick after a load rides on the controls the state was saved holding.** Only the next
  tick reads the kit's pad. steep's state was saved with the brake held. `capture.sh` releases the
  pad before the recorder loads the state. Before the fix, the port took the next row's pad (nothing
  held) for tick 1, and steep parted at the first compared tick (tick 2, front spin 0.0814 vs
  0.0355). The results by tick-1 input:
  - With the brake on tick 1 alone, ticks 2–74 are bit-exact.
  - With the brake on ticks 1 and 2, tick 3 parts.
  - With the brake on ticks 1, 2 and 3, tick 3 parts the other way.
  - With the brake on no tick, it parts at tick 2.

  No row can show this, because the first row is tick 2 and a row's pad is the one two ticks on
  (`PAD_LEADS`). The first capture of steep pressed the gas at the load, so the gas came two ticks early
  in the port. I re-recorded it with a second's wait.
- **The tire's side law holds a steep traverse to the bit.** On both faces, every lane is the
  original's on every tick while a wheel is on the face: path, slip flags, spins, local forces and
  velocities, surfaces and the balance. That covers side loads up to 1.8 × load, wheelspin, and a
  face from 48° down to 36°.
- **What parts first is the takeoff.** On the first tick both wheels are off (steep 75, steep2 98):
  - The world velocity is still bit-exact, but the angular acceleration (`ang_hist.dx`) parts.
    steep's yaw lane is −6.607 in the original and −6.283 in the port. steep2's is −1.625 vs −1.379.
  - The angular velocity follows by about 2% (steep ang_vel 0.2095/−0.0610/0.4596 vs
    0.2059/−0.0700/0.4557).
  - The path parts by an ulp 16 and 9 ticks later. steep2's rear spinning flag parts at 134, after the
    landing.
  - The torque that differs is the airborne one: `air::airborne_control` (the stick's moment, the
    unyaw/unroll or the damped branch) or the air state's takeoff. That is not the tire, so it is
    filed as S177 with the recipe for an ee proof.

## How it was proved

- Save-state captures replayed bit for bit: `crates/game-sim/tests/replay.rs`,
  `the_slip_cases_replay_from_the_save_state`, steep and steep2 (250 rows each). Ratchets:
  - steep: path through tick 90, slip flags through 250 (every tick).
  - steep2: path through 106, slip flags through 133.
- The first-tick input was found by running the brake on ticks {}, {1}, {1,2} and {1,2,3} (a throwaway
  env switch, removed). Only {1} is bit-exact.
- S106's four cases (their captures and images copied from s2's context) still pass their ratchets
  with the state's own first-tick controls. side now parts on no lane at all.
- `./kit check -- -p game-sim --test replay`: 93 tests pass.

## Not verified / not 1:1

- **The takeoff off a steep face is not 1:1.** The angular acceleration parts on the first airborne
  tick of both rides (S177). The ratchets stop the paths at 90 and 106 for that reason.
- **No slide on the face.** Neither wheel locked or slid while on the face, so the side law's sliding
  branch on a steep face is not covered by these captures. It is covered on the flat by S58's turn
  and brake. A sliding traverse needs more speed onto the face than tele.py's standing start gives.
  Teleporting with a velocity is the way, and it was not tried.
- **Teleported states, not a human's ride.** Each state was moved in RAM by tele.py and let settle 20
  frames with the brake held. They are real ride states, but nobody rode into them.
- **Only race125p on free09's dirt (surface 13)**, one face, the stick at −0.39 (`stick l -0.3`) and
  full gas. Other bikes, materials, steeper faces (over 48°) or a held downhill stick were not
  recorded.
- **The first-tick rule** (the state's own controls drive tick 1, then the pad) is measured on steep's
  state alone, where the brake is what it shows. On S106's states it changes nothing, because the pad
  holds what the state held. On first_recording's own cases it is not applied (`None`). Whether it
  would raise or lower their ratchets was not tried, so S58's four cases keep their old input.
- **The captures and RAM images live in s1's context only.** Elsewhere the steep cases print "no
  capture" and pass unchecked.
- **Quarter speed only** (NominalScalar 0.25), as S58 and S106 recorded.

## Next

Done here. S177 is the takeoff off a steep face.
