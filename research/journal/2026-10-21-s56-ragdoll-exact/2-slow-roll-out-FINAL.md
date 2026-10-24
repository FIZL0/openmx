# 2 — the slow roll-out (FINAL)

Task S56. 2026-10-23. The user reopened it on 2026-10-22 ("do what can be done now, the parts that wait
on another task go to it"). By then S61 (the port's mode machine), S62 (contact with the moving bike),
S105 and S108 had landed. The port replays the s56 and s62 wrecks through to the rider at rest, bit for
bit against the original's update (S61's and S62's journals). That left two of this task's gaps open:
no truly slow tip-over, and front parting from the capture at 2628.

## What I did

- **Found why no slow tip-over could be ridden.** The original makes it impossible (see below). S62's 19
  pad scripts never had a chance.
- **Made the slowest wreck the original has, and captured it.** Starting from recording2's state with the
  bike standing, `research/s56_roll.py` pauses over AT-SPI and adds 20 ft/s upward to every copy of the
  chassis velocity, then resumes. On the first airborne tick it pauses again, adds 18 rad/s of roll about
  the bike's own forward axis to every copy of the angular velocity, and saves. The hop crosses the
  airborne roll limit: **reason 8, at 2.3 ft/s.**
  - Then `research/s62_wreck.py roll --pad research/s56/roll.pad --state 5 --frames 700` captured it (pad
    idle). 700 rows, frames 2481..3180, 0 torn, 0 dropped.
  - States: roll-2481/2482/2483/2881, plus roll-start (frame 2473, the state with the hop and roll
    written in). Each is unpacked next to itself as `.ee.bin`/`.spr.bin`, all in context/capture/s62
    (git-ignored, **this slot's context only**).
- **New test `the_port_replays_the_slow_roll_out`** (crates/game-sim/tests/ragdoll_wrecks.rs). It is S62's
  replay loop over the roll:
  - The original's update must reproduce the capture.
  - The port's actor runs on its own state, and every number it writes must equal the original's update.
  - This holds on every tick.
- **Filed S112** for front 2628, with the lead below.
- oracle.txt: an S56 note under S62's notes, with the roll lock, the limits and the capture.

## What the original does

- The ground step (`./kit fn 0x00257a00`) sets the chassis' three angular freedom flags each tick:
  - (1, 1, 0) while a wheel is down (the rider's +0x574);
  - (1, 1, 1) in the air.
- So **a ridden bike on the ground cannot roll at all.** Its lean is the rider and the animation, not the
  body.
- Proved live on copy 2:
  - The flags read 1, 1, 0 on the standing bike.
  - A 1.5 rad/s roll written into all four angular velocity copies was gone on the next tick.
- The port already does this: `chassis::set_flags`, which ride.rs calls with the ground step's flags.
- The wreck test (`./kit fn 0x002522c0`) has no ground roll limit. Its roll test (euler +0x98 against
  +0x778, 1.4835 rad) runs only while airborne.
- In the air the roll is levelled as well:
  - 2.5 rad/s written reached only 0.26 rad;
  - 12 rad/s reached 1.26 rad, short of the limit, and the bike landed and was snapped upright;
  - 18 rad/s crossed the limit.
- What a slow bike *can* wreck by, from the same function:
  - the g loads (reasons 1–4);
  - reason 5;
  - the ground pitch limits (8/11);
  - the rider body's contacts (6/7): a contact whose +0x24 is over 6, or 4 with +0x588 set.
  None of these comes from a bike slowly falling over.

## How it was proved

Save-state capture plus oracle replay, the same method as S56 part 1 and S62.

| wreck | ticks | the capture's own | off bike | settles | settled ticks | bike hits |
|---|---|---|---|---|---|---|
| roll | 2482..3180 | all 699 | 1 | 1 (22 ticks in) | 677 | 0 |

- The counts are asserted exactly.
- To check that the test ran rather than skipped, `ticks >= 99990` was asserted for one run. It failed
  with `Went { ticks: 699, … }`.

## The front 2628 lead (not fixed, S112)

In front.csv, from 2627 to 2628, bone 0 goes from y 156.523 to 156.084 while its vy stays near +6. x also
moves back 0.22, and z gains 0.04 where the tick before gained 0.46. Bones 1 and 15–17 move back the same
way. The next row picks up again.

The wreck bike is about 4.5 ft away in z and flipping (its up y goes from 0.32 to 0.42). The rows are
not torn: stamps step by 2 and the frame counter by 1. It looks like a push-out against the bike that
the Ee's search does not pair. `every_sphere_test_of_the_wrecks_is_the_bike_meshs` stops at front's 2627,
so nothing checks the search at 2628. Not tested.

## Not verified / not 1:1

- **The roll is not a ridden wreck.** The hop and the roll rate were written into RAM by hand. Riding
  can't produce a slow tip-over, because the original holds the grounded bike's roll at zero. The roll
  is the slowest wreck I could make, not the "slow tip-over" the task named, and it enters by the
  airborne roll limit (8), not by falling.
- **The roll's entry tick is not compared.** The recorder's first row came 8 ticks after the loaded
  state (2473 → 2481), and 2481 is already the wreck's first tick. The first state is from the entry's
  own tick, so the entry and its draws are not in this replay. The four other entries (reasons 1, 2, 11
  and 3) are compared in S61's and S62's tests.
- **The roll has no bone hits on the bike.** The moving push-out isn't exercised by this capture.
- **The roll capture is only in s2's context.** In other slots the test skips, and passes as it skips.
  Copy context/capture/s62/roll* to run it there.
- **Front past 2627 is still not the capture's own** (S62's open item). The lead is above, and it is now
  S112.
- **"Replay the same pad in the port from the same savestate":** the port's whole ride (the bike up to
  the wreck, then the ragdoll) is not replayed from the save state. The comparison starts from each
  wreck's state, with the bike side laid in from the capture. A whole-ride replay needs the port's ride
  started from a PCSX2 save state, which no task provides.

## Next

Nothing in S56. S112 is front 2628.
