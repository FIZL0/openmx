# 1 — atv force (FINAL)

Task MD64. 2026-10-29.

## What I did

- Scope decision, made at the start: the brief names six SLUS-21104 functions (ground_head, the rest of
  ground_step, wheel_contacts, air_state, the update's tail, tire_phy). That is too much for one session
  to prove properly. atv_quad_update's tail (2408 bytes, surface tables) and atv_quad_tire_phy (4444
  bytes, a .phy reader that has to be checked against two images' wheels) stand apart from the other
  four, so I filed them as **MD64a** and **MD64b** and did the three listeners here: the PreProcess, the
  Force body and its contact distributor, and the PostProcess.
- What the listeners call of the bike and the vehicle (input limiter, gearbox, clutch, preload,
  backflip, engine, wheel suspension/tires/integrate, vehicle post…) belongs to MD63 and to the wheel
  port, and none of it is ported yet. So I did not fake it: the listeners hand each such call, in the
  original's order and with the original's arguments, to a `quad::Moto` (`fn call(&mut self, Call, &mut
  Quad, &mut Body)`), and `Vec<Call>` is the impl that just lists them. Once MD63 lands, a real driver
  implements `Moto` and the listeners need no change.
- `crates/game-sim/src/quad.rs`:
  - `Quad` lost `speed: F` and `pitch: F`. They were the `z` of +0xc30 (contact velocity) and the `x`
    of +0x360 (euler), and the PostProcess writes those quadwords whole, so they are now
    `contact_vel: Vf` and `euler: Vf`, read through `Quad::speed()` / `Quad::pitch()`. MD37b's four steps
    only change in how they read these.
  - New fields on `Quad`, `Wheel`, `Params` and `Body`, each with its doc. Offsets are in oracle.txt's
    `# MD64` block, not in code.
  - `ground_head` (PreProcess), `ground_step` (Force body), `wheel_contacts` (the four-way distributor;
    it reuses `chassis::WheelContact`, `set_contact` and `ContactCall`), `air_state` (PostProcess), plus
    the `Call` enum and the `Moto` trait.
- `crates/game-sim/tests/atv_quad.rs`:
  - The wheels are now 0x1000 apart and 0x8b0 long, because +0x838 and +0x8ac sit far into the wheel.
  - New regions: the vehicle state and the collision record.
  - Pokes for every new field, and the generator fills them.
  - Four new tests (below). MD37b's five still pass on the bigger state.
- research/oracle.txt: an `# MD64` block of fields, and eight new rows: atv_vehicle_force,
  atv_vehicle_post, atv_node_position, atv_quat_to_euler, atv_wheel_set_contact, atv_list_at,
  atv_collide_against_search, atv_bike_head_tail. I added more rows at first, but most of the callees
  were already named in MD40's block, so I removed my duplicates.

## What the original does

Recipes: `./kit fn atv_quad_ground_head -o atv` (and `atv_quad_ground_step`, `atv_quad_air_state`,
`atv_quad_wheel_contacts`). Without Ghidra names, use `./kit dis <addr> --len N -p atv`. On the PC, the
same with `pc_quad_*`. All three listeners return at once unless the quad has a vehicle state that is
not 3 or 4. (On the PC the state is at +0xb9c/+0xba4. The PS2 reads +0xbc4 for all three.)

**PreProcess** (PC pc_quad_ground_head: the same clauses, branch for branch):
1. The input limiter (vt +0x34c).
2. Off the ground, the front brake is zeroed.
3. If the balance words are equal, both fronts' +0x30 = 1. Otherwise both are 0, and:
   - With control off, both fronts' steering command is 0.
   - Otherwise the command is the lock × (stick × fade). The fade is the steering scale mapped over
     speed, or `scale_1` when the two speeds are equal.
   - With the full-lock option, the timer runs while |stick| ≥ 0.987 and resets otherwise. The
     command is then `mula(stick, angle·min(1, timer/time)) + (stick·fade)·(lock − angle)`.
   - The command goes to RF, LF and +0x12c8.
4. The donut goes to LR and then RR. It applies only with all four down, under the donut speed,
   deflection > 0.9 (the test is `!(≤ 0.9)`) and gas ≠ 0. Otherwise it is 0.
   - Its value comes from two ramps either side of −π/2. Each ramp's end can equal its fixed end,
     which gives that end's value. Two of those four leaves can never be reached; the original keeps
     them anyway.
5. Springs: the front and rear rates mapped over speed_len from 14.6666 to 29.3332. LF/LR first,
   then copied to RF/RR.
6. +0x1250 and body +0xc0 are set to (0,0,0,1).
7. AutoShift (dt, f13 = LR +0x838), Clutch (dt), then vt +0x4d0.

**Force body** (PC pc_quad_ground_step: the same order; the PC's first call is the base
`Vehicle::vf0_at_60`):
1. The vehicle's Force.
2. With control on, in order: stand_upright, roll_pitch, preload, backflip, spin-air, engine update
   (with the engine at +0x142c), air drag, airborne control, the ground push.
3. Then, control or not:
   - wheel_clear ×4;
   - the distributor (dt);
   - suspension ×4 (dt);
   - tire forces ×4 (dt);
   - the rear-pair flag;
   - wheel_integrate ×4 (dt);
   - rev limit (engine);
   - peak rates.

   Each ×4 goes LF, RF, LR, RR.

**Distributor** (PC pc_quad_wheel_contacts: the same, through the PC's own search slot):
1. +0x143c = 0. The search's vt +200 (clear), then collide_against_search(search, actors, 1).
2. If search byte +0x126 bit 0x20 is set, then for each contact i:
   - Each listener of the node's owner gets vt +0xc(elem, 0, 0).
   - The node's vehicle is kept only when its +0x1a0 is 2.
   - The wheel is picked by `c.ole` against 0. `z ≤ 0` means a rear, `x ≤ 0` means the left, so a NaN
     goes front/right.
   - set_contact(wheel, point +0x98, normal +0x94, owner, vehicle).
   - For a rear contact, +0x143c = the node.

**PostProcess** (PC pc_quad_air_state: the same branches, except the crossup, see below):
1. Motion:
   - speed_len = sqrt(mula/madda/madd of the velocity).
   - +0x350 = the velocity into the body frame.
   - quat_axes → +0xbe0/+0xbf0.
   - quat_to_euler → +0x360.
   - +0x340 = the angular velocity into the world.
2. Contact flags:
   - was = airborne.
   - on_ground / all_down / front_down / rear_down from the four +0xdc.
   - airborne = !on_ground.
   - The hang time is zeroed on take-off, then += dt while airborne.
3. +0xc30 = the first touching wheel's +0x190 (LF, RF, LR, RR), else +0x350.
4. Crossup:
   - The flattened heading and the flattened velocity, each normalized by VU rsqrt; the velocity
     only when !flipping && !spinning && speed_len > 10.
   - sin = |heading × vel|, clamped to [−1, 1] three times, written out.
   - angle = π/2 − the table lookup of sin, negated when the cross's y < 0.
   - Then the authority ramp: authority ≤ 0 keeps the angle; a time of 0 gives 0; otherwise
     map_clamp(auth, time, 0, 0, angle).
5. Record +0x24 = up.y ≤ 0.25.
6. The vehicle's PostProcess (dt), the node's position into +0xbd0, then a copy to +0xc40.

The crossup's table lookup is inlined as `mula(frac, hi) + (1 − frac)·lo`. `AcosTable::at` multiplies
in the other order. The products are independent and the add commutes, so the two are equal: a mutation
that swapped the order in the full-lock sum also passed, the same kind of case. The PC computes the
crossup with an arc sine of the cross's length (FUN_0041d4a0) and no clamps. The PS2 is the reference.

## How it was proved

Function oracles in `crates/game-sim/tests/atv_quad.rs`, each 10,000 generated states. Each test runs
the original in the interpreter off MD37a's `atv-r-spawn.ee.bin`, hooks every unported callee to log it
in the port's `Call` terms, and checks each callee's arguments (quad, engine, node, wheel). It then
compares every word of the quad, body, world, state, record and the four 0x8b0 wheels, and compares the
call lists:

- `the_quads_pre_process_is_the_originals`: 11 path tallies, at least 30 states each.
  - Paths: off, balancing, control off, plain steering, flat fade, full lock held, full lock let go,
    front brake let go.
  - All six reachable donut leaves, including the back ramp ending exactly at −π.
- `the_quads_force_body_is_the_originals`: off, control off, control on, and a lone right front.
  stand_upright, roll_pitch and the ground push run for real on both sides.
- `the_quads_contact_distributor_is_the_originals`: the search, record, nodes, owners, listener lists
  and vehicles are laid out in RAM. The two virtuals land on hooked addresses with no code.
  - Paths: nothing to look at, no contacts, each wheel, vehicle kept, vehicle dropped, listeners told.
  - Calls compared: ClearRecord, Collide, then Notify(owner, j).
- `the_quads_post_process_is_the_originals`:
  - First checks the image's arc-cosine table against `AcosTable::build()`, all 30,001 entries bit for
    bit. It matched, so the original reads the port's table.
  - Paths: each contact-velocity source, take-off, crossup measured or not, all three authority
    branches, and upright, tipped, and up.y exactly 0.25 (a quaternion found by ulp search).

Run: `./kit check -- -p game-sim --test atv_quad`, 9 tests in about 30 s. Run directly, the tests
printed no skip.

Mutations, to check that the tests can fail. Each changed one line of the port; all were restored.
- These failed, as they should:
  - the rear node set from the owner;
  - the crossup off by 1e-7;
  - the hang time counting double;
  - `<=` → `<` on the tipped flag;
  - the −π donut leaf's value;
  - the d = 0 forward leaf.
- These two survived, both expected:
  - the tipped constant at 0.2501, because no state lands in (0.25, 0.2501];
  - the full-lock sum's two products swapped, because the sum commutes.

## Not verified / not 1:1

- **Nothing the listeners call of the bike, the engine, the wheels or the vehicle is ported.** It is
  listed as `quad::Call`s, and the oracles hook those calls. So this proves the order, the arguments
  and everything the listeners do themselves. It does not prove the quad's ride. That needs MD63 (the
  bike step) and a wheel port, and then MD37b's replay.
- **No ride replay.** MD37a's csvs are not replayed. The Moto driver that would run them does not
  exist yet (MD37b, after MD62/MD63/MD64/MD65).
- **Only the atv_ (Race) image** (`atv-r-spawn.ee.bin`) was used. The atv_s_ code is the same binary,
  but its spawn values were not read into a test.
- **atv_quad_update's tail and atv_quad_tire_phy are not done.** They are filed as MD64a and MD64b.
- **Some field names are guessed from use, not from strings:**
  - `Params::full_lock*` (+0x12f0..+0x12fc), from the 0.987 stick test;
  - `Body::reset_each_frame` (body +0xc0, nothing traced reads it);
  - `Quad::attitude` (+0x1250, "the peak-rates vector": the Force body's last call is the bike's peak
    rates, but I did not trace that +0x1250 is what it writes);
  - `Quad::tipped` (record +0x24);
  - `Wheel::balancing` (+0x30).
- The inputs are generated, not captured.
  - Spawn values (steering fade 0/0.9/110/0.2, springs 315/382.2 and 525/554.4, donut angles) were read
    from the image and written into oracle.txt. Ranges around them are covered.
  - States outside the generator's ranges (NaNs, huge speeds) are not.
  - The distributor's NaN rule (`!(≤ 0)`) is copied from the asm but never fed a NaN.
- The tipped threshold at 0.2501 against 0.25 is not told apart (see the mutations). The boundary
  itself is tested.
- `the_quads_pre_process_is_the_originals` and the others check that the original wrote nothing
  outside the laid-out regions. The fake search/record structures are read-only to it, and they are
  not compared.

## Next

Nothing in MD64's scope is left. MD64a (the update tail) and MD64b (tire_phy) are filed. The quad's
listeners are waiting for a `Moto` that runs MD63's bike steps and a wheel port. MD37b's replay is
where that comes together.
