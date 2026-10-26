# 1 — plane crash ragdoll (FINAL)

Task S143. 2026-10-26.

## What I did

- Wrote a PINE capture of the biplane through a crash (`capture/pilot_watch.py`). It loads the biplane
  recording's save state (copied into slot 7 by `capture/slot7.sh`) and plays a pad script on the kit pad.
  Every tick it logs:
  - the plane's wreck word, clock and measure;
  - its rigid body (both positions, velocity, both rotations, rates);
  - its pad and throttle;
  - the model's root node;
  - the pilot body's mode words and request block;
  - all 22 joint nodes of the pilot.

  Scripts (the take-off is S57's):
  - `nose`: dive into the ground at ~135 ft/s;
  - `wing`: a hard left bank held into the ground, wrecked at 69.8 ft/s;
  - `belly`: a shallow push in at full throttle, wrecked at 92 ft/s;
  - `waggle`: the nose crash, with the stick hard left, then right, back and forward while wrecked.

  The logs are in `context/s143/*.txt`. `capture/to_csv.py` turns them into
  `context/capture/s143/{nose,wing,belly,waggle}.csv` (body as float bits, bones plane-local). The
  helpers are `capture/pilot_read.py` (a summary a tick) and `capture/lean_rows.py`.
- Looked at S57's `run1.mp4` (s5 context) around the first wreck and its put-back. The helmet is in the
  rear cockpit mid-tumble, and the cut back to the bay has no poof: the picture just cuts.
- Ran the port's shell into the ground with a `--play` tape (`capture/crash_tape.py`) and
  `--trace-picture`. Its pilot stays in the seat too.
- Wrote `crates/game-sim/tests/plane_crash_pilot.rs`. It loads the disc's plane (rays included), parks
  it at free06's `[Biplane1]`, seeds it from each capture three ticks before the wreck, and flies it on
  free06's ground and props. It has five tests (below).
- Filed **S161** (the put-back spot, below).

## What the original does

- **The pilot is never thrown.**
  - In all four crashes the pilot body's mode words (`+0x47c` of the body) never change.
  - The body's root stays at its seat, (0, 3.34, −3.00) in the model root node's frame, to within
    0.01 ft on every untorn row (nose's worst is 0.0014 ft).
  - Every joint keeps its place relative to the root to within 0.01 ft while the stick is held.
  - The pilot is posed by the plane's clips the whole time, and the lean keeps following the stick
    through the wreck. In `waggle` the request's phase/mix targets (request `+0x170`/`+0x174`) go to
    mix 0, then 1, then 0.5 as the stick moves while wrecked, and the joints move up to 0.32 ft with them.
  - The plane never writes its request's wrecked flag. This was already in the oracle and in
    `game_sim::biplane_anim`.
  - So the original has no pilot ragdoll. The port also draws the pilot seated through a wreck
    (`openmx` `pilots`, V39) with the lean on the stored pad (`vehicle::PilotLean`, which reads
    `Flight::pad`). That pad is stored through a wreck too. **This part of the port already matches, so
    there was nothing to port.**
- **What tumbles is the plane.** The wreck (`./kit fn 0x3535d0`, the MX override `0x354aa0`) multiplies
  the spin by 4 and sets WreckDamping, WreckInertia and WreckGravity. The tumble after it is the rigid body
  on the ground: the nose crash rebounds to 72 ft/s up at its second contact and climbs ~50 ft. The
  port's flight steps this bit for bit (below).
- **The put-back** (`./kit fn 0x3536e0` → vtable +0x198 = `./kit fn 0x355b00`) comes when the wreck clock
  passes 5 s, 151 ticks after entry at dt 0.033329 (clock 4.999495 on the last wrecked tick). There is no
  poof. The body is put back at rest at (2304, 138.614, 2281), facing (0, 0, −1), and the model's root
  node at (2304, 135.114, 2278.5). That is the body (the centre of gravity) on the spawn point, with the
  model 2.5 ft behind it and 3.5 ft below.

## How it was proved

All in `crates/game-sim/tests/plane_crash_pilot.rs`, against the four save-state captures above:

- `each_crash_wrecks_on_the_originals_tick`: seeded three ticks before, the port wrecks on the game's
  tick in nose, wing and belly. On that tick it is within 0.14 ft/s and 0.004 ft of the game's body
  (tolerance 0.5 ft/s and 0.05 ft).
- `the_wreck_tumbles_as_the_originals_a_tick_at_a_time`: before each wrecked tick the port's body is
  put where the capture had it, then stepped once. The result is **bit-exact on every compared tick**:
  nose 150/150, wing 150/150, belly 124/124. The tolerance is 0.001 ft and ft/s, and at least 90% must
  be bit-exact. Belly's other rows sit beside a torn read; see below.
- `each_wreck_is_put_back_after_151_ticks`: flown free from three ticks before the wreck (no reseeding),
  the port is wrecked 151 ticks in each crash.
  - Its last wrecked clock is the game's, bit for bit.
  - It is put back on the game's row.
  - It faces the game's way.
  - It is at rest wherever the game's put-back row was read on time.
- `the_pilot_rides_every_wreck_out_in_the_seat`: the capture's pilot, as described above. Torn rows
  are left out: on wing's 6 and belly's 9 the whole body is off the seat by the same vector.
- `the_pilots_lean_follows_the_stick_through_the_wreck`: through `waggle`'s wreck the port's stored
  elevator and aileron (what the drawn lean reads) are the game's on every wrecked tick, and the stick
  moves at least twice.

Flown free, without reseeding, the tumble parts from the game's after a few ticks: nose at its second
contact (72 vs 54 ft/s up), wing ~15 ticks in. Belly stays within 0.5 ft for ~60 ticks. Reseeded, every
tick is exact. So the free-run parting is the seed missing hidden state (the last force and moment,
the engine and aerodynamic lag, the contact records), which the contacts then amplify. It is not a
difference in the wreck step.

## Not verified / not 1:1

- **The put-back spot.** The port stands the model's origin on `machine.txt`'s Position. The original
  stands the body (the CofG) there, with its own height. So the port's put-back is 2.5 ft further along
  the plane's length and 0.11 ft lower (2304, 138.5, 2283.5 against 2304, 138.614, 2281). The test
  prints the spot and does not assert it. I did not fix it here because it moves the parked bay and the
  chord's reset too, and those have tests and markers of their own. Filed as **S161**.
- **Crashes into objects** (a tree, a building, another vehicle) were not captured. All four crashes hit
  the terrain. Which bodies the pilot collides with is moot, since it has no body of its own in the
  original, but the plane's wreck against props is not checked tick by tick here.
- **The pilot's bones in the port are not compared bone by bone.** The port draws the pilot in the
  shell, which game-sim does not have. Its seat reference (root 0, 3.5, −3.0 off the plane's origin, from
  `--trace-picture`) is a different frame from the capture's model root node (root 0, 3.34, −3.00). The
  original's own seat offset is (0, 3, −3, 1) per the oracle's `mxbiplane_load`. The test proves only
  that the stored pad driving the lean matches; the lean's clip weights are V39's, not re-checked here.
- **Torn and late rows.** The frame-counter trigger tears as the memory says.
  - belly has 9 stale rows in its wreck, each followed by a row two ticks on, plus one more two-tick
    row. Its put-back row was read a tick late, so its rest and facing are not compared.
  - wing and belly each have a few rows where the plane's model node and the pilot's nodes were read a
    tick apart (the whole body off its seat together).
  - Those rows are left out, not explained by anything the game does.
- **`waggle` ends before its put-back** (the 26 s run ran out). It is used only for the lean.
- The free-run tumble is not 1:1 after the first few contact ticks, for the seeding reason above. The
  exactness claim is the reseeded one.

## Next

S161 takes the put-back spot.
