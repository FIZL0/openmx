# 1 — clutch (FINAL)

Task P67. 2026-10-24.

## What I did

- **Looked up the clutch in the original's table first (P73's work, so no new reading).** The original
  has a clutch control: command 0x10, which preset 0 (the shipped one) and preset 1 bind to **L1**
  (preset 2 to R2, preset 3 to L2). The port already read it: `Controls::through` takes `clutch` off
  `Command::CLUTCH`, and P40's new layout only moves Cross/Square to R2/L2, so **L1 was already the
  clutch under the new layout** before this task. What was missing was the proof and the collision
  with the reset chord. So the clutch binding itself did not change.
- **The chord, under the new layout only:** `game_sim::ride::Controls::chord_together` (ride.rs) keeps
  `respawn` only if the half that was already down went down at most `CHORD_TOGETHER_MS` = 250 ms
  before the other half's press. It reads the input layer's `held_ms`, so it is a function of the
  reports and replays the same way. It can only take a reset away, never add one. The clutch and the
  trick still reach the ride as they are; only the put-back is held back.
- **The shell applies it per seat** wherever it applies the Markers switch: a new `Seat::together`
  in main.rs, set every frame from the seat's layout (`seats.scheme(from) == Scheme::New`), and on a
  tape from `--controls` (as the Markers switch and the plane's fade already are). In `Seat::pass` it
  runs after `vehicle::hands`. Online, the choice goes in the switch byte as bit 5 of
  `vehicle::Wanted` (`together`), and `net/rides.rs` applies it when every machine steps that seat.
  A byte of 0 (an idle pad, an older build) means the original's chord.
- Docs: `Scheme::New`'s doc comment and the shell's module doc say L1 is the clutch and the chord is
  pressed together. `a_wanted_machine_goes_in_one_byte` (tests/netplay.rs) round-trips the new bit.
- **Why it is a window and not something else.** I could not separate the chord from the clutch in
  the report: L1 is both `CLUTCH` and `RESET_LEFT`, and R1 is both `TRICK_3` and `RESET_RIGHT`, in the
  same shipped table. Dropping L1 from the report for one frame would let the clutch out and then
  re-press it, which would start the chord. Moving the chord was ruled out by the brief ("L1+R1 stays
  the put-back"). So the rule is on the ride's controls, gated by the seat's layout. The window comes
  from recording2's one human chord (s29's `recording2-freeride.csv`): L1 went down on row 1211 and
  R1 on row 1212, one 30 Hz tick apart, and L1 is never held anywhere else in that recording.
- Analogue or not: the original's bike reads 0x10 as a **switch** (`bike +0x644 = 1.0/0`, oracle.txt
  P73 block), so the new layout leaves the clutch on its button and does not move it to a trigger.
  The port still passes the button's pressure through, as it does for every lever. That is P77's,
  and for a host pad (whose L1 reports 0xff) it gives the same 1.0.
- opentennis has no chord or clutch to take from (grep `chord`/`simultaneous`: nothing).

## What the original does

- **The table:** 0x10 is the clutch, read in `./kit fn 0x00200f00`'s builder. Its reader sets the
  bike's lever at `+0x644` to 1.0 while the command's value is not 0, and to 0 otherwise. The bike's
  input limiter (`bike_input_limiter`) walks `+0x65c` toward it at `ClutchInRateLimit` /
  `ClutchOutRateLimit`. That limiter is ported as `Held::step`.
- **What the clutch does** (already ported, `mv::clutch` = `bike_clutch`, `mv::engine_update` = S15's
  oracle-proved engine):
  - **Slip to rev up:** while the clutch is in, both frictions are scaled by `1 − clutch`, which is 0,
    so the crank turns against nothing and revs on `EngineInertia`. The rear wheel gets no shaft torque.
  - **Preload of the launch:** with the clutch fully in and the gas pinned, the launch ramp is re-armed
    (`clutch_timer = 0`).
  - **The launch:** once the lever is fully out, the ramp starts. Its torque multiplier starts at
    `MaxClutchTorqueScale` at a standstill (falling to 1 by `MaxClutchTorqueScaleSpeed`) and fades to
    1 over `ClutchTorqueScaleTime`. The centre of gravity moves back by `ClutchCofGTravelBack`, fading
    the same way.
  - The tutorial has a "Clutch Movie" tip beside the "Preload Movie" one (`preload_tip_popup` in
    oracle.txt), so it is a technique the game teaches.
- **The chord in the original** (P20): 0x4c (L1) pressed while 0x4d (R1) is held, or the other way
  round, *however long* the first one has been down. So on the disc a clutch held and then a trick on
  R1 puts the bike back. The original layout keeps exactly that.

## How it was proved

`under_the_new_layout_l1_is_the_clutch` in `crates/openmx/tests/ride.rs`. It runs at 60 Hz through
the shell's own `pad::Scheme` remap, the ported poll, the seat's chord rule and `Sim::step`, on the
stock bike patched for riding (`ridden_bike`). First it rests one second on an idle pad so the bike
settles from the spawn. Then:

1. **Revving with the wheel free:** R2 + L1 for 3 s from a standstill. The revs never fall and go from
   1800 to 3616 rpm. Speed stays 0.000 ft/s and rear-wheel spin stays 0.000 (the asserts are < 0.5 on
   every frame).
2. **The launch:** L1 is let go with R2 still held. The launch multiplier peaks at 2.483 (the file's
   2.5, already fading), and the bike is at 31.72 ft/s one second later.
3. **The original layout matches bit for bit:** the same run on the original layout, with Cross + L1,
   gives identical `to_bits` for rpm, speed, spin, multiplier and put-back on every frame.
4. **The chord, under both layouts:** L1 then R1 one frame later, R1 then L1, and both pressed on the
   same frame each put the bike back exactly once, on the second press. With L1 held 1 s and then R1:
   nothing under New, a put-back under Original. The same with R1 held first and L1 tapped. At the
   window's edge (60 Hz, 16 ms a frame on the input clock), L1 held 15 frames before R1 still counts
   as together, and 17 frames does not.

The existing `each_layout_binds_the_originals_commands_and_no_other` (pad.rs) still checks the
command map. I did not change it. The `./kit check` filtered run passes; the full gate runs in
`./kit done`.

## Not verified / not 1:1

- **The new layout's chord is the port's own, not the original's.** By design: under New, a held L1
  or R1 older than 250 ms no longer starts the put-back, and on the disc it does. The original layout
  keeps the disc's chord, and the test checks both.
- **250 ms is a choice, not a measurement.** It rests on one human chord (one tick apart, in
  recording2). No recording has a clutch held into an R1 trick, and no player has tried the window.
- **The plane's reset chord is unchanged.** `vehicle.rs` reads `Controls::from_pad(..).respawn` for the
  biplane, where L1/R1 are the rudder and not a clutch, so a rudder held and then the other one still
  resets the plane under either layout, as in the original.
- **A tape does not record which chord it rode.** Like the Markers switch and the plane's fade, a tape
  plays under the command line's `--controls`. A tape made under `original` and played under `new`
  (the default) differs only if it holds a lever older than 250 ms when the other shoulder goes down.
- **The held clutch was not captured on the original.** No PINE capture holds L1 at a standstill. The
  free rev and the launch are the ported `engine_update` (oracle-proved in S15) and `bike_clutch` (S3),
  run through the shell. That this bike revs slowly (1800 → 3616 rpm in 3 s) comes from the stock
  stand-in bike's `EngineInertia 0.4`; the disc's race125p was not ridden here.
- **The clutch is still a strength and not a switch** (P77, left open): a host pad whose L1 reports a
  partial pressure gives a part clutch, which the original does not have. Every pad here sends 0xff.

## Next

none — FINAL.
