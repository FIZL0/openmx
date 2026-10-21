# 1 — biplane plane relative (FINAL)

Task P69. 2026-10-21.

## What I did

- `crates/openmx/src/pad.rs`: `PlaneScheme` now has three schemes.
  - `New` is still the default, but it now means **relative to the plane**. The report reaches the plane
    exactly as the pad sent it, and `PlaneScheme::every_bank()` tells the plane's step to lift the
    original's knife-edge elevator fade (see below).
  - `Screen` is P50's scheme, unchanged: upside down, the stick's y, Up/Down and L1/R1 are turned round.
  - `Original` is the disc's.

  `fly` now turns the report round only under `Screen`. `parse_list` has its own words: `new`, `screen` and
  `original`. A `controls.txt` saved before P69 says `biplane = new`, and that now reads as plane-relative,
  which is what the user asked for.
- `crates/openmx/src/vehicle.rs`: `Plane::every_bank` and `Garage::set_every_bank`. When the flag is on,
  `Plane::step` sets the flight's `joystick.min_elevator` to 1 for that frame and puts the file's value back
  afterwards. The fade is `remap(roll, ramp, off, 1, MinElevator)` clamped to its outputs, so with
  `MinElevator` at 1 the fade is exactly 1 at every roll. `Plane::reset` keeps the flag.
- `crates/openmx/src/main.rs`: before each pass, each seat sets `set_every_bank` from its scheme. That happens
  next to P50's `fly`. On a `--play` tape it comes from `--biplane-controls`' first seat. `--biplane-controls`
  takes `new|screen|original` per seat. The help text now describes plane-relative.
- `crates/openmx/src/menu.rs`: the Controls row's biplane half reads `plane relative` / `plane original`. It
  used to read `plane steady` / `plane flips`, and "flips" was wrong: the original never flips a sign. The
  menu still offers only New and Original, as the brief says. `controls.txt`'s comment explains the new
  meaning. P50's screen scheme can only be picked with `--biplane-controls screen`. P81 rewords the screen
  itself, so I left its layout alone.
- opentennis has no flying vehicle, so I took nothing from it.

## Why it is not only a pad remap

The brief asks for "remap in the pad layer as P50 does". A report cannot carry the whole thing.

**The original's stick is already plane-relative in sign.** P50's journal shows this: I6's input step and
the control law never turn a sign by attitude. Down on the stick is always "nose toward the plane's own up"
(the pilot's head). So the plane-relative report is simply the pad's report, untouched.

**The original fades the elevator out on its side.** The control law (`game_sim::biplane::controls`, S34)
multiplies the stick's elevator by a fade whenever the nose is within `|nose_y| < 0.8` of level. The disc's
`biplane.phy` sets the fade like this (read off the disc in a probe test this session):

- `ElevatorRampOffRollAngle` 30°: the fade starts here.
- `ElevatorOffRollAngle` 60°: the fade reaches `MinElevator` here.
- `MinElevator` is 0, because the file lacks the key (S33).

So between 60° and 120° of bank, the stick's y and the d-pad's Up/Down move nothing. Between 30–60° and
120–150° they move it partly. No byte in a report can undo a multiplication by zero. So the plane-relative
scheme has a second half, a per-frame parameter that `vehicle.rs` applies. The simulation code is untouched,
and under `Original` (or `Screen`) the flag is off and the flight reads its file exactly as before.

User-visible effect: under P50's `New` the stick flipped just past knife edge (`INVERTED_MARGIN`), which is
the flip the user reported. It also went dead from 60° to 120°. Under the new default it does neither.

## What the original does

- I6's input step and S34's control law, as P50's journal describes them. The fade's angles and its 0 floor
  come from the disc's own file. The flight test prints them through the port's loader, and S33 proved that
  loader against the RAM image.
- The autopilot (`game_sim::biplane::autopilot`) is left as the original's. While the aileron stick is in its
  dead zone, the stick's elevator asks for a pitch rate, plus the coordinated-turn pitch rate for the current
  bank (the roll is held to 70°). A PI controller then chases that, clamped to `±MaxElevator`. On a bank with
  the stick centred, the controller is already at its stop pulling for the turn. That is why "pulled against
  centred" adds nothing there, under either scheme.

## How it was proved

- **Flight**: `the_default_biplane_stick_pulls_the_nose_toward_the_planes_up_at_every_bank` in
  `crates/openmx/tests/ride.rs`. It flies the disc's `biplane.phy` in the shell's own order: attitude, scheme
  elevator, `fly`, then poll and pass. Thirty seconds of gas get the plane level. Full right stick then rolls
  it, stopping at each bank in turn. From each bank I flew half a second with the stick pulled, and half a
  second with it pushed. Both are read as the nose's turn toward the plane's own up at the start.

  | bank (roll) | nose_y | plane-relative: pulled | plane-relative: pulled − pushed | original: pulled | original: pulled − pushed |
  |---|---|---|---|---|---|
  | 0 (0.0) | 0.18 | 0.1106 | 0.3165 | 0.1106 | 0.3165 |
  | 45 (45.9) | −0.06 | 0.0053 | 0.1830 | 0.0053 | 0.0053 |
  | 90 (90.3) | −0.35 | 0.1308 | 0.1186 | 0.1308 | 0.0000 |
  | 135 (135.5) | −0.53 | 0.2577 | 0.4917 | 0.2577 | 0.4917 |
  | inverted (173.1) | −0.59 | 0.3572 | 0.5476 | 0.3572 | 0.5476 |

  The test asserts:
  - Under the default, pulled turns the nose toward the plane's up at every bank.
  - The stick's say (pulled − pushed) is at least a quarter of upright's at every bank, and never less than
    the original's.
  - At 45° the original's stick has under a tenth of the default's say.
  - At 90°, with the nose within the fade's 0.8, the original's stick has exactly 0.
  - Upright, the two schemes are bit-identical.
  - The flight still holds `MinElevator` 0 after the flight.
- **The original's setting rides bit for bit**:
  - `upside_down_the_screen_biplane_controls_turn_the_elevator_and_rudder_round` (pad.rs, 20,000 generated
    reports) now checks that `Original` and `New` return the report byte for byte at any attitude.
  - `every_bank` is false under `Original`, so `Plane::step` does not touch the flight. The P50 flight test
    (`upside_down_the_screen_biplane_controls_fly_the_way_they_do_upright`) still checks, field for field,
    that the original's scheme flies the plane exactly as the raw report does. It also still passes for
    `Screen`, with its old numbers.
- **Words**: `the_biplane_schemes_read_and_write_their_words` (pad.rs) checks the default, per-seat parsing,
  `list_text`, refusals, and which scheme lifts the fade.
- **Shell smoke**: `openmx --track free06 --vehicle biplane --headless --frames 30 --biplane-controls
  new,screen,original` printed `biplane controls: new,screen,original,original,…`.
  `--biplane-controls sideways` is refused with `new, screen or original`.

## Not verified / not 1:1

- **The plane-relative default is deliberately not the original.** It lifts the original's knife-edge
  elevator fade, which the original applies by `MinElevator`/`ElevatorOffRollAngle`. That is the point of
  the task. `Original` keeps the fade.
- **It is not a pure pad remap, as the brief asked.** The fade cannot be undone in a report (see above), so
  half of the scheme is a per-frame `min_elevator` override in `vehicle::Plane::step`. As a result, a
  `--record` tape of the plane under the default does not hold the whole input. It replays the same only
  under the same `--biplane-controls` (seat 0's is used on `--play`). Under `Original` a tape is complete,
  as before.
- **The autopilot's coordinated-turn pull is kept.** Banked with the stick centred, the original's autopilot
  already holds the elevator at its stop for the turn. So at 90° the stick's say is about 0.37 of upright's
  (0.1186 against 0.3165), not all of it. Pushing the stick does now push against the turn, where under the
  original it did nothing. The user asked about the stick, not the autopilot, so I did not change the
  autopilot. If they want an even response at every bank, the next lever is the feed-forward.
- **At 45° the pulled stick barely turns the nose toward the plane's up** (0.0053 in half a second). This
  is the same under both schemes, because the elevator is already at its stop. The roll that got the plane
  there had let the nose fall, and the elevator's stop is all the plane has to pull it back.
- **"Toward the belly" was taken to mean a pull-up.** The brief says "pulls the plane's nose up toward its
  belly". The test asserts the nose turns toward the plane's own up (the pilot's head, the belly leading),
  which is the original's stick-down at every attitude and the usual pull-back. If the user meant the
  opposite sign, that is one flip of the y byte in `PlaneScheme::fly` for `New`.
- **Not flown by a person or by the original.** Flown only by the port's ported flight model on a flat
  floor (the contact search is not ported). It was not compared with a PCSX2 run: the fade is the original's
  own code (S34), and the default removes it on purpose.
- **Netplay does not carry the plane** (online.rs: no machine swap online), so the flag never has to agree
  between peers. When the plane goes online, the seat's `every_bank` has to travel with its inputs.
- **The Controls screen's labels** (`plane relative` / `plane original`) are interim. P81 rewrites that
  screen.

## Next

none — P81 words the Controls screen.
