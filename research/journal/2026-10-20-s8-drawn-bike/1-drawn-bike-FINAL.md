# 1 — drawn bike (FINAL)

Task S8. 2026-10-20.

S8 asks to raise `ACCEPTANCE_AGREES_THROUGH` as far as the per-lane diff allows. It is still 0. This entry
says why. In short, the acceptance capture does not watch the chassis the port simulates, so no lane can
honestly agree past row 0. It also lists what was fixed on the way. It follows
`2026-10-15-s8-s8-acceptance/4`.

## What I did

- **The wheel mounts, bit-exact (commit a28cc90).** S28 had left a choice open: was the 1.4-unit tick-1
  rise caused by the mounts or by the ground? It was the mounts. The original hangs its wheels off the bike
  *model's* `TireFront`/`TireRear` nodes, not off the `.phy`'s mounts. The bike model is `125_faisst`; the
  recording's save state carries `125_faisst_engine` and `125_faisst_plasticsall`. I added
  `game_render::psg::Model::{world_vu, place_in}`, which is the original's VU0 node lookup. With it, the
  port's mounts equal the save state's to the bit. The renderer's host-float `world` was 1 ulp off. Test:
  `the_acceptance_bike_hangs_its_wheels_where_the_original_does`. The acceptance fixture now uses these
  mounts (`Ridden`, `the_wheels_the_human_rode`).
- **`SaveState`** (in `tests/replay.rs`) reads the recording's own save state from `context/states/`. It
  follows the game manager to the rider, the chassis and the vehicle actor. `leads(cap)` checks that the
  saved frame counter is one before the capture's first row. It skips with a reason when the state is
  absent.
- **The drivetrain is seeded from the save state.** `Drivetrain::seed` copies the clutch ramp (timer
  1.5278419, scale start 3, cofg weight 1), the gas and clutch levers (0 and 1), gear 1, locked 0, launch 1,
  the curve position (10, 2, 0.5) and rpm 4500. Before this, the port started with a fresh clutch ramp and
  launched on tick 1. That caused a 0.0024 sink that the original does not have.
- **New capture spec `watch/s8-start.toml`.** It records the chassis's own transform, quaternion, cofg,
  speed and angular velocity next to the acceptance lanes, plus the clutch, engine and wheel spin words.
  It starts from the recording's save state with an idle pad. `context/capture/s8-start.csv` has 120 rows,
  frames 2774–2893. From 2775 to 2802 its `bike_xform`/`bike_vel` equal the acceptance capture's to the
  bit. At 2803 the human's input starts.
- **New test `the_acceptance_capture_watches_the_drawn_bike_not_the_chassis`.** It pins findings 1 and 2
  below against the save state.
- I rewrote the `ACCEPTANCE_AGREES_THROUGH` doc to describe the current state.

## What the original does

1. **The save state is frame 2773, one physics step before acceptance row 0 (2774).** The first row of
   s8-start is read before any step runs, so it equals the save state. Port tick *t* compares with frame
   2773 + *t*.
2. **The capture's `bike_xform` (vehicle actor +0x180) is the drawn bike's root node, not the chassis.** The
   writer is `./kit fn 0x0027c2d0`. It calls `./kit fn 0x001b2a50` on the node at rider+0x964 with output
   rider+0xbd0. The children of that node are `Dummy_Foot_L`/`_R`, with the model's locals. The chassis node
   is at rider+0x18 (= chassis+0x254): local at node+0x40, world at node+0x80, row 3 at +0xb0. The drawn
   root's local relative to the chassis is 1.49° of roll, −0.47° of pitch and (−0.0646, 0.00896, 0.0223)
   of translation. It held rigid over 120 idle frames. It is most likely the vehicle-animation pose of a
   bike resting on the rider's foot.
3. **The capture's `bike_vel` (vehicle +0x130) is the chassis's velocity (+0x260) × 0.8**, to the bit on
   every lane.
4. **Ground height inside a cell is a triangle, not a bilinear blend.** Under the front tire (free09 cell
   478,369, spacing 9, t = (0.847, 0.130), corners 208.50183 / 208.52643 / 207.97713 / 208.32965) the
   original finds 208.45465. The port's `Ground::at` blends bilinearly to 208.4905, which is 0.036 high.
   Splitting the cell along its (+x)–(+z) diagonal gives 208.45439. Rear tire (cell 478,368): original
   208.52266, bilinear 208.52429, split 208.52269. The normals match the blended vertex normals, not the
   triangle's plane. The function that does this was not read; these numbers come from the save state and
   the capture.

## How it was proved

- The mounts: a bit comparison against the save state's chassis words
  (`the_acceptance_bike_hangs_its_wheels_where_the_original_does`).
- Findings 2 and 3: a bit comparison over 120 rows of `context/capture/s8-start.csv` and the save state
  (`the_acceptance_capture_watches_the_drawn_bike_not_the_chassis`, which asserts drawn root = vehicle
  +0x180, chassis node = rider+0x18, the two more than 0.05 apart, and +0x130 = +0x260 × 0.8).
- Finding 4: hand arithmetic in Python on the archive's height grid against the save state's contact
  points. It is not a test.
- `./kit check` passes and `./kit audit` is clean.

The port's run after all of the above, against the acceptance capture: tick 1 `bike.pos[1]` is 208.0297
against 208.02884 (S28 left 208.10735). `pos[0]` agrees through tick 27, but that only means x moved less
than an ulp of 4309, so it proves nothing. Seeded fully from the save state's chassis (transform, quat,
vel, histories, compression, drivetrain) with a throwaway probe and compared against s8-start's chassis
lanes, the port still drifts: vel.y is +0.074 high by tick 2 and forward acceleration is about
0.25 u/s² high. The front compression is 0.49 against the original's 0.46, which is finding 4 at work.

## Not verified / not 1:1

- **`ACCEPTANCE_AGREES_THROUGH` stays 0.** The fixture compares the port's chassis against the original's
  drawn root and its 0.8-scaled velocity (findings 2 and 3). An honest agreement needs either a fixture
  lane for the chassis (re-record with `ch_xform`/`ch_vel`) or the drawn root's pose ported into Sim.
  Neither was done here.
- **`acceptance_start` still seeds `chassis.vel` from `bike_vel`**, so it starts 20% low, and it puts the
  chassis where the drawn root is, 0.065 off. I left this as is because the fixture's lanes are the drawn
  root's. Correcting the start while comparing against those lanes would just trade one mismatch for
  another.
- **The writer of vehicle +0x130 (the × 0.8) was not located.** The ratio is measured, bit-exact on every
  row, but not read from code.
- **The drawn root's pose source is not identified.** "Vehicle animation" is a guess from its rigidity and
  its foot children.
- **`world::Ground::at`'s in-cell arithmetic is not 1:1.** The triangle split fits to about 3e-4 but is not
  ported or proved, and the function was not read. `world.rs` is D6's module, so I left it alone to keep
  the merges clean.
- **Not seeded at row 0:** wheel spin, compression and spring histories, and the chassis histories. They
  change across the one step between the save state and row 0, and the acceptance capture does not carry
  them.
- **The shell (`crates/openmx/src/main.rs`) still places mounts with the host-float `world`.** That is
  1 ulp off the original. It is not S8's file.
- **`FREERIDE_MODEL = "125_faisst"` is inferred** from the save state's part names and the bit-exact mounts.
  Nothing in the capture names the model.
- Carried over from earlier entries: wrecks are off in the fixture, and the RNG seed covers 32 of 64 bits.

## Next

1. **Port the ground's triangle split** (D6, `world::Ground::at`). Read the original's height lookup with
   `./kit xref` from the wheel contact code, prove it against the two contact points above, and then
   re-run the full-seed comparison. The vel.y drift should mostly go away.
2. **Give the acceptance fixture a chassis lane.** Add `ch_xform` and `ch_vel` (chassis+0x254→+0x80 and
   +0x260) to `watch/acceptance.toml` and re-record from the human's recording (play a *copy*). Then compare
   the port's chassis against those lanes, seed `acceptance_start` from them, and raise the gate.
3. Seed the wheel and chassis histories from the save state, stepping once if needed, using the
   `SaveState` reader in `tests/replay.rs`.
