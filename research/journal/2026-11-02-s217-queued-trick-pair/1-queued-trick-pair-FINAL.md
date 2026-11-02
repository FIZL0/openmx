# 1 — queued trick pair (FINAL)

Task S217. 2026-11-02.

## What I did

- Made a free09 tape with a queued trick pair (`capture/make_tape.py`, from C8's tape, which keeps
  the human's run up to the jump the bike leaves at ~2650 and lands at ~2718). Triangle with the
  stick full right over fields 549..556 gives flatliner (slot 7). Circle with the stick full up over
  fields 579..586, while flatliner runs, queues the Taunt (slot 0). The stick is centred between and
  after the presses up to field 680, so the bike flies square. Neither button is held into a hold.
- Played it in PCSX2 with C8's recipe (`capture/play.sh`, spec `capture/s217.toml`): 380 rows,
  2374..2753, no torn rows. Saved to `context/capture/s217/run1.csv` in slot s1, with save states
  `run1-2675.p2s` (mid-flatliner) and `run1-2695.p2s` (mid-Taunt) beside it. The command was
  `SAVE=2675:8,2695:9 ./kit pcsx2 with -- bash <journal>/capture/play.sh <pid> context/recordings/s217/s217.p2m2 2374 380 context/capture/s217/run1.csv`.
  The tape and its copy of C8's save state are in `context/recordings/s217/`.
- Found the bike's own animation object, so the capture could read the bike's joints. The spec's
  header says how.
- Added `a_queued_pair_matches_the_original_across_the_switch` to
  `crates/game-render/tests/trick_chain.rs`, next to S214's test, which is the port side. It rides
  the same two gestures at the capture's dt and lays each port tick on the capture's tick, starting
  where flatliner starts.
- Filed B10: the bike's trick fades are not the original's.

## What the original does

The ride, tick by tick (frame_counter):

| tick | what |
|---|---|
| 2649 | the gesture (group 2, right) is seen, still on the ground |
| 2650 | state 7, flatliner, airborne, request seq 1 |
| 2664..2667 | the Taunt's gesture (group 1, up), queued |
| 2676 | state 0, the Taunt, seq 2, flatliner at elapsed 0.8666 when it pops |
| 2676..2700 | the body's stunt cross runs 0 → 1 (`d_cross`, from 0.5189) |
| 2718 | landed (wreck reason 12); 2719 state 40 |

**The body** crosses from flatliner's clip into the Taunt's over 24 ticks, which S214's port does
too.

**The bike** has a motion object of its own at rider `+0x68` (also `+0x7a0`). Its directory at
`+0x1e0` is `Bike\Anim\`, and it has 22 joints. The joint array is at `+0x234` (0x60 stride, t at
+0x40, q at +0x50), the snapshot buffers are at `+0x248`/`+0x24c`, the playing clip is at `+0x3c+0x14`
(`+0x50`), and the clip time in seconds is at `+0x54`. It is the same base class as the body's object
but not the 60-state machine: its `+0x8f0` is not a state. On C8's state there are four such objects,
one per free09 rider. At the switch the bike does not cross the way the body does:

- its clip pointer changes (to the Taunt's bike clip) and its clip time starts over at one tick
  (0.0333), climbing by dt from there;
- it fades from a snapshot of its joints taken at the switch, linearly, over exactly **10 ticks**.
  TireRear goes from 123.6° to 0 at 12.36° a tick, and the `Bike` root from 120° at ~12° a tick.
  That is a ramp rate of 3 a second, the motion library's own constructor default
  (`motion::Params::blend_rate`), and not the 4.0 V12 measured on the body's object;
- the same 10-tick fade brings flatliner in at 2650 (TireRear 64.1° → 0, TireFront 83.8° → 0).

After the fade the bike follows the Taunt's clip at its own clock.

## How it was proved

A PINE recording compared tick by tick: `run1.csv`, 380 rows, one run. The new test,
`cargo test -p game-render --test trick_chain a_queued -- --nocapture`, prints
`switch 26 ticks in, landing 68: the rider within 1.48°, the bike 123.6° off after the switch (B10)`
and asserts, against the capture:

- the body's state and the trick's elapsed on all 68 ticks from flatliner's first to the landing
  (to within 1e-4). The switch comes on the same tick, 2676, with flatliner at the same 0.8666;
- the rider's 22 joint rotations within 2° from tick 7 (the fade-in done) to the landing. They are
  within 1.5° across the switch, and the step on the switch tick is 0.9° on both sides;
- the bike's 22 joints within 2° from tick 10 to the switch (≤ 1.3°) and from 24 ticks after it to
  the landing (≤ 0.5°);
- that the bike *is* more than 20° off over the 24 ticks after the switch, so that closing B10
  shows up here. The `Bike` root alone is up to 50° off over 2683..2699.

So nothing snaps on either side, and S214's fix holds against the original. The rider is 1:1
across the switch. The bike is 1:1 outside the switch's 24 ticks. Inside them, the port's bike
takes the body's 24-tick cross where the original's restarts and fades over 10.

## Not verified / not 1:1

- **The bike's fade at a switch (B10).** The original restarts the bike's clip and fades over 10
  ticks from a snapshot. The port crosses it over the body's 24 ticks. Up to 50° on the root,
  2683..2699. Not fixed here: the task was the comparison, and the fix is a change to the bike's
  `anim::Stance::frame` path, which needs its own capture-checked test.
- **The bike's fade-in rate.** The original's bike fades into a trick over 10 ticks (rate 3). The
  port uses `vehicle::params()`'s 4.0 (7.5 ticks), measured on the body's object. The test does not
  hold the bike over its first 10 ticks. This is part of B10.
- **The bike's ramp field was not located.** Nothing in `+0x3c..+0x94` or `+0x240..+0x300` reads
  as a ramp. The 10 ticks are read off the joints' linear fade, not off a field.
- **The tyres' snapshot.** TireFront/TireRear's snapshot holds a rolled wheel angle (from the
  transform node, as in V15). Whether the wheel drive overwrites it when drawn was not checked, so
  the 123.6° on TireRear may not be visible.
- **The first 7 ticks of the rider's fade-in are not compared.** The port's rider falls from 400 ft
  in the test harness and the original's takes off from the lip, so the pose each fades from differs
  (up to 94°, decaying to under 1° by tick 7). That is a harness artefact, not a measured difference.
- **The joint order** of the capture's arrays is taken to be the port's skeleton order (Bike,
  Dummy_Foot_L, ... and Pelvis, LwrTorso, ...). It was not read from the original's objects. The
  quaternions agreeing to under 1.5° for most of the ride supports it.
- **One run, one pair**, flatliner into the Taunt, both bike-clip tricks. A pair where the second
  trick has no bike clip, and a held first trick, were not filmed.
- **The bike's motion class was not decompiled.** The rate of 3 and "restart with a snapshot" are
  read off the capture.

## Next

B10: give the bike its own restart at the switch (clip at time 0, a snapshot fade at rate 3), then
hold the bike over the whole ride in `a_queued_pair_matches_the_original_across_the_switch`.
