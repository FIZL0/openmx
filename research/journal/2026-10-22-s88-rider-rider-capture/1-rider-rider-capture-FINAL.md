# 1 — rider rider capture (FINAL)

Task S88. 2026-10-22.

## What I did

- **`crates/game-sim/tests/rider_rider_capture.rs`** (new). Six tests, run on S93's RAM images through the
  EE interpreter, against S93's split captures:
  - `the_original_tick_from_each_image_is_the_contact_row`: for 3403 (long) and 5998 (short), the image
    equals the row before. One run of the level's collision node from it lands on the contact row for both
    chassis: centre, force, moment, shock, twist, velocity and rates, bit for bit. The right rider wrecks
    with reason 1 (long: P1; short: P2), and the other stays up.
  - `the_port_finds_the_originals_contacts`: `rider_contact::gather` on the two images' frames gives the
    same lists as the original's search, in the same order. Every field matches bit for bit (kind, from,
    on_body, normal, point, arms, surface, other, mover). The capture's entries (kind, other, end, normal,
    point) agree as well. Long: 4 entries a rider. Short: 2.
  - `the_port_resolves_every_rider_call_as_the_original`: a hook on the original's resolve covers all four
    rider calls in each tick (2 riders × 2 substeps). It builds the port's `Bump` from RAM as the call
    finds it and runs `Bump::resolve`, then runs the original. The comparison is bit for bit:
    - the own chassis: velocity, rates, shock, twist, force, torque;
    - the other chassis: velocity, rates, shock, twist;
    - every contact's arms.
  - `the_tick_with_the_ports_resolve_is_the_contact_row`: the port's resolve stands in for the original's
    in all four calls, writing its outputs where the original would. The tick still lands on the capture
    row for both chassis, and the same rider wrecks. So the original's wreck decision, fed the port's
    shock, still says Front.
  - `the_original_steps_the_last_player_first`: the order. See below.
  - `the_original_tick_into_the_wreck_is_row_3414`: the original's side of 3414. See below.
- **The port's player order is now the last player first.** This is S78's open question, settled against
  the images:
  - `Sim::step_all` iterates the players in reverse and returns the frames in player order.
  - `net/rides.rs` steps the seats in reverse.
  - `tests/rider_contact.rs` `online_seats_find_the_same_contacts` loops in reverse to match.
  - S54's tests in `tests/ride.rs`: `stacked()` now puts player 1 on top and the last player at the spawn,
    so each rider still meets the one above before that one has moved. Then:
    - `stacked_riders_joust` and `a_ghost_passes_through_another_rider` assert from the bottom up;
    - `two_riders_step_in_player_order` became `two_riders_step_the_last_player_first`, rebuilt by hand in
      the new order.
  - The doc lines in `tests/players/mod.rs` were updated. The N1 platform vectors did not move, because
    those four riders never meet.
- **A new RAM image, `context/states/s93-long-3413.ram`** (git-ignored, in s3's context). I made it as S93
  made the others, on copy 3 at NominalScalar 0.25:
  `./kit pcsx2 with -- bash research/s93/play_at.sh PID context/recordings/split/splitscreen.p2m2 3413:save:9`,
  then `./kit state ram 9 --out context/states/s93-long-3413.ram`. The copy needs
  `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1 QT_ACCESSIBILITY=1` (and `WAYLAND_DISPLAY`) at `./kit pcsx2 start`,
  or AT-SPI finds no app. The image's own counter reads 3413, and it equals row 3413 for both chassis.
- **`research/oracle.txt`** has a comment-only S88 block. **`plan`**: S106 (the 3414 port side).

## What the original does

- **The level's collision node** in the split free09 images is at 0x0061e720. The resolve call is
  `./kit fn 0x00270dc0` (record, first substep). Its update is `./kit fn 0x0026f800` (node, dt):
  1. every object is emptied, then every object searches, in list order;
  2. then the dynamics world;
  3. then the scene tree's children, which are the vehicles.

  Each vehicle's ground step resolves its own list at the top of each substep.
- **Order.** I traced the riders' resolve calls in each tick of all seven S93 images (both start states,
  the two rider contacts and the three biplane ones). Every one is P2 first substep, P2 second, P1 first,
  P1 second. So P2's vehicle updates whole before P1's, and P1 resolves against P2's chassis after P2's
  step has moved it. S54 read "player order" statically and said the vehicle node's parent was not traced.
  The run-time walk is the reverse. With two players, "the last first" and "P2 first" are the same thing.
- **A downed rider at 3414.**
  - P1's riding object is gone from the node's list. The wreck bike's object (a dynamics body's,
    0x01333240) is in its place.
  - P1's riding object is not resolved, but its collidable is still struck. P2's two own rays hit P1's
    frozen frame (other = P1's riding record), and their mirrors land in that record (the capture's P1
    entries are kind 1, other = P2).
  - P2 also resolves a mirrored hit from the wreck bike.
  - The tick from 3413 lands on row 3414 bit for bit for both riders. Nobody new wrecks.

## How it was proved

- **Save-state captures, run in the interpreter.** I used three images (3402, 3413, 5997) against
  `context/capture/split/splitscreen{,2}.csv`, comparing both chassis bit for bit before and after the tick.
- **Function oracle on every rider resolve call in those ticks:** 4 for each rider-rider contact, 8 in total.
- **The port's gather against the original's search** on both rider-rider images.
- **The order trace** on 7 images.
- **The tests do run.** A pass prints nothing, so I flipped one assertion in each of four tests (wreck
  reason, empty list, order). Each went red, then I put it back.
- **`./kit check`**: passed. The one red run was `four_peers_and_eight_riders_on_a_bad_network`, the known
  load flake; the rerun passed.

## Not verified / not 1:1

- **3414 is not replayed in the port.** The port takes a downed rider out of the gather, so nobody can
  strike it. The ride also has no wreck bike object (S105) whose mirror P2 resolves. Only the original's
  side is proved. Follow-up: S106.
- **The whole port tick is not compared, only the contact.** I compared the port's search and its resolve
  inside the original's tick. The rest of the port's ground step (integration, the ride's other forces)
  is not compared against the capture here. That is the ride's own acceptance (S64 and others).
- **The wreck decision is proved inside the original only.** The substitution run shows the original's
  wreck test reaching the captured verdict from the port's resolve outputs. The port's own wreck test
  (`air::wreck_test` on `g_peak`) is not run on these ticks.
- **More than two players.** The images have two. "The last player first" for 3 to 8 is an extrapolation.
  The other readings ("P2 first, then the rest in order", or an order set by node creation) cannot be told
  apart here.
- **Torn rows.** 5998's row holds P1 as P2's update left it, not as the tick ended: its centre equals
  5997's, and 5999 has moved it by two ticks' worth. The test compares P1 there and P2 at the tick's end.
  At 3403, P1's wreck reason (set inside the tick in the interpreter) shows a row late, at 3404. Both are
  readings of the capture's timing, not of the game.
- **The port's resolve read its inputs from the original's RAM** (the list and both chassis as each call
  found them). It was not fed from the port's own state of the previous tick.
- **The resolve's other side for a non-rider** (the biplanes, the wreck bike) is not compared. Those are
  S87's.

## Next

Done. S106 is the 3414 port side.
