# 1 — mxvatv car step (FINAL)

Task MD124. 2026-11-11.

## What I did

- crates/game-sim/src/atv_car.rs, new section "MX Unleashed's tire step (MD124)": `tire_step` and its
  parts (`suspension`, `roll`, `slip`, `surface_force`, `forces`, `contact_velocity`, and the body's
  add-force-at and add-velocity). They run on `TireKeys` (MD39b's tire keys and the 16 surfaces as
  `crate::car::Surface`) and `TireState` (what the step reads and writes besides `Tire`). The brake and
  steer pointers are a `Lever` naming one of `State`'s `brake`, `brake_force`, `steer` or `rear_steer`.
  `Keys` gained `tires`, and `State` gained `steer`, `rear_steer` and `tires`. `Pieces::answer` now
  answers `Call::TireStep`, except when the tire stands on a mover or was handed one (then it goes to
  `rest`: filed as MD130). The sine and arc cosine tables are built once in a `OnceLock`, as quad.rs
  does.
- tests/mxvatv_car_frame.rs:
  - `mod tire` (the offsets), `read_tire_keys`, `read_tire_state`, `tire_words` (poked before and
    compared after, like `mx_words`).
  - `generate_tires`. It aims each tire's ground point along its own travel, at a chosen share
    (−0.5..3) of it, using a native quaternion turn. It puts the normal near the car's up (15%
    anywhere) and sets a third of the slip ratios near ±1 to lock. ABS is on half the time, with its
    slips degenerate 20%. 10% of the pointers are null, every surface is stirred, and a third of the
    bodies are slowed into the slow-slip model. The fronts' `handed_on` is zeroed: nothing in the frame
    writes it, and a junk word would send the original down the mover path.
  - The left front's ground normal is the anti-slide's front normal (one word), so it is generated once.
  - `Unhooked` maps `TireStep` to the step's address and tags 20 branches (`tire_tags`).
  - The checked blocks per tire grew from 0x200 to 0x9b4, so a write anywhere in the tire's step area
    that is not compared fails.
  - New tests `tire_step_is_mx_unleasheds` (seed 0x0124_a001) and
    `all_six_pieces_with_the_tire_step_are_mx_unleasheds` (seed 0x0124_a002, 46 branches).
- research/oracle.txt `# MD124`: the functions, field map, constants and image values.
- Filed MD130: the mover branches.

## What the original does

The step `./kit dis -p atv 0x004b5ca0 0x004b5cf0` calls the suspension (0x004b5cf0), the roll (0x004b62c0,
handed the brake word) and the forces (0x004b6400). It is MX Unleashed's tire step on SLUS-21104's field
layout (MD39b), with fused `adda.s 0`/`madd.s`/`msub.s` wherever MXU multiplies and adds.

- **Suspension.** The travel runs from the mount (less the centre of mass, turned, plus the position)
  down to mount + drop. The ground plane's cut along it gives the ratio, the contact and the
  compression (the length from the lower end).
  - When the tire is compressed it is grounded and the contact velocity is taken.
  - Bottomed past the max: if the tire is solid, or the ratio is < 2, the body is pushed out along the
    normal by the excess (capped at the max) and its velocity into the ground is removed with the
    add-velocity (`vt +0x4c`, velocity and correction). Either way the compression is held at the max
    and the contact velocity is taken again.
  - Load = max(0, spring·comp − damper·v), with v clamped to ±rate. The damper is bounce when v > 0,
    rebound otherwise. The load is pushed along the normal at the contact through `vt +0x84`
    (add force at a world point: moment += unturn(q, r × f), force += f).
  - Not grounded: a handed tire takes the handed load and stands-on words, and its lower point is
    turned but not offset by the position. Otherwise load 0 and point velocity (0,0,0,1).
  - Last, the compression eases back no faster than decomp rate·h, and the last compression is that
    value capped at the max.
- **Roll.** torque += −clamp(spin/2, ±1)·wheel friction. While braked, it also adds braking torque ·
  half · (−brake) · ABS scale. The ABS scale is 1 − (|ratio| − lo)/(hi − lo), clamped to 0..1, and
  applies only with ABS on, grounded, and lo ≠ hi.
- **Forces.**
  - The steer word × max steer is the steer angle. The force is reset to (0,0,0,1).
  - In the air, only the slip runs.
  - Grounded, the frame (0x004b6680) is built by the math library's basis from the steered heading and
    the ground normal. The heading comes from the table's coarse lookup with plain wraps. Its w is
    stack garbage, which basis never reads. Then the velocity is taken into the frame, followed by the
    slip and the surface force. The force is turned out by the frame onto the body (`vt +0x80`), with
    its moment about the centre of mass (`vt +0x78`) unless lone_front is set.
  - Then last force = force. The wheel angle advances by h·clamp(spin, ±angle rate) and wraps once,
    fused. The torque goes to last torque and is zeroed. Side ratio = force.x / load, or 0 at load ≤ 0.
- **Slip** (0x004b6a00).
  - Braked, with the ratio > 0.95: locked, spin 0. Otherwise slipping = |ratio| > 0.2, and spin +=
    h·(1/I)·(torque − last force.z·radius), clamped to the top speed.
  - Fast (|v.z| > 20): the slip angle is the acos table of v.z/|v| (signed by v.x), the ratio is
    (v.z − spin·r)/|v.z|, and the tan comes from the coarse table, fused-wrapped twice.
  - Slow: the low-speed grip latches below 5 when v.x changes sign against the last velocity, and lets
    go above 5. While it holds, force.x −= 1.4·v.x·sqrt(100·load). The tan relaxes toward v.x/|v.z|,
    the angle is a rational arc tangent of it (0.28 fit), and the ratio relaxes at rate 5.
  - Sliding = |angle| > 0.2. Camber = −roll ratio × the acos lean of the car's normal.
  - Clamps: angle to ±π/2, ratio to ±0.98, camber to ±0.09.
- **Surface force** (0x004b70b0). MXU's combined-slip model on the tire's surface byte. Its tables run
  from +0x5a4, 16 per field. The load is capped at the max normal load, and a zero load adds nothing.

## How it was proved

Function oracle in tests/mxvatv_car_frame.rs, run under the frame oracle. For every frame, SLUS-21104's
world step runs with the tire step's hook removed, so the original's step code runs on the image's
four tires and body. Against it, `atv_car::frame` runs with `Pieces` answering the step.

- 3,000 generated frames (4 tires, ~4 substeps each) in `tire_step_is_mx_unleasheds`, and 3,000 more
  with all six pieces unhooked.
- In every frame, every tire key and state word, the car, its body and the call log are bit-equal, and
  the original wrote nothing in the tire blocks (0x9b4 each) that is not compared.
- All 20 tire branches were seen, each at least 514 times (log of the forced-fail run).
- A mutant (4/π one ulp off) went red at case 23.
- The whole file (11 tests) passes.

## Not verified / not 1:1

- The contact velocity's mover branches (a tire standing on, or handed, another object's contact,
  stands-on words ≠ 0) are not ported. `Pieces` hands such a step to `rest`, and the test never
  generates one. Filed as MD130.
- Proved on the buggy image only, with keys stirred around its tires; the baja's and truck's tire
  keys were not used as a base.
- A surface byte above 15 is masked to 0..15 in the port. The original would index past the tables
  (reading other tire words). Not generated, since the loader never writes one.
- The frame's heading w is stack garbage in the original and 0 in the port. Basis never reads it, and
  nothing stores it.
- The acos table's interpolation multiplies in the other operand order (frac·hi first). This makes no
  difference to the result, and the test agrees bit for bit.

## Next

Nothing for this task. MD130 (mover contact velocity) and MD106c (the whole frame) follow.

## Gate note

The first `./kit done` went red: mxvatv_bike_ride was SIGKILLed near its end, three times, in s1 and s3
as well. Main had already fixed it in 3bbecc9b3: the parts-box test built a looping node tree, and
walking it ran out of memory. That commit is cherry-picked unchanged onto this branch, so the gate
can pass. The merge sees the same change on both sides.
