# 1 — s35 the biplane moves (FINAL)

Task S35. 2026-10-20.

## What I did

- `crates/game-sim/src/biplane_anim.rs`, a new module:
  - **the pilot's clips**: `register`/`flies` (the rider build's record per flying machine), `Driver::mount` (the added record the body plays);
  - **the plane's request block**: `Pilot::block` (the ctor's), `Pilot::axes`/`Pilot::ask` (the measure step's writes), `Reset` (the reset flag and its latch);
  - **the moving parts**: `surfaces` → `Surfaces` (flap, rear-flap, rudder and prop angles, and which prop cone).
- `vehicle_anim::Library::add`: the engine's generic "add an animation" call, which `register` uses.
- `crates/game-sim/tests/biplane_anim.rs`:
  - `the_biplane_asks_for_the_originals_clips` (the plan's test name);
  - `the_biplanes_parts_move_with_its_controls`.
- `watch/biplane-pilot.toml` and `playback/` here, so the capture can be made again.
- An S35 block at the end of `research/oracle.txt`.

**Why not `biplane.rs` / `tests/biplane.rs`, as the plan names them.** S34 created both files on its branch at the same time as this task. If I had added either, the runner's merge would hit an add/add conflict. So the work is in its own module and its own test file. The test keeps the plan's name. `pub mod biplane_anim;` sits beside `vehicle_anim` in `lib.rs`, not in alphabetical order, because S34's `pub mod biplane;` takes the alphabetical spot.

**Why a new capture.** C7's capture (`task/C7`, unmerged) has the plane's controls, surfaces, prop and crash, but nothing of the pilot. I re-recorded C7's playback with the spec above: same recording, same state, frame counters 10971..12688, 1718 rows, every row one tick on from the last.
- The playback is C7's script with my spec swapped in: `playback/play_capture.sh PID context/recordings/biplane.p2m2 10971 1718 context/capture/biplane/pilot.csv`, under `./kit pcsx2 with`.
- The csv is in a subfolder rather than in `acceptance/`, because a biplane csv in `acceptance/` reds `air.rs`.
- It is git-ignored, like every capture.

## What the original does

**Clip selection — three pieces, none of which knows what a plane is.**

1. *The rider build* (`./kit fn 248150`). For every machine-list entry of kind 1 (biplane) or 2 (helicopter), it adds a record to each rider body: key = the machine actor's type word, clips `biplane_roll_bck` then `biplane_roll_fwd`, each loaded and then bound.
   - It does nothing unless bit 0 of the body's flag byte is set.
   - It skips a key it already has, so the first registration stays.
   - An empty name gives an empty half.
2. *The mount* (`./kit dis 0x24ea70 0x24eb50`, a vtable slot that Ghidra has no function for).
   - Two gates: the body takes a request only if it has none, and only when the call's mask shares a bit with the body's.
   - It then points the body at the request and sets the added-record index to the *first* record whose key equals the type of the request's owner, or −1.
   - The plane's type word is 1 on every tick, and the pilot's body has index 0.
3. *The plane's measure step* (`./kit fn 355000`, the fourth part of MXBiplane's measure). Each tick, if `(+8 & +0x694)`, it writes the request block it owns at plane +0x690. The fields that matter to the pilot:
   - `at_base` cleared (the ctor sets it);
   - `held` = the reset flag;
   - the offset = a pointer to `PelvisBoxPosition` (the step reads the offset through that pointer);
   - `airborne` = not on the ground;
   - phase target = `0.5 * (1 - aileron stick)`, mix target = `0.5 * (1 + elevator stick)`;
   - band = airspeed; base and rotation = the rigid body's.

   Nothing writes the gesture, wreck, commanded, cut, settle, seq or progress fields, so they keep the ctor's values (gesture 24 = none, the rest 0).

With a record mounted, the body's step (already ported, `Driver::frame`'s added-record arm) plays `Blend::Two`:
- `a` = `biplane_roll_bck` at the eased phase;
- `b` = `biplane_roll_fwd`;
- mixed by the eased mix.

That is the whole pilot: sideways stick rolls through the back clip, fore-aft stick mixes toward the forward one.

The body's chooser still runs. On this flight it only picks the airborne row (35) or the riding grid (30). Its pick never reaches a clip, but it sets the state, the fade rate and the ramp, and the test checks all three.

**The reset flag** (`./kit fn 352910`, `./kit fn 351df0`). The reset raises +0x588. The frame step clears it only when the step's own copy (+0x58c) was already up, then copies it. So `held` is up for the reset's tick and the tick after it, and the lags snap.

**Order inside a tick, from the capture** (it says so on every tick):
1. reset;
2. measure, writing from the sticks the *previous* tick's controls left;
3. the latch;
4. the surface step;
5. the wreck test.

The pilot's body eases toward *this* tick's block. So the pilot leans one tick behind the plane's own surfaces.

**The moving parts** (`./kit fn 352ff0`):
- **Wing flaps.** Each is put back on its rest matrix, then turned about the hinge kept beside it (`R(hinge, a) · rest` in row-vector order) by +aileron on the left pair and −aileron on the right pair. A zero angle is not turned.
- **Rear flaps.** The rotation is replaced by one about x by `−(elevator fraction × MaxElevator)`; the position is kept.
- **Rudder.** The rotation is replaced by one about y by −rudder.
- **Prop, not wrecked.** `PropDummy`'s rotation is replaced by one about z by the engine's prop angle, `propconeblur` is stood at the dummy's world matrix and shown, and `propconebent` is hidden.
- **Prop, wrecked.** The prop is not turned: the bent cone is shown and the blur hidden.

## How it was proved

A PINE recording compared frame by frame: `context/capture/biplane/pilot.csv`, 1718 ticks of the human flying the biplane, including two wrecks and two resets.

`the_biplane_asks_for_the_originals_clips` checks the following, on every tick and **bit for bit**:
- the body's request is the plane's +0x690 block;
- the mounted record index (0);
- the reset latch (both words);
- `held`, `at_base`, `airborne` and the band;
- the two axis targets: 1714 of 1717 ticks; the other three are torn rows, below;
- the chooser's pick;
- the body's state and previous state, both lags (value, tau and ratio), ramp and rate;
- the selection itself: `Blend::Two` with the back and forward roll handles at the clamped lags.

The fade rates are read off the first restart of each state; they are the body's, not the plane's.

`the_biplanes_parts_move_with_its_controls` builds matrices from `surfaces` and checks them against the original's bone matrices on every tick:
- rear flap, rudder and prop: within 1e-5;
- top-left flap: within 2e-3;
- each flap's position is kept exactly;
- a wrecked prop does not move.

Gate: `./kit check` passes.

## Not verified / not 1:1

- **Three rows match neither tick's sticks** (frame counters 12020, 12049, 12465). Their request targets fit neither that row's sticks nor the previous row's. The row after each one is written from the odd row's sticks again, so I read them as torn reads, not as a different rule, but I did not prove it. The test takes the block as captured on those three rows.
- **One row was read before the body stepped** (12167). Its lags equal the previous row's, and the next row is exactly two steps on. The test checks that and moves on.
- **The bit that gates the rider build's records** (bit 0 of the body's flag byte): the list ctor clears it and I did not find its setter. Every body on the state has it set. `register` takes it as an argument.
- **The mount's two gates** (already requesting; the mask against the body's own) are left to the caller. Nothing here models the mask, and its vt +0x140 call and the placement hand-off after it are not ported.
- **The measure's gate** `(+8 & +0x694)` is not modelled. It was open on every tick of the recording (+8 = 3, +0x694 = 1).
- **Kinds 3–5** register steering clips of their own in the same build (`buggy_steer`/`baja_steer` and so on). They are not ported: they are not the biplane.
- **The helicopter (kind 2)** gets the same pair keyed by its own type word. I did not check its type word or that it mounts the same way: it is not in this recording.
- **The pilot's chooser** is `rider_anim::Body::select`. The test asserts only its two picks on this flight (airborne → 35, else 30) and does not run it. Its standing-still fidget branch (which draws from the generator) is never reached here, so whether a parked plane makes its pilot fidget is unproved.
- **What the measure writes that the pilot does not use yet** is not ported: `PelvisBoxSize` (+0x784), the hand/foot dummies, the transform, and +0x83c = plane +0x178 (meaning not known).
- **The surfaces match only to tolerance, as the brief allows.** The wing flap's hinge in the test is the rest matrix's first row, normalised. The plane keeps its own copy, which I did not capture: that is the 1e-3. Only one flap of each pair was captured (top left, top right, rear left, rudder, prop). The other three are the same call with the same or negated angle.
- **Nothing draws it yet.** `game-render` has no biplane, so this has not been looked at beside a screenshot. "Matches by eye" here means "matches the original's bone matrices", which is stronger for the angles and says nothing about the model.

## What S34 (or whoever owns the plane object) has to wire in

1. **Each tick, in the plane's measure:** `Pilot { elevator, aileron: the stick commands as last tick's controls left them (S34's Pad::elevator/aileron), held: reset.just, on_ground, speed: |velocity|, base, rotation, seat: PelvisBoxPosition }.ask(&mut block)`. The block starts as `Pilot::block()`. Then call `reset.frame()` in the plane's frame step, and `reset.reset()` from the plane's reset.
2. **Each tick, for the moving parts:** `surfaces(Surfaces::elevator, MaxElevator, Surfaces::aileron, Surfaces::rudder, Engine::prop_angle, crash reason != 0)`, after the reset and before the wreck test. The renderer turns the angles into the bone matrices as `Surfaces`' doc says.
3. **At the rider build:** `register(&mut body_library, true, machine_type, anim)` for every machine-list entry where `flies(kind)`.
4. **When a rider is put on the plane:** `body.veh.mount(&library, PLANE)`, and point the body's step at the plane's block (`block.request`). The pilot's chooser then reads `block.airborne`.

## Next

Nothing for this task. The wiring above belongs to whichever task builds the plane object (S38's body and world step, per S34's split).
