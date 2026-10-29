# 1 — car tire model (FINAL)

Task S149b. 2026-10-29.

## What I did

- **Captures.** S149a's three tapes (baja, buggy, monster truck; all on free07, where S149a's switch parks
  the car at 0x00605270) were captured again with wider tire blocks. Every row now carries each tire's
  first 0x2c0 bytes and its 0x9b0..0x9d0 tail, beside the car's +0x200..+0x700 and the engine's
  +0..+0x140. This folder's `capture.sh` and `car_tire.toml` (S149a's, widened) wrote
  `context/capture/s149b/{baja,buggy,truck}.csv`, 650 rows each (FIRST 401/467/459, `car_kind` 5/3/4).
- **`crates/game-sim/src/car.rs`**, after the loader:
  - `TireState`: the words the tire carries from substep to substep.
  - `Ground`: the ground point, normal and solid flag a tire is handed.
  - `hand_off`: the rear tires' load when off the ground.
  - `Tire::contact` / `spring` / `settle`: the suspension, with its compression and decompression limits.
  - `Tire::roll`: rolling resistance, brake and ABS.
  - `Tire::forces`: steer, spin, slip, the surface force, wheel angle and the bookkeeping.
  - The parts the tests call on their own: `slip_at_speed`, `camber`, `surface_force`, `grip`,
    `clamp_slip`.
  - All of it uses ps2_float under car.rs's `deny(float_arithmetic)`.
- **`crates/game-sim/tests/car_tire.rs`** (new), 7 tests, all bit-exact. See below.
- **`research/oracle.txt`**: an S149b block. It has 9 name lines (the tire pass, the step and its parts),
  and the tire's field map in comments.

## What the original does

- **Substeps.** The world cuts a frame into `(int)(dt*115)+1` substeps, 4 at 30 Hz, each `dt/n`. Every
  substep, the car's tire pass (`./kit dis 0x00397f40`, reached through the vtable thunk 0x003994e0,
  this −0x50) steps each tire. Before stepping the rears, it hands them the larger front load (+0xc8 flag,
  +0xcc load, +0xd0/+0xd4 contacts), whenever a front is down. A capture row therefore holds only the
  4th substep.
- **The tire step** (`./kit fn 0x0039bbd0`): suspension, roll, forces.
- **Suspension** (`./kit dis 0x0039bc20`):
  - It zeroes the compression +0xdc and the grounded flag +0xc0. If the tire is solid or touching
    (+0xb4/+0xb8), it builds the axle points +0x150/+0x160 from the body (quat-rotate +0x80/+0xf0 at
    body+0x170, plus body+0x60).
  - Let d = top − bottom. If d·n ≤ 0, it stops. Otherwise ratio = ((ground − bottom)·n)/(d·n); if that
    is ≤ 0, it stops.
  - contact +0x140 = d·ratio + bottom, and compression = |contact − bottom|. If that is > 0, the tire is
    grounded and the contact velocity is computed (`./kit dis 0x0039b670`: +0x170).
  - **Past the limit +0x3c**, the compression is held at the limit. If the ground is solid, or ratio < 2,
    the body is first pushed up along the normal by min(max(c − limit, 0), limit). That moves body+0x60,
    applies an impulse through the body's vtable +0x3c, and computes the velocity again.
  - **Spring and dampers:** v = clamp(+0x170·n, ±rate +0x40). The damper is rebound +0x50 when v ≤ 0,
    else bounce +0x4c. load +0xe4 = max(0, spring +0x48 × c − damper × v), applied through vtable +0x68.
  - **Off the ground:** a handed tire takes +0xcc as its load, the hand-off's contacts, and a rebuilt
    +0x160. Otherwise the load is 0 and +0x170 = (0,0,0,1).
  - **Last**, the compression comes back out no faster than the decompression rate × dt, and +0xe0 =
    min(c, limit).
- **Roll** (`./kit fn 0x0039c220`):
  - torque += −clamp(spin/2, ±1) × WheelFriction.
  - With the brake on, it then adds MaxBrakingTorque × (half × (scale × −brake)).
  - ABS (+0xa8, on a grounded tire) gives scale = clamp(1 − (|slip| − lo)/(hi − lo), 0, 1), unless
    lo == hi.
- **Forces** (`./kit dis 0x0039c360`):
  - It sets the steer, and resets the force to (0,0,0,1).
  - A grounded tire gets its frame and local velocity (`./kit dis 0x0039c5e0`: +0x220, +0x270..+0x2a0);
    an airborne one gets (0,0,0,1).
  - Then the slip (`./kit dis 0x0039c950`) and, if grounded, the surface force (`./kit dis 0x0039cfe0`),
    applied through vtable +100 and, unless +0xc4, +0x5c.
  - Bookkeeping: last force; wheel angle += dt × clamp(spin, ±MaxTireAngleRate), wrapped to ±pi; last
    torque = torque, then torque = 0; side ratio = Fx/N (0 with no load).
- **Slip:**
  - **Locked:** braking with slip ratio > 0.95 locks the wheel (spin 0). Otherwise
    spin += (1/I)(torque − last Fz × r) dt, clamped to the top wheel speed +0xa0, which is 293.33 on the
    undriven fronts.
  - **Fast:** above 20 ft/s forward, the slip angle comes off the acos table and the slip ratio is
    (vz − spin r)/|vz|.
  - **Slow:** below that, it relaxes the slip tangent and ratio. Below 5 ft/s it sets the low-speed grip
    when the lateral velocity changes sign, and clears it above 5. While the grip is on, it takes
    1.4 v sqrt(100 N) off Fx.
  - Then come the sliding flag, the camber from the normal as the car sees it (+0x130, by
    CamberRollRatio), and the clamps (±pi/2, ±0.98, ±0.09).
- **Surface force:** a combined-slip rational shape (C1..C4) over the cornering stiffness and the
  A0/A1/1/A2 load curve, with the friction falling off by FrictionEndPoint, and the camber thrust from A3
  and 1/A4. The load is capped at MaxNormalLoad.
- **No aligning moment.** Nothing in the tire's code reads `MomentScale` (+0x874). In an objdump of
  boot.elf, the only access in 0x0039a000..0x0039e000 is the setter's store at 0x0039af14. The ELF's
  other two loads of +0x874 (0x00391de8, 0x003935e0) are integer bitmask tests on another object.

## How it was proved

All three cars, 650 captured rows each. `./kit check -- -p game-sim --test car_tire` passes all 7 tests.

- **What a row fixes on its own, against the row** (`the_{baja,buggy,truck}_tires_match_the_capture`):
  - the contact, grounded flag and load from the row's axle and ground points;
  - the rear hand-off from the fronts;
  - the surface force (with the low-speed grip) from the row's slip, load and camber;
  - the camber from +0x130;
  - a fast tire's slip from +0x220 and the spin.

  Per car: baja 33,716, buggy 33,722 and truck 30,284 words, all bit-exact. Two truck rows are skipped
  as torn (below).
- **The whole step under the interpreter** (`the_*_tire_step_matches_the_original`):
  - **Setup.** For each tire, every pair of consecutive rows, the original's 0x0039bbd0 runs in
    `ps2_oracle::Ee` on S149a's switched RAM image, with the substep dt. The tire comes from the row
    before, overlaid with what the car step lays in before the model: top wheel speed, flags, hand-off,
    surface, ground point/normal/car normal, contacts, friction scale, and the engine's torque. The row's
    car block and body go in too.
  - **Stand-ins.** The frame and contact-velocity helpers hand back the row's words; the body's vtable
    calls apply nothing.
  - **The port** then runs contact/spring/settle/roll/forces from the same tire and inputs.
  - **Results:** baja 95,122, buggy 95,866 and truck 93,769 words, all bit-exact. These runs cover:
    - grounded and airborne tires (2,286 / 310 baja tire-substeps);
    - fast and slow slip;
    - braking (270), locked wheels, the low-speed grip;
    - the hand-off (54 / 22 / 31);
    - the bottom-out push (2 baja, 2 truck).
- **ABS** (`abs_matches_the_original_on_every_car`): no retail car turns ABS on. So for every braked
  grounded row, the original's roll (0x0039c220) runs with ABS forced on, once with the window
  0.02..0.3 and once with 0.1..0.1. The port does the same. That is 432 + 540 + 184 words, all
  bit-exact, with 168 / 174 / 10 rows' slip inside the window.
- `./kit audit` is clean.

## Not verified / not 1:1

- **Substeps 1–3 of every tick are never seen.** A row is the 4th substep, so a tick-by-tick replay from
  the capture cannot work (an attempt left 24k of 85k words off). The stateful part is proved by the
  original's own step on captured tires instead. This proves the tire function, not a whole drive; the
  drive replay is S149d's ("a captured drive replays bit-exact").
- **The inputs laid into the oracle are the row's, not the substep's.** That makes the two sides agree
  with each other, but the inputs are not quite what substep 4 saw. For example, +0x130 is rewritten
  after the step on some ticks (truck fc 1105). Both sides read the same words, so the comparison is
  fair; but which car-step words come in when is S149d's to pin down.
- **Not ported; their outputs come from the rows:**
  - the tire frame and local contact velocity (0x0039c5e0);
  - the contact point's world velocity (0x0039b670, which also writes +0x180/+0x190);
  - the axle points +0x150/+0x160 built from the body.
  These all need the body and the car step (S149d). The port takes them as arguments.
- **The body side of the tire is not ported:** applying the load and force to the body, the bottom-out
  push of body+0x60, and its impulse through vtable +0x3c. `contact` returns the push for S149d to apply.
- **Not modelled:** the hand-off's contacts (+0xd0/+0xd4 → +0x1ac/+0x1b0), and the airborne handed
  tire's rebuilt +0x160. The tests read the axle points of a handed airborne rear from a suspension-only
  run with the hand-off cleared.
- **Inputs the tests take as given:**
  - the engine's torque into +0x1f0 is read from the engine block (S149c);
  - +0x9b4 (friction scale) is taken from the row; what sets it is not looked at.
  - The tail word +0x9b0 is unnamed and unused.
- **No aligning moment exists to port.** The task asks for one, but the original's tire step computes
  none (see above). `MomentScale` is loaded and then unused.
- **Torn rows.** The truck's fc 1031 and fc 1105 were read while the original was part way through a tire
  step. In fc 1031, fr has its new load but fc 1030's slip tangent and camber, and a force with nothing
  added yet. In fc 1105, the normal is a substep ahead of every tire's camber. The row-only test skips
  both by name. The whole-step test uses them as inputs only, and is bit-exact on them.
- **ABS is proved only on synthetic windows** (0.02..0.3, 0.1..0.1) over captured tires, because no
  retail car enables it.
- **Coverage gaps:**
  - All drives were on free07: one track, so a narrow set of surfaces.
  - All three cars drive the rear wheels, so a driven front's top wheel speed never ran (a front's
    +0xa0 is always 293.33).
  - The buggy never bottomed out.
- **Stalled rows were left in.** The baja's frame counters 402..409 and 17 single truck rows did not step
  the car. The row-only checks still pass on them, and the whole step skips any pair that is not
  consecutive.

## Next

S149d (the car step) can call `Tire::contact`/`spring`/`settle`/`roll`/`forces` with axle points,
velocities and the frame of its own, and apply the returned push and the loads/forces to the body.
S149c supplies the torque.
