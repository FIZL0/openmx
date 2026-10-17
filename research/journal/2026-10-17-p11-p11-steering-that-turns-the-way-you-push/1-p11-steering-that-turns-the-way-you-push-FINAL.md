# 1 — p11 steering that turns the way you push (FINAL)

Task P11. 2026-10-17.

The report: the bike steers the wrong way and will not come back up. Traced on `free06` with the gas at
0.2 and the stick pinned full right for thirteen seconds — the heading rose for four seconds and then
*fell* for four more with the stick still over, the roll sat parked at −0.738 rad for two hundred
frames, and full lock bought only about 45 degrees of heading in four seconds at 28 ft/s.

Two defects, one line each, and neither of them is a sign in the lean-to-steer arithmetic.

## What I did

1. **The steering command never reached the hinge.** `mv::pose` works out the angle the front wheel is
   asked for (`mv::steer_command`: `MaxSteerAngle * (lean * steer_scale(speed))`) and leaves it on
   `front.mv.steer_command`; the contact patch steers by `front.w.steer_angle`, a different struct's
   copy of the same word, and the only writer of *that* was `wheel::steer_integrate`, which runs only
   while the bike is being balanced on the wheel. So nothing commanded it, it stood at zero for ever,
   and the bike was never steered at all. `ride.rs`' `Ride::frame` now bridges the two, once a frame
   where the pose makes the command and only when not balancing — `front.w.steer_angle =
   front.mv.steer_command`. The heading the report watched move was the terrain moving it, which is
   why it reversed and why four seconds of full lock was worth 45 degrees.

2. **`ride.rs`' `set_axes` built the bike node's frame as a reflection.** It took row0 as
   `forward × up`; for a bike facing `+z` with up `+y` that is `(−1, 0, 0)` — the bike's *left* — and
   the three rows have determinant −1. The original's own VU0 orthonormaliser, `math::basis` (proved,
   S7/S3), builds `right = up × forward`. The consequence is not cosmetic: `mv::no_hander` reads the
   node's own roll back out of the node (`ground_roll_angle(unrotate(Node, normal))`) to lag it
   towards `MaxRollCommand * lean`, and a mirrored node hands it the negation of what it just wrote.
   The lag then fights itself and settles at `g·dt/(2 − g·dt)` of the command — a twentieth of it at
   `RollGain = 6` and a 1/60 s substep. Measured: 1.75 degrees of lean for a 40-degree command, and
   the wrong way round for the turn it was in. Fixed to `up × forward` with the up rebuilt as
   `forward × right`, the same lean reaches 28.6 degrees and leans into the turn.

3. **Tests** (`crates/game-sim/tests/ride.rs`, new section 9 — section 8 at merge, which arrived on `main` as P8’s — and the mirror in
   `crates/openmx/tests/ride.rs`):
   * `it_turns_the_way_the_stick_is_pushed` and its pad-driven twin are un-ignored — P10 wrote them
     against exactly this defect.
   * `it_holds_a_turn_for_as_long_as_it_is_held`: thirteen seconds of full right lock, second by
     second, every one of them turning right and none of them turning left. Second by second because
     an average over thirteen seconds is the one statistic that cannot tell a steady turn from a turn
     that reverses halfway.
   * `it_comes_back_upright_when_the_stick_is_centred`: five seconds of lock leans the drawn bike past
     20 degrees and holds it there; a second after the stick centres it is level, and it stays level.
   * `the_lean_goes_the_way_the_originals_does`: the handedness of lean against heading rate, read out
     of the human's recording and asserted of the port's own turn. This is the test defect 2 would
     fail.
   * `turned()` in both files now sums the per-tick heading changes instead of subtracting the first
     heading from the last. P10 wrote the subtraction because nothing turned; a bike that really holds
     full lock for four seconds comes round past half a circle, and the subtraction reads that as a
     turn the other way (it was the first failure after the bridge went in: "turning right: the bike
     turned at −0.75 rad/s").
   * A bike that leans had to be built for this: `leaning_bike()` patches `MaxRollCommand = 40` and
     `MaxRollSpeed = 25` into `ridden_bike()`. The shared `freeride-bike.phy.txt` has
     `MaxRollCommand = 0.4` — four tenths of a *degree* after the loader's multiply — and no
     `MaxRollSpeed` at all, which reads as zero and shorts `mv::no_hander`'s command to zero with it,
     so the fixture bike cannot lean a degree whatever the stick does. The fixture is left alone
     because four other tasks ride it.

## What the original does

* **One word, two owners.** The front wheel's steering angle is `front+0x8ac`. The ground step's pose
  writes it once a frame as `MaxSteerAngle * (lean * steer_scale(speed))`; the contact patch reads it
  when it builds the patch's frame; the renderer snapshots it to `+0x874` for the bars; and
  `wheel_steer_integrate` takes it over — integrating a rate into it — only while the bike is
  balanced on that wheel, which is the case `wheel::integrate`'s three branches already port. The port
  split the word across `wheel::Wheel` and `mv::Wheel` and `Wheel::to_wheel()` bridges nine fields
  between them but not this one. The bridge is in `Ride::frame` rather than in `to_wheel` because
  `to_wheel` runs per substep and the original's write is once a frame, at the pose.
* **Two things lean, and only one of them is the bike you see.** The chassis *body*'s roll rate is
  pinned on the ground by the pose's own flags — `(1, 1, !on_ground)` riding, `(1, 0, 1)` balancing —
  so a grounded body cannot roll however hard it is pushed, and nothing on the ground can right it
  either. What leans in a turn is the bike *node*: `move::no_hander` rolls it about its own forward
  towards `MaxRollCommand * lean` (widened by the donut, mapped up from a standstill by
  `MaxRollSpeed`) with `RollGain` of first-order lag, and `balance` and `no_hander` are its only
  readers. The −0.738 rad the report saw parked for two hundred frames is the shell's HUD reading
  `chassis.euler.z` — the body — and on sloped terrain that accumulates out of free pitch and yaw;
  the bike a rider watches lean is the node, and it is the node that comes back up.

## How it was proved

* **The human's recording**, `context/capture/acceptance/acceptance-freeride.csv` — the minute from the
  original, 1811 rows at 30 Hz, 1266 of them upright and 1146 consecutive pairs moving faster than
  10 ft/s. Lean is `asin(bike_xform.m01)`, heading `atan2(m20, m22)`:
  of the 806 pairs leaning more than 0.05 rad, 71% have the heading turning *into* the lean, and the
  bucket medians are monotone in the lean (−0.4 rad → +0.90 rad/s, −0.5 → +0.93, −0.7 → +0.96, and
  mirrored the other way) with the roll saturating at −0.734/+0.762 rad — 42 to 44 degrees, which is a
  `MaxRollCommand` of about 45. `the_lean_goes_the_way_the_originals_does` is that measurement as a
  test, over the same rows, against the port's own held turn tick for tick. Before defect 2 was fixed
  the port leaned the other way and would have failed it; after it, every tick of the steady part of
  the port's turn leans into it.
  This is also why `bike_xform` looks like the *node* and not the body: a body whose roll rate is
  pinned on the ground cannot reach 42 degrees of roll in a turn, and the drawn bike does.
* **The port, ridden.** `./kit check` is green at 540 tests, with the three new tests above and the two
  un-ignored ones. Thirteen seconds of held right lock on the leaning bike: the heading advances
  every second of all thirteen, the hinge stays at 23 degrees, the bike holds 26 ft/s and never
  wrecks. Five seconds then centred: lean −28.6 degrees → −0.03 degrees within 40 frames, and the
  turn stops with the hinge.
* **The platform vectors moved and say why.** A1's hundred-tick hash and A3's canonical-run hashes
  move with the node's rows, which are hashed state; regenerated with
  `print_the_vectors` and pasted in, as that file's head prescribes. `POS_Y`, `VEL_Y` and
  `FINAL_DUMP_HEAD` did **not** move: the canonical run touches no stick, so the bike's own motion is
  unchanged and only the node's rows differ. That is the shape a reflection-fix should have.

## Not verified / not 1:1

* **Not re-measured on the emulator.** The report came from a shell trace on `free06` with a human
  thumb on the stick; `./kit record --trace` cannot hold a stick, no GUI automation reaches this
  desktop's keyboard, and `openmx` is not one of this task's files. What replaced it is the capture
  (for the original's handedness) and the port's own ride tests (for the port's). The absolute heading
  rate on `free06`'s terrain at gas 0.2 is therefore still unmeasured after the fix.
* **The turn rate is not bit-exact and cannot be yet.** `ride.rs`' `patch` is P4's tire stand-in, not
  the original's tire model, so how fast a given hinge angle comes round is the port's arithmetic.
  What is asserted is the handedness, the persistence and the envelope (never sharper than the
  recording's p99 of 2.40 rad/s).
* **Full lock at the minute's top speed scrubs the speed off.** The minute's `turning right` leg comes
  in at 75 ft/s and winds down to 0.6 ft/s before the lock starts pulling it round on the spot — about
  a g of lateral load with no lean to carry it, because the body's roll is pinned on the ground. That
  may be the tire stand-in, it may be a missing lean-into-the-turn on the body, and nothing in the
  capture holds full lock at 51 mph to compare against, so the leg now asserts that the bike is still
  being ridden at the end rather than a speed floor. Someone should come back to it with the tire
  model.
* **The body does not come back up on the ground, by design.** The test for "comes back upright" is
  the node's. `air::airborne_control`'s unroll term — the body's one righting mechanism, in the air,
  with the stick centred — is not exercised here: getting the body rolled in the air needs a world and
  a stunt, and `air.rs` is not one of this task's files.
* **`set_axes`' arrangement is still the port's.** The handedness is now the original's
  (`math::basis`), but which row the original's unported orthonormalising callee trusts and which it
  rebuilds is still a choice this port makes: forward given, up rebuilt.
* **The replay fixture's `bike_xform` mapping is now suspect.** The capture's roll saturating at 42
  degrees says that column is the node, and `crates/game-sim/tests/*` replay work maps it onto the
  chassis quaternion. Not touched here — it is another task's fixture — but it cannot be both.
* **The renderer's handlebars are still unwired.** The original snapshots the hinge to `+0x874` for
  the bars; the port's renderer reads its own copy and nothing writes it. Out of scope (V-tasks).
* **P12 is this task's second half.** `plan/TODO.md` lists P12 as "the stick is not connected to the
  front wheel", which is defect 1 above; P11's own test could not be paid without it. P12 should be a
  no-op — whoever takes it should check and tick it rather than write it again. (`plan/TODO.md` also
  has two tasks numbered P11; this is the steering one.)

## Next

Nothing outstanding for the steering itself. The two things worth a task of their own are in the list
above: the scrub at full lock (needs the tire model, P4's successor) and the `bike_xform`-is-the-node
question, which is a fixture's assumption rather than this port's code.
