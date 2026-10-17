# 1 — p10 a minute you can ride (FINAL)

Task P10. 2026-10-17.

## What I did

Three new files, and nothing else touched:

- `crates/game-sim/tests/ride.rs` — **section 7 appended** (the file's sections 1–6 are untouched): a
  scripted minute ridden from [`Controls`] built by hand.
- `crates/openmx/tests/ride.rs` (new) — the same minute, read out of a pad script and ridden through the
  shell's own report bytes, the input layer and `Sim::poll`/`Sim::step`.
- `crates/openmx/tests/ride-minute.pad` (new) — the minute in the kit's pad notation, which is also a
  script a human can send to a real machine with `./kit pad script`.

**The minute.** Ten legs, 3600 frames of 1/60 s, over a flat floor with one take-off ramp on it:

    the launch                 120   full throttle from a standstill
    flat out                  1200   held there until the speed stops climbing
    turning right              240   the stick hard over, the throttle still pinned
    straight after the right   120
    turning left               240   the stick hard the other way
    straight after the left    120
    the run-up                 180   a ramp goes in 250 ft ahead, on the bike's own heading
    the jump                   300   over it, through the air, and down
    the brake                  600   the brake pinned, the throttle shut
    stopped                    480   hands off the pad

and each leg asserts what a rider would notice, not a number the parameters decide: that it goes, that it
reaches a speed and keeps it, that it comes round the way the stick is pushed, that it leaves the ground
and lands on its wheels, that it stops and stays stopped, and that none of it wrecks. The two harnesses
produce the **same numbers frame for frame** — the per-leg line printed by `none_of_the_minute_wrecks` is
identical in both files — which is itself the statement that the pad chain delivers the hands the script
writes down.

**Three defects found. The port cannot be ridden today, and the minute says so in three places.** Each is
a line, each reproduces, none is this task's file to fix (they are P8's `ride.rs` and `move.rs`), and each
ships as an `#[ignore]`d test whose doc comment is the whole story:

1. **The steering command never reaches the hinge.** `mv::steer_command` (`move.rs:680`) works out
   `max_steer_angle * lean * steer_scale` every substep, and nothing ever writes it anywhere:
   `ride.rs`'s `patch` (~`ride.rs:753`) reads `wheel.w.steer_angle`, whose only writer is
   `wheel::steer_integrate`, which `wheel::integrate` (`wheel.rs:560-592`) runs **only when the wheel is
   `balancing`** and otherwise skips with a comment that the commanded angle stands. Nothing commands it,
   so it stands at zero. Measured: with the stick hard over for four seconds at 75 ft/s the yaw rate is
   `-0.0000023 rad/s` — not a weak turn, no turn. `grep -n steer crates/game-sim/src/ride.rs` is the
   whole proof: there is no assignment from `mv.steer_command` to a wheel.
2. **Braking cannot stop the bike.** Ten seconds of full brake takes 68 ft/s down to 27.4 and holds it
   there. 27.4 ft/s is this bike's road speed at `MinRPM` in first gear: with the clutch locked,
   `mv::engine_update` (`move.rs:889-942`) sets `rpm = road_rpm` and then clamps it up into
   `min_rpm..rev_limit`, so below about 18 mph the engine is *driving* the rear wheel against the brake.
   The engine sits at exactly 1800 rpm (`MinRPM`) for the whole brake and stopped legs, measured. The one
   rule that would let the clutch out is `ride.rs:1255`'s `if s.speed < mv::IDLE_SPEED &&
   s.gas_rate_limited == 0`, and `IDLE_SPEED` is 7.33 ft/s — a speed the bike can no longer reach.
3. **`ride.clutch_in` is a latch.** `ride.rs:1255` sets it to `F::ONE` and nothing anywhere clears it
   (`grep -n clutch_in crates/game-sim/src/ride.rs`: a write, a read into `engine_update`, and the hash
   walk). One frame at rest with the throttle shut is enough. Measured, on the rideable fixture:

       settle   0 frames | ten seconds of full throttle: top 65.7 ft/s
       settle   1 frame  | ten seconds of full throttle: top  7.8 ft/s, ends 0.0, clutch_in 1, rpm 7474
       settle  60 frames | ten seconds of full throttle: top  0.0 ft/s, clutch_in 1, rpm 7025

   The 7.8 is the drop transient sliding, not drive. **This is the shell's own start state** — `openmx`
   spawns the rider standing still and the player reaches for the throttle some frames later — so the
   measured cost of this one latch is that the bike in the shell never moves at all, which is what P10 is
   about. The minute only rides because its throttle is down on frame zero, which is a drag racer's
   launch and is deliberate: see `it_pulls_away_after_a_frame_off_the_throttle`.

All three raised with `./kit plan add`.

**Why ignored and not failing.** Other tasks are in flight in other worktrees and a red `./kit check` is
their problem too; deleting the three legs would lose the finding instead. Ignored, they are a list of
what the port owes a rider, they compile, and each one fails today for the reason its `#[ignore]` string
names (verified by running them with `--ignored`, output above). The day one is paid, the attribute comes
off. Precedent for the shape: `platforms.rs:904`, `replay.rs:576`.

## What the original does

Nothing in this task is a port of a function, so there is no address to run against it. What the original
does here is what the human's recording does, and the recording is the only source of an absolute number
in either file: `context/capture/acceptance/acceptance-freeride.csv`, 1811 rows at dt 0.03333, of which
1266 are rows the bike is upright for. Measured over those, off the `bike_vel` columns and the transform:

- **top speed 75.78 ft/s** — `CAPTURED_TOP_SPEED`;
- **sharpest turn 2.40 rad/s** (99th percentile of the frame-to-frame heading change) —
  `CAPTURED_TURN_RATE`;
- **hardest slowing 70.9 ft/s²** (1st percentile of the frame-to-frame speed change) —
  `CAPTURED_SLOWING`;
- 39 rows under 1 ft/s, i.e. the original comes to a stop and stands there, repeatedly. That is what
  makes defect 2 the port's and not the bike's.

Percentiles and not extremes: a respawn moves the bike across the map in one row, and the extremes
(136 ft/s², −1146 ft/s², 3.72 rad/s) are those rows.

The original's own launch is *slower* than the minute's (0.5 ft/s at half a second, 15.4 at two seconds,
74.5 at six), because the human feathers a clutch where the script pins the throttle. So the launch leg
asserts a shape ("it is moving by half a second, it is doing a useful speed by two") and not the
recording's profile.

## How it was proved

- **A bit-exact replay fixture:** no. Neither file is a comparison against a recording of the original —
  it cannot be one, because the bike is a stand-in (below).
- **The recording, as a bound and a calibration:** `the_minutes_numbers_are_the_originals`
  (`game-sim/tests/ride.rs`) re-derives the three consts above from the CSV and fails if any has moved by
  more than a percent, then asserts the minute's own top speed and worst deceleration stay inside the
  recording's envelope. Skipped with a printed line in a checkout with no `context/` (it is game data),
  which is why every other assert in both files is a bound a rider would recognise rather than a number
  off the capture.
- **A deliberate negative:** `the_minute_notices_a_bike_that_loops_out` rides the same minute on the same
  bike geared four times too tall (`FinalGearRatio = 14.4`) and requires that the launch leg wreck or
  come off upright, and that the minute wreck somewhere — so the legs above are known to be
  asserting something.
- **The two harnesses against each other:** the hand-built `Controls` and the pad script produce
  identical per-leg numbers, so the report bytes, the dead zone, the rate limits and
  `Controls::from_pad`'s binding are all carrying the script intact. `the_pad_script_is_the_minute` makes
  that explicit: `down cross` is a throttle of 1.0 on the first frame, `stick l 1 0` reaches the bike at
  full lock after the dead zone's stretch, `release` leaves nothing held.
- **The three defects:** each measured on the fixture and traced to a line, as above.

## Not verified / not 1:1

- **The bike is not the game's.** Both files ride the workspace's hand-written stand-in
  (`game-sim/tests/freeride-bike.phy.txt` and `game_sim::stock::BIKE`, which are the same parameters
  line for line — I diffed them; only the comments differ). No number in it is the disc's, so no leg can
  assert a value the parameters decide. A minute on the disc's bike may behave differently in ways
  nothing here would notice.
- **Two keys are patched in memory**, in `ridden_bike` in each file, and the shared `.phy` is left alone
  so other tasks' tests do not move:
  - `[Joystick]` gains `SteerScale1 = 1.0`, `SteerScale2 = 0.25`, `SteerSpeed1 = 5`, `SteerSpeed2 = 50`.
    The committed bike has none of the four, so `phy.rs:844-847` loads the steering scale as zero and
    `steer_command` multiplies the rider's lean by nothing — on that bike the stick is not connected to
    anything whatever `ride.rs` does. The four values are a *shape* (full lock at walking pace falling to
    a quarter at 50 mph) and are not claimed to be the disc's. They are also why defect 1 is now visible:
    without them the zero yaw rate has two possible causes.
  - `Drag = 0.5` → `0.06`, which is a **calibration**: it puts this bike's terminal speed at 74.9 ft/s
    against the recording's 75.78, which is what makes `CAPTURED_TOP_SPEED` an assertable number.
    `the_minutes_numbers_are_the_originals` is what keeps it honest; nothing says the disc's drag is 0.06.
- **The whole minute runs in first gear.** Measured: the gearbox never leaves gear 1 and the engine peaks
  at 4865 rpm against `UpShiftRPM = 9500`. So `mv::shift` and `mv::set_gear` are entered every frame and
  never do anything, and nothing here proves a gear change. A fifth-gear top speed would need the
  stand-in's gearing changed, which is not this task's file.
- **A realistic first gear loops the bike out.** The ×4 gearbox used as the negative above wrecks the
  minute. That is close to a real bike's first gear on this chassis, so
  there is a drivability question behind it the minute does not answer: whether the port can feed out a
  gear that tall at all. Not raised as a defect — the gearing is mine, not the disc's.
- **The ramp world is by eye.** 250 ft ahead, a rise of 0.3 over a run of 15 ft, placed on the bike's own
  heading; the lip's normal is the slope's, and nothing rounds the transition. No capture of the
  original's jumps was compared against it: what the leg proves is that the bike leaves a lip and comes
  down on its wheels, not that the flight matches the game's. Measured here: 86 frames of air, peak 11.1
  ft, up never below 0.900 in the air, 1.000 on the ground afterwards.
- **The tolerances are by eye**, deliberately. "> 3 ft/s at half a second", "> 20 at two seconds",
  "within 10% of the top speed", "the last quarter never falls 3% off the peak", "up > 0.9". They are
  loose because the point is the legs; a bike that drifts 5% slower will not fail this, and is not meant
  to.
- **The recording disagrees with itself about speed by a factor of 1.254.** Its `bike_vel` columns top
  out at 75.78 ft/s and the speed derived from its own `bike_xform` positions and its `dt` column tops
  out at 95.07. Something is scaled by something this task could not see (1.25 is 30/24, which smells
  like a sampling rate). The columns are used, because they are the recorder's value for the quantity the
  port calls `chassis.speed` and because the port's terminal speed lands within a foot a second of them.
  If the positions turn out to be the truth, `CAPTURED_TOP_SPEED` is 25% low and the drag patch is wrong
  with it — the legs' shapes would survive, the one calibrated number would not.
- **The two `.phy` texts can drift.** `game-sim/tests/freeride-bike.phy.txt` and `stock::BIKE` are two
  copies on purpose (`src` cannot `include_str!` a `tests` fixture), so the two files could end up riding
  different bikes. Both `ridden_bike`s assert their patch keys exist, which catches a key that moves but
  not a value that does.
- **The pad script was never sent to a real machine.** `ride-minute.pad` is valid notation and the test
  reads it frame by frame, but `./kit pad script` against PCSX2 was not run (the kit's sender is
  wall-clock, not frame-exact, so it could not have proved a number anyway). The subset of the notation
  the reader implements is `down`/`up`/`press`/`dpad`/`stick`/`sleep`/`wait`/`release`/`#`, and anything
  else is a panic rather than a skip.
- **The openmx test hands over no wreck files.** `Rider::carries_wreck` is not called, which is the state
  the shell runs in when the disc has no `Wreckbike.phy` — wrecks are still *called* (`rider.rs:440`
  sets `out.wreck` either way), there is just no body to tumble. So `none_of_the_minute_wrecks` is a real
  assertion there, but a wreck's aftermath is not exercised in that file (section 6 of the game-sim file
  is where that lives).
- **Nothing here is a 1:1 claim about the original's handling.** The minute does not know what the game's
  cornering, braking distances or jump arcs are. It knows that the original reached 75.78 ft/s, turned at
  up to 2.40 rad/s, slowed at up to 70.9 ft/s² and came to a stop 39 times — and it is loud when the port
  cannot do those things at all.

## Next

Nothing in this task is unfinished; the next session's work is the three defects, which are now plan
tasks. Pay them in this order, because each one hides the next from a player: the clutch latch (the bike
cannot pull away at all), then the steering (it cannot turn), then the brake (it cannot stop). When each
lands, take the `#[ignore]` off the leg that names it in **both** `crates/game-sim/tests/ride.rs` and
`crates/openmx/tests/ride.rs` — they are deliberately the same assertions on the two sides of the pad.
