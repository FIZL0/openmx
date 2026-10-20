# 1 — the biplane flies a ride (FINAL)

Task S40. 2026-10-20.

## What I did

- **Split the task.** The brief says to wire S38's body and world step and S39's wheels into one per-tick
  update. Both tasks are still open: `task/S38` and `task/S39` have no commits, and I6 (the pad mapping)
  is open too. So this session did the part that does not depend on them, and the wiring is now **S41**
  ("after S38, S39, I6"). The split is noted on S40 in `plan/TODO.md`.
- **`crates/game-sim/src/flight.rs`** (new): `Flight`, the whole plane as one value. It holds S34's
  parameters (`Joystick`, `Autopilot`, `Engine`, `Aero`), a `Body` (position, velocity, quaternion and
  angular rate), the pad, the surfaces, the measure's `Attitude` and `Air`, and the crash word and timer.
  - `Flight::new(&phy::Plane)` builds it off `biplane.phy` the way the loader and the constructors do.
  - `Flight::frame(dt, pad)` is the per-tick hook. Today it runs only the controls' preforce half.
  - `impl State for Flight` names every field under `plane.*`.
- **A plane ride is a lane.** `Sim` has a new field, `plane: Option<Flight>`. `impl State for Sim`
  walks it only when it is `Some`, so a bike state's walk, and every bike hash, is byte for byte what
  it was. When it is `Some`, `Sim::step` steps the plane and not the bike: the parked bike is left where
  it stands, as P19's swap leaves it. The `Sim` size pin in `tests/hash.rs` went from 6648 to 7120. The
  fresh-state leaf count (750) did not move.
- **`the_biplane_acceptance_run`** in `crates/game-sim/tests/replay.rs`. It builds a fixture out of C7's
  capture (`context/capture/acceptance/biplane.csv`, 1,718 rows), watches 29 plane lanes (`PLANE`), seeds
  the start from row 0 (`biplane_start`), flies over Free06 (`over_free06`), and has its own ratchet,
  `BIPLANE_AGREES_THROUGH = 0`. It uses the same verdict and per-lane lines as S8 and S32, and skips with a
  line if the capture or the disc is missing.
- **`crates/game-sim/tests/flight.rs`** (new), two tests:
  - `the_plane_is_built_off_its_file_as_the_original_builds_it`
  - `a_plane_ride_is_a_lane_and_a_bike_ride_is_not_moved_by_it`

## What the original does

The plane's parameter blocks are filled by the `.phy` reader R8 named `biplane_read_phy` (its keys and
offsets are in research/oracle.txt). Two constructors add constants:
- the engine's (`./kit fn 3500a0`): the drawn prop's 217.2486 rpm at +0x44, and idle and top rpm 500
  and 2500;
- the aero model's (`./kit fn 34fa90`): the density at +0x34.

S34's journal describes the frame: preforce listeners once, `(int)(dt*115)+1` force substeps, the
integrate and constraint passes, then the measure.

The measure (`./kit fn 352960`) copies the drawn matrix's row 2 into +0x5d0..+0x5d8, which makes the nose's
`y` (+0x5d4) the drawn matrix's `m21`. That is where `biplane_start` seeds it from.

## How it was proved

- **The build.** The 61 parameter words and the true-speed flag of `Flight::new(Plane::read_bytes(<the
  disc's biplane.phy>))` are bit-exact against the live plane in `biplane.p2m2_SaveState` (0x00c44a40).
  These are the joystick's 11, the autopilot's 6, the engine's 8 and the aero's 36.
- **The lane.** A bike `Sim` has no `plane.*` leaf. A `Sim` with a plane hashes differently, names its
  fields, and steps the plane without moving the bike's position or velocity bits.
- **The bike is unmoved.** `./kit check` gives TESTS PASSED, with every pinned bike hash and tape included
  (`tests/platforms.rs`, `tests/hash.rs`, both bike acceptance runs).
- **The ground.** The plane's first row (drawn origin at 2303.71, 135.09, 2266.56, `on_ground` 1) is
  0.095 over Free06's heightfield. `over_free06` asserts it is within 2 units, so the fixture is flown over
  the right area.
- **The engine column.** `prop_turn` on row 0 equals the constructor's 217.2486 to the bit. That checks the
  column is the engine object the spec names.
- **The verdict on this worktree:** tick 0 agrees on all 29 lanes. The run parts at tick 1 on
  `plane.body.pos[0]` (2303.7048 vs 2303.7058). The plane is creeping at 0.64 ft/s and the port has no
  body integrator. Per lane:
  - throttle, engine power and rpm hold through tick 24, until the human's first Cross;
  - the controls' elevator holds through 110, the aileron through 113, the rudder and the tail wheel
    through 263, and the brake through 1388 (hands off, the controls hold them where the human's
    pad has not moved them yet);
  - `grounded` holds through 139 (the first takeoff is at 140), and `crash` through 692 (the first crash
    is at 693);
  - everything the body or the measure writes parts at tick 1.

## Not verified / not 1:1

- **The plane does not fly.** `Flight::frame` runs `biplane::controls` and nothing else. There is no
  autopilot, engine or aero in a substep, no rigid body or world step (S38), no tires, collision or crash
  test (S39), and no measure (S38). That is why the ratchet is 0. All of it is S41.
- **The pad is not read.** I6's mapping from the pad to the plane's eight commands is not ported, so
  `Sim::step` flies the plane with `Pad::default()`. The recorded pad reaches the `Sim`'s input layer and
  stops there.
- **The true-speed flag is set by hand.** In `Flight::new`, `true_speed: true` matches the live plane,
  but the aero constructor writes 0 there. I did not find the code that sets it to 1. I searched the
  constructors, the loader and the plane's init path, and none of them stores to aero+0x30.
- **The preforce `dt` is assumed to be the frame's `dt`.** The world step hands its preforce listeners
  its own `dt`, and that `dt` was not traced back to the frame's.
- **Some start values are not captured and are left at the build's values:**
  - the engine's noise clock (0);
  - the joystick's ninth word (+0x388, not the controls');
  - the wreck check's other words (`crash_down` is captured but not in the port's struct).
  The note (`eng_sound`) is seeded, but stepping it needs the smooth noise, which is unported (S34).
- **The pad lanes are not watched.** `thr_up` .. `rud_cmd` are seeded from row 0, but the frame overwrites
  them with the hands-off pad, so watching them would only measure I6's absence.
- **The bike in a plane ride is `Sim::new`'s stand-in, not the parked Race 125.** It is never stepped, so
  nothing reads it, but its lanes are not the capture's `bike_*`.
- **P19's shell `Plane` (crates/openmx/src/vehicle.rs) is not switched to `Flight`.** The shell's
  biplane still stands where it was parked. That move belongs with S41, once a frame moves the plane.
- **Free06 is handed to the replay but the plane step does not ask it anything** (no tires yet). Only
  the area's identity is checked, against the first row.
- **The RNG is not drawn by the plane.** If the wreck check or the noise draws from `rand()`, a later
  ratchet will show it.
- **`./kit audit` fails on two lines of `plan/TODO.md` (S38's and S39's addresses).** Those lines came in
  with S34's commit on main, not this one. I left them alone because they are not this task's text.

## Next

S41: port S38 and S39 first. Then fill `Flight::frame` in the original's order and raise
`BIPLANE_AGREES_THROUGH`. The per-lane lines say where to look first: the body's position, velocity and
rate at tick 1.
